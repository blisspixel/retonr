use rusqlite::{Connection, params};

use super::super::codec::PreparedCohort;
use super::super::text;
use super::insert;
use crate::{StoreResult, WriteDisposition};

pub(super) fn insert_plan(
    connection: &Connection,
    prepared: &PreparedCohort<'_>,
) -> StoreResult<WriteDisposition> {
    let plan = &prepared.records.plan;
    let limits = plan.limits();
    insert(
        connection,
        "INSERT OR IGNORE INTO candidate_judge_plans (
            candidate_judge_plan_id, schema_version, generation_qualification_plan_id,
            generation_suite_manifest_id, generation_repetition_id, candidate_selection_policy_id,
            candidate_a_generation_system_id, candidate_b_generation_system_id,
            judge_generation_system_id, case_material_set_digest, rubric_digest,
            prompt_contract_digest, output_schema_digest, case_count, presentation_seed,
            order_policy, attempts_per_order, maximum_judge_cases, maximum_source_bytes,
            maximum_candidate_bytes, maximum_complete_input_bytes, maximum_context_tokens,
            maximum_output_tokens, maximum_response_bytes, maximum_elapsed_milliseconds,
            canonical_json
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18,
            ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26
        )",
        params![
            text::digest_text(plan.candidate_judge_plan_id().digest()),
            i64::from(plan.schema_version()),
            text::digest_text(plan.qualification_plan_id().digest()),
            text::digest_text(plan.suite_manifest_id().digest()),
            text::digest_text(plan.repetition_id().digest()),
            text::digest_text(plan.selection_policy_id().digest()),
            text::digest_text(plan.candidate_a_generation_system_id().digest()),
            text::digest_text(plan.candidate_b_generation_system_id().digest()),
            text::digest_text(plan.judge_generation_system_id().digest()),
            text::digest_text(plan.case_material_set_digest()),
            text::digest_text(plan.rubric_digest()),
            text::digest_text(plan.prompt_contract_digest()),
            text::digest_text(plan.output_schema_digest()),
            prepared.closed.case_count,
            prepared.closed.seed.as_str(),
            text::order_policy(plan.order_policy()),
            i64::from(plan.attempts_per_order()),
            i64::from(limits.maximum_judge_cases()),
            i64::from(limits.maximum_source_bytes()),
            i64::from(limits.maximum_candidate_bytes()),
            i64::from(limits.maximum_complete_input_bytes()),
            i64::from(limits.maximum_context_tokens()),
            i64::from(limits.maximum_output_tokens()),
            i64::from(limits.maximum_response_bytes()),
            i64::from(limits.maximum_elapsed_milliseconds()),
            &prepared.encoded.plan,
        ],
    )
}

pub(super) fn insert_schedule(
    connection: &Connection,
    prepared: &PreparedCohort<'_>,
) -> StoreResult<WriteDisposition> {
    let schedule = &prepared.records.schedule;
    insert(
        connection,
        "INSERT OR IGNORE INTO candidate_judge_schedules (
            candidate_judge_schedule_id, schema_version, candidate_judge_plan_id,
            candidate_receipt_pair_set_id, case_count, presentation_seed, entry_count,
            canonical_json
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            text::digest_text(schedule.candidate_judge_schedule_id().digest()),
            i64::from(schedule.schema_version()),
            text::digest_text(schedule.candidate_judge_plan_id().digest()),
            text::digest_text(schedule.candidate_receipt_pair_set_id().digest()),
            prepared.closed.case_count,
            prepared.closed.seed.as_str(),
            prepared.closed.entry_count,
            &prepared.encoded.schedule,
        ],
    )
}
