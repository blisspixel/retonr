use std::fmt;

use super::{CandidateJudgeEvidenceClassV1, CandidateJudgeJoinRecordV1};
use crate::generation_qualification::{
    CandidateDeterministicEvaluationId, CandidateGenerationReceiptSetId, CandidateJudgeJoinId,
    CandidateJudgeObservationBatchId, CandidateJudgePlanId, CandidateJudgeRequestAggregateId,
    CandidateJudgeResponseAggregateId, CandidateJudgeScheduleId, CandidateReceiptPairSetId,
    FrozenExternalComponentSetId, GenerationSystemId, ManagedGenerationPathId,
    ManagedLocalJudgeReceiptId, ManagedOllamaEffectiveRuntimeStateJoinId, RuntimeAdmissionJoinId,
};
use crate::{
    ArtifactId, EffectivePackageEvidenceV2Id, EffectiveRuntimeStateId, ModelPackageManifestId,
    RuntimeBuildId, RuntimePackageManifestId,
};
use rewrite_types::Digest;

impl CandidateJudgeJoinRecordV1 {
    /// Returns the portable schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
    /// Returns the exact judge-plan identity.
    #[must_use]
    pub const fn candidate_judge_plan_id(&self) -> &CandidateJudgePlanId {
        &self.candidate_judge_plan_id
    }
    /// Returns the ordered candidate receipt-pair identity.
    #[must_use]
    pub const fn candidate_receipt_pair_set_id(&self) -> &CandidateReceiptPairSetId {
        &self.candidate_receipt_pair_set_id
    }
    /// Returns candidate A's exact receipt-set identity.
    #[must_use]
    pub const fn candidate_a_receipt_set_id(&self) -> &CandidateGenerationReceiptSetId {
        &self.candidate_a_receipt_set_id
    }
    /// Returns candidate B's exact receipt-set identity.
    #[must_use]
    pub const fn candidate_b_receipt_set_id(&self) -> &CandidateGenerationReceiptSetId {
        &self.candidate_b_receipt_set_id
    }
    /// Returns candidate A's generation-system identity.
    #[must_use]
    pub const fn candidate_a_generation_system_id(&self) -> &GenerationSystemId {
        &self.candidate_a_generation_system_id
    }
    /// Returns candidate B's generation-system identity.
    #[must_use]
    pub const fn candidate_b_generation_system_id(&self) -> &GenerationSystemId {
        &self.candidate_b_generation_system_id
    }
    /// Returns the judge generation-system identity.
    #[must_use]
    pub const fn judge_generation_system_id(&self) -> &GenerationSystemId {
        &self.judge_generation_system_id
    }
    /// Returns the passed deterministic evaluation identity.
    #[must_use]
    pub const fn deterministic_evaluation_id(&self) -> &CandidateDeterministicEvaluationId {
        &self.deterministic_evaluation_id
    }
    /// Returns the exact case-material set digest.
    #[must_use]
    pub const fn case_material_set_digest(&self) -> &Digest {
        &self.case_material_set_digest
    }
    /// Returns the permutation-aware suite-pair digest.
    #[must_use]
    pub const fn suite_pair_digest(&self) -> &Digest {
        &self.suite_pair_digest
    }
    /// Returns the exact two-order judge schedule identity.
    #[must_use]
    pub const fn judge_schedule_id(&self) -> &CandidateJudgeScheduleId {
        &self.judge_schedule_id
    }
    /// Returns the successfully closed managed receipt identity.
    #[must_use]
    pub const fn managed_local_judge_receipt_id(&self) -> &ManagedLocalJudgeReceiptId {
        &self.managed_local_judge_receipt_id
    }
    /// Returns the ordered request aggregate identity.
    #[must_use]
    pub const fn judge_request_aggregate_id(&self) -> &CandidateJudgeRequestAggregateId {
        &self.judge_request_aggregate_id
    }
    /// Returns the ordered response aggregate identity.
    #[must_use]
    pub const fn judge_response_aggregate_id(&self) -> &CandidateJudgeResponseAggregateId {
        &self.judge_response_aggregate_id
    }
    /// Returns the normalized observation-batch identity.
    #[must_use]
    pub const fn judge_observation_batch_id(&self) -> &CandidateJudgeObservationBatchId {
        &self.judge_observation_batch_id
    }
    /// Returns the exact successful attempt count.
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
    /// Returns the judge effective-runtime-state join identity.
    #[must_use]
    pub const fn judge_effective_runtime_state_join_id(
        &self,
    ) -> &ManagedOllamaEffectiveRuntimeStateJoinId {
        &self.judge_effective_runtime_state_join_id
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
    /// Returns the retained judge runtime installation generation.
    #[must_use]
    pub const fn judge_runtime_installation_generation(&self) -> u64 {
        self.judge_runtime_installation_generation
    }
    /// Returns the retained judge model installation generation.
    #[must_use]
    pub const fn judge_model_installation_generation(&self) -> u64 {
        self.judge_model_installation_generation
    }
    /// Returns the inert canonical triage-report digest.
    #[must_use]
    pub const fn triage_report_digest(&self) -> &Digest {
        &self.triage_report_digest
    }
    /// Returns the content-derived join identity.
    #[must_use]
    pub const fn candidate_judge_join_id(&self) -> &CandidateJudgeJoinId {
        &self.id
    }
    /// Returns the triage-only evidence class.
    #[must_use]
    pub const fn evidence_class(&self) -> CandidateJudgeEvidenceClassV1 {
        self.evidence_class
    }
    /// Returns false because this record never proves candidate semantics.
    #[must_use]
    pub const fn candidate_semantics_proven(&self) -> bool {
        self.candidate_semantics_proven
    }
    /// Returns false because this record never proves judge correctness.
    #[must_use]
    pub const fn judge_correctness_proven(&self) -> bool {
        self.judge_correctness_proven
    }
    /// Returns false because this join grants no qualification authority.
    #[must_use]
    pub const fn qualified(&self) -> bool {
        self.qualified
    }
}

impl fmt::Debug for CandidateJudgeJoinRecordV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CandidateJudgeJoinRecordV1")
            .field("candidate_judge_join_id", &self.id)
            .field("attempt_count", &self.attempt_count)
            .field("evidence_class", &self.evidence_class)
            .finish_non_exhaustive()
    }
}
