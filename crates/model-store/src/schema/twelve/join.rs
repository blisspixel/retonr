use rusqlite::Connection;

use crate::StoreResult;

const JOIN_SQL: &str = "CREATE TABLE candidate_judge_join_records (
    candidate_judge_join_id TEXT PRIMARY KEY NOT NULL
        CHECK(length(candidate_judge_join_id) = 64)
        CHECK(candidate_judge_join_id = lower(candidate_judge_join_id))
        CHECK(candidate_judge_join_id NOT GLOB '*[^0-9a-f]*'),
    schema_version INTEGER NOT NULL CHECK(schema_version = 1),
    candidate_judge_plan_id TEXT NOT NULL
        CHECK(length(candidate_judge_plan_id) = 64)
        CHECK(candidate_judge_plan_id = lower(candidate_judge_plan_id))
        CHECK(candidate_judge_plan_id NOT GLOB '*[^0-9a-f]*'),
    candidate_receipt_pair_set_id TEXT NOT NULL
        CHECK(length(candidate_receipt_pair_set_id) = 64)
        CHECK(candidate_receipt_pair_set_id = lower(candidate_receipt_pair_set_id))
        CHECK(candidate_receipt_pair_set_id NOT GLOB '*[^0-9a-f]*'),
    candidate_a_receipt_set_id TEXT NOT NULL
        CHECK(length(candidate_a_receipt_set_id) = 64)
        CHECK(candidate_a_receipt_set_id = lower(candidate_a_receipt_set_id))
        CHECK(candidate_a_receipt_set_id NOT GLOB '*[^0-9a-f]*'),
    candidate_b_receipt_set_id TEXT NOT NULL
        CHECK(length(candidate_b_receipt_set_id) = 64)
        CHECK(candidate_b_receipt_set_id = lower(candidate_b_receipt_set_id))
        CHECK(candidate_b_receipt_set_id NOT GLOB '*[^0-9a-f]*'),
    candidate_a_generation_system_id TEXT NOT NULL
        CHECK(length(candidate_a_generation_system_id) = 64)
        CHECK(candidate_a_generation_system_id = lower(candidate_a_generation_system_id))
        CHECK(candidate_a_generation_system_id NOT GLOB '*[^0-9a-f]*'),
    candidate_b_generation_system_id TEXT NOT NULL
        CHECK(length(candidate_b_generation_system_id) = 64)
        CHECK(candidate_b_generation_system_id = lower(candidate_b_generation_system_id))
        CHECK(candidate_b_generation_system_id NOT GLOB '*[^0-9a-f]*'),
    judge_generation_system_id TEXT NOT NULL
        CHECK(length(judge_generation_system_id) = 64)
        CHECK(judge_generation_system_id = lower(judge_generation_system_id))
        CHECK(judge_generation_system_id NOT GLOB '*[^0-9a-f]*'),
    deterministic_evaluation_id TEXT NOT NULL
        CHECK(length(deterministic_evaluation_id) = 64)
        CHECK(deterministic_evaluation_id = lower(deterministic_evaluation_id))
        CHECK(deterministic_evaluation_id NOT GLOB '*[^0-9a-f]*'),
    case_material_set_digest TEXT NOT NULL
        CHECK(length(case_material_set_digest) = 64)
        CHECK(case_material_set_digest = lower(case_material_set_digest))
        CHECK(case_material_set_digest NOT GLOB '*[^0-9a-f]*'),
    suite_pair_digest TEXT NOT NULL
        CHECK(length(suite_pair_digest) = 64)
        CHECK(suite_pair_digest = lower(suite_pair_digest))
        CHECK(suite_pair_digest NOT GLOB '*[^0-9a-f]*'),
    judge_schedule_id TEXT NOT NULL
        CHECK(length(judge_schedule_id) = 64)
        CHECK(judge_schedule_id = lower(judge_schedule_id))
        CHECK(judge_schedule_id NOT GLOB '*[^0-9a-f]*'),
    managed_local_judge_receipt_id TEXT NOT NULL
        CHECK(length(managed_local_judge_receipt_id) = 64)
        CHECK(managed_local_judge_receipt_id = lower(managed_local_judge_receipt_id))
        CHECK(managed_local_judge_receipt_id NOT GLOB '*[^0-9a-f]*'),
    judge_request_aggregate_id TEXT NOT NULL
        CHECK(length(judge_request_aggregate_id) = 64)
        CHECK(judge_request_aggregate_id = lower(judge_request_aggregate_id))
        CHECK(judge_request_aggregate_id NOT GLOB '*[^0-9a-f]*'),
    judge_response_aggregate_id TEXT NOT NULL
        CHECK(length(judge_response_aggregate_id) = 64)
        CHECK(judge_response_aggregate_id = lower(judge_response_aggregate_id))
        CHECK(judge_response_aggregate_id NOT GLOB '*[^0-9a-f]*'),
    judge_observation_batch_id TEXT NOT NULL
        CHECK(length(judge_observation_batch_id) = 64)
        CHECK(judge_observation_batch_id = lower(judge_observation_batch_id))
        CHECK(judge_observation_batch_id NOT GLOB '*[^0-9a-f]*'),
    attempt_count INTEGER NOT NULL
        CHECK(attempt_count BETWEEN 2 AND 512 AND attempt_count % 2 = 0),
    judge_runtime_admission_join_id TEXT NOT NULL
        CHECK(length(judge_runtime_admission_join_id) = 64)
        CHECK(judge_runtime_admission_join_id = lower(judge_runtime_admission_join_id))
        CHECK(judge_runtime_admission_join_id NOT GLOB '*[^0-9a-f]*'),
    judge_managed_generation_path_id TEXT NOT NULL
        CHECK(length(judge_managed_generation_path_id) = 64)
        CHECK(judge_managed_generation_path_id = lower(judge_managed_generation_path_id))
        CHECK(judge_managed_generation_path_id NOT GLOB '*[^0-9a-f]*'),
    judge_frozen_external_component_set_id TEXT NOT NULL
        CHECK(length(judge_frozen_external_component_set_id) = 64)
        CHECK(judge_frozen_external_component_set_id =
            lower(judge_frozen_external_component_set_id))
        CHECK(judge_frozen_external_component_set_id NOT GLOB '*[^0-9a-f]*'),
    judge_runtime_package_manifest_id TEXT NOT NULL
        CHECK(length(judge_runtime_package_manifest_id) = 64)
        CHECK(judge_runtime_package_manifest_id = lower(judge_runtime_package_manifest_id))
        CHECK(judge_runtime_package_manifest_id NOT GLOB '*[^0-9a-f]*'),
    judge_runtime_build_id TEXT NOT NULL
        CHECK(length(judge_runtime_build_id) = 64)
        CHECK(judge_runtime_build_id = lower(judge_runtime_build_id))
        CHECK(judge_runtime_build_id NOT GLOB '*[^0-9a-f]*'),
    judge_effective_runtime_state_id TEXT NOT NULL
        CHECK(length(judge_effective_runtime_state_id) = 64)
        CHECK(judge_effective_runtime_state_id = lower(judge_effective_runtime_state_id))
        CHECK(judge_effective_runtime_state_id NOT GLOB '*[^0-9a-f]*'),
    judge_effective_runtime_state_join_id TEXT NOT NULL
        CHECK(length(judge_effective_runtime_state_join_id) = 64)
        CHECK(judge_effective_runtime_state_join_id =
            lower(judge_effective_runtime_state_join_id))
        CHECK(judge_effective_runtime_state_join_id NOT GLOB '*[^0-9a-f]*'),
    judge_model_package_manifest_id TEXT NOT NULL
        CHECK(length(judge_model_package_manifest_id) = 64)
        CHECK(judge_model_package_manifest_id = lower(judge_model_package_manifest_id))
        CHECK(judge_model_package_manifest_id NOT GLOB '*[^0-9a-f]*'),
    judge_model_artifact_id TEXT NOT NULL
        CHECK(length(judge_model_artifact_id) = 64)
        CHECK(judge_model_artifact_id = lower(judge_model_artifact_id))
        CHECK(judge_model_artifact_id NOT GLOB '*[^0-9a-f]*'),
    judge_effective_package_evidence_v2_id TEXT NOT NULL
        CHECK(length(judge_effective_package_evidence_v2_id) = 64)
        CHECK(judge_effective_package_evidence_v2_id =
            lower(judge_effective_package_evidence_v2_id))
        CHECK(judge_effective_package_evidence_v2_id NOT GLOB '*[^0-9a-f]*'),
    judge_runtime_installation_generation INTEGER NOT NULL
        CHECK(judge_runtime_installation_generation BETWEEN 1 AND 9223372036854775807),
    judge_model_installation_generation INTEGER NOT NULL
        CHECK(judge_model_installation_generation BETWEEN 1 AND 9223372036854775807),
    triage_report_digest TEXT NOT NULL
        CHECK(length(triage_report_digest) = 64)
        CHECK(triage_report_digest = lower(triage_report_digest))
        CHECK(triage_report_digest NOT GLOB '*[^0-9a-f]*'),
    evidence_class TEXT NOT NULL CHECK(evidence_class = 'managed_local_judge_triage'),
    candidate_semantics_proven INTEGER NOT NULL CHECK(candidate_semantics_proven = 0),
    judge_correctness_proven INTEGER NOT NULL CHECK(judge_correctness_proven = 0),
    qualified INTEGER NOT NULL CHECK(qualified = 0),
    canonical_json BLOB NOT NULL CHECK(length(canonical_json) BETWEEN 1 AND 262144),
    CHECK(candidate_a_receipt_set_id != candidate_b_receipt_set_id),
    CHECK(
        candidate_a_generation_system_id != candidate_b_generation_system_id
        AND judge_generation_system_id != candidate_a_generation_system_id
        AND judge_generation_system_id != candidate_b_generation_system_id
    ),
    FOREIGN KEY (
        candidate_judge_plan_id,
        candidate_a_generation_system_id,
        candidate_b_generation_system_id,
        judge_generation_system_id,
        case_material_set_digest
    ) REFERENCES candidate_judge_plans(
        candidate_judge_plan_id,
        candidate_a_generation_system_id,
        candidate_b_generation_system_id,
        judge_generation_system_id,
        case_material_set_digest
    ) ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (candidate_judge_plan_id, judge_schedule_id, attempt_count)
        REFERENCES candidate_judge_schedules(
            candidate_judge_plan_id,
            candidate_judge_schedule_id,
            entry_count
        ) ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (
        judge_request_aggregate_id,
        candidate_judge_plan_id,
        judge_schedule_id,
        attempt_count
    ) REFERENCES candidate_judge_request_aggregates(
        candidate_judge_request_aggregate_id,
        candidate_judge_plan_id,
        candidate_judge_schedule_id,
        entry_count
    ) ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (
        judge_response_aggregate_id,
        judge_request_aggregate_id,
        candidate_judge_plan_id,
        judge_schedule_id,
        attempt_count
    ) REFERENCES candidate_judge_response_aggregates(
        candidate_judge_response_aggregate_id,
        candidate_judge_request_aggregate_id,
        candidate_judge_plan_id,
        candidate_judge_schedule_id,
        entry_count
    ) ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (
        judge_observation_batch_id,
        judge_request_aggregate_id,
        candidate_judge_plan_id,
        judge_schedule_id,
        attempt_count
    ) REFERENCES candidate_judge_observation_batches(
        candidate_judge_observation_batch_id,
        candidate_judge_request_aggregate_id,
        candidate_judge_plan_id,
        candidate_judge_schedule_id,
        entry_count
    ) ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (managed_local_judge_receipt_id)
        REFERENCES managed_local_judge_receipts(managed_local_judge_receipt_id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (candidate_a_generation_system_id)
        REFERENCES generation_system_records(generation_system_id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (candidate_b_generation_system_id)
        REFERENCES generation_system_records(generation_system_id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (judge_generation_system_id)
        REFERENCES generation_system_records(generation_system_id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (judge_runtime_package_manifest_id)
        REFERENCES runtime_package_manifests(runtime_package_manifest_id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (judge_runtime_build_id)
        REFERENCES runtime_build_identities(runtime_build_id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (judge_model_package_manifest_id)
        REFERENCES model_package_manifests(model_package_manifest_id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (judge_effective_runtime_state_id)
        REFERENCES effective_runtime_states(effective_runtime_state_id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (judge_effective_package_evidence_v2_id)
        REFERENCES effective_package_evidence_v2(effective_package_evidence_v2_id)
        ON UPDATE RESTRICT ON DELETE RESTRICT
) STRICT;";

pub(super) fn create(connection: &Connection) -> StoreResult<()> {
    connection.execute_batch(JOIN_SQL)?;
    Ok(())
}
