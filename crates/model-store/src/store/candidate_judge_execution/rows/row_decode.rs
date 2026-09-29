use rusqlite::Row;

use crate::{StoreError, StoreResult};

const MAX_JSON_BYTES: usize = 4 * 1_024 * 1_024;

pub(crate) struct PlanRow {
    pub(crate) id: String,
    pub(crate) schema_version: i64,
    pub(crate) qualification_plan_id: String,
    pub(crate) suite_id: String,
    pub(crate) repetition_id: String,
    pub(crate) policy_id: String,
    pub(crate) candidate_a: String,
    pub(crate) candidate_b: String,
    pub(crate) judge: String,
    pub(crate) case_material: String,
    pub(crate) rubric: String,
    pub(crate) prompt: String,
    pub(crate) output_schema: String,
    pub(crate) case_count: i64,
    pub(crate) seed: String,
    pub(crate) order_policy: String,
    pub(crate) attempts_per_order: i64,
    pub(crate) maximum_judge_cases: i64,
    pub(crate) maximum_source_bytes: i64,
    pub(crate) maximum_candidate_bytes: i64,
    pub(crate) maximum_complete_input_bytes: i64,
    pub(crate) maximum_context_tokens: i64,
    pub(crate) maximum_output_tokens: i64,
    pub(crate) maximum_response_bytes: i64,
    pub(crate) maximum_elapsed_milliseconds: i64,
    pub(crate) bytes: Vec<u8>,
}

pub(crate) struct ScheduleRow {
    pub(crate) id: String,
    pub(crate) schema_version: i64,
    pub(crate) plan_id: String,
    pub(crate) pair_set_id: String,
    pub(crate) case_count: i64,
    pub(crate) seed: String,
    pub(crate) entry_count: i64,
    pub(crate) bytes: Vec<u8>,
}

pub(crate) struct RequestRow {
    pub(crate) id: String,
    pub(crate) schema_version: i64,
    pub(crate) plan_id: String,
    pub(crate) schedule_id: String,
    pub(crate) entry_count: i64,
    pub(crate) bytes: Vec<u8>,
}

pub(crate) struct ResponseRow {
    pub(crate) id: String,
    pub(crate) schema_version: i64,
    pub(crate) plan_id: String,
    pub(crate) schedule_id: String,
    pub(crate) request_id: String,
    pub(crate) entry_count: i64,
    pub(crate) bytes: Vec<u8>,
}

pub(crate) struct BatchRow {
    pub(crate) id: String,
    pub(crate) schema_version: i64,
    pub(crate) plan_id: String,
    pub(crate) schedule_id: String,
    pub(crate) request_id: String,
    pub(crate) entry_count: i64,
    pub(crate) bytes: Vec<u8>,
}

pub(crate) struct ReceiptRow {
    pub(crate) id: String,
    pub(crate) schema_version: i64,
    pub(crate) plan_id: String,
    pub(crate) schedule_id: String,
    pub(crate) judge: String,
    pub(crate) admission: String,
    pub(crate) path: String,
    pub(crate) frozen: String,
    pub(crate) runtime_package: String,
    pub(crate) runtime_build: String,
    pub(crate) model_package: String,
    pub(crate) model_artifact: String,
    pub(crate) runtime_generation: i64,
    pub(crate) model_generation: i64,
    pub(crate) preflight: String,
    pub(crate) retained_preflight: String,
    pub(crate) request_id: String,
    pub(crate) response_id: String,
    pub(crate) batch_id: String,
    pub(crate) residency: String,
    pub(crate) process: String,
    pub(crate) native_load: String,
    pub(crate) connection: String,
    pub(crate) state_observation: String,
    pub(crate) runtime_state: String,
    pub(crate) runtime_state_join: String,
    pub(crate) package_evidence: String,
    pub(crate) first_ordinal: i64,
    pub(crate) last_ordinal: i64,
    pub(crate) attempt_count: i64,
    pub(crate) cleanup: String,
    pub(crate) runtime_revalidation: String,
    pub(crate) model_revalidation: String,
    pub(crate) evidence_class: String,
    pub(crate) bytes: Vec<u8>,
}

pub(crate) struct JoinRow {
    pub(crate) id: String,
    pub(crate) schema_version: i64,
    pub(crate) plan_id: String,
    pub(crate) pair_set_id: String,
    pub(crate) receipt_a: String,
    pub(crate) receipt_b: String,
    pub(crate) candidate_a: String,
    pub(crate) candidate_b: String,
    pub(crate) judge: String,
    pub(crate) deterministic_id: String,
    pub(crate) case_material: String,
    pub(crate) suite_pair: String,
    pub(crate) schedule_id: String,
    pub(crate) receipt_id: String,
    pub(crate) request_id: String,
    pub(crate) response_id: String,
    pub(crate) batch_id: String,
    pub(crate) attempt_count: i64,
    pub(crate) admission: String,
    pub(crate) path: String,
    pub(crate) frozen: String,
    pub(crate) runtime_package: String,
    pub(crate) runtime_build: String,
    pub(crate) runtime_state: String,
    pub(crate) runtime_state_join: String,
    pub(crate) model_package: String,
    pub(crate) model_artifact: String,
    pub(crate) package_evidence: String,
    pub(crate) runtime_generation: i64,
    pub(crate) model_generation: i64,
    pub(crate) triage: String,
    pub(crate) evidence_class: String,
    pub(crate) semantics_proven: i64,
    pub(crate) correctness_proven: i64,
    pub(crate) qualified: i64,
    pub(crate) bytes: Vec<u8>,
}

pub(crate) const PLAN_SQL: &str = "SELECT candidate_judge_plan_id, schema_version, \
generation_qualification_plan_id, generation_suite_manifest_id, generation_repetition_id, \
candidate_selection_policy_id, candidate_a_generation_system_id, candidate_b_generation_system_id, \
judge_generation_system_id, case_material_set_digest, rubric_digest, prompt_contract_digest, \
output_schema_digest, case_count, presentation_seed, order_policy, attempts_per_order, \
maximum_judge_cases, maximum_source_bytes, maximum_candidate_bytes, maximum_complete_input_bytes, \
maximum_context_tokens, maximum_output_tokens, maximum_response_bytes, maximum_elapsed_milliseconds, \
typeof(canonical_json), length(canonical_json), canonical_json FROM candidate_judge_plans \
WHERE candidate_judge_plan_id = ?1";

pub(crate) const SCHEDULE_SQL: &str = "SELECT candidate_judge_schedule_id, schema_version, \
candidate_judge_plan_id, candidate_receipt_pair_set_id, case_count, presentation_seed, entry_count, \
typeof(canonical_json), length(canonical_json), canonical_json FROM candidate_judge_schedules \
WHERE candidate_judge_plan_id = ?1";

pub(crate) const REQUEST_SQL: &str = "SELECT candidate_judge_request_aggregate_id, schema_version, \
candidate_judge_plan_id, candidate_judge_schedule_id, entry_count, typeof(canonical_json), \
length(canonical_json), canonical_json FROM candidate_judge_request_aggregates \
WHERE candidate_judge_plan_id = ?1";

pub(crate) const RESPONSE_SQL: &str = "SELECT candidate_judge_response_aggregate_id, schema_version, \
candidate_judge_plan_id, candidate_judge_schedule_id, candidate_judge_request_aggregate_id, \
entry_count, typeof(canonical_json), length(canonical_json), canonical_json \
FROM candidate_judge_response_aggregates WHERE candidate_judge_plan_id = ?1";

pub(crate) const BATCH_SQL: &str = "SELECT candidate_judge_observation_batch_id, schema_version, \
candidate_judge_plan_id, candidate_judge_schedule_id, candidate_judge_request_aggregate_id, \
entry_count, typeof(canonical_json), length(canonical_json), canonical_json \
FROM candidate_judge_observation_batches WHERE candidate_judge_plan_id = ?1";

pub(crate) const RECEIPT_SQL: &str = "SELECT managed_local_judge_receipt_id, schema_version, \
candidate_judge_plan_id, candidate_judge_schedule_id, judge_generation_system_id, \
judge_runtime_admission_join_id, judge_managed_generation_path_id, \
judge_frozen_external_component_set_id, judge_runtime_package_manifest_id, judge_runtime_build_id, \
judge_model_package_manifest_id, judge_model_artifact_id, judge_runtime_installation_generation, \
judge_model_installation_generation, managed_preflight_digest, retained_session_preflight_digest, \
judge_request_aggregate_id, judge_response_aggregate_id, judge_observation_batch_id, \
residency_receipt_aggregate_digest, process_observation_aggregate_digest, \
native_load_observation_aggregate_digest, connection_observation_aggregate_digest, \
effective_runtime_state_observation_aggregate_digest, judge_effective_runtime_state_id, \
judge_effective_runtime_state_join_id, judge_effective_package_evidence_v2_id, \
first_response_ordinal, last_response_ordinal, attempt_count, cleanup_disposition, \
runtime_package_revalidation_status, model_package_revalidation_status, evidence_class, \
typeof(canonical_json), length(canonical_json), canonical_json FROM managed_local_judge_receipts \
WHERE candidate_judge_plan_id = ?1";

pub(crate) const JOIN_SQL: &str = "SELECT candidate_judge_join_id, schema_version, \
candidate_judge_plan_id, candidate_receipt_pair_set_id, candidate_a_receipt_set_id, \
candidate_b_receipt_set_id, candidate_a_generation_system_id, candidate_b_generation_system_id, \
judge_generation_system_id, deterministic_evaluation_id, case_material_set_digest, \
suite_pair_digest, judge_schedule_id, managed_local_judge_receipt_id, judge_request_aggregate_id, \
judge_response_aggregate_id, judge_observation_batch_id, attempt_count, \
judge_runtime_admission_join_id, judge_managed_generation_path_id, \
judge_frozen_external_component_set_id, judge_runtime_package_manifest_id, judge_runtime_build_id, \
judge_effective_runtime_state_id, judge_effective_runtime_state_join_id, \
judge_model_package_manifest_id, judge_model_artifact_id, judge_effective_package_evidence_v2_id, \
judge_runtime_installation_generation, judge_model_installation_generation, triage_report_digest, \
evidence_class, candidate_semantics_proven, judge_correctness_proven, qualified, \
typeof(canonical_json), length(canonical_json), canonical_json FROM candidate_judge_join_records \
WHERE candidate_judge_plan_id = ?1";

pub(crate) fn owned_blob(kind: &str, length: i64, bytes: Vec<u8>) -> StoreResult<Vec<u8>> {
    let bounded = usize::try_from(length)
        .ok()
        .is_some_and(|value| (1..=MAX_JSON_BYTES).contains(&value));
    if kind == "blob" && bounded && i64::try_from(bytes.len()).ok() == Some(length) {
        Ok(bytes)
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn blob_at(row: &Row<'_>, index: usize) -> rusqlite::Result<Vec<u8>> {
    let kind: String = row.get(index)?;
    let length: i64 = row.get(index + 1)?;
    let bytes: Vec<u8> = row.get(index + 2)?;
    owned_blob(&kind, length, bytes)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
}

pub(crate) fn map_plan(row: &Row<'_>) -> rusqlite::Result<PlanRow> {
    Ok(PlanRow {
        id: row.get(0)?,
        schema_version: row.get(1)?,
        qualification_plan_id: row.get(2)?,
        suite_id: row.get(3)?,
        repetition_id: row.get(4)?,
        policy_id: row.get(5)?,
        candidate_a: row.get(6)?,
        candidate_b: row.get(7)?,
        judge: row.get(8)?,
        case_material: row.get(9)?,
        rubric: row.get(10)?,
        prompt: row.get(11)?,
        output_schema: row.get(12)?,
        case_count: row.get(13)?,
        seed: row.get(14)?,
        order_policy: row.get(15)?,
        attempts_per_order: row.get(16)?,
        maximum_judge_cases: row.get(17)?,
        maximum_source_bytes: row.get(18)?,
        maximum_candidate_bytes: row.get(19)?,
        maximum_complete_input_bytes: row.get(20)?,
        maximum_context_tokens: row.get(21)?,
        maximum_output_tokens: row.get(22)?,
        maximum_response_bytes: row.get(23)?,
        maximum_elapsed_milliseconds: row.get(24)?,
        bytes: blob_at(row, 25)?,
    })
}

pub(crate) fn map_schedule(row: &Row<'_>) -> rusqlite::Result<ScheduleRow> {
    Ok(ScheduleRow {
        id: row.get(0)?,
        schema_version: row.get(1)?,
        plan_id: row.get(2)?,
        pair_set_id: row.get(3)?,
        case_count: row.get(4)?,
        seed: row.get(5)?,
        entry_count: row.get(6)?,
        bytes: blob_at(row, 7)?,
    })
}

pub(crate) fn map_request(row: &Row<'_>) -> rusqlite::Result<RequestRow> {
    Ok(RequestRow {
        id: row.get(0)?,
        schema_version: row.get(1)?,
        plan_id: row.get(2)?,
        schedule_id: row.get(3)?,
        entry_count: row.get(4)?,
        bytes: blob_at(row, 5)?,
    })
}

pub(crate) fn map_response(row: &Row<'_>) -> rusqlite::Result<ResponseRow> {
    Ok(ResponseRow {
        id: row.get(0)?,
        schema_version: row.get(1)?,
        plan_id: row.get(2)?,
        schedule_id: row.get(3)?,
        request_id: row.get(4)?,
        entry_count: row.get(5)?,
        bytes: blob_at(row, 6)?,
    })
}

pub(crate) fn map_batch(row: &Row<'_>) -> rusqlite::Result<BatchRow> {
    Ok(BatchRow {
        id: row.get(0)?,
        schema_version: row.get(1)?,
        plan_id: row.get(2)?,
        schedule_id: row.get(3)?,
        request_id: row.get(4)?,
        entry_count: row.get(5)?,
        bytes: blob_at(row, 6)?,
    })
}

pub(crate) fn map_receipt(row: &Row<'_>) -> rusqlite::Result<ReceiptRow> {
    Ok(ReceiptRow {
        id: row.get(0)?,
        schema_version: row.get(1)?,
        plan_id: row.get(2)?,
        schedule_id: row.get(3)?,
        judge: row.get(4)?,
        admission: row.get(5)?,
        path: row.get(6)?,
        frozen: row.get(7)?,
        runtime_package: row.get(8)?,
        runtime_build: row.get(9)?,
        model_package: row.get(10)?,
        model_artifact: row.get(11)?,
        runtime_generation: row.get(12)?,
        model_generation: row.get(13)?,
        preflight: row.get(14)?,
        retained_preflight: row.get(15)?,
        request_id: row.get(16)?,
        response_id: row.get(17)?,
        batch_id: row.get(18)?,
        residency: row.get(19)?,
        process: row.get(20)?,
        native_load: row.get(21)?,
        connection: row.get(22)?,
        state_observation: row.get(23)?,
        runtime_state: row.get(24)?,
        runtime_state_join: row.get(25)?,
        package_evidence: row.get(26)?,
        first_ordinal: row.get(27)?,
        last_ordinal: row.get(28)?,
        attempt_count: row.get(29)?,
        cleanup: row.get(30)?,
        runtime_revalidation: row.get(31)?,
        model_revalidation: row.get(32)?,
        evidence_class: row.get(33)?,
        bytes: blob_at(row, 34)?,
    })
}

pub(crate) fn map_join(row: &Row<'_>) -> rusqlite::Result<JoinRow> {
    Ok(JoinRow {
        id: row.get(0)?,
        schema_version: row.get(1)?,
        plan_id: row.get(2)?,
        pair_set_id: row.get(3)?,
        receipt_a: row.get(4)?,
        receipt_b: row.get(5)?,
        candidate_a: row.get(6)?,
        candidate_b: row.get(7)?,
        judge: row.get(8)?,
        deterministic_id: row.get(9)?,
        case_material: row.get(10)?,
        suite_pair: row.get(11)?,
        schedule_id: row.get(12)?,
        receipt_id: row.get(13)?,
        request_id: row.get(14)?,
        response_id: row.get(15)?,
        batch_id: row.get(16)?,
        attempt_count: row.get(17)?,
        admission: row.get(18)?,
        path: row.get(19)?,
        frozen: row.get(20)?,
        runtime_package: row.get(21)?,
        runtime_build: row.get(22)?,
        runtime_state: row.get(23)?,
        runtime_state_join: row.get(24)?,
        model_package: row.get(25)?,
        model_artifact: row.get(26)?,
        package_evidence: row.get(27)?,
        runtime_generation: row.get(28)?,
        model_generation: row.get(29)?,
        triage: row.get(30)?,
        evidence_class: row.get(31)?,
        semantics_proven: row.get(32)?,
        correctness_proven: row.get(33)?,
        qualified: row.get(34)?,
        bytes: blob_at(row, 35)?,
    })
}
