use std::time::Instant;

use rewrite_engine::{
    PreparedCandidateGenerator, ProtectedKind, ProtectionPlan, validate_rewrite_options,
};
use rewrite_grounded::{
    GroundedError, GroundedRequest, GroundedSentinel, GroundedSentinelKind, GroundedStrategy,
    GroundedTrace,
};
use rewrite_inference::{InferenceBackend, InferenceErrorKind, OperationContext};
use rewrite_text_adapter::TextAdapter;
use rewrite_types::{CancellationToken, RewriteMode, RewriteOptions, RewriteRecord};

use crate::{AppError, CandidateCheckResult, run_plain_text_transaction};

/// Owned plain-text input for grounded local rewriting.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroundedRewriteRequest {
    /// Complete source bytes.
    pub source: Vec<u8>,
    /// Exact caller-declared terms that must remain unchanged.
    pub protected_terms: Vec<String>,
    /// Requested rewrite strength.
    pub mode: RewriteMode,
    /// Explicit style context, or an empty string when unavailable.
    pub style_context: String,
}

/// Safe transaction result and redacted generation provenance.
#[derive(Clone, Debug, PartialEq)]
pub struct GroundedRewriteResult {
    /// Rewritten bytes, or the exact original bytes after abstention.
    pub output: Vec<u8>,
    /// Common versioned transaction record without raw content.
    pub record: RewriteRecord,
    /// Generation provenance, absent when no backend call completed.
    pub trace: Option<GroundedTrace>,
}

/// Application service joining grounded generation to the common validation path.
pub struct GroundedRewriteService<'a> {
    strategy: GroundedStrategy,
    backend: &'a dyn InferenceBackend,
}

impl<'a> GroundedRewriteService<'a> {
    /// Creates a service from an already validated strategy and backend port.
    #[must_use]
    pub const fn new(strategy: GroundedStrategy, backend: &'a dyn InferenceBackend) -> Self {
        Self { strategy, backend }
    }

    /// Generates candidates and applies output only after the common gates pass.
    ///
    /// Cancellation returns the original bytes with a cancelled abstention. Other
    /// backend failures remain operational errors and never produce output bytes.
    ///
    /// # Errors
    ///
    /// Returns [`AppError`] for invalid input or policy, backend failure, engine
    /// failure, or adapter failure. Candidate rejection is a successful abstention.
    pub async fn rewrite(
        &self,
        request: GroundedRewriteRequest,
        cancellation: &CancellationToken,
        deadline: Option<Instant>,
    ) -> Result<GroundedRewriteResult, AppError> {
        let parsed = TextAdapter::parse(&request.source)?;
        let options = RewriteOptions {
            mode: request.mode,
            protected_terms: request.protected_terms,
            ..RewriteOptions::default()
        };
        validate_rewrite_options(&options)?;
        let Some(unit) = parsed.document().rewrite_units.first() else {
            let generator = PreparedCandidateGenerator::new(Vec::new());
            let transaction =
                run_plain_text_transaction(&parsed, &generator, &options, cancellation)?;
            return Ok(with_trace(transaction, None));
        };
        let protection = ProtectionPlan::build(&unit.text, &options.protected_terms)?;
        let (masked_source, protected_values) = protection.into_parts();
        let strategy_request = GroundedRequest {
            unit_id: unit.id.clone(),
            masked_source,
            sentinels: protected_values
                .into_iter()
                .map(|value| GroundedSentinel {
                    token: value.token,
                    kind: map_protected_kind(value.kind),
                })
                .collect(),
            mode: request.mode,
            style_context: request.style_context,
        };
        let generation = match self
            .strategy
            .generate(
                &strategy_request,
                self.backend,
                OperationContext::new(cancellation, deadline),
            )
            .await
        {
            Ok(generation) => generation,
            Err(error) if is_cancelled(&error) => {
                let cancelled = CancellationToken::new();
                cancelled.cancel();
                let generator = PreparedCandidateGenerator::new(Vec::new());
                let transaction =
                    run_plain_text_transaction(&parsed, &generator, &options, &cancelled)?;
                return Ok(with_trace(transaction, None));
            }
            Err(error) => return Err(error.into()),
        };
        let generator = PreparedCandidateGenerator::new(generation.candidates);
        let transaction = run_plain_text_transaction(&parsed, &generator, &options, cancellation)?;
        Ok(with_trace(transaction, Some(generation.trace)))
    }
}

const fn map_protected_kind(kind: ProtectedKind) -> GroundedSentinelKind {
    match kind {
        ProtectedKind::DeclaredTerm => GroundedSentinelKind::DeclaredTerm,
        ProtectedKind::Url => GroundedSentinelKind::Url,
        ProtectedKind::Email => GroundedSentinelKind::Email,
        ProtectedKind::Number => GroundedSentinelKind::Number,
    }
}

fn is_cancelled(error: &GroundedError) -> bool {
    matches!(
        error,
        GroundedError::Discovery(error) | GroundedError::Generation(error)
            if error.kind == InferenceErrorKind::Cancelled
    )
}

fn with_trace(
    transaction: CandidateCheckResult,
    trace: Option<GroundedTrace>,
) -> GroundedRewriteResult {
    GroundedRewriteResult {
        output: transaction.output,
        record: transaction.record,
        trace,
    }
}

#[cfg(test)]
mod tests;
