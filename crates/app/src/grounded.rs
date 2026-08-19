use std::{
    future::Future,
    path::Path,
    task::{Context, Poll, Waker},
    time::Instant,
};

use rewrite_engine::{
    ClaimShadowObserver, PreparedCandidateGenerator, ProtectedKind, ProtectionError,
    ProtectionPlan, validate_rewrite_options,
};
use rewrite_grounded::{
    GROUNDED_POLICY_SCHEMA_VERSION, GroundedError, GroundedPolicy, GroundedRequest,
    GroundedSentinel, GroundedSentinelKind, GroundedStrategy, GroundedTrace,
};
use rewrite_inference::{
    CONFORMANCE_BACKEND_ID, ConformanceInferenceBackend, InferenceBackend, InferenceErrorKind,
    OperationContext, ReasoningPolicy, SamplingParameters, candidate_output_contract,
};
use rewrite_model::{
    ActiveArtifactBinding, ArtifactId, ArtifactRole, LicenseDecision, QualificationStatus,
};
use rewrite_text_adapter::{ParsedTextDocument, TextAdapter};
use rewrite_types::{
    CancellationToken, GENERATION_PROVENANCE_SCHEMA_VERSION, GeneratedCandidate,
    GenerationProvenance, GenerationRuntimeProvenance, GenerationUsageProvenance, RewriteMode,
    RewriteOptions, RewriteRecord, RewriteUnitId,
};

use crate::{
    AppError, ArtifactRepository, ArtifactRepositoryErrorKind, CandidateCheckResult,
    ClaimExtractionContext, ClaimExtractionError, ClaimShadowJoinBinding, ClaimShadowJoinService,
    PreparedClaimShadowSet, run_plain_text_transaction,
};

/// Owned plain-text input for grounded local rewriting.
#[derive(Clone, Debug, PartialEq)]
pub struct GroundedRewriteRequest {
    /// Complete source bytes.
    pub source: Vec<u8>,
    /// Exact caller-declared terms that must remain unchanged.
    pub protected_terms: Vec<String>,
    /// Requested rewrite strength.
    pub mode: RewriteMode,
    /// Explicit style context, or an empty string when unavailable.
    pub style_context: String,
    /// Optional extractor binding for informational shadow comparison.
    ///
    /// Absence, backend unavailability, or incomplete extraction leaves the
    /// hard gates unchanged. A recorded conflict cannot reject a candidate.
    pub claim_shadow: Option<ClaimShadowJoinBinding>,
}

/// Safe transaction result with any redacted generation provenance in its record.
#[derive(Clone, Debug, PartialEq)]
pub struct GroundedRewriteResult {
    /// Rewritten bytes, or the exact original bytes after abstention.
    pub output: Vec<u8>,
    /// Common versioned transaction record without raw content.
    pub record: RewriteRecord,
}

/// Product prompt used by the in-process fake-backend conformance path.
pub const CONFORMANCE_PROMPT_TEMPLATE: &str = "Rewrite conservatively and preserve every sentinel.";

/// Recovered generation binding attached to in-process fake-backend conformance.
pub struct AttachedConformanceRewrite {
    strategy: GroundedStrategy,
    backend: ConformanceInferenceBackend,
}

impl AttachedConformanceRewrite {
    /// Runs grounded generation and the common gates without starting a runtime.
    ///
    /// # Errors
    ///
    /// Returns [`AppError`] for invalid input, backend failure, or engine failure.
    pub fn rewrite(
        &self,
        request: GroundedRewriteRequest,
        cancellation: &CancellationToken,
        deadline: Option<Instant>,
    ) -> Result<GroundedRewriteResult, AppError> {
        let service = GroundedRewriteService::new(self.strategy.clone(), &self.backend);
        block_ready(service.rewrite(request, cancellation, deadline))
    }
}

/// Current grounded-rewrite selection.
pub struct GroundedRewriteSelection;

impl GroundedRewriteSelection {
    /// Requires a selected qualified local generation artifact.
    ///
    /// When no repository is supplied this fails closed. It does not start a
    /// runtime, access the network, or invent a production backend.
    ///
    /// # Errors
    ///
    /// Returns [`AppError::GroundedUnavailable`].
    pub const fn require_selected() -> Result<(), AppError> {
        Err(AppError::GroundedUnavailable)
    }

    /// Recovers one generation binding and attaches in-process fake conformance.
    ///
    /// The path never starts a runtime, pulls a model, or opens a network path.
    /// Only a recovered binding whose qualification names the retained fake
    /// backend can attach. Claims stay unadmitted.
    ///
    /// # Errors
    ///
    /// Returns [`AppError::GroundedUnavailable`] when no generation binding is
    /// selected, [`AppError::GroundedSelectionMismatch`] when a requested
    /// identity disagrees, [`AppError::GroundedRuntimeUnavailable`] when a
    /// binding exists but the qualification is not the fake conformance
    /// backend, or [`AppError::GroundedRepository`] when the repository cannot
    /// be inspected.
    pub fn require_ready(
        data_directory: Option<&Path>,
        requested: Option<&ArtifactId>,
    ) -> Result<AttachedConformanceRewrite, AppError> {
        let binding = Self::select(data_directory, requested)?;
        let Some(data_directory) = data_directory else {
            return Err(AppError::GroundedUnavailable);
        };
        let repository =
            ArtifactRepository::new(data_directory).map_err(|_| AppError::GroundedRepository)?;
        let qualification = repository
            .generation_qualification(&binding.qualification_id)
            .map_err(|_| AppError::GroundedRepository)?;
        if qualification.runtime.backend != CONFORMANCE_BACKEND_ID {
            return Err(AppError::GroundedRuntimeUnavailable);
        }
        if qualification.status != QualificationStatus::Qualified
            || !qualification
                .supported_roles
                .contains(&ArtifactRole::Generation)
            || qualification.license_decision == LicenseDecision::Rejected
        {
            return Err(AppError::GroundedUnavailable);
        }
        let output = candidate_output_contract();
        let prompt_template = CONFORMANCE_PROMPT_TEMPLATE.to_owned();
        let strategy = GroundedStrategy::new(GroundedPolicy {
            schema_version: GROUNDED_POLICY_SCHEMA_VERSION,
            artifact_id: binding.artifact_id.clone(),
            artifact_digest: binding.artifact_digest.clone(),
            prompt_template_digest: rewrite_types::Digest::sha256(prompt_template.as_bytes()),
            prompt_template,
            output,
            candidate_count: 1,
            source_byte_limit: qualification.source_byte_limit,
            input_byte_limit: qualification.source_byte_limit.saturating_add(8_192),
            context_token_limit: qualification.context_token_limit,
            output_token_limit: 256,
            candidate_byte_limit: qualification.source_byte_limit,
            sampling: SamplingParameters {
                temperature: 0.0,
                top_p: 1.0,
                seed: Some(1),
            },
            reasoning: ReasoningPolicy::Disabled,
        })?;
        let backend = ConformanceInferenceBackend::bind(
            binding.artifact_id,
            binding.artifact_digest,
            qualification.runtime,
            None,
        )
        .map_err(|_| AppError::GroundedRuntimeUnavailable)?;
        Ok(AttachedConformanceRewrite { strategy, backend })
    }

    /// Recovers one active generation binding without attaching a runtime.
    ///
    /// # Errors
    ///
    /// Returns [`AppError`] when no binding is selected, the requested identity
    /// disagrees, or the repository cannot be inspected.
    pub fn select(
        data_directory: Option<&Path>,
        requested: Option<&ArtifactId>,
    ) -> Result<ActiveArtifactBinding, AppError> {
        let Some(data_directory) = data_directory else {
            return if requested.is_some() {
                Err(AppError::GroundedSelectionMismatch)
            } else {
                Err(AppError::GroundedUnavailable)
            };
        };
        let repository =
            ArtifactRepository::new(data_directory).map_err(|_| AppError::GroundedRepository)?;
        let binding = match repository.active_generation_binding() {
            Ok(binding) => binding,
            Err(error) if error.kind() == ArtifactRepositoryErrorKind::NotInitialized => {
                return if requested.is_some() {
                    Err(AppError::GroundedSelectionMismatch)
                } else {
                    Err(AppError::GroundedUnavailable)
                };
            }
            Err(_) => return Err(AppError::GroundedRepository),
        };
        match (binding, requested) {
            (Some(binding), Some(artifact_id)) if artifact_id != &binding.artifact_id => {
                Err(AppError::GroundedSelectionMismatch)
            }
            (Some(binding), _) => Ok(binding),
            (None, Some(_)) => Err(AppError::GroundedSelectionMismatch),
            (None, None) => Err(AppError::GroundedUnavailable),
        }
    }

    /// Validates one plain-text source without generating candidates.
    ///
    /// # Errors
    ///
    /// Returns [`AppError::TextAdapter`] when the source is not a supported
    /// UTF-8 document.
    pub fn validate_source(source: &[u8]) -> Result<(), AppError> {
        TextAdapter::parse(source)?;
        Ok(())
    }
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
    /// An optional claim-shadow binding records independently produced comparison
    /// without changing hard-gate eligibility.
    ///
    /// # Errors
    ///
    /// Returns [`AppError`] for invalid input or policy, backend failure, engine
    /// failure, adapter failure, or an invalid claim-shadow binding. Candidate
    /// rejection is a successful abstention.
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
            return empty_transaction(&parsed, &options, cancellation);
        };
        let protection = match ProtectionPlan::build(&unit.text, &options.protected_terms) {
            Ok(protection) => protection,
            Err(
                ProtectionError::ReservedTokenInSource | ProtectionError::AmbiguousSurfaceMapping,
            ) => {
                return empty_transaction(&parsed, &options, cancellation);
            }
            Err(error) => return Err(error.into()),
        };
        let (masked_source, protected_values) = protection.clone().into_parts();
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
                return cancelled_transaction(&parsed, Vec::new(), &options, None);
            }
            Err(error) => return Err(error.into()),
        };
        let shadow = match prepare_claim_shadow(
            self.backend,
            request.claim_shadow.as_ref(),
            unit.id.clone(),
            &unit.text,
            &protection,
            &generation.candidates,
            cancellation,
            deadline,
        )
        .await
        {
            Ok(shadow) => shadow,
            Err(ClaimExtractionError::Cancelled) => {
                return cancelled_transaction(
                    &parsed,
                    generation.candidates,
                    &options,
                    Some(generation.trace),
                );
            }
            Err(error) => return Err(error.into()),
        };
        let generator = PreparedCandidateGenerator::new(generation.candidates);
        let transaction = run_plain_text_transaction(
            &parsed,
            &generator,
            &options,
            shadow
                .as_ref()
                .map(|observer| observer as &dyn ClaimShadowObserver),
            cancellation,
        )?;
        Ok(with_trace(transaction, Some(generation.trace)))
    }
}

fn empty_transaction(
    parsed: &ParsedTextDocument,
    options: &RewriteOptions,
    cancellation: &CancellationToken,
) -> Result<GroundedRewriteResult, AppError> {
    let generator = PreparedCandidateGenerator::new(Vec::new());
    let transaction = run_plain_text_transaction(parsed, &generator, options, None, cancellation)?;
    Ok(with_trace(transaction, None))
}

fn cancelled_transaction(
    parsed: &ParsedTextDocument,
    candidates: Vec<GeneratedCandidate>,
    options: &RewriteOptions,
    trace: Option<GroundedTrace>,
) -> Result<GroundedRewriteResult, AppError> {
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    let generator = PreparedCandidateGenerator::new(candidates);
    let transaction = run_plain_text_transaction(parsed, &generator, options, None, &cancelled)?;
    Ok(with_trace(transaction, trace))
}

#[expect(
    clippy::too_many_arguments,
    reason = "the helper binds one optional shadow join after generation"
)]
async fn prepare_claim_shadow(
    backend: &dyn InferenceBackend,
    binding: Option<&ClaimShadowJoinBinding>,
    unit_id: RewriteUnitId,
    source: &str,
    protection: &ProtectionPlan,
    candidates: &[GeneratedCandidate],
    cancellation: &CancellationToken,
    deadline: Option<Instant>,
) -> Result<Option<PreparedClaimShadowSet>, ClaimExtractionError> {
    let Some(binding) = binding else {
        return Ok(None);
    };
    let restored: Vec<String> = candidates
        .iter()
        .filter_map(|candidate| protection.restore(&candidate.text).ok())
        .collect();
    let set = ClaimShadowJoinService::new(backend)
        .prepare_for_candidates(
            binding,
            unit_id,
            source,
            restored.iter().map(String::as_str),
            ClaimExtractionContext {
                cancellation,
                deadline,
            },
        )
        .await?;
    if set.is_empty() {
        Ok(None)
    } else {
        Ok(Some(set))
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

fn block_ready<T>(future: impl Future<Output = T>) -> T {
    let mut future = Box::pin(future);
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    match future.as_mut().poll(&mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("conformance-backed rewrite must complete immediately"),
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
    mut transaction: CandidateCheckResult,
    trace: Option<GroundedTrace>,
) -> GroundedRewriteResult {
    if let Some(trace) = trace {
        transaction.record = transaction.record.with_generation(map_trace(trace));
    }
    GroundedRewriteResult {
        output: transaction.output,
        record: transaction.record,
    }
}

fn map_trace(trace: GroundedTrace) -> GenerationProvenance {
    GenerationProvenance {
        schema_version: GENERATION_PROVENANCE_SCHEMA_VERSION,
        strategy_id: trace.strategy_id,
        runtime: GenerationRuntimeProvenance {
            backend: trace.runtime.backend,
            version: trace.runtime.version,
            digest: trace.runtime.digest,
        },
        artifact_id: trace.artifact_id.digest().clone(),
        artifact_digest: trace.artifact_digest,
        prompt_template_digest: trace.prompt_template_digest,
        input_digest: trace.input_digest,
        output_schema_digest: trace.output_schema_digest,
        candidate_count: trace.candidate_count,
        usage: GenerationUsageProvenance {
            input_tokens: trace.usage.input_tokens,
            output_tokens: trace.usage.output_tokens,
            generation_micros: trace.usage.generation_micros,
        },
    }
}

#[cfg(test)]
mod tests;
