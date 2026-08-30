use std::fmt;

use super::{
    ManagedLocalJudgeEvidenceClassV1, ManagedLocalJudgeReceiptRecordV1,
    ManagedLocalJudgeReceiptSuccessStatusV1,
};
use crate::generation_qualification::{
    CandidateJudgeObservationBatchId, CandidateJudgePlanId, CandidateJudgeRequestAggregateId,
    CandidateJudgeResponseAggregateId, CandidateJudgeScheduleId, FrozenExternalComponentSetId,
    GenerationSystemId, ManagedGenerationPathId, ManagedLocalJudgeReceiptId,
    ManagedOllamaEffectiveRuntimeStateJoinId, RuntimeAdmissionJoinId,
};
use crate::{
    ArtifactId, EffectivePackageEvidenceV2Id, EffectiveRuntimeStateId, ModelPackageManifestId,
    RuntimeBuildId, RuntimePackageManifestId,
};
use rewrite_types::Digest;

impl ManagedLocalJudgeReceiptRecordV1 {
    /// Returns the portable schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
    /// Returns the content-derived managed receipt identity.
    #[must_use]
    pub const fn managed_local_judge_receipt_id(&self) -> &ManagedLocalJudgeReceiptId {
        &self.id
    }
    /// Returns the bound plan identity.
    #[must_use]
    pub const fn candidate_judge_plan_id(&self) -> &CandidateJudgePlanId {
        &self.candidate_judge_plan_id
    }
    /// Returns the bound schedule identity.
    #[must_use]
    pub const fn candidate_judge_schedule_id(&self) -> &CandidateJudgeScheduleId {
        &self.candidate_judge_schedule_id
    }
    /// Returns the judge system identity.
    #[must_use]
    pub const fn judge_generation_system_id(&self) -> &GenerationSystemId {
        &self.judge_generation_system_id
    }
    /// Returns the successful attempt count.
    #[must_use]
    pub const fn attempt_count(&self) -> u32 {
        self.attempt_count
    }
    /// Returns the judge runtime-admission join identity.
    #[must_use]
    pub const fn judge_runtime_admission_join_id(&self) -> &RuntimeAdmissionJoinId {
        &self.judge_runtime_admission_join_id
    }
    /// Returns the judge managed-generation path identity.
    #[must_use]
    pub const fn judge_managed_generation_path_id(&self) -> &ManagedGenerationPathId {
        &self.judge_managed_generation_path_id
    }
    /// Returns the frozen external-component set identity.
    #[must_use]
    pub const fn judge_frozen_external_component_set_id(&self) -> &FrozenExternalComponentSetId {
        &self.judge_frozen_external_component_set_id
    }
    /// Returns the judge runtime-package manifest identity.
    #[must_use]
    pub const fn judge_runtime_package_manifest_id(&self) -> &RuntimePackageManifestId {
        &self.judge_runtime_package_manifest_id
    }
    /// Returns the judge runtime-build identity.
    #[must_use]
    pub const fn judge_runtime_build_id(&self) -> &RuntimeBuildId {
        &self.judge_runtime_build_id
    }
    /// Returns the judge effective-runtime-state identity.
    #[must_use]
    pub const fn judge_effective_runtime_state_id(&self) -> &EffectiveRuntimeStateId {
        &self.judge_effective_runtime_state_id
    }
    /// Returns the judge model-package manifest identity.
    #[must_use]
    pub const fn judge_model_package_manifest_id(&self) -> &ModelPackageManifestId {
        &self.judge_model_package_manifest_id
    }
    /// Returns the exact judge model artifact identity.
    #[must_use]
    pub const fn judge_model_artifact_id(&self) -> &ArtifactId {
        &self.judge_model_artifact_id
    }
    /// Returns the batch-derived judge effective-package evidence identity.
    #[must_use]
    pub const fn judge_effective_package_evidence_v2_id(&self) -> &EffectivePackageEvidenceV2Id {
        &self.judge_effective_package_evidence_v2_id
    }
    /// Returns the retained runtime installation generation.
    #[must_use]
    pub const fn judge_runtime_installation_generation(&self) -> u64 {
        self.judge_runtime_installation_generation
    }
    /// Returns the retained model installation generation.
    #[must_use]
    pub const fn judge_model_installation_generation(&self) -> u64 {
        self.judge_model_installation_generation
    }
    /// Returns the bound request aggregate identity.
    #[must_use]
    pub const fn judge_request_aggregate_id(&self) -> &CandidateJudgeRequestAggregateId {
        &self.judge_request_aggregate_id
    }
    /// Returns the bound response aggregate identity.
    #[must_use]
    pub const fn judge_response_aggregate_id(&self) -> &CandidateJudgeResponseAggregateId {
        &self.judge_response_aggregate_id
    }
    /// Returns the bound observation-batch identity.
    #[must_use]
    pub const fn judge_observation_batch_id(&self) -> &CandidateJudgeObservationBatchId {
        &self.judge_observation_batch_id
    }
    /// Returns the managed preflight aggregate digest.
    #[must_use]
    pub const fn managed_preflight_digest(&self) -> &Digest {
        &self.managed_preflight_digest
    }
    /// Returns the retained-session preflight response digest.
    #[must_use]
    pub const fn retained_session_preflight_digest(&self) -> &Digest {
        &self.retained_session_preflight_digest
    }
    /// Returns the schedule-indexed residency aggregate digest.
    #[must_use]
    pub const fn residency_receipt_aggregate_digest(&self) -> &Digest {
        &self.residency_receipt_aggregate_digest
    }
    /// Returns the schedule-indexed process-observation aggregate digest.
    #[must_use]
    pub const fn process_observation_aggregate_digest(&self) -> &Digest {
        &self.process_observation_aggregate_digest
    }
    /// Returns the schedule-indexed native-load aggregate digest.
    #[must_use]
    pub const fn native_load_observation_aggregate_digest(&self) -> &Digest {
        &self.native_load_observation_aggregate_digest
    }
    /// Returns the schedule-indexed connection aggregate digest.
    #[must_use]
    pub const fn connection_observation_aggregate_digest(&self) -> &Digest {
        &self.connection_observation_aggregate_digest
    }
    /// Returns the schedule-indexed effective-state aggregate digest.
    #[must_use]
    pub const fn effective_runtime_state_observation_aggregate_digest(&self) -> &Digest {
        &self.effective_runtime_state_observation_aggregate_digest
    }
    /// Returns the first resident response ordinal.
    #[must_use]
    pub const fn first_response_ordinal(&self) -> u64 {
        self.first_response_ordinal
    }
    /// Returns the last resident response ordinal.
    #[must_use]
    pub const fn last_response_ordinal(&self) -> u64 {
        self.last_response_ordinal
    }
    /// Returns the batch-specific effective-state join identity.
    #[must_use]
    pub const fn judge_effective_runtime_state_join_id(
        &self,
    ) -> &ManagedOllamaEffectiveRuntimeStateJoinId {
        &self.judge_effective_runtime_state_join_id
    }
    /// Returns the cleanup disposition, fixed successful in V1.
    #[must_use]
    pub const fn cleanup_disposition(&self) -> ManagedLocalJudgeReceiptSuccessStatusV1 {
        self.cleanup_disposition
    }
    /// Returns the runtime-package revalidation status.
    #[must_use]
    pub const fn runtime_package_revalidation_status(
        &self,
    ) -> ManagedLocalJudgeReceiptSuccessStatusV1 {
        self.runtime_package_revalidation_status
    }
    /// Returns the model-package revalidation status.
    #[must_use]
    pub const fn model_package_revalidation_status(
        &self,
    ) -> ManagedLocalJudgeReceiptSuccessStatusV1 {
        self.model_package_revalidation_status
    }
    /// Returns the triage-only managed judge evidence class.
    #[must_use]
    pub const fn evidence_class(&self) -> ManagedLocalJudgeEvidenceClassV1 {
        self.evidence_class
    }
}

impl fmt::Debug for ManagedLocalJudgeReceiptRecordV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagedLocalJudgeReceiptRecordV1")
            .field("managed_local_judge_receipt_id", &self.id)
            .field("attempt_count", &self.attempt_count)
            .field("evidence_class", &self.evidence_class)
            .finish_non_exhaustive()
    }
}
