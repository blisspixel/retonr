use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{
    CANDIDATE_JUDGE_SCHEMA_VERSION, CandidateJudgeObservationBatchV1, CandidateJudgePlanV1,
    CandidateJudgeRequestAggregateV1, CandidateJudgeResponseAggregateV1, CandidateJudgeScheduleV1,
    CandidateJudgeTriageReportRelationshipV1, MAX_CANDIDATE_JUDGE_JOIN_JSON_BYTES,
    ManagedLocalJudgeReceiptRecordV1,
};
use crate::generation_qualification::codec::{
    append_digest, append_u32, append_u64, validate_canonical_json,
};
use crate::generation_qualification::{
    CANDIDATE_JUDGE_JOIN_ID_DOMAIN, CandidateDeterministicEvaluationId,
    CandidateDeterministicEvaluationRecordV1, CandidateDeterministicEvaluationStatusV1,
    CandidateGenerationReceiptSetId, CandidateGenerationReceiptSetV1, CandidateJudgeJoinId,
    CandidateJudgeObservationBatchId, CandidateJudgePlanId, CandidateJudgeRequestAggregateId,
    CandidateJudgeResponseAggregateId, CandidateJudgeScheduleId, CandidateReceiptPairSetId,
    FrozenExternalComponentSetId, GENERATION_QUALIFICATION_SCHEMA_VERSION,
    GenerationQualificationContractError, GenerationSystemId, GenerationSystemRecordV1,
    ManagedGenerationPathId, ManagedLocalJudgeReceiptId, ManagedOllamaEffectiveRuntimeStateJoinId,
    RuntimeAdmissionJoinId,
};
use crate::{
    ArtifactId, EffectivePackageEvidenceV2Id, EffectiveRuntimeStateId, ModelPackageManifestId,
    RuntimeBuildId, RuntimePackageManifestId,
};
use rewrite_types::Digest;

mod accessors;

/// Closed evidence class for a portable candidate-to-judge join.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateJudgeEvidenceClassV1 {
    /// Managed local judge evidence remains probabilistic and triage-only.
    ManagedLocalJudgeTriage,
}

/// Exact portable records required to derive or decode one candidate-judge join.
#[derive(Clone, Copy)]
pub struct CandidateJudgeJoinRecordV1Relations<'a> {
    /// Exact pre-output judge plan.
    pub plan: &'a CandidateJudgePlanV1,
    /// Complete selected candidate A receipt set.
    pub candidate_a_receipt_set: &'a CandidateGenerationReceiptSetV1,
    /// Complete selected candidate B receipt set.
    pub candidate_b_receipt_set: &'a CandidateGenerationReceiptSetV1,
    /// Passed deterministic hard-gate record.
    pub deterministic_evaluation: &'a CandidateDeterministicEvaluationRecordV1,
    /// Exact two-order judge schedule.
    pub schedule: &'a CandidateJudgeScheduleV1,
    /// Complete request aggregate.
    pub request_aggregate: &'a CandidateJudgeRequestAggregateV1,
    /// Complete response aggregate.
    pub response_aggregate: &'a CandidateJudgeResponseAggregateV1,
    /// Complete normalized observation batch.
    pub observation_batch: &'a CandidateJudgeObservationBatchV1,
    /// Successfully closed managed local-judge receipt.
    pub managed_receipt: &'a ManagedLocalJudgeReceiptRecordV1,
    /// Separately reloaded judge generation system.
    pub judge_system: &'a GenerationSystemRecordV1,
    /// Inert framing of the canonical compatibility triage report.
    pub triage_report: &'a CandidateJudgeTriageReportRelationshipV1,
}

/// Portable inert exact relationship closure from candidates to judge evidence.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateJudgeJoinRecordV1 {
    schema_version: u32,
    candidate_judge_plan_id: CandidateJudgePlanId,
    candidate_receipt_pair_set_id: CandidateReceiptPairSetId,
    candidate_a_receipt_set_id: CandidateGenerationReceiptSetId,
    candidate_b_receipt_set_id: CandidateGenerationReceiptSetId,
    candidate_a_generation_system_id: GenerationSystemId,
    candidate_b_generation_system_id: GenerationSystemId,
    judge_generation_system_id: GenerationSystemId,
    deterministic_evaluation_id: CandidateDeterministicEvaluationId,
    case_material_set_digest: Digest,
    suite_pair_digest: Digest,
    judge_schedule_id: CandidateJudgeScheduleId,
    managed_local_judge_receipt_id: ManagedLocalJudgeReceiptId,
    judge_request_aggregate_id: CandidateJudgeRequestAggregateId,
    judge_response_aggregate_id: CandidateJudgeResponseAggregateId,
    judge_observation_batch_id: CandidateJudgeObservationBatchId,
    attempt_count: u32,
    judge_runtime_admission_join_id: RuntimeAdmissionJoinId,
    judge_managed_generation_path_id: ManagedGenerationPathId,
    judge_frozen_external_component_set_id: FrozenExternalComponentSetId,
    judge_runtime_package_manifest_id: RuntimePackageManifestId,
    judge_runtime_build_id: RuntimeBuildId,
    judge_effective_runtime_state_id: EffectiveRuntimeStateId,
    judge_effective_runtime_state_join_id: ManagedOllamaEffectiveRuntimeStateJoinId,
    judge_model_package_manifest_id: ModelPackageManifestId,
    judge_model_artifact_id: ArtifactId,
    judge_effective_package_evidence_v2_id: EffectivePackageEvidenceV2Id,
    judge_runtime_installation_generation: u64,
    judge_model_installation_generation: u64,
    triage_report_digest: Digest,
    evidence_class: CandidateJudgeEvidenceClassV1,
    candidate_semantics_proven: bool,
    judge_correctness_proven: bool,
    qualified: bool,
    #[serde(skip)]
    id: CandidateJudgeJoinId,
}

impl CandidateJudgeJoinRecordV1 {
    /// Derives one inert join after closing every named portable relationship.
    ///
    /// # Errors
    ///
    /// Returns an error for any stale, failed, foreign, or incomplete relationship.
    pub fn new(
        relations: CandidateJudgeJoinRecordV1Relations<'_>,
    ) -> Result<Self, GenerationQualificationContractError> {
        Self::build(CANDIDATE_JUDGE_SCHEMA_VERSION, relations, None)
    }

    fn build(
        schema_version: u32,
        relations: CandidateJudgeJoinRecordV1Relations<'_>,
        expected: Option<&Wire>,
    ) -> Result<Self, GenerationQualificationContractError> {
        validate_relations(schema_version, relations)?;
        let receipt = relations.managed_receipt;
        let deterministic = relations.deterministic_evaluation;
        let mut value = Self {
            schema_version,
            candidate_judge_plan_id: relations.plan.candidate_judge_plan_id().clone(),
            candidate_receipt_pair_set_id: deterministic.candidate_receipt_pair_set_id().clone(),
            candidate_a_receipt_set_id: relations.candidate_a_receipt_set.receipt_set_id().clone(),
            candidate_b_receipt_set_id: relations.candidate_b_receipt_set.receipt_set_id().clone(),
            candidate_a_generation_system_id: relations
                .candidate_a_receipt_set
                .generation_system_id()
                .clone(),
            candidate_b_generation_system_id: relations
                .candidate_b_receipt_set
                .generation_system_id()
                .clone(),
            judge_generation_system_id: relations.judge_system.generation_system_id().clone(),
            deterministic_evaluation_id: deterministic.deterministic_evaluation_id().clone(),
            case_material_set_digest: deterministic.case_material_set_digest().clone(),
            suite_pair_digest: deterministic.suite_pair_digest().clone(),
            judge_schedule_id: relations.schedule.candidate_judge_schedule_id().clone(),
            managed_local_judge_receipt_id: receipt.managed_local_judge_receipt_id().clone(),
            judge_request_aggregate_id: relations.request_aggregate.request_aggregate_id().clone(),
            judge_response_aggregate_id: relations
                .response_aggregate
                .response_aggregate_id()
                .clone(),
            judge_observation_batch_id: relations.observation_batch.observation_batch_id().clone(),
            attempt_count: receipt.attempt_count(),
            judge_runtime_admission_join_id: receipt.judge_runtime_admission_join_id().clone(),
            judge_managed_generation_path_id: receipt.judge_managed_generation_path_id().clone(),
            judge_frozen_external_component_set_id: receipt
                .judge_frozen_external_component_set_id()
                .clone(),
            judge_runtime_package_manifest_id: receipt.judge_runtime_package_manifest_id().clone(),
            judge_runtime_build_id: receipt.judge_runtime_build_id().clone(),
            judge_effective_runtime_state_id: receipt.judge_effective_runtime_state_id().clone(),
            judge_effective_runtime_state_join_id: receipt
                .judge_effective_runtime_state_join_id()
                .clone(),
            judge_model_package_manifest_id: receipt.judge_model_package_manifest_id().clone(),
            judge_model_artifact_id: receipt.judge_model_artifact_id().clone(),
            judge_effective_package_evidence_v2_id: receipt
                .judge_effective_package_evidence_v2_id()
                .clone(),
            judge_runtime_installation_generation: receipt.judge_runtime_installation_generation(),
            judge_model_installation_generation: receipt.judge_model_installation_generation(),
            triage_report_digest: relations.triage_report.digest().clone(),
            evidence_class: CandidateJudgeEvidenceClassV1::ManagedLocalJudgeTriage,
            candidate_semantics_proven: false,
            judge_correctness_proven: false,
            qualified: false,
            id: CandidateJudgeJoinId(Digest::sha256(b"uninitialized candidate judge join")),
        };
        if expected.is_some_and(|wire| !wire.matches(&value)) {
            return Err(
                GenerationQualificationContractError::CandidateJudgeJoinRelationshipMismatch,
            );
        }
        value.id = CandidateJudgeJoinId(Digest::sha256(&value.canonical_bytes()));
        if serde_json::to_vec(&value)
            .map_err(|_| GenerationQualificationContractError::InvalidEncoding)?
            .len()
            > MAX_CANDIDATE_JUDGE_JOIN_JSON_BYTES
        {
            return Err(GenerationQualificationContractError::EncodedRecordTooLarge);
        }
        Ok(value)
    }

    /// Parses bounded canonical JSON and reloads the complete portable closure.
    ///
    /// # Errors
    ///
    /// Returns an error for oversized, malformed, noncanonical, or stale input.
    pub fn from_json_bytes(
        bytes: &[u8],
        relations: CandidateJudgeJoinRecordV1Relations<'_>,
    ) -> Result<Self, GenerationQualificationContractError> {
        if bytes.len() > MAX_CANDIDATE_JUDGE_JOIN_JSON_BYTES {
            return Err(GenerationQualificationContractError::EncodedRecordTooLarge);
        }
        let wire: Wire = serde_json::from_slice(bytes)
            .map_err(|_| GenerationQualificationContractError::InvalidEncoding)?;
        let value = Self::build(wire.schema_version, relations, Some(&wire))?;
        validate_canonical_json(bytes, &value)?;
        Ok(value)
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        let mut output = CANDIDATE_JUDGE_JOIN_ID_DOMAIN.to_vec();
        append_u32(&mut output, self.schema_version);
        for digest in self.digests_before_attempt_count() {
            append_digest(&mut output, digest);
        }
        append_u32(&mut output, self.attempt_count);
        for digest in self.digests_after_attempt_count() {
            append_digest(&mut output, digest);
        }
        append_u64(&mut output, self.judge_runtime_installation_generation);
        append_u64(&mut output, self.judge_model_installation_generation);
        append_digest(&mut output, &self.triage_report_digest);
        output.extend_from_slice(&[0, 0, 0, 0]);
        output
    }

    fn digests_before_attempt_count(&self) -> [&Digest; 15] {
        [
            self.candidate_judge_plan_id.digest(),
            self.candidate_receipt_pair_set_id.digest(),
            self.candidate_a_receipt_set_id.digest(),
            self.candidate_b_receipt_set_id.digest(),
            self.candidate_a_generation_system_id.digest(),
            self.candidate_b_generation_system_id.digest(),
            self.judge_generation_system_id.digest(),
            self.deterministic_evaluation_id.digest(),
            &self.case_material_set_digest,
            &self.suite_pair_digest,
            self.judge_schedule_id.digest(),
            self.managed_local_judge_receipt_id.digest(),
            self.judge_request_aggregate_id.digest(),
            self.judge_response_aggregate_id.digest(),
            self.judge_observation_batch_id.digest(),
        ]
    }

    fn digests_after_attempt_count(&self) -> [&Digest; 10] {
        [
            self.judge_runtime_admission_join_id.digest(),
            self.judge_managed_generation_path_id.digest(),
            self.judge_frozen_external_component_set_id.digest(),
            self.judge_runtime_package_manifest_id.digest(),
            self.judge_runtime_build_id.digest(),
            self.judge_effective_runtime_state_id.digest(),
            self.judge_effective_runtime_state_join_id.digest(),
            self.judge_model_package_manifest_id.digest(),
            self.judge_model_artifact_id.digest(),
            self.judge_effective_package_evidence_v2_id.digest(),
        ]
    }
}

fn validate_relations(
    schema_version: u32,
    relations: CandidateJudgeJoinRecordV1Relations<'_>,
) -> Result<(), GenerationQualificationContractError> {
    if schema_version != CANDIDATE_JUDGE_SCHEMA_VERSION
        || schema_version != GENERATION_QUALIFICATION_SCHEMA_VERSION
    {
        return Err(GenerationQualificationContractError::UnsupportedSchema(
            schema_version,
        ));
    }
    let plan = relations.plan;
    let deterministic = relations.deterministic_evaluation;
    let receipt = relations.managed_receipt;
    if deterministic.status() != CandidateDeterministicEvaluationStatusV1::Passed
        || deterministic.candidate_a_receipt_set_id()
            != relations.candidate_a_receipt_set.receipt_set_id()
        || deterministic.candidate_b_receipt_set_id()
            != relations.candidate_b_receipt_set.receipt_set_id()
        || plan.qualification_plan_id() != relations.candidate_a_receipt_set.qualification_plan_id()
        || plan.qualification_plan_id() != relations.candidate_b_receipt_set.qualification_plan_id()
        || plan.suite_manifest_id() != relations.candidate_a_receipt_set.suite_manifest_id()
        || plan.suite_manifest_id() != relations.candidate_b_receipt_set.suite_manifest_id()
        || plan.suite_manifest_id() != deterministic.suite_manifest_id()
        || plan.repetition_id() != relations.candidate_a_receipt_set.repetition_id()
        || plan.repetition_id() != relations.candidate_b_receipt_set.repetition_id()
        || plan.repetition_id() != deterministic.repetition_id()
        || plan.selection_policy_id() != relations.candidate_a_receipt_set.selection_policy_id()
        || plan.selection_policy_id() != relations.candidate_b_receipt_set.selection_policy_id()
        || plan.candidate_a_generation_system_id()
            != relations.candidate_a_receipt_set.generation_system_id()
        || plan.candidate_b_generation_system_id()
            != relations.candidate_b_receipt_set.generation_system_id()
        || plan.judge_generation_system_id() != relations.judge_system.generation_system_id()
        || plan.case_material_set_digest() != deterministic.case_material_set_digest()
        || relations.schedule.candidate_judge_plan_id() != plan.candidate_judge_plan_id()
        || relations.schedule.candidate_receipt_pair_set_id()
            != deterministic.candidate_receipt_pair_set_id()
        || relations.request_aggregate.candidate_judge_plan_id() != plan.candidate_judge_plan_id()
        || relations.request_aggregate.candidate_judge_schedule_id()
            != relations.schedule.candidate_judge_schedule_id()
        || relations.request_aggregate.entry_count() != relations.schedule.entry_count()
        || relations.response_aggregate.candidate_judge_plan_id() != plan.candidate_judge_plan_id()
        || relations.response_aggregate.candidate_judge_schedule_id()
            != relations.schedule.candidate_judge_schedule_id()
        || relations
            .response_aggregate
            .candidate_judge_request_aggregate_id()
            != relations.request_aggregate.request_aggregate_id()
        || relations.observation_batch.candidate_judge_plan_id() != plan.candidate_judge_plan_id()
        || relations.observation_batch.candidate_judge_schedule_id()
            != relations.schedule.candidate_judge_schedule_id()
        || relations
            .observation_batch
            .candidate_judge_request_aggregate_id()
            != relations.request_aggregate.request_aggregate_id()
        || relations.response_aggregate.entry_count() != relations.schedule.entry_count()
        || relations.observation_batch.entry_count() != relations.schedule.entry_count()
        || receipt.candidate_judge_plan_id() != plan.candidate_judge_plan_id()
        || receipt.candidate_judge_schedule_id() != relations.schedule.candidate_judge_schedule_id()
        || receipt.judge_generation_system_id() != relations.judge_system.generation_system_id()
        || receipt.judge_request_aggregate_id()
            != relations.request_aggregate.request_aggregate_id()
        || receipt.judge_response_aggregate_id()
            != relations.response_aggregate.response_aggregate_id()
        || receipt.judge_observation_batch_id()
            != relations.observation_batch.observation_batch_id()
        || receipt.attempt_count() != relations.schedule.entry_count()
    {
        return Err(GenerationQualificationContractError::CandidateJudgeJoinRelationshipMismatch);
    }
    Ok(())
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    schema_version: u32,
    candidate_judge_plan_id: CandidateJudgePlanId,
    candidate_receipt_pair_set_id: CandidateReceiptPairSetId,
    candidate_a_receipt_set_id: CandidateGenerationReceiptSetId,
    candidate_b_receipt_set_id: CandidateGenerationReceiptSetId,
    candidate_a_generation_system_id: GenerationSystemId,
    candidate_b_generation_system_id: GenerationSystemId,
    judge_generation_system_id: GenerationSystemId,
    deterministic_evaluation_id: CandidateDeterministicEvaluationId,
    case_material_set_digest: Digest,
    suite_pair_digest: Digest,
    judge_schedule_id: CandidateJudgeScheduleId,
    managed_local_judge_receipt_id: ManagedLocalJudgeReceiptId,
    judge_request_aggregate_id: CandidateJudgeRequestAggregateId,
    judge_response_aggregate_id: CandidateJudgeResponseAggregateId,
    judge_observation_batch_id: CandidateJudgeObservationBatchId,
    attempt_count: u32,
    judge_runtime_admission_join_id: RuntimeAdmissionJoinId,
    judge_managed_generation_path_id: ManagedGenerationPathId,
    judge_frozen_external_component_set_id: FrozenExternalComponentSetId,
    judge_runtime_package_manifest_id: RuntimePackageManifestId,
    judge_runtime_build_id: RuntimeBuildId,
    judge_effective_runtime_state_id: EffectiveRuntimeStateId,
    judge_effective_runtime_state_join_id: ManagedOllamaEffectiveRuntimeStateJoinId,
    judge_model_package_manifest_id: ModelPackageManifestId,
    judge_model_artifact_id: ArtifactId,
    judge_effective_package_evidence_v2_id: EffectivePackageEvidenceV2Id,
    judge_runtime_installation_generation: u64,
    judge_model_installation_generation: u64,
    triage_report_digest: Digest,
    evidence_class: CandidateJudgeEvidenceClassV1,
    candidate_semantics_proven: bool,
    judge_correctness_proven: bool,
    qualified: bool,
}

impl Wire {
    fn matches(&self, value: &CandidateJudgeJoinRecordV1) -> bool {
        serde_json::to_value(self).ok() == serde_json::to_value(value).ok()
    }
}
