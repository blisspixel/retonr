//! Application service that composes the engine with document adapters.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use rewrite_engine::{
    CancellationToken, CandidateGenerator, LiteralSemanticEvaluator, ProvidedCandidateGenerator,
    RewriteEngine, StructureAssessment, StructureValidator,
};
use rewrite_text_adapter::{
    MAX_PLAIN_TEXT_BYTES, ParsedTextDocument, TextAdapter, TextAdapterError,
};
use rewrite_types::{
    Digest, ReasonCode, RewriteMode, RewriteOptions, RewriteRecord, RewriteStatus, RewriteUnit,
};
use thiserror::Error;

mod artifact_import;
mod artifact_inventory;
mod artifact_reconciliation;
mod artifact_removal;
mod artifact_repository;
mod artifact_set_import;
mod artifact_set_inventory;
mod artifact_set_reconciliation;
mod artifact_set_removal;
mod artifact_storage;
mod claim_extraction;
mod grounded;
mod installed_ollama_import;
mod package_attestation;
mod reviewed_ollama_runtime_import;
mod runtime_artifact_lease;
mod runtime_artifact_set_lease;
mod runtime_attestation;
mod source_inspection;
#[cfg(test)]
mod symlink_test_support;

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
    ArtifactInstallationKey, ArtifactRepository, ArtifactRepositoryBackupKey,
    ArtifactRepositoryError, ArtifactRepositoryErrorKind, ArtifactRepositoryImportDisposition,
    ArtifactRepositoryImportResult, ArtifactRepositoryMigrationDisposition,
    ArtifactRepositoryMigrationLimits, ArtifactRepositoryMigrationResult,
    ArtifactRepositoryPendingOperations, ArtifactRepositoryReconciliationResult,
    ArtifactRepositoryRemovalResult, ArtifactRepositorySchemaInspection,
    ArtifactRepositorySchemaStatus, ArtifactRepositorySetImportResult,
    ArtifactRepositorySetReconciliationResult, ArtifactRepositorySetRemovalResult,
    ArtifactSetInstallationKey,
};
pub use artifact_set_import::{
    ArtifactSetImportDisposition, ArtifactSetImportError, ArtifactSetImportLimits,
    ArtifactSetImportProgress, ArtifactSetImportResult, ArtifactSetImportStage,
    OfflineArtifactSetImportRequest,
};
pub use artifact_set_inventory::{
    ArtifactSetInventoryError, ArtifactSetInventoryLimits, ArtifactSetInventoryProgress,
    ArtifactSetInventoryReport, ArtifactSetInventoryService, ArtifactSetInventoryStage,
    ArtifactSetTreeConflict, OversizedArtifactSet, RegisteredArtifactSetBytes,
    RegisteredArtifactSetInspection, UnexpectedArtifactSetEntryCounts, VerifiedArtifactSetOrphan,
};
pub use artifact_set_reconciliation::{
    ArtifactSetReconciliationError, ArtifactSetReconciliationLimits,
    ArtifactSetReconciliationProgress, ArtifactSetReconciliationRequest,
    ArtifactSetReconciliationResult, ArtifactSetReconciliationService,
    ArtifactSetReconciliationStage,
};
pub use artifact_set_removal::{
    ArtifactSetRemovalError, ArtifactSetRemovalLimits, ArtifactSetRemovalProgress,
    ArtifactSetRemovalRecoveryError, ArtifactSetRemovalRequest, ArtifactSetRemovalResult,
    ArtifactSetRemovalService, ArtifactSetRemovalStage,
};
pub use claim_extraction::{
    CLAIM_PAIR_OPERATION_ID, CLAIM_PAIR_PROMPT_TEMPLATE, ClaimExtractionContext,
    ClaimExtractionError, ClaimExtractionPair, ClaimExtractionRequest, ClaimExtractionService,
    ClaimShadowJoinBinding, ClaimShadowJoinDisposition, ClaimShadowJoinService,
    PreparedClaimShadow, PreparedClaimShadowSet,
};
pub use grounded::{
    AttachedConformanceRewrite, CONFORMANCE_PROMPT_TEMPLATE, GroundedRewriteRequest,
    GroundedRewriteResult, GroundedRewriteSelection, GroundedRewriteService,
};
pub use installed_ollama_import::{
    InstalledOllamaModelSource, OllamaModelImportError, OllamaModelImportEvidence,
    OllamaModelImportLimits, OllamaModelImportResult, OllamaModelReference,
    PackageManifestWriteDisposition,
};
pub use package_attestation::{
    ModelPackageAttestationEvidence, ModelPackageLease, PACKAGE_ATTESTATION_SCHEMA_VERSION,
    PackageAttestationError, PackageAttestationScope, PackageAttestationService,
    RuntimePackageAttestationEvidence, RuntimePackageLease, RuntimePackageLeaseLimits,
};
pub use reviewed_ollama_runtime_import::{
    OllamaRuntimeImportError, OllamaRuntimeImportEvidence, OllamaRuntimeImportLimits,
    OllamaRuntimeImportResult, ReviewedOllamaRuntimeSource,
};
pub use rewrite_engine::{ClaimShadowObserver, EngineError, ProtectionError};
pub use rewrite_text_adapter::{
    CarrierPresence, ControlCounts, LineEndingKind, PlainTextInventory, TextEncoding,
};
pub use runtime_artifact_lease::{RuntimeArtifactLease, RuntimeArtifactLeaseLimits};
pub use runtime_artifact_set_lease::{
    ArtifactSetLeaseError, RuntimeArtifactSetLease, RuntimeArtifactSetLeaseLimits,
};
pub use runtime_attestation::{
    ManagedRuntimeAttestationRequest, ManagedRuntimeIdentityFacts, ManagedRuntimeStateFacts,
    RuntimeAttestationError, RuntimeAttestationLimits, RuntimeAttestationPersistence,
    RuntimeAttestationResult, RuntimeAttestationService, WriteDisposition, host_runtime_target,
};
pub use source_inspection::inspect_plain_text;

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
    /// No qualified local generation artifact is selected.
    #[error("grounded rewrite requires a selected qualified local artifact")]
    GroundedUnavailable,
    /// A requested artifact identity does not match the active generation binding.
    #[error("requested artifact is not the active qualified generation binding")]
    GroundedSelectionMismatch,
    /// A qualified generation artifact is selected, but no local runtime is attached.
    #[error("grounded rewrite requires an attached local runtime")]
    GroundedRuntimeUnavailable,
    /// The artifact repository could not be inspected for a generation binding.
    #[error("grounded rewrite could not inspect the artifact repository")]
    GroundedRepository,
    /// Independent claim extraction failed before an informational shadow join.
    #[error(transparent)]
    ClaimExtraction(#[from] claim_extraction::ClaimExtractionError),
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
        Self::check_with_cancellation(request, &CancellationToken::new())
    }

    /// Checks one candidate and observes cancellation before engine work.
    ///
    /// # Errors
    ///
    /// Returns [`AppError`] for unsupported input or an operational engine
    /// failure. Cancellation is a successful abstention.
    pub fn check_with_cancellation(
        request: CandidateCheckRequest,
        cancellation: &CancellationToken,
    ) -> Result<CandidateCheckResult, AppError> {
        Self::check_with_claim_shadow(request, cancellation, None)
    }

    /// Checks one candidate and records independently produced claim comparison.
    ///
    /// A present observer cannot authorize a rewrite. Literal-token failure
    /// still abstains, and a claim conflict cannot reject a candidate that
    /// already passed the hard gates.
    ///
    /// # Errors
    ///
    /// Returns [`AppError`] for unsupported input or an operational engine
    /// failure. Candidate policy rejection is returned as a successful abstention.
    pub fn check_with_claim_shadow(
        request: CandidateCheckRequest,
        cancellation: &CancellationToken,
        claim_shadow: Option<&dyn ClaimShadowObserver>,
    ) -> Result<CandidateCheckResult, AppError> {
        let candidate = strip_utf8_bom_prefix(&request.candidate);
        if candidate.len() > MAX_CANDIDATE_CHECK_BYTES {
            return Err(AppError::CandidateTooLarge {
                actual: candidate.len(),
                maximum: MAX_CANDIDATE_CHECK_BYTES,
            });
        }
        let parsed = TextAdapter::parse(&request.source)?;
        let generator = ProvidedCandidateGenerator::new(vec![candidate]);
        let options = RewriteOptions {
            mode: RewriteMode::Literal,
            protected_terms: request.protected_terms,
            ..RewriteOptions::default()
        };
        run_plain_text_transaction(&parsed, &generator, &options, claim_shadow, cancellation)
    }
}

fn strip_utf8_bom_prefix(candidate: &str) -> String {
    candidate
        .strip_prefix('\u{FEFF}')
        .unwrap_or(candidate)
        .to_owned()
}

fn run_plain_text_transaction(
    parsed: &ParsedTextDocument,
    generator: &dyn CandidateGenerator,
    options: &RewriteOptions,
    claim_shadow: Option<&dyn ClaimShadowObserver>,
    cancellation: &CancellationToken,
) -> Result<CandidateCheckResult, AppError> {
    let structure = PlainTextStructure { parsed };
    let mut engine = RewriteEngine::new(generator, &LiteralSemanticEvaluator, &structure);
    if let Some(observer) = claim_shadow {
        engine = engine.with_claim_shadow(observer);
    }
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
    fn validate(&self, unit: &RewriteUnit, candidate: &str) -> StructureAssessment {
        if !TextAdapter::replacement_preserves_text_safety(&unit.text, candidate) {
            return StructureAssessment::UnsafeText;
        }
        if TextAdapter::replacement_preserves_structure(self.parsed, candidate) {
            StructureAssessment::Preserved
        } else {
            StructureAssessment::Changed
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
            "\u{feff}Hello, world!\r\n",
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
    fn protected_literals_reject_extensions_and_leading_decimal_changes() {
        let cases = [
            (
                "See https://example.com now",
                "See https://example.com/ now",
            ),
            ("Pay 10 now", "Pay $10 now"),
            ("Rate 50 now", "Rate 50% now"),
            ("Value is .5.", "Value is 5."),
            ("Pay 10 now", "Pay 10.0 now"),
        ];
        for (source, candidate) in cases {
            let result = CandidateCheckService::check(request(source.as_bytes(), candidate))
                .expect("policy rejection is not an operational error");
            assert_eq!(
                result.record.reason,
                Some(ReasonCode::ProtectedValueChanged),
                "{source} -> {candidate}"
            );
            assert_eq!(result.output, source.as_bytes());
        }
    }

    #[test]
    fn trailing_url_punctuation_may_change() {
        let source = b"See https://example.com.";
        let result = CandidateCheckService::check(request(source, "See https://example.com!"))
            .expect("URL wrapper punctuation is eligible");
        assert_eq!(result.record.status, RewriteStatus::Rewritten);
        assert_eq!(result.output, b"See https://example.com!");
    }

    #[test]
    fn protection_occurrence_overflow_is_a_limit_error() {
        use rewrite_engine::{EngineError, MAX_PROTECTED_OCCURRENCES, ProtectionError};

        let source = "1 ".repeat(MAX_PROTECTED_OCCURRENCES + 1);
        let error = CandidateCheckService::check(request(source.as_bytes(), &source))
            .expect_err("occurrence overflow must not enter a rewrite decision");
        assert!(matches!(
            error,
            AppError::Engine(EngineError::Protection(ProtectionError::ResourceLimit))
        ));
    }

    #[test]
    fn cancelled_check_abstains_without_rewriting() {
        use rewrite_types::CancellationToken;

        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let result = CandidateCheckService::check_with_cancellation(
            request(b"Hello world", "Hello, world!"),
            &cancellation,
        )
        .expect("cancellation is a policy outcome");
        assert_eq!(result.record.status, RewriteStatus::Abstained);
        assert_eq!(result.record.reason, Some(ReasonCode::Cancelled));
        assert_eq!(result.output, b"Hello world");
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
