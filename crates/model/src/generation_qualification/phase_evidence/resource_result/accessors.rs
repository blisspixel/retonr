use super::{
    GenerationResourceAttemptResultId, GenerationResourceAttemptResultRecordV1,
    GenerationResourceExceededLimitV1, GenerationResourceObservationProfileV1,
};
use crate::generation_qualification::{
    CandidateGenerationAttemptRecordId, CandidateGenerationReceiptId, GenerationCaseId,
    GenerationQualificationPlanId, GenerationRepetitionId, GenerationSuiteManifestId,
    GenerationSystemId, PlannedCandidateAttemptId,
};
use rewrite_types::Digest;

impl GenerationResourceAttemptResultRecordV1 {
    /// Returns the record schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
    /// Returns the exact target generation system.
    #[must_use]
    pub const fn generation_system_id(&self) -> &GenerationSystemId {
        &self.generation_system_id
    }
    /// Returns the exact qualification plan.
    #[must_use]
    pub const fn generation_qualification_plan_id(&self) -> &GenerationQualificationPlanId {
        &self.generation_qualification_plan_id
    }
    /// Returns the exact suite manifest.
    #[must_use]
    pub const fn suite_manifest_id(&self) -> &GenerationSuiteManifestId {
        &self.suite_manifest_id
    }
    /// Returns the exact case.
    #[must_use]
    pub const fn case_id(&self) -> &GenerationCaseId {
        &self.case_id
    }
    /// Returns the exact repetition.
    #[must_use]
    pub const fn repetition_id(&self) -> &GenerationRepetitionId {
        &self.repetition_id
    }
    /// Returns the exact planned attempt.
    #[must_use]
    pub const fn planned_attempt_id(&self) -> &PlannedCandidateAttemptId {
        &self.planned_attempt_id
    }
    /// Returns the exact completed attempt record.
    #[must_use]
    pub const fn attempt_record_id(&self) -> &CandidateGenerationAttemptRecordId {
        &self.attempt_record_id
    }
    /// Returns the exact final candidate-generation receipt.
    #[must_use]
    pub const fn candidate_generation_receipt_id(&self) -> &CandidateGenerationReceiptId {
        &self.candidate_generation_receipt_id
    }
    /// Returns the exact preregistered resource-policy digest.
    #[must_use]
    pub const fn resource_policy_digest(&self) -> &Digest {
        &self.resource_policy_digest
    }
    /// Returns the closed observation profile.
    #[must_use]
    pub const fn observation_profile(&self) -> GenerationResourceObservationProfileV1 {
        self.observation_profile
    }
    /// Returns the prompt token count.
    #[must_use]
    pub const fn prompt_token_count(&self) -> u64 {
        self.prompt_token_count
    }
    /// Returns the generated token count.
    #[must_use]
    pub const fn generated_token_count(&self) -> u64 {
        self.generated_token_count
    }
    /// Returns provider total duration in nanoseconds.
    #[must_use]
    pub const fn total_duration_nanoseconds(&self) -> u64 {
        self.total_duration_nanoseconds
    }
    /// Returns provider load duration in nanoseconds.
    #[must_use]
    pub const fn load_duration_nanoseconds(&self) -> u64 {
        self.load_duration_nanoseconds
    }
    /// Returns provider prompt-evaluation duration in nanoseconds.
    #[must_use]
    pub const fn prompt_evaluation_duration_nanoseconds(&self) -> u64 {
        self.prompt_evaluation_duration_nanoseconds
    }
    /// Returns provider generation-evaluation duration in nanoseconds.
    #[must_use]
    pub const fn evaluation_duration_nanoseconds(&self) -> u64 {
        self.evaluation_duration_nanoseconds
    }
    /// Returns whole attempt elapsed time in nanoseconds.
    #[must_use]
    pub const fn attempt_elapsed_nanoseconds(&self) -> u64 {
        self.attempt_elapsed_nanoseconds
    }
    /// Returns time to first response in nanoseconds.
    #[must_use]
    pub const fn first_response_elapsed_nanoseconds(&self) -> u64 {
        self.first_response_elapsed_nanoseconds
    }
    /// Returns managed cleanup elapsed time in nanoseconds.
    #[must_use]
    pub const fn cleanup_elapsed_nanoseconds(&self) -> u64 {
        self.cleanup_elapsed_nanoseconds
    }
    /// Returns worker high-water resident bytes.
    #[must_use]
    pub const fn worker_high_water_resident_bytes(&self) -> u64 {
        self.worker_high_water_resident_bytes
    }
    /// Returns installed runtime payload bytes.
    #[must_use]
    pub const fn runtime_installed_payload_bytes(&self) -> u64 {
        self.runtime_installed_payload_bytes
    }
    /// Returns installed model payload bytes.
    #[must_use]
    pub const fn model_installed_payload_bytes(&self) -> u64 {
        self.model_installed_payload_bytes
    }
    /// Returns checked runtime-plus-model installed footprint bytes.
    #[must_use]
    pub const fn installed_footprint_bytes(&self) -> u64 {
        self.installed_footprint_bytes
    }
    /// Returns caller-supplied exceeded-limit identities in semantic order.
    #[must_use]
    pub fn exceeded_limits(&self) -> &[GenerationResourceExceededLimitV1] {
        &self.exceeded_limits
    }
    /// Returns the content-derived result identity.
    #[must_use]
    pub const fn resource_attempt_result_id(&self) -> &GenerationResourceAttemptResultId {
        &self.id
    }
}
