//! Application service that composes the engine with document adapters.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

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
mod candidate_attempt_precursor;
mod candidate_check;
mod claim_extraction;
pub mod document_input;
mod document_review;
pub mod effective_runtime_state_observation;
mod generation_api;
mod generation_effective_package;
mod generation_evidence_bundle;
mod generation_system_policy;
mod grounded;
mod installed_ollama_import;
mod lint;
mod managed_judge_observation_authority;
mod managed_judge_precursor;
mod model_license_control;
mod package_attestation;
mod reviewed_ollama_runtime_import;
mod runtime_admission_authority;
mod runtime_admission_evidence;
mod runtime_admission_runner;
mod runtime_artifact_lease;
mod runtime_artifact_set_lease;
mod runtime_attestation;
mod runtime_source_build_bundle;
mod runtime_source_build_evidence;
mod runtime_source_build_execution;
mod runtime_source_build_program_closure;
mod runtime_source_build_report;
mod runtime_source_build_review;
mod source_inspection;
mod static_model_interpretation;
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
pub(crate) use candidate_check::run_plain_text_transaction;
pub use candidate_check::{
    AppError, CandidateCheckRequest, CandidateCheckResult, CandidateCheckService,
};
pub use claim_extraction::{
    CLAIM_PAIR_OPERATION_ID, CLAIM_PAIR_PROMPT_TEMPLATE, ClaimExtractionContext,
    ClaimExtractionError, ClaimExtractionPair, ClaimExtractionRequest, ClaimExtractionService,
    ClaimShadowJoinBinding, ClaimShadowJoinDisposition, ClaimShadowJoinService,
    PreparedClaimShadow, PreparedClaimShadowSet,
};
pub use document_review::{
    DocumentReviewError, DocumentReviewRequest, DocumentReviewResult, DocumentReviewService,
    EditorialReview, MAX_DOCUMENT_REVIEW_FINDINGS, MAX_DOCUMENT_REVIEW_PREVIEW_BYTES,
};
pub use generation_api::*;
pub use grounded::{
    AttachedConformanceRewrite, CONFORMANCE_PROMPT_TEMPLATE, GroundedRewriteRequest,
    GroundedRewriteResult, GroundedRewriteSelection, GroundedRewriteService,
};
pub use installed_ollama_import::{
    InstalledOllamaModelSource, OllamaModelImportError, OllamaModelImportEvidence,
    OllamaModelImportLimits, OllamaModelImportResult, OllamaModelReference,
    PackageManifestWriteDisposition,
};
pub use lint::EditorialLintService;
pub use package_attestation::*;
pub use reviewed_ollama_runtime_import::{
    OllamaRuntimeImportError, OllamaRuntimeImportEvidence, OllamaRuntimeImportLimits,
    OllamaRuntimeImportResult, ReviewedOllamaRuntimeSource,
};
pub use rewrite_engine::{ClaimShadowObserver, EngineError, ProtectionError};
pub use rewrite_text_adapter::{
    CarrierPresence, ControlCounts, LineEndingKind, PlainTextInventory, TextEncoding,
};
pub use rewrite_types::{
    CharacterBudget, EditLevel, EditorialComparison, EditorialFinding, LayoutConstraints,
    LineBudget,
};
pub use runtime_admission_authority::{
    MANAGED_GENERATION_PATH_REVIEW_PROCEDURE_ID, MANAGED_GENERATION_PATH_REVIEW_PROCEDURE_VERSION,
    MANAGED_GENERATION_PATH_REVIEW_SCHEMA_VERSION, MAX_MANAGED_GENERATION_PATH_REVIEW_BYTES,
    RuntimeAdmissionAuthorityError, VerifiedAdmittedRuntime, VerifiedAdmittedRuntimeId,
    VerifiedManagedGenerationPath,
};
pub use runtime_admission_evidence::{
    CompiledRuntimeAdmissionAllPassReview, CompiledRuntimeAdmissionEvidenceAssembly,
    CompiledRuntimeAdmissionLicenseControl, CompiledRuntimeAdmissionSourceLineageControl,
    CompiledRuntimeAdmissionTransformationControl, MAX_RUNTIME_ADMISSION_EVIDENCE_BUNDLE_BYTES,
    MAX_RUNTIME_ADMISSION_EVIDENCE_DESTINATION_ENTRIES,
    MAX_RUNTIME_ADMISSION_EVIDENCE_FOUNDATION_JSON_BYTES,
    MAX_RUNTIME_ADMISSION_EVIDENCE_STAGING_ROOTS, MAX_RUNTIME_ADMISSION_EVIDENCE_TREE_ENTRIES,
    MAX_RUNTIME_ADMISSION_EVIDENCE_TREE_PLAN_JSON_BYTES,
    MAX_RUNTIME_ADMISSION_STATIC_CONTROL_JSON_BYTES,
    MAX_RUNTIME_ADMISSION_STATIC_REVIEW_JSON_BYTES,
    RUNTIME_ADMISSION_EVIDENCE_BUNDLE_MANIFEST_PATH, RUNTIME_ADMISSION_EVIDENCE_FOUNDATION_PATH,
    RUNTIME_ADMISSION_EVIDENCE_FOUNDATION_SCHEMA_VERSION, RUNTIME_ADMISSION_EVIDENCE_MEMBER_COUNT,
    RUNTIME_ADMISSION_EVIDENCE_TREE_PLAN_SCHEMA_VERSION, RUNTIME_ADMISSION_LICENSE_PROCEDURE_ID,
    RUNTIME_ADMISSION_LICENSE_PROCEDURE_VERSION, RUNTIME_ADMISSION_SOURCE_LINEAGE_PROCEDURE_ID,
    RUNTIME_ADMISSION_SOURCE_LINEAGE_PROCEDURE_VERSION,
    RUNTIME_ADMISSION_TRANSFORMATION_PROCEDURE_ID,
    RUNTIME_ADMISSION_TRANSFORMATION_PROCEDURE_VERSION, RuntimeAdmissionAllPassReviewCompiler,
    RuntimeAdmissionAllPassReviewError, RuntimeAdmissionAllPassReviewRequest,
    RuntimeAdmissionEvidenceAssemblyCompiler, RuntimeAdmissionEvidenceAssemblyError,
    RuntimeAdmissionEvidenceAssemblyMember, RuntimeAdmissionEvidenceAssemblyPublisher,
    RuntimeAdmissionEvidenceBundleDestination, RuntimeAdmissionEvidenceBundleError,
    RuntimeAdmissionEvidenceBundleLease, RuntimeAdmissionEvidenceBundleLimits,
    RuntimeAdmissionEvidenceBundleSource, RuntimeAdmissionEvidenceBundleVerifier,
    RuntimeAdmissionEvidenceContractError, RuntimeAdmissionEvidenceFoundation,
    RuntimeAdmissionEvidenceFoundationId, RuntimeAdmissionEvidenceFoundationInput,
    RuntimeAdmissionEvidenceMember, RuntimeAdmissionEvidencePlannedMember,
    RuntimeAdmissionEvidenceTreePlan, RuntimeAdmissionEvidenceTreePlanId,
    RuntimeAdmissionFoundationBindingError, RuntimeAdmissionLicenseControlCompiler,
    RuntimeAdmissionLicenseControlId, RuntimeAdmissionLicenseControlVerifier,
    RuntimeAdmissionSourceLineageControlCompiler, RuntimeAdmissionSourceLineageControlId,
    RuntimeAdmissionSourceLineageControlVerifier, RuntimeAdmissionStaticControlError,
    RuntimeAdmissionTransformationControlCompiler, RuntimeAdmissionTransformationControlId,
    RuntimeAdmissionTransformationControlVerifier, VerifiedPassedRuntimeAdmissionLicenseControl,
    VerifiedPassedRuntimeAdmissionSourceLineageControl,
    VerifiedPassedRuntimeAdmissionTransformationControl, VerifiedRuntimeAdmissionFoundationBinding,
};
pub use runtime_admission_runner::{
    MAX_RUNTIME_ADMISSION_MANAGED_FINAL_RECORD_BYTES,
    MAX_RUNTIME_ADMISSION_NATIVE_LOAD_RECORD_BYTES, RuntimeAdmissionCloudDisableLiveEvidence,
    RuntimeAdmissionFinalOperation, RuntimeAdmissionFinalOperationRequest,
    RuntimeAdmissionFinalVerification, RuntimeAdmissionFinalVerificationRequest,
    RuntimeAdmissionManagedFinalRecord, RuntimeAdmissionManagedFinalRecordId,
    RuntimeAdmissionManagedFinalRecordVerifier, RuntimeAdmissionManagedRuntimeLease,
    RuntimeAdmissionManagedStartupEvidence, RuntimeAdmissionNativeClosureDiscovery,
    RuntimeAdmissionNativeClosureDiscoveryRequest, RuntimeAdmissionNativeLoadRecord,
    RuntimeAdmissionNativeLoadRecordId, RuntimeAdmissionNativeLoadRecordVerifier,
    RuntimeAdmissionObservedFinalOperation, RuntimeAdmissionRunner, RuntimeAdmissionRunnerError,
    RuntimeAdmissionRunnerLimits, VerifiedPassedRuntimeAdmissionCloudDisableControl,
    VerifiedPassedRuntimeAdmissionManagedStartupControl,
    VerifiedPassedRuntimeAdmissionNativeClosureControl,
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
pub use runtime_source_build_bundle::{
    MAX_RUNTIME_SOURCE_BUILD_BUNDLE_TREE_ENTRIES, RuntimeSourceBuildBundleError,
    RuntimeSourceBuildBundleLease, RuntimeSourceBuildBundleLimits, RuntimeSourceBuildBundleSource,
    RuntimeSourceBuildBundleVerifier, RuntimeSourceBuildCapabilities,
    RuntimeSourceBuildCapabilityFiles, RuntimeSourceBuildComponentCapability,
    VerifiedRuntimeSourceBuildBundle,
};
pub use runtime_source_build_evidence::{
    MAX_RUNTIME_SOURCE_BUILD_EVIDENCE_BUNDLE_BYTES,
    MAX_RUNTIME_SOURCE_BUILD_EVIDENCE_DESTINATION_ENTRIES,
    MAX_RUNTIME_SOURCE_BUILD_EVIDENCE_STAGING_ROOTS,
    MAX_RUNTIME_SOURCE_BUILD_EVIDENCE_TREE_ENTRIES,
    RUNTIME_SOURCE_BUILD_EVIDENCE_BUNDLE_MANIFEST_PATH, RUNTIME_SOURCE_BUILD_INPUT_MANIFEST_PATH,
    RUNTIME_SOURCE_BUILD_PLAN_BINDING_PATH, RUNTIME_SOURCE_BUILD_REPORT_PATH,
    RuntimeSourceBuildEvidenceBundleDestination, RuntimeSourceBuildEvidenceBundleError,
    RuntimeSourceBuildEvidenceBundleLease, RuntimeSourceBuildEvidenceBundleLimits,
    RuntimeSourceBuildEvidenceBundlePublisher, RuntimeSourceBuildEvidenceBundleSource,
    RuntimeSourceBuildEvidenceBundleVerifier,
};
pub use runtime_source_build_execution::{
    MAX_RUNTIME_SOURCE_BUILD_OUTPUT_TREE_ENTRIES, RuntimeSourceBuildExecution,
    RuntimeSourceBuildExecutionError, RuntimeSourceBuildExecutionPair, RuntimeSourceBuildExecutor,
    RuntimeSourceBuildFailure, RuntimeSourceBuildOutputSource,
};
pub use runtime_source_build_program_closure::{
    ExecutableRuntimeSourceBuildBundleLease, RetainedProgramBootstrapOutputSources,
    RetainedProgramExecutableClosureError, RetainedProgramExecutableClosureId,
    RetainedProgramExecutableClosureVerifier,
};
pub use runtime_source_build_report::{
    RuntimeSourceBuildManagedEvidence, RuntimeSourceBuildReportCompilation,
    RuntimeSourceBuildReportCompilationError, RuntimeSourceBuildReportCompilationLimits,
    RuntimeSourceBuildReportCompiler,
};
pub use runtime_source_build_review::{
    RuntimeSourceBuildReviewCompilation, RuntimeSourceBuildReviewCompilationError,
    RuntimeSourceBuildReviewCompiler,
};
pub use source_inspection::{MAX_CANDIDATE_CHECK_BYTES, inspect_plain_text};
