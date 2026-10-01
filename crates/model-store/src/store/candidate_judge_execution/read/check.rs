use rewrite_model::{
    CandidateJudgeJoinRecordV1, CandidateJudgeObservationBatchV1, CandidateJudgePlanV1,
    CandidateJudgeRequestAggregateV1, CandidateJudgeResponseAggregateV1, CandidateJudgeScheduleV1,
    ManagedLocalJudgeReceiptRecordV1,
};
use rewrite_types::Digest;

use super::super::codec::CohortRecords;
use super::super::derive::{self, Closed};
use super::super::text;
use super::Present;
use crate::{StoreError, StoreResult};

pub(super) fn columns(
    present: &Present,
    records: &CohortRecords,
    closed: &Closed,
) -> StoreResult<()> {
    plan(&present.plan, &records.plan, closed)?;
    schedule(&present.schedule, &records.schedule, closed)?;
    request(&present.request, &records.requests)?;
    response(&present.response, &records.responses)?;
    batch(&present.batch, &records.observations)?;
    receipt(&present.receipt, &records.receipt, closed)?;
    join(&present.join, &records.join, closed)
}

pub(super) fn snapshot_columns(
    present: &Present,
    records: &super::super::StoredCandidateJudgeExecutionV1,
    closed: &Closed,
) -> StoreResult<()> {
    plan(&present.plan, records.plan(), closed)?;
    schedule(&present.schedule, records.schedule(), closed)?;
    request(&present.request, records.request_aggregate())?;
    response(&present.response, records.response_aggregate())?;
    batch(&present.batch, records.observation_batch())?;
    receipt(&present.receipt, records.managed_receipt(), closed)?;
    join(&present.join, records.join(), closed)
}

fn plan(
    row: &super::super::rows::PlanRow,
    value: &CandidateJudgePlanV1,
    closed: &Closed,
) -> StoreResult<()> {
    let limits = value.limits();
    same(&row.id, value.candidate_judge_plan_id().digest())?;
    same_u32(row.schema_version, value.schema_version())?;
    same(
        &row.qualification_plan_id,
        value.qualification_plan_id().digest(),
    )?;
    same(&row.suite_id, value.suite_manifest_id().digest())?;
    same(&row.repetition_id, value.repetition_id().digest())?;
    same(&row.policy_id, value.selection_policy_id().digest())?;
    same(
        &row.candidate_a,
        value.candidate_a_generation_system_id().digest(),
    )?;
    same(
        &row.candidate_b,
        value.candidate_b_generation_system_id().digest(),
    )?;
    same(&row.judge, value.judge_generation_system_id().digest())?;
    same(&row.case_material, value.case_material_set_digest())?;
    same(&row.rubric, value.rubric_digest())?;
    same(&row.prompt, value.prompt_contract_digest())?;
    same(&row.output_schema, value.output_schema_digest())?;
    same_i64(row.case_count, closed.case_count)?;
    same_text(&row.seed, &closed.seed)?;
    same_text(&row.order_policy, text::order_policy(value.order_policy()))?;
    same_u32(row.attempts_per_order, value.attempts_per_order())?;
    same_u32(row.maximum_judge_cases, limits.maximum_judge_cases())?;
    same_u32(row.maximum_source_bytes, limits.maximum_source_bytes())?;
    same_u32(
        row.maximum_candidate_bytes,
        limits.maximum_candidate_bytes(),
    )?;
    same_u32(
        row.maximum_complete_input_bytes,
        limits.maximum_complete_input_bytes(),
    )?;
    same_u32(row.maximum_context_tokens, limits.maximum_context_tokens())?;
    same_u32(row.maximum_output_tokens, limits.maximum_output_tokens())?;
    same_u32(row.maximum_response_bytes, limits.maximum_response_bytes())?;
    same_u32(
        row.maximum_elapsed_milliseconds,
        limits.maximum_elapsed_milliseconds(),
    )
}

fn schedule(
    row: &super::super::rows::ScheduleRow,
    value: &CandidateJudgeScheduleV1,
    closed: &Closed,
) -> StoreResult<()> {
    same(&row.id, value.candidate_judge_schedule_id().digest())?;
    same_u32(row.schema_version, value.schema_version())?;
    same(&row.plan_id, value.candidate_judge_plan_id().digest())?;
    same(
        &row.pair_set_id,
        value.candidate_receipt_pair_set_id().digest(),
    )?;
    same_i64(row.case_count, closed.case_count)?;
    same_text(&row.seed, &closed.seed)?;
    same_i64(row.entry_count, closed.entry_count)
}

fn request(
    row: &super::super::rows::RequestRow,
    value: &CandidateJudgeRequestAggregateV1,
) -> StoreResult<()> {
    same(&row.id, value.request_aggregate_id().digest())?;
    same_u32(row.schema_version, value.schema_version())?;
    same(&row.plan_id, value.candidate_judge_plan_id().digest())?;
    same(
        &row.schedule_id,
        value.candidate_judge_schedule_id().digest(),
    )?;
    same_u32(row.entry_count, value.entry_count())
}

fn response(
    row: &super::super::rows::ResponseRow,
    value: &CandidateJudgeResponseAggregateV1,
) -> StoreResult<()> {
    same(&row.id, value.response_aggregate_id().digest())?;
    same_u32(row.schema_version, value.schema_version())?;
    same(&row.plan_id, value.candidate_judge_plan_id().digest())?;
    same(
        &row.schedule_id,
        value.candidate_judge_schedule_id().digest(),
    )?;
    same(
        &row.request_id,
        value.candidate_judge_request_aggregate_id().digest(),
    )?;
    same_u32(row.entry_count, value.entry_count())
}

fn batch(
    row: &super::super::rows::BatchRow,
    value: &CandidateJudgeObservationBatchV1,
) -> StoreResult<()> {
    same(&row.id, value.observation_batch_id().digest())?;
    same_i64(row.schema_version, 1)?;
    same(&row.plan_id, value.candidate_judge_plan_id().digest())?;
    same(
        &row.schedule_id,
        value.candidate_judge_schedule_id().digest(),
    )?;
    same(
        &row.request_id,
        value.candidate_judge_request_aggregate_id().digest(),
    )?;
    same_u32(row.entry_count, value.entry_count())
}

fn receipt(
    row: &super::super::rows::ReceiptRow,
    value: &ManagedLocalJudgeReceiptRecordV1,
    closed: &Closed,
) -> StoreResult<()> {
    same(&row.id, value.managed_local_judge_receipt_id().digest())?;
    same_u32(row.schema_version, value.schema_version())?;
    same(&row.plan_id, value.candidate_judge_plan_id().digest())?;
    same(
        &row.schedule_id,
        value.candidate_judge_schedule_id().digest(),
    )?;
    same(&row.judge, value.judge_generation_system_id().digest())?;
    same(
        &row.admission,
        value.judge_runtime_admission_join_id().digest(),
    )?;
    same(&row.path, value.judge_managed_generation_path_id().digest())?;
    same(
        &row.frozen,
        value.judge_frozen_external_component_set_id().digest(),
    )?;
    same(
        &row.runtime_package,
        value.judge_runtime_package_manifest_id().digest(),
    )?;
    same(&row.runtime_build, value.judge_runtime_build_id().digest())?;
    same(
        &row.model_package,
        value.judge_model_package_manifest_id().digest(),
    )?;
    same(
        &row.model_artifact,
        value.judge_model_artifact_id().digest(),
    )?;
    same_u64(
        row.runtime_generation,
        value.judge_runtime_installation_generation(),
    )?;
    same_u64(
        row.model_generation,
        value.judge_model_installation_generation(),
    )?;
    same(&row.preflight, value.managed_preflight_digest())?;
    same(
        &row.retained_preflight,
        value.retained_session_preflight_digest(),
    )?;
    same(&row.request_id, value.judge_request_aggregate_id().digest())?;
    same(
        &row.response_id,
        value.judge_response_aggregate_id().digest(),
    )?;
    same(&row.batch_id, value.judge_observation_batch_id().digest())?;
    same(&row.residency, value.residency_receipt_aggregate_digest())?;
    same(&row.process, value.process_observation_aggregate_digest())?;
    same(
        &row.native_load,
        value.native_load_observation_aggregate_digest(),
    )?;
    same(
        &row.connection,
        value.connection_observation_aggregate_digest(),
    )?;
    same(
        &row.state_observation,
        value.effective_runtime_state_observation_aggregate_digest(),
    )?;
    same(
        &row.runtime_state,
        value.judge_effective_runtime_state_id().digest(),
    )?;
    same(
        &row.runtime_state_join,
        value.judge_effective_runtime_state_join_id().digest(),
    )?;
    same(
        &row.package_evidence,
        value.judge_effective_package_evidence_v2_id().digest(),
    )?;
    same_i64(row.first_ordinal, closed.first_ordinal)?;
    same_i64(row.last_ordinal, closed.last_ordinal)?;
    same_i64(row.attempt_count, closed.attempt_count)?;
    same_text(
        &row.cleanup,
        text::receipt_success(value.cleanup_disposition()),
    )?;
    same_text(
        &row.runtime_revalidation,
        text::receipt_success(value.runtime_package_revalidation_status()),
    )?;
    same_text(
        &row.model_revalidation,
        text::receipt_success(value.model_package_revalidation_status()),
    )?;
    same_text(
        &row.evidence_class,
        text::receipt_evidence(value.evidence_class()),
    )
}

fn join(
    row: &super::super::rows::JoinRow,
    value: &CandidateJudgeJoinRecordV1,
    closed: &Closed,
) -> StoreResult<()> {
    same(&row.id, value.candidate_judge_join_id().digest())?;
    same_u32(row.schema_version, value.schema_version())?;
    same(&row.plan_id, value.candidate_judge_plan_id().digest())?;
    same(
        &row.pair_set_id,
        value.candidate_receipt_pair_set_id().digest(),
    )?;
    same(&row.receipt_a, value.candidate_a_receipt_set_id().digest())?;
    same(&row.receipt_b, value.candidate_b_receipt_set_id().digest())?;
    same(
        &row.candidate_a,
        value.candidate_a_generation_system_id().digest(),
    )?;
    same(
        &row.candidate_b,
        value.candidate_b_generation_system_id().digest(),
    )?;
    same(&row.judge, value.judge_generation_system_id().digest())?;
    same(
        &row.deterministic_id,
        value.deterministic_evaluation_id().digest(),
    )?;
    same(&row.case_material, value.case_material_set_digest())?;
    same(&row.suite_pair, value.suite_pair_digest())?;
    same(&row.schedule_id, value.judge_schedule_id().digest())?;
    same(
        &row.receipt_id,
        value.managed_local_judge_receipt_id().digest(),
    )?;
    same(&row.request_id, value.judge_request_aggregate_id().digest())?;
    same(
        &row.response_id,
        value.judge_response_aggregate_id().digest(),
    )?;
    same(&row.batch_id, value.judge_observation_batch_id().digest())?;
    same_i64(row.attempt_count, closed.attempt_count)?;
    same(
        &row.admission,
        value.judge_runtime_admission_join_id().digest(),
    )?;
    same(&row.path, value.judge_managed_generation_path_id().digest())?;
    same(
        &row.frozen,
        value.judge_frozen_external_component_set_id().digest(),
    )?;
    same(
        &row.runtime_package,
        value.judge_runtime_package_manifest_id().digest(),
    )?;
    same(&row.runtime_build, value.judge_runtime_build_id().digest())?;
    same(
        &row.runtime_state,
        value.judge_effective_runtime_state_id().digest(),
    )?;
    same(
        &row.runtime_state_join,
        value.judge_effective_runtime_state_join_id().digest(),
    )?;
    same(
        &row.model_package,
        value.judge_model_package_manifest_id().digest(),
    )?;
    same(
        &row.model_artifact,
        value.judge_model_artifact_id().digest(),
    )?;
    same(
        &row.package_evidence,
        value.judge_effective_package_evidence_v2_id().digest(),
    )?;
    same_u64(
        row.runtime_generation,
        value.judge_runtime_installation_generation(),
    )?;
    same_u64(
        row.model_generation,
        value.judge_model_installation_generation(),
    )?;
    same(&row.triage, value.triage_report_digest())?;
    same_text(
        &row.evidence_class,
        text::join_evidence(value.evidence_class()),
    )?;
    flag_clear(row.semantics_proven, value.candidate_semantics_proven())?;
    flag_clear(row.correctness_proven, value.judge_correctness_proven())?;
    flag_clear(row.qualified, value.qualified())
}

fn same(actual: &str, expected: &Digest) -> StoreResult<()> {
    same_text(actual, expected.as_str())
}

fn same_text(actual: &str, expected: &str) -> StoreResult<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn same_i64(actual: i64, expected: i64) -> StoreResult<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn same_u32(actual: i64, expected: u32) -> StoreResult<()> {
    same_i64(actual, i64::from(expected))
}

fn same_u64(actual: i64, expected: u64) -> StoreResult<()> {
    same_i64(actual, derive::sql_u64(expected)?)
}

fn flag_clear(actual: i64, proven: bool) -> StoreResult<()> {
    if actual == 0 && !proven {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}
