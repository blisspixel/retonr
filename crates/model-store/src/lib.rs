//! Durable `SQLite` storage for artifact lifecycle authority and inert model evidence.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod artifact_set_installation;
mod artifact_set_removal;
mod binding;
mod error;
mod integrity;
mod lifecycle;
mod migration;
mod record;
mod removal;
mod schema;
mod store;

pub use artifact_set_installation::{
    ArtifactSetInstallationEpoch, ArtifactSetInstallationWriteDisposition,
    StoredArtifactSetInstallation,
};
pub use artifact_set_removal::StoredArtifactSetRemoval;
pub use error::{StoreError, StoreResult};
pub use lifecycle::ExclusiveArtifactLifecycleLock;
pub use migration::{
    ExistingStoreMigration, StoreMigrationDisposition, StoreMigrationResult, StoreSchemaStatus,
    required_store_schema_version,
};
pub use removal::{
    ArtifactInstallationEpoch, ArtifactRemovalPhase, StoredArtifactInstallation,
    StoredArtifactRemoval,
};
pub use store::candidate_generation_attempt_admission::{
    CandidateGenerationAttemptAdmissionClassV1, CandidateGenerationAttemptAdmissionV1,
};
pub use store::candidate_generation_attempt_precursor::{
    CandidateGenerationAttemptPrecursorCheckpointV1Input,
    CandidateGenerationAttemptPrecursorCheckpointV1Readback,
    CandidateGenerationAttemptPrecursorCheckpointV1TransactionError,
    CandidateGenerationAttemptPrecursorCheckpointV1WriteDisposition,
};
pub use store::candidate_generation_evidence_storage::{
    CandidateGenerationEvidenceBundleStorageV1, CandidateGenerationEvidenceStorageContractError,
    CandidateGenerationEvidenceStorageRootId, CandidateGenerationEvidenceStorageV1Limits,
    MAX_CANDIDATE_GENERATION_EVIDENCE_STORAGE_AGGREGATE_BYTES,
    MAX_CANDIDATE_GENERATION_EVIDENCE_STORAGE_TREE_DEPTH,
    MAX_CANDIDATE_GENERATION_EVIDENCE_STORAGE_TREE_ENTRIES,
};
pub use store::candidate_generation_execution::{
    CandidateGenerationExecutionV1Input, CandidateGenerationExecutionV1ReadInput,
    CandidateGenerationExecutionV1Readback, CandidateGenerationExecutionV1TransactionError,
    CandidateGenerationExecutionV1WriteDisposition, StoredCandidateGenerationExecutionV1,
};
pub use store::candidate_generation_execution_state::CandidateGenerationExecutionV1State;
pub use store::candidate_judge_execution::{
    CandidateJudgeExecutionV1Input, CandidateJudgeExecutionV1ReadInput,
    CandidateJudgeExecutionV1TransactionError, CandidateJudgeExecutionV1WriteDisposition,
    CandidateJudgeObservationFactV1, StoredCandidateJudgeExecutionV1,
};
pub use store::generation_qualification_plan_foundation::{
    GenerationQualificationPlanFoundationV1, GenerationQualificationPlanFoundationV1Input,
    GenerationQualificationPlanFoundationV1Readback,
    GenerationQualificationPlanFoundationV1TransactionError,
    GenerationQualificationPlanFoundationV1WriteDisposition,
};
pub use store::generation_qualification_preregistration::{
    GenerationQualificationPreregistrationFoundationV1Input,
    GenerationQualificationPreregistrationReadInput,
    GenerationQualificationPreregistrationReadback,
    GenerationQualificationPreregistrationTransactionError,
    GenerationQualificationPreregistrationV1Input,
    GenerationQualificationPreregistrationWriteDisposition,
    StoredGenerationQualificationPreregistration,
};
pub use store::generation_qualification_terminal_evidence::{
    GenerationQualificationTerminalEvidenceV1Input,
    GenerationQualificationTerminalEvidenceV1ReadInput,
    GenerationQualificationTerminalEvidenceV1TransactionError,
    GenerationQualificationTerminalEvidenceV1WriteDisposition,
    StoredGenerationQualificationTerminalEvidenceV1,
};
pub use store::generation_system_foundation::{
    GenerationSystemFoundationV1, GenerationSystemFoundationV1Input,
    GenerationSystemFoundationV1Readback, GenerationSystemFoundationV1TransactionError,
    GenerationSystemFoundationV1WriteDisposition,
};
pub use store::phase_policy_denial::{
    GenerationHumanAdjudicationPolicyDenialV1Input,
    GenerationHumanAdjudicationPolicyDenialV1ReadInput, GenerationResourcePolicyDenialV1Input,
    GenerationResourcePolicyDenialV1ReadInput, PhasePolicyDenialV1TransactionError,
};
pub use store::{
    ArtifactStateStore, InstallationWriteDisposition, RemovalCompletionDisposition,
    RemovalPreparationDisposition, StoredArtifactSetState, StoredArtifactState, WriteDisposition,
};
