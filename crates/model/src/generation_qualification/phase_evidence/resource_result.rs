//! Inert resource observations for one completed target attempt.

use std::fmt;

use schemars::JsonSchema;
use serde::Serialize;

use rewrite_types::Digest;

use super::super::{
    CandidateGenerationAttemptRecordId, CandidateGenerationAttemptRecordV1,
    CandidateGenerationReceiptId, CandidateGenerationReceiptV1, GenerationCaseId,
    GenerationCaseManifestV1, GenerationQualificationOperationPolicyV1,
    GenerationQualificationPlanId, GenerationRepetitionId, GenerationRepetitionRecordV1,
    GenerationSuiteManifestId, GenerationSystemId, PlannedCandidateAttemptId,
    PlannedCandidateAttemptV1,
};
use super::common::{
    GenerationQualificationPhaseEvidenceError, GenerationQualificationPhaseScopeV1,
    validate_canonical_json,
};
use super::phase_id;

mod accessors;
mod validation;
mod wire;

use validation::{canonical_bytes, validate_relationships};
use wire::ResourceResultWire;

/// Resource-attempt result identity domain.
pub const GENERATION_RESOURCE_ATTEMPT_RESULT_ID_DOMAIN: &[u8] =
    b"retonr:generation-resource-attempt-result:v1\0";
/// Maximum JSON bytes accepted for one resource-attempt result.
pub const MAX_GENERATION_RESOURCE_ATTEMPT_RESULT_JSON_BYTES: usize = 16_384;
pub(super) const MAX_GENERATION_RESOURCE_ATTEMPT_RESULT_CANONICAL_BYTES: usize = 4_096;

phase_id!(
    GenerationResourceAttemptResultId,
    "Content identity of one inert generation resource-attempt result."
);

/// Closed resource observation profile for V1 managed Ollama qualification.
#[derive(Clone, Copy, Debug, serde::Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationResourceObservationProfileV1 {
    /// Ollama 0.32.15 managed on the approved Linux V1 execution profile.
    ManagedOllamaV0_32_15LinuxV1,
}

/// Closed resource limit identity in required semantic order.
#[derive(Clone, Copy, Debug, serde::Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationResourceExceededLimitV1 {
    /// Attempt wall-clock elapsed limit.
    AttemptElapsed,
    /// Time-to-first-response limit.
    FirstResponse,
    /// Managed cleanup elapsed limit.
    Cleanup,
    /// Worker high-water resident-byte limit.
    WorkerHighWaterResident,
    /// Installed runtime-plus-model footprint limit.
    InstalledFootprint,
}

impl GenerationResourceExceededLimitV1 {
    pub(super) const fn tag(self) -> u8 {
        match self {
            Self::AttemptElapsed => 0,
            Self::FirstResponse => 1,
            Self::Cleanup => 2,
            Self::WorkerHighWaterResident => 3,
            Self::InstalledFootprint => 4,
        }
    }
}

/// Exact typed records required to derive one target resource result.
#[derive(Clone, Copy)]
pub struct GenerationResourceAttemptResultRecordV1Relations<'a> {
    /// Exact target, plan, and suite scope.
    pub scope: GenerationQualificationPhaseScopeV1<'a>,
    /// Exact operation policy that designates the scope system as target.
    pub operation_policy: &'a GenerationQualificationOperationPolicyV1,
    /// Exact case selected by the planned attempt.
    pub case: &'a GenerationCaseManifestV1,
    /// Exact repetition selected by the planned attempt.
    pub repetition: &'a GenerationRepetitionRecordV1,
    /// Exact completed planned attempt.
    pub planned_attempt: &'a PlannedCandidateAttemptV1,
    /// Exact completed attempt record.
    pub attempt_record: &'a CandidateGenerationAttemptRecordV1,
    /// Exact final candidate-generation receipt.
    pub candidate_generation_receipt: &'a CandidateGenerationReceiptV1,
}

/// Required caller-observed resource facts for one completed attempt.
///
/// These facts and `exceeded_limits` are inert portable data. Construction does
/// not establish conformance with an application-approved resource policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationResourceAttemptResultRecordV1Input {
    /// Observed prompt token count.
    pub prompt_token_count: u64,
    /// Observed generated token count.
    pub generated_token_count: u64,
    /// Provider-reported total duration in nanoseconds.
    pub total_duration_nanoseconds: u64,
    /// Provider-reported load duration in nanoseconds.
    pub load_duration_nanoseconds: u64,
    /// Provider-reported prompt-evaluation duration in nanoseconds.
    pub prompt_evaluation_duration_nanoseconds: u64,
    /// Provider-reported generation-evaluation duration in nanoseconds.
    pub evaluation_duration_nanoseconds: u64,
    /// Whole managed-attempt elapsed time in nanoseconds.
    pub attempt_elapsed_nanoseconds: u64,
    /// Time to first response in nanoseconds.
    pub first_response_elapsed_nanoseconds: u64,
    /// Managed cleanup elapsed time in nanoseconds.
    pub cleanup_elapsed_nanoseconds: u64,
    /// Worker high-water resident bytes.
    pub worker_high_water_resident_bytes: u64,
    /// Installed runtime payload bytes.
    pub runtime_installed_payload_bytes: u64,
    /// Installed model payload bytes.
    pub model_installed_payload_bytes: u64,
    /// Checked runtime-plus-model installed footprint bytes.
    pub installed_footprint_bytes: u64,
    /// Caller-supplied exceeded-limit identities in closed semantic order.
    pub exceeded_limits: Vec<GenerationResourceExceededLimitV1>,
}

/// Inert resource result for one completed target-system planned attempt.
///
/// The record validates exact typed relationships and observation consistency.
/// It does not prove measurement provenance, policy conformance, qualification,
/// activation, or live-use authority.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationResourceAttemptResultRecordV1 {
    schema_version: u32,
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
    #[serde(skip)]
    id: GenerationResourceAttemptResultId,
}

impl GenerationResourceAttemptResultRecordV1 {
    /// Derives one inert result from exact typed relationships and observations.
    ///
    /// # Errors
    ///
    /// Returns a content-free error for a non-target or cross-scope record,
    /// incomplete attempt closure, inconsistent observation, or invalid limit list.
    pub fn new(
        relations: GenerationResourceAttemptResultRecordV1Relations<'_>,
        input: GenerationResourceAttemptResultRecordV1Input,
    ) -> Result<Self, GenerationQualificationPhaseEvidenceError> {
        Self::build(relations, input, None)
    }

    /// Decodes canonical bounded JSON and revalidates every typed relationship.
    ///
    /// # Errors
    ///
    /// Returns a content-free error for oversized, malformed, noncanonical,
    /// unsupported, substituted, cross-scope, or internally inconsistent input.
    pub fn from_json_bytes(
        bytes: &[u8],
        relations: GenerationResourceAttemptResultRecordV1Relations<'_>,
        expected_input: &GenerationResourceAttemptResultRecordV1Input,
    ) -> Result<Self, GenerationQualificationPhaseEvidenceError> {
        if bytes.len() > MAX_GENERATION_RESOURCE_ATTEMPT_RESULT_JSON_BYTES {
            return Err(GenerationQualificationPhaseEvidenceError::EncodedRecordTooLarge);
        }
        let wire: ResourceResultWire = serde_json::from_slice(bytes)
            .map_err(|_| GenerationQualificationPhaseEvidenceError::InvalidEncoding)?;
        if wire.schema_version != super::super::GENERATION_QUALIFICATION_SCHEMA_VERSION {
            return Err(GenerationQualificationPhaseEvidenceError::UnsupportedSchema);
        }
        let value = Self::build(relations, expected_input.clone(), Some(&wire))?;
        validate_canonical_json(bytes, &value)?;
        Ok(value)
    }

    /// Revalidates every relationship, observation, and derived identity.
    ///
    /// # Errors
    ///
    /// Returns a content-free error if fresh checked derivation differs.
    pub fn validate_against(
        &self,
        relations: GenerationResourceAttemptResultRecordV1Relations<'_>,
        expected_input: &GenerationResourceAttemptResultRecordV1Input,
    ) -> Result<(), GenerationQualificationPhaseEvidenceError> {
        let expected = Self::new(relations, expected_input.clone())?;
        if &expected == self {
            Ok(())
        } else {
            Err(GenerationQualificationPhaseEvidenceError::RelationshipMismatch)
        }
    }

    fn build(
        relations: GenerationResourceAttemptResultRecordV1Relations<'_>,
        input: GenerationResourceAttemptResultRecordV1Input,
        wire: Option<&ResourceResultWire>,
    ) -> Result<Self, GenerationQualificationPhaseEvidenceError> {
        validate_relationships(relations, &input)?;
        let mut value = Self::from_relations(relations, input);
        let canonical = canonical_bytes(&value)?;
        value.id = GenerationResourceAttemptResultId::from_canonical_bytes(&canonical);
        if wire.is_some_and(|candidate| !candidate.matches(&value)) {
            return Err(GenerationQualificationPhaseEvidenceError::RelationshipMismatch);
        }
        Ok(value)
    }

    fn from_relations(
        relations: GenerationResourceAttemptResultRecordV1Relations<'_>,
        input: GenerationResourceAttemptResultRecordV1Input,
    ) -> Self {
        Self {
            schema_version: super::super::GENERATION_QUALIFICATION_SCHEMA_VERSION,
            generation_system_id: relations
                .scope
                .generation_system
                .generation_system_id()
                .clone(),
            generation_qualification_plan_id: relations
                .scope
                .qualification_plan
                .qualification_plan_id()
                .clone(),
            suite_manifest_id: relations.scope.suite.suite_manifest_id().clone(),
            case_id: relations.case.case_id().clone(),
            repetition_id: relations.repetition.repetition_id().clone(),
            planned_attempt_id: relations.planned_attempt.planned_attempt_id().clone(),
            attempt_record_id: relations.attempt_record.attempt_record_id().clone(),
            candidate_generation_receipt_id: relations
                .candidate_generation_receipt
                .receipt_id()
                .clone(),
            resource_policy_digest: relations.operation_policy.resource_policy_digest().clone(),
            observation_profile:
                GenerationResourceObservationProfileV1::ManagedOllamaV0_32_15LinuxV1,
            prompt_token_count: input.prompt_token_count,
            generated_token_count: input.generated_token_count,
            total_duration_nanoseconds: input.total_duration_nanoseconds,
            load_duration_nanoseconds: input.load_duration_nanoseconds,
            prompt_evaluation_duration_nanoseconds: input.prompt_evaluation_duration_nanoseconds,
            evaluation_duration_nanoseconds: input.evaluation_duration_nanoseconds,
            attempt_elapsed_nanoseconds: input.attempt_elapsed_nanoseconds,
            first_response_elapsed_nanoseconds: input.first_response_elapsed_nanoseconds,
            cleanup_elapsed_nanoseconds: input.cleanup_elapsed_nanoseconds,
            worker_high_water_resident_bytes: input.worker_high_water_resident_bytes,
            runtime_installed_payload_bytes: input.runtime_installed_payload_bytes,
            model_installed_payload_bytes: input.model_installed_payload_bytes,
            installed_footprint_bytes: input.installed_footprint_bytes,
            exceeded_limits: input.exceeded_limits,
            id: GenerationResourceAttemptResultId::from_canonical_bytes(b"uninitialized"),
        }
    }
}

impl fmt::Debug for GenerationResourceAttemptResultRecordV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GenerationResourceAttemptResultRecordV1")
            .field("resource_attempt_result_id", &self.id)
            .field("exceeded_limit_count", &self.exceeded_limits.len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
