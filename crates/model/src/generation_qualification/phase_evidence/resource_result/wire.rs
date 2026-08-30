use schemars::JsonSchema;
use serde::Deserialize;

use rewrite_types::Digest;

use super::{
    GenerationResourceAttemptResultRecordV1, GenerationResourceExceededLimitV1,
    GenerationResourceObservationProfileV1,
};
use crate::generation_qualification::{
    CandidateGenerationAttemptRecordId, CandidateGenerationReceiptId, GenerationCaseId,
    GenerationQualificationPlanId, GenerationRepetitionId, GenerationSuiteManifestId,
    GenerationSystemId, PlannedCandidateAttemptId,
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct ResourceResultWire {
    pub(super) schema_version: u32,
    generation_system_id: GenerationSystemId,
    generation_qualification_plan_id: GenerationQualificationPlanId,
    suite_manifest_id: GenerationSuiteManifestId,
    case_id: GenerationCaseId,
    repetition_id: GenerationRepetitionId,
    planned_attempt_id: PlannedCandidateAttemptId,
    attempt_record_id: CandidateGenerationAttemptRecordId,
    candidate_generation_receipt_id: CandidateGenerationReceiptId,
    resource_policy_digest: Digest,
    observation_profile: GenerationResourceObservationProfileV1,
    prompt_token_count: u64,
    generated_token_count: u64,
    total_duration_nanoseconds: u64,
    load_duration_nanoseconds: u64,
    prompt_evaluation_duration_nanoseconds: u64,
    evaluation_duration_nanoseconds: u64,
    attempt_elapsed_nanoseconds: u64,
    first_response_elapsed_nanoseconds: u64,
    cleanup_elapsed_nanoseconds: u64,
    worker_high_water_resident_bytes: u64,
    runtime_installed_payload_bytes: u64,
    model_installed_payload_bytes: u64,
    installed_footprint_bytes: u64,
    exceeded_limits: Vec<GenerationResourceExceededLimitV1>,
}

impl ResourceResultWire {
    pub(super) fn matches(&self, value: &GenerationResourceAttemptResultRecordV1) -> bool {
        self.schema_version == value.schema_version
            && self.generation_system_id == value.generation_system_id
            && self.generation_qualification_plan_id == value.generation_qualification_plan_id
            && self.suite_manifest_id == value.suite_manifest_id
            && self.case_id == value.case_id
            && self.repetition_id == value.repetition_id
            && self.planned_attempt_id == value.planned_attempt_id
            && self.attempt_record_id == value.attempt_record_id
            && self.candidate_generation_receipt_id == value.candidate_generation_receipt_id
            && self.resource_policy_digest == value.resource_policy_digest
            && self.observation_profile == value.observation_profile
            && self.prompt_token_count == value.prompt_token_count
            && self.generated_token_count == value.generated_token_count
            && self.total_duration_nanoseconds == value.total_duration_nanoseconds
            && self.load_duration_nanoseconds == value.load_duration_nanoseconds
            && self.prompt_evaluation_duration_nanoseconds
                == value.prompt_evaluation_duration_nanoseconds
            && self.evaluation_duration_nanoseconds == value.evaluation_duration_nanoseconds
            && self.attempt_elapsed_nanoseconds == value.attempt_elapsed_nanoseconds
            && self.first_response_elapsed_nanoseconds == value.first_response_elapsed_nanoseconds
            && self.cleanup_elapsed_nanoseconds == value.cleanup_elapsed_nanoseconds
            && self.worker_high_water_resident_bytes == value.worker_high_water_resident_bytes
            && self.runtime_installed_payload_bytes == value.runtime_installed_payload_bytes
            && self.model_installed_payload_bytes == value.model_installed_payload_bytes
            && self.installed_footprint_bytes == value.installed_footprint_bytes
            && self.exceeded_limits == value.exceeded_limits
    }
}
