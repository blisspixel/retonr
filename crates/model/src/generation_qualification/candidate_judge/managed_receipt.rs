use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{
    CANDIDATE_JUDGE_SCHEMA_VERSION, CandidateJudgeObservationBatchV1, CandidateJudgePlanV1,
    CandidateJudgeRequestAggregateV1, CandidateJudgeResponseAggregateV1, CandidateJudgeScheduleV1,
    MAX_MANAGED_LOCAL_JUDGE_RECEIPT_JSON_BYTES,
};
use crate::generation_qualification::codec::{
    append_digest, append_u32, append_u64, validate_canonical_json,
};
use crate::generation_qualification::{
    CandidateJudgeObservationBatchId, CandidateJudgePlanId, CandidateJudgeRequestAggregateId,
    CandidateJudgeResponseAggregateId, CandidateJudgeScheduleId, FrozenExternalComponentSetId,
    GENERATION_QUALIFICATION_SCHEMA_VERSION, GenerationQualificationContractError,
    GenerationSystemId, GenerationSystemRecordV1, MANAGED_LOCAL_JUDGE_RECEIPT_ID_DOMAIN,
    ManagedGenerationPathId, ManagedLocalJudgeReceiptId, ManagedOllamaEffectiveRuntimeStateJoinId,
    RuntimeAdmissionJoinId,
};
use crate::{
    ArtifactId, EffectivePackageEvidenceV2Id, EffectiveRuntimeStateId, ModelPackageManifestId,
    RuntimeBuildId, RuntimePackageManifestId,
};
use rewrite_types::Digest;

mod accessors;

const MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT_V1: u64 = 7;
const MANAGED_JUDGE_RESPONSES_PER_ATTEMPT_V1: u64 = 9;
const FIRST_MANAGED_JUDGE_RESPONSE_ORDINAL_V1: u64 = MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT_V1 + 1;

/// Closed successful postcondition vocabulary for a managed judge bracket.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagedLocalJudgeReceiptSuccessStatusV1 {
    /// The named cleanup or package revalidation completed successfully.
    Succeeded,
}

/// Closed evidence class for version 1 managed local-judge receipts.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagedLocalJudgeEvidenceClassV1 {
    /// Managed local-judge evidence remains probabilistic triage evidence.
    ManagedLocalJudgeTriage,
}

/// Exact portable records reloaded while deriving or decoding a managed receipt.
#[derive(Clone, Copy)]
pub struct ManagedLocalJudgeReceiptRecordV1Relations<'a> {
    /// Exact judge plan.
    pub plan: &'a CandidateJudgePlanV1,
    /// Exact complete judge schedule.
    pub schedule: &'a CandidateJudgeScheduleV1,
    /// Exact separately loaded judge system.
    pub judge_system: &'a GenerationSystemRecordV1,
    /// Ordered content-free request identities.
    pub request_aggregate: &'a CandidateJudgeRequestAggregateV1,
    /// Ordered content-free response identities.
    pub response_aggregate: &'a CandidateJudgeResponseAggregateV1,
    /// Ordered normalized observation identities.
    pub observation_batch: &'a CandidateJudgeObservationBatchV1,
}

/// App-derived inert facts bound by one managed local-judge receipt record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedLocalJudgeReceiptRecordV1Input {
    /// Exact runtime installation generation retained through the bracket.
    pub judge_runtime_installation_generation: u64,
    /// Exact model installation generation retained through the bracket.
    pub judge_model_installation_generation: u64,
    /// Aggregate of all pre-traffic managed preflight evidence.
    pub managed_preflight_digest: Digest,
    /// Exact retained-session preflight response digest.
    pub retained_session_preflight_digest: Digest,
    /// Schedule-indexed residency-receipt aggregate digest.
    pub residency_receipt_aggregate_digest: Digest,
    /// Schedule-indexed process-observation aggregate digest.
    pub process_observation_aggregate_digest: Digest,
    /// Schedule-indexed native-load-observation aggregate digest.
    pub native_load_observation_aggregate_digest: Digest,
    /// Schedule-indexed connection-observation aggregate digest.
    pub connection_observation_aggregate_digest: Digest,
    /// Schedule-indexed effective-state-observation aggregate digest.
    pub effective_runtime_state_observation_aggregate_digest: Digest,
    /// App-owned current-state join for the complete managed bracket.
    pub judge_effective_runtime_state_join_id: ManagedOllamaEffectiveRuntimeStateJoinId,
    /// First retained response ordinal in the resident session.
    pub first_response_ordinal: u64,
    /// Last retained response ordinal in the resident session.
    pub last_response_ordinal: u64,
}

/// Inert portable receipt for one successfully closed managed judge bracket.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedLocalJudgeReceiptRecordV1 {
    schema_version: u32,
    candidate_judge_plan_id: CandidateJudgePlanId,
    candidate_judge_schedule_id: CandidateJudgeScheduleId,
    judge_generation_system_id: GenerationSystemId,
    judge_runtime_admission_join_id: RuntimeAdmissionJoinId,
    judge_managed_generation_path_id: ManagedGenerationPathId,
    judge_frozen_external_component_set_id: FrozenExternalComponentSetId,
    judge_runtime_package_manifest_id: RuntimePackageManifestId,
    judge_runtime_build_id: RuntimeBuildId,
    judge_model_package_manifest_id: ModelPackageManifestId,
    judge_model_artifact_id: ArtifactId,
    judge_runtime_installation_generation: u64,
    judge_model_installation_generation: u64,
    managed_preflight_digest: Digest,
    retained_session_preflight_digest: Digest,
    judge_request_aggregate_id: CandidateJudgeRequestAggregateId,
    judge_response_aggregate_id: CandidateJudgeResponseAggregateId,
    judge_observation_batch_id: CandidateJudgeObservationBatchId,
    residency_receipt_aggregate_digest: Digest,
    process_observation_aggregate_digest: Digest,
    native_load_observation_aggregate_digest: Digest,
    connection_observation_aggregate_digest: Digest,
    effective_runtime_state_observation_aggregate_digest: Digest,
    judge_effective_runtime_state_id: EffectiveRuntimeStateId,
    judge_effective_runtime_state_join_id: ManagedOllamaEffectiveRuntimeStateJoinId,
    judge_effective_package_evidence_v2_id: EffectivePackageEvidenceV2Id,
    first_response_ordinal: u64,
    last_response_ordinal: u64,
    attempt_count: u32,
    cleanup_disposition: ManagedLocalJudgeReceiptSuccessStatusV1,
    runtime_package_revalidation_status: ManagedLocalJudgeReceiptSuccessStatusV1,
    model_package_revalidation_status: ManagedLocalJudgeReceiptSuccessStatusV1,
    evidence_class: ManagedLocalJudgeEvidenceClassV1,
    #[serde(skip)]
    id: ManagedLocalJudgeReceiptId,
}

impl ManagedLocalJudgeReceiptRecordV1 {
    /// Creates one inert record for a fully successful managed bracket.
    ///
    /// # Errors
    ///
    /// Returns an error unless every aggregate and successful postcondition closes.
    pub fn new(
        relations: ManagedLocalJudgeReceiptRecordV1Relations<'_>,
        input: ManagedLocalJudgeReceiptRecordV1Input,
    ) -> Result<Self, GenerationQualificationContractError> {
        Self::build(CANDIDATE_JUDGE_SCHEMA_VERSION, relations, input, None)
    }

    fn build(
        schema_version: u32,
        relations: ManagedLocalJudgeReceiptRecordV1Relations<'_>,
        input: ManagedLocalJudgeReceiptRecordV1Input,
        expected: Option<&Wire>,
    ) -> Result<Self, GenerationQualificationContractError> {
        validate_relations(schema_version, relations, &input)?;
        let attempt_count = relations.schedule.entry_count();
        let mut value = Self {
            schema_version,
            candidate_judge_plan_id: relations.plan.candidate_judge_plan_id().clone(),
            candidate_judge_schedule_id: relations.schedule.candidate_judge_schedule_id().clone(),
            judge_generation_system_id: relations.judge_system.generation_system_id().clone(),
            judge_runtime_admission_join_id: relations
                .judge_system
                .runtime_admission_join_id()
                .clone(),
            judge_managed_generation_path_id: relations
                .judge_system
                .managed_generation_path_id()
                .clone(),
            judge_frozen_external_component_set_id: relations
                .judge_system
                .frozen_external_component_set_id()
                .clone(),
            judge_runtime_package_manifest_id: relations
                .judge_system
                .runtime_package_manifest_id()
                .clone(),
            judge_runtime_build_id: relations.judge_system.runtime_build_id().clone(),
            judge_model_package_manifest_id: relations
                .judge_system
                .model_package_manifest_id()
                .clone(),
            judge_model_artifact_id: relations.judge_system.model_artifact_id().clone(),
            judge_runtime_installation_generation: input.judge_runtime_installation_generation,
            judge_model_installation_generation: input.judge_model_installation_generation,
            managed_preflight_digest: input.managed_preflight_digest,
            retained_session_preflight_digest: input.retained_session_preflight_digest,
            judge_request_aggregate_id: relations.request_aggregate.request_aggregate_id().clone(),
            judge_response_aggregate_id: relations
                .response_aggregate
                .response_aggregate_id()
                .clone(),
            judge_observation_batch_id: relations.observation_batch.observation_batch_id().clone(),
            residency_receipt_aggregate_digest: input.residency_receipt_aggregate_digest,
            process_observation_aggregate_digest: input.process_observation_aggregate_digest,
            native_load_observation_aggregate_digest: input
                .native_load_observation_aggregate_digest,
            connection_observation_aggregate_digest: input.connection_observation_aggregate_digest,
            effective_runtime_state_observation_aggregate_digest: input
                .effective_runtime_state_observation_aggregate_digest,
            judge_effective_runtime_state_id: relations
                .judge_system
                .effective_runtime_state_id()
                .clone(),
            judge_effective_runtime_state_join_id: input.judge_effective_runtime_state_join_id,
            judge_effective_package_evidence_v2_id: relations
                .judge_system
                .effective_package_evidence_v2_id()
                .clone(),
            first_response_ordinal: input.first_response_ordinal,
            last_response_ordinal: input.last_response_ordinal,
            attempt_count,
            cleanup_disposition: ManagedLocalJudgeReceiptSuccessStatusV1::Succeeded,
            runtime_package_revalidation_status: ManagedLocalJudgeReceiptSuccessStatusV1::Succeeded,
            model_package_revalidation_status: ManagedLocalJudgeReceiptSuccessStatusV1::Succeeded,
            evidence_class: ManagedLocalJudgeEvidenceClassV1::ManagedLocalJudgeTriage,
            id: ManagedLocalJudgeReceiptId(Digest::sha256(b"uninitialized judge receipt")),
        };
        if expected.is_some_and(|wire| !wire.matches(&value)) {
            return Err(
                GenerationQualificationContractError::ManagedLocalJudgeReceiptRelationshipMismatch,
            );
        }
        value.id = ManagedLocalJudgeReceiptId(Digest::sha256(&value.canonical_bytes()));
        if serde_json::to_vec(&value)
            .map_err(|_| GenerationQualificationContractError::InvalidEncoding)?
            .len()
            > MAX_MANAGED_LOCAL_JUDGE_RECEIPT_JSON_BYTES
        {
            return Err(GenerationQualificationContractError::EncodedRecordTooLarge);
        }
        Ok(value)
    }

    /// Parses bounded canonical JSON and reloads every portable relationship.
    ///
    /// # Errors
    ///
    /// Returns an error for oversized, malformed, noncanonical, or stale input.
    pub fn from_json_bytes(
        bytes: &[u8],
        relations: ManagedLocalJudgeReceiptRecordV1Relations<'_>,
    ) -> Result<Self, GenerationQualificationContractError> {
        if bytes.len() > MAX_MANAGED_LOCAL_JUDGE_RECEIPT_JSON_BYTES {
            return Err(GenerationQualificationContractError::EncodedRecordTooLarge);
        }
        let wire: Wire = serde_json::from_slice(bytes)
            .map_err(|_| GenerationQualificationContractError::InvalidEncoding)?;
        let input = wire.input();
        let value = Self::build(wire.schema_version, relations, input, Some(&wire))?;
        validate_canonical_json(bytes, &value)?;
        Ok(value)
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        let mut output = MANAGED_LOCAL_JUDGE_RECEIPT_ID_DOMAIN.to_vec();
        append_u32(&mut output, self.schema_version);
        for digest in self.identity_digests_before_generations() {
            append_digest(&mut output, digest);
        }
        append_u64(&mut output, self.judge_runtime_installation_generation);
        append_u64(&mut output, self.judge_model_installation_generation);
        for digest in self.identity_digests_after_generations() {
            append_digest(&mut output, digest);
        }
        append_u64(&mut output, self.first_response_ordinal);
        append_u64(&mut output, self.last_response_ordinal);
        append_u32(&mut output, self.attempt_count);
        output.extend_from_slice(&[0, 0, 0, 0]);
        output
    }

    fn identity_digests_before_generations(&self) -> [&Digest; 10] {
        [
            self.candidate_judge_plan_id.digest(),
            self.candidate_judge_schedule_id.digest(),
            self.judge_generation_system_id.digest(),
            self.judge_runtime_admission_join_id.digest(),
            self.judge_managed_generation_path_id.digest(),
            self.judge_frozen_external_component_set_id.digest(),
            self.judge_runtime_package_manifest_id.digest(),
            self.judge_runtime_build_id.digest(),
            self.judge_model_package_manifest_id.digest(),
            self.judge_model_artifact_id.digest(),
        ]
    }

    fn identity_digests_after_generations(&self) -> [&Digest; 13] {
        [
            &self.managed_preflight_digest,
            &self.retained_session_preflight_digest,
            self.judge_request_aggregate_id.digest(),
            self.judge_response_aggregate_id.digest(),
            self.judge_observation_batch_id.digest(),
            &self.residency_receipt_aggregate_digest,
            &self.process_observation_aggregate_digest,
            &self.native_load_observation_aggregate_digest,
            &self.connection_observation_aggregate_digest,
            &self.effective_runtime_state_observation_aggregate_digest,
            self.judge_effective_runtime_state_id.digest(),
            self.judge_effective_runtime_state_join_id.digest(),
            self.judge_effective_package_evidence_v2_id.digest(),
        ]
    }
}

fn validate_relations(
    schema_version: u32,
    relations: ManagedLocalJudgeReceiptRecordV1Relations<'_>,
    input: &ManagedLocalJudgeReceiptRecordV1Input,
) -> Result<(), GenerationQualificationContractError> {
    if schema_version != CANDIDATE_JUDGE_SCHEMA_VERSION
        || schema_version != GENERATION_QUALIFICATION_SCHEMA_VERSION
    {
        return Err(GenerationQualificationContractError::UnsupportedSchema(
            schema_version,
        ));
    }
    let count = relations.schedule.entry_count();
    let expected_last_response_ordinal = u64::from(count)
        .checked_mul(MANAGED_JUDGE_RESPONSES_PER_ATTEMPT_V1)
        .and_then(|response_count| {
            MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT_V1.checked_add(response_count)
        });
    if relations.schedule.candidate_judge_plan_id() != relations.plan.candidate_judge_plan_id()
        || relations.plan.judge_generation_system_id()
            != relations.judge_system.generation_system_id()
        || relations.request_aggregate.candidate_judge_plan_id()
            != relations.plan.candidate_judge_plan_id()
        || relations.request_aggregate.candidate_judge_schedule_id()
            != relations.schedule.candidate_judge_schedule_id()
        || relations.response_aggregate.candidate_judge_plan_id()
            != relations.plan.candidate_judge_plan_id()
        || relations.response_aggregate.candidate_judge_schedule_id()
            != relations.schedule.candidate_judge_schedule_id()
        || relations
            .response_aggregate
            .candidate_judge_request_aggregate_id()
            != relations.request_aggregate.request_aggregate_id()
        || relations.observation_batch.candidate_judge_plan_id()
            != relations.plan.candidate_judge_plan_id()
        || relations.observation_batch.candidate_judge_schedule_id()
            != relations.schedule.candidate_judge_schedule_id()
        || relations
            .observation_batch
            .candidate_judge_request_aggregate_id()
            != relations.request_aggregate.request_aggregate_id()
        || relations.response_aggregate.entry_count() != count
        || relations.request_aggregate.entry_count() != count
        || relations.observation_batch.entry_count() != count
        || relations
            .observation_batch
            .observations()
            .iter()
            .zip(relations.response_aggregate.responses())
            .any(|(observation, response)| {
                observation
                    .candidate_judge_response()
                    .candidate_judge_response_id()
                    != response.candidate_judge_response_id()
            })
        || input.judge_runtime_installation_generation == 0
        || input.judge_model_installation_generation == 0
        || input.first_response_ordinal != FIRST_MANAGED_JUDGE_RESPONSE_ORDINAL_V1
        || expected_last_response_ordinal != Some(input.last_response_ordinal)
    {
        return Err(
            GenerationQualificationContractError::ManagedLocalJudgeReceiptRelationshipMismatch,
        );
    }
    Ok(())
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    schema_version: u32,
    candidate_judge_plan_id: CandidateJudgePlanId,
    candidate_judge_schedule_id: CandidateJudgeScheduleId,
    judge_generation_system_id: GenerationSystemId,
    judge_runtime_admission_join_id: RuntimeAdmissionJoinId,
    judge_managed_generation_path_id: ManagedGenerationPathId,
    judge_frozen_external_component_set_id: FrozenExternalComponentSetId,
    judge_runtime_package_manifest_id: RuntimePackageManifestId,
    judge_runtime_build_id: RuntimeBuildId,
    judge_model_package_manifest_id: ModelPackageManifestId,
    judge_model_artifact_id: ArtifactId,
    judge_runtime_installation_generation: u64,
    judge_model_installation_generation: u64,
    managed_preflight_digest: Digest,
    retained_session_preflight_digest: Digest,
    judge_request_aggregate_id: CandidateJudgeRequestAggregateId,
    judge_response_aggregate_id: CandidateJudgeResponseAggregateId,
    judge_observation_batch_id: CandidateJudgeObservationBatchId,
    residency_receipt_aggregate_digest: Digest,
    process_observation_aggregate_digest: Digest,
    native_load_observation_aggregate_digest: Digest,
    connection_observation_aggregate_digest: Digest,
    effective_runtime_state_observation_aggregate_digest: Digest,
    judge_effective_runtime_state_id: EffectiveRuntimeStateId,
    judge_effective_runtime_state_join_id: ManagedOllamaEffectiveRuntimeStateJoinId,
    judge_effective_package_evidence_v2_id: EffectivePackageEvidenceV2Id,
    first_response_ordinal: u64,
    last_response_ordinal: u64,
    attempt_count: u32,
    cleanup_disposition: ManagedLocalJudgeReceiptSuccessStatusV1,
    runtime_package_revalidation_status: ManagedLocalJudgeReceiptSuccessStatusV1,
    model_package_revalidation_status: ManagedLocalJudgeReceiptSuccessStatusV1,
    evidence_class: ManagedLocalJudgeEvidenceClassV1,
}

impl Wire {
    fn input(&self) -> ManagedLocalJudgeReceiptRecordV1Input {
        ManagedLocalJudgeReceiptRecordV1Input {
            judge_runtime_installation_generation: self.judge_runtime_installation_generation,
            judge_model_installation_generation: self.judge_model_installation_generation,
            managed_preflight_digest: self.managed_preflight_digest.clone(),
            retained_session_preflight_digest: self.retained_session_preflight_digest.clone(),
            residency_receipt_aggregate_digest: self.residency_receipt_aggregate_digest.clone(),
            process_observation_aggregate_digest: self.process_observation_aggregate_digest.clone(),
            native_load_observation_aggregate_digest: self
                .native_load_observation_aggregate_digest
                .clone(),
            connection_observation_aggregate_digest: self
                .connection_observation_aggregate_digest
                .clone(),
            effective_runtime_state_observation_aggregate_digest: self
                .effective_runtime_state_observation_aggregate_digest
                .clone(),
            judge_effective_runtime_state_join_id: self
                .judge_effective_runtime_state_join_id
                .clone(),
            first_response_ordinal: self.first_response_ordinal,
            last_response_ordinal: self.last_response_ordinal,
        }
    }

    fn matches(&self, value: &ManagedLocalJudgeReceiptRecordV1) -> bool {
        serde_json::to_value(self).ok() == serde_json::to_value(value).ok()
    }
}
