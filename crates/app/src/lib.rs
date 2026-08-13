//! Application service that composes the engine with document adapters.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use rewrite_engine::{
    CancellationToken, CandidateGenerator, EngineError, LiteralSemanticEvaluator, ProtectionError,
    ProvidedCandidateGenerator, RewriteEngine, StructureValidator,
};
use rewrite_text_adapter::{
    MAX_PLAIN_TEXT_BYTES, ParsedTextDocument, TextAdapter, TextAdapterError,
};
use rewrite_types::{
    Digest, GateResult, ReasonCode, RewriteMode, RewriteOptions, RewriteRecord, RewriteStatus,
    RewriteUnit,
};
use thiserror::Error;

mod artifact_import;
mod artifact_inventory;
mod artifact_reconciliation;
mod artifact_removal;
mod artifact_repository;
mod artifact_storage;
mod grounded;
mod runtime_artifact_lease;

pub use artifact_import::{
    ArtifactImportError, ArtifactImportLimits, ArtifactImportProgress, ArtifactImportResult,
    ArtifactImportStage, OfflineArtifactImportRequest, OfflineArtifactImportService,
};
pub use artifact_inventory::{
    ArtifactInventoryError, ArtifactInventoryLimits, ArtifactInventoryProgress,
    ArtifactInventoryReport, ArtifactInventoryService, ArtifactInventoryStage,
    ContentAddressConflict, OrphanManifestAssociation, OversizedArtifactFile,
    PendingArtifactRemovalInspection, RegisteredArtifactBytes, RegisteredArtifactInspection,
    UnexpectedArtifactEntryCounts, VerifiedArtifactOrphan,
};
pub use artifact_reconciliation::{
    ArtifactOrphanReconciliationProgress, ArtifactOrphanReconciliationRequest,
    ArtifactOrphanReconciliationResult, ArtifactOrphanReconciliationService,
    ArtifactOrphanReconciliationStage, ArtifactReconciliationDisposition,
    ArtifactReconciliationError, ArtifactReconciliationLimits,
};
pub use artifact_removal::{
    ArtifactRemovalDisposition, ArtifactRemovalError, ArtifactRemovalLimits,
    ArtifactRemovalProgress, ArtifactRemovalRecoveryError, ArtifactRemovalRequest,
    ArtifactRemovalResult, ArtifactRemovalService, ArtifactRemovalStage,
};
pub use artifact_repository::{
    ArtifactInstallationKey, ArtifactRepository, ArtifactRepositoryError,
    ArtifactRepositoryErrorKind, ArtifactRepositoryImportDisposition,
    ArtifactRepositoryImportResult, ArtifactRepositoryReconciliationResult,
    ArtifactRepositoryRemovalResult,
};
pub use grounded::{GroundedRewriteRequest, GroundedRewriteResult, GroundedRewriteService};
pub use runtime_artifact_lease::{RuntimeArtifactLease, RuntimeArtifactLeaseLimits};

/// Maximum accepted source or candidate size for the plain-text check service.
pub const MAX_CANDIDATE_CHECK_BYTES: usize = MAX_PLAIN_TEXT_BYTES;

/// Owned input for the deterministic candidate-check workflow.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateCheckRequest {
    /// Complete source bytes.
    pub source: Vec<u8>,
    /// Restored UTF-8 candidate text for the complete plain-text body.
    pub candidate: String,
    /// Exact caller-declared terms that must remain unchanged.
    pub protected_terms: Vec<String>,
}

/// Safe output and redacted audit record from candidate checking.
#[derive(Clone, Debug, PartialEq)]
pub struct CandidateCheckResult {
    /// Rewritten bytes, or the exact original bytes after abstention.
    pub output: Vec<u8>,
    /// Versioned transaction record without raw document content.
    pub record: RewriteRecord,
}

/// Operational application-service failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AppError {
    /// Candidate exceeds the bounded plain-text workflow limit.
    #[error("candidate is {actual} bytes; the supported maximum is {maximum}")]
    CandidateTooLarge {
        /// Observed candidate length.
        actual: usize,
        /// Configured application limit.
        maximum: usize,
    },
    /// Plain-text ingestion or reconstruction failed.
    #[error(transparent)]
    TextAdapter(#[from] TextAdapterError),
    /// Rewrite-engine orchestration failed.
    #[error(transparent)]
    Engine(#[from] EngineError),
    /// Protected-value planning failed before grounded generation.
    #[error(transparent)]
    Protection(#[from] ProtectionError),
    /// Grounded candidate generation failed before validation.
    #[error(transparent)]
    Grounded(#[from] rewrite_grounded::GroundedError),
}

/// Entry point for model-free validation of one supplied candidate.
#[derive(Clone, Copy, Debug, Default)]
pub struct CandidateCheckService;

impl CandidateCheckService {
    /// Checks one candidate under literal-mode fidelity policy.
    ///
    /// The original bytes are returned exactly whenever the engine abstains or
    /// final adapter verification fails.
    ///
    /// # Errors
    ///
    /// Returns [`AppError`] for unsupported input or an operational engine
    /// failure. Candidate policy rejection is returned as a successful abstention.
    pub fn check(request: CandidateCheckRequest) -> Result<CandidateCheckResult, AppError> {
        if request.candidate.len() > MAX_CANDIDATE_CHECK_BYTES {
            return Err(AppError::CandidateTooLarge {
                actual: request.candidate.len(),
                maximum: MAX_CANDIDATE_CHECK_BYTES,
            });
        }
        let parsed = TextAdapter::parse(&request.source)?;
        let generator = ProvidedCandidateGenerator::new(vec![request.candidate]);
        let options = RewriteOptions {
            mode: RewriteMode::Literal,
            protected_terms: request.protected_terms,
            ..RewriteOptions::default()
        };
        run_plain_text_transaction(&parsed, &generator, &options, &CancellationToken::new())
    }
}

fn run_plain_text_transaction(
    parsed: &ParsedTextDocument,
    generator: &dyn CandidateGenerator,
    options: &RewriteOptions,
    cancellation: &CancellationToken,
) -> Result<CandidateCheckResult, AppError> {
    let structure = PlainTextStructure { parsed };
    let engine = RewriteEngine::new(generator, &LiteralSemanticEvaluator, &structure);
    let outcome = engine.run(parsed.document(), options, cancellation)?;
    let proposed = TextAdapter::apply(parsed, &outcome.edits)?;
    let verification = TextAdapter::verify(parsed, &proposed, &outcome.edits);

    let (output, status, reason, selected_candidates) = if verification.valid {
        (
            proposed,
            outcome.status,
            outcome.reason,
            outcome.selected_candidates,
        )
    } else {
        (
            parsed.original().to_vec(),
            RewriteStatus::Abstained,
            Some(ReasonCode::ReassemblyVerification),
            Vec::new(),
        )
    };
    let record = RewriteRecord::new(
        parsed.document().document_id.clone(),
        parsed.document().source_digest.clone(),
        Digest::sha256(&output),
        status,
        reason,
        selected_candidates,
        outcome.assessments,
    );
    Ok(CandidateCheckResult { output, record })
}

struct PlainTextStructure<'a> {
    parsed: &'a ParsedTextDocument,
}

impl StructureValidator for PlainTextStructure<'_> {
    fn validate(&self, unit: &RewriteUnit, candidate: &str) -> GateResult {
        if !TextAdapter::replacement_preserves_text_safety(&unit.text, candidate) {
            return GateResult::fail(
                "plain_text_safety",
                "unsafe_text_control",
                "candidate introduced an unsafe text control",
            );
        }
        if TextAdapter::replacement_preserves_structure(self.parsed, candidate) {
            GateResult::pass("plain_text_structure")
        } else {
            GateResult::fail(
                "plain_text_structure",
                "newline_skeleton_changed",
                "candidate changed the source newline sequence",
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use rewrite_types::{ReasonCode, RewriteStatus};

    use super::{
        AppError, CandidateCheckRequest, CandidateCheckService, MAX_CANDIDATE_CHECK_BYTES,
    };

    fn request(source: &[u8], candidate: &str) -> CandidateCheckRequest {
        CandidateCheckRequest {
            source: source.to_vec(),
            candidate: candidate.to_owned(),
            protected_terms: Vec::new(),
        }
    }

    #[test]
    fn accepts_punctuation_and_preserves_bom_and_newline() {
        let result = CandidateCheckService::check(request(
            b"\xEF\xBB\xBFHello world\r\n",
            "Hello, world!\r\n",
        ))
        .expect("valid deterministic request");
        assert_eq!(result.record.status, RewriteStatus::Rewritten);
        assert_eq!(result.output, b"\xEF\xBB\xBFHello, world!\r\n");
        assert_eq!(
            result.record.output_digest,
            rewrite_types::Digest::sha256(&result.output)
        );
    }

    #[test]
    fn changed_literal_abstains_with_exact_original() {
        let source = b"Version 2 costs $10.";
        let result = CandidateCheckService::check(request(source, "Version 3 costs $10."))
            .expect("candidate rejection is not an operational error");
        assert_eq!(result.record.status, RewriteStatus::Abstained);
        assert_eq!(
            result.record.reason,
            Some(ReasonCode::ProtectedValueChanged)
        );
        assert_eq!(result.output, source);
        assert!(result.record.selected_candidates.is_empty());
    }

    #[test]
    fn structural_change_abstains_with_exact_original() {
        let source = b"Hello\nworld\n";
        let result = CandidateCheckService::check(request(source, "Hello world\n"))
            .expect("candidate rejection is not an operational error");
        assert_eq!(result.record.reason, Some(ReasonCode::StructureChanged));
        assert_eq!(result.output, source);
    }

    #[test]
    fn empty_source_is_unchanged() {
        let result = CandidateCheckService::check(request(b"", "ignored"))
            .expect("empty UTF-8 is supported");
        assert_eq!(
            result.record.status,
            RewriteStatus::UnchangedNoEligibleContent
        );
        assert!(result.output.is_empty());
    }

    #[test]
    fn rejects_invalid_utf8() {
        let error = CandidateCheckService::check(request(b"a\xFF", "a"))
            .expect_err("invalid UTF-8 must not enter the engine");
        assert!(matches!(error, AppError::TextAdapter(_)));
    }

    #[test]
    fn rejects_unsafe_control_with_exact_original() {
        let source = b"Hello world";
        let result = CandidateCheckService::check(request(source, "Hello\u{1b} world"))
            .expect("unsafe candidate is a policy rejection");
        assert_eq!(result.record.reason, Some(ReasonCode::UnsafeText));
        assert_eq!(result.output, source);
    }

    #[test]
    fn rejects_oversized_candidate() {
        let error = CandidateCheckService::check(request(
            b"short",
            &"a".repeat(MAX_CANDIDATE_CHECK_BYTES + 1),
        ))
        .expect_err("candidate limit is enforced");
        assert!(matches!(error, AppError::CandidateTooLarge { .. }));
    }
}
