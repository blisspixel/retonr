use rusqlite::{Connection, params};

pub(super) fn legal_plan(connection: &Connection, identity: char) -> rusqlite::Result<usize> {
    plan(connection, identity, "both_orders", 1, "1", 'c', 256)
}

pub(super) fn plan(
    connection: &Connection,
    identity: char,
    order_policy: &str,
    attempts_per_order: i64,
    presentation_seed: &str,
    judge: char,
    maximum_response_bytes: i64,
) -> rusqlite::Result<usize> {
    connection.execute(
        "INSERT INTO candidate_judge_plans (
             candidate_judge_plan_id, schema_version, generation_qualification_plan_id,
             generation_suite_manifest_id, generation_repetition_id,
             candidate_selection_policy_id, candidate_a_generation_system_id,
             candidate_b_generation_system_id, judge_generation_system_id,
             case_material_set_digest, rubric_digest, prompt_contract_digest,
             output_schema_digest, case_count, presentation_seed, order_policy,
             attempts_per_order, maximum_judge_cases, maximum_source_bytes,
             maximum_candidate_bytes, maximum_complete_input_bytes,
             maximum_context_tokens, maximum_output_tokens, maximum_response_bytes,
             maximum_elapsed_milliseconds, canonical_json
         ) VALUES (
             ?1, 1, ?2, ?2, ?2, ?2, ?3, ?4, ?5, ?2, ?2, ?2, ?2, 1, ?6, ?7, ?8,
             256, 1024, 1024, 4096, 4096, 256, ?9, 1000, ?10
         )",
        params![
            super::digest(identity),
            super::digest('d'),
            super::digest('a'),
            super::digest('b'),
            super::digest(judge),
            presentation_seed,
            order_policy,
            attempts_per_order,
            maximum_response_bytes,
            b"{}"
        ],
    )
}

pub(super) fn schedule(
    connection: &Connection,
    identity: char,
    case_count: i64,
    entry_count: i64,
    presentation_seed: &str,
) -> rusqlite::Result<usize> {
    connection.execute(
        "INSERT INTO candidate_judge_schedules (
             candidate_judge_schedule_id, schema_version, candidate_judge_plan_id,
             candidate_receipt_pair_set_id, case_count, presentation_seed, entry_count,
             canonical_json
         ) VALUES (?1, 1, ?2, ?2, ?3, ?4, ?5, ?6)",
        params![
            super::digest(identity),
            super::digest('d'),
            case_count,
            presentation_seed,
            entry_count,
            b"{}"
        ],
    )
}

pub(super) fn receipt(
    connection: &Connection,
    identity: char,
    cleanup_disposition: &str,
    runtime_package_revalidation_status: &str,
    model_or_evidence: &str,
) -> rusqlite::Result<usize> {
    let (model_status, evidence_class) = if model_or_evidence == "succeeded_live" {
        ("succeeded", "live")
    } else {
        (model_or_evidence, "managed_local_judge_triage")
    };
    connection.execute(
        "INSERT INTO managed_local_judge_receipts (
             managed_local_judge_receipt_id, schema_version, candidate_judge_plan_id,
             candidate_judge_schedule_id, judge_generation_system_id,
             judge_runtime_admission_join_id, judge_managed_generation_path_id,
             judge_frozen_external_component_set_id, judge_runtime_package_manifest_id,
             judge_runtime_build_id, judge_model_package_manifest_id, judge_model_artifact_id,
             judge_runtime_installation_generation, judge_model_installation_generation,
             managed_preflight_digest, retained_session_preflight_digest,
             judge_request_aggregate_id, judge_response_aggregate_id,
             judge_observation_batch_id, residency_receipt_aggregate_digest,
             process_observation_aggregate_digest, native_load_observation_aggregate_digest,
             connection_observation_aggregate_digest,
             effective_runtime_state_observation_aggregate_digest,
             judge_effective_runtime_state_id, judge_effective_runtime_state_join_id,
             judge_effective_package_evidence_v2_id, first_response_ordinal,
             last_response_ordinal, attempt_count, cleanup_disposition,
             runtime_package_revalidation_status, model_package_revalidation_status,
             evidence_class, canonical_json
         ) VALUES (
             ?1, 1, ?2, ?2, ?2, ?2, ?2, ?2, ?2, ?2, ?2, ?2, 1, 1, ?2, ?2, ?2, ?2, ?2,
             ?2, ?2, ?2, ?2, ?2, ?2, ?2, ?2, 8, 25, 2, ?3, ?4, ?5, ?6, ?7
         )",
        params![
            super::digest(identity),
            super::digest('d'),
            cleanup_disposition,
            runtime_package_revalidation_status,
            model_status,
            evidence_class,
            b"{}"
        ],
    )
}

pub(super) fn join(
    connection: &Connection,
    identity: char,
    evidence_class: &str,
    candidate_semantics_proven: i64,
    judge_correctness_proven: i64,
    qualified: i64,
) -> rusqlite::Result<usize> {
    connection.execute(
        "INSERT INTO candidate_judge_join_records (
             candidate_judge_join_id, schema_version, candidate_judge_plan_id,
             candidate_receipt_pair_set_id, candidate_a_receipt_set_id,
             candidate_b_receipt_set_id, candidate_a_generation_system_id,
             candidate_b_generation_system_id, judge_generation_system_id,
             deterministic_evaluation_id, case_material_set_digest, suite_pair_digest,
             judge_schedule_id, managed_local_judge_receipt_id, judge_request_aggregate_id,
             judge_response_aggregate_id, judge_observation_batch_id, attempt_count,
             judge_runtime_admission_join_id, judge_managed_generation_path_id,
             judge_frozen_external_component_set_id, judge_runtime_package_manifest_id,
             judge_runtime_build_id, judge_effective_runtime_state_id,
             judge_effective_runtime_state_join_id, judge_model_package_manifest_id,
             judge_model_artifact_id, judge_effective_package_evidence_v2_id,
             judge_runtime_installation_generation, judge_model_installation_generation,
             triage_report_digest, evidence_class, candidate_semantics_proven,
             judge_correctness_proven, qualified, canonical_json
         ) VALUES (
             ?1, 1, ?2, ?2, ?3, ?4, ?3, ?4, ?5, ?2, ?2, ?2, ?2, ?2, ?2, ?2, ?2, 2,
             ?2, ?2, ?2, ?2, ?2, ?2, ?2, ?2, ?2, ?2, 1, 1, ?2, ?6, ?7, ?8, ?9, ?10
         )",
        params![
            super::digest(identity),
            super::digest('d'),
            super::digest('a'),
            super::digest('b'),
            super::digest('c'),
            evidence_class,
            candidate_semantics_proven,
            judge_correctness_proven,
            qualified,
            b"{}"
        ],
    )
}

pub(super) fn result(
    connection: &Connection,
    identity: char,
    stage: &str,
    receipt_set: Option<&str>,
    evaluation: Option<&str>,
    join_id: Option<&str>,
) -> rusqlite::Result<usize> {
    connection.execute(
        "INSERT INTO generation_repeatability_result_records (
             generation_repeatability_result_id, generation_system_id,
             generation_qualification_plan_id, generation_suite_manifest_id,
             generation_repetition_id, generation_attempt_ledger_manifest_id,
             terminal_stage, candidate_generation_receipt_set_id,
             candidate_deterministic_evaluation_id, candidate_judge_join_id, canonical_json
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            super::digest(identity),
            super::digest('1'),
            super::digest('2'),
            super::digest('3'),
            super::digest(identity),
            super::digest('5'),
            stage,
            receipt_set,
            evaluation,
            join_id,
            b"{}"
        ],
    )
}
