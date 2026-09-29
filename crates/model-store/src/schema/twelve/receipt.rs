use rusqlite::Connection;

use crate::StoreResult;

const RECEIPT_SQL: &str = "CREATE TABLE managed_local_judge_receipts (
    managed_local_judge_receipt_id TEXT PRIMARY KEY NOT NULL
        CHECK(length(managed_local_judge_receipt_id) = 64)
        CHECK(managed_local_judge_receipt_id = lower(managed_local_judge_receipt_id))
        CHECK(managed_local_judge_receipt_id NOT GLOB '*[^0-9a-f]*'),
    schema_version INTEGER NOT NULL CHECK(schema_version = 1),
    candidate_judge_plan_id TEXT NOT NULL
        CHECK(length(candidate_judge_plan_id) = 64)
        CHECK(candidate_judge_plan_id = lower(candidate_judge_plan_id))
        CHECK(candidate_judge_plan_id NOT GLOB '*[^0-9a-f]*'),
    candidate_judge_schedule_id TEXT NOT NULL
        CHECK(length(candidate_judge_schedule_id) = 64)
        CHECK(candidate_judge_schedule_id = lower(candidate_judge_schedule_id))
        CHECK(candidate_judge_schedule_id NOT GLOB '*[^0-9a-f]*'),
    judge_generation_system_id TEXT NOT NULL
        CHECK(length(judge_generation_system_id) = 64)
        CHECK(judge_generation_system_id = lower(judge_generation_system_id))
        CHECK(judge_generation_system_id NOT GLOB '*[^0-9a-f]*'),
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
    judge_model_package_manifest_id TEXT NOT NULL
        CHECK(length(judge_model_package_manifest_id) = 64)
        CHECK(judge_model_package_manifest_id = lower(judge_model_package_manifest_id))
        CHECK(judge_model_package_manifest_id NOT GLOB '*[^0-9a-f]*'),
    judge_model_artifact_id TEXT NOT NULL
        CHECK(length(judge_model_artifact_id) = 64)
        CHECK(judge_model_artifact_id = lower(judge_model_artifact_id))
        CHECK(judge_model_artifact_id NOT GLOB '*[^0-9a-f]*'),
    judge_runtime_installation_generation INTEGER NOT NULL
        CHECK(judge_runtime_installation_generation BETWEEN 1 AND 9223372036854775807),
    judge_model_installation_generation INTEGER NOT NULL
        CHECK(judge_model_installation_generation BETWEEN 1 AND 9223372036854775807),
    managed_preflight_digest TEXT NOT NULL
        CHECK(length(managed_preflight_digest) = 64)
        CHECK(managed_preflight_digest = lower(managed_preflight_digest))
        CHECK(managed_preflight_digest NOT GLOB '*[^0-9a-f]*'),
    retained_session_preflight_digest TEXT NOT NULL
        CHECK(length(retained_session_preflight_digest) = 64)
        CHECK(retained_session_preflight_digest = lower(retained_session_preflight_digest))
        CHECK(retained_session_preflight_digest NOT GLOB '*[^0-9a-f]*'),
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
    residency_receipt_aggregate_digest TEXT NOT NULL
        CHECK(length(residency_receipt_aggregate_digest) = 64)
        CHECK(residency_receipt_aggregate_digest = lower(residency_receipt_aggregate_digest))
        CHECK(residency_receipt_aggregate_digest NOT GLOB '*[^0-9a-f]*'),
    process_observation_aggregate_digest TEXT NOT NULL
        CHECK(length(process_observation_aggregate_digest) = 64)
        CHECK(process_observation_aggregate_digest =
            lower(process_observation_aggregate_digest))
        CHECK(process_observation_aggregate_digest NOT GLOB '*[^0-9a-f]*'),
    native_load_observation_aggregate_digest TEXT NOT NULL
        CHECK(length(native_load_observation_aggregate_digest) = 64)
        CHECK(native_load_observation_aggregate_digest =
            lower(native_load_observation_aggregate_digest))
        CHECK(native_load_observation_aggregate_digest NOT GLOB '*[^0-9a-f]*'),
    connection_observation_aggregate_digest TEXT NOT NULL
        CHECK(length(connection_observation_aggregate_digest) = 64)
        CHECK(connection_observation_aggregate_digest =
            lower(connection_observation_aggregate_digest))
        CHECK(connection_observation_aggregate_digest NOT GLOB '*[^0-9a-f]*'),
    effective_runtime_state_observation_aggregate_digest TEXT NOT NULL
        CHECK(length(effective_runtime_state_observation_aggregate_digest) = 64)
        CHECK(effective_runtime_state_observation_aggregate_digest =
            lower(effective_runtime_state_observation_aggregate_digest))
        CHECK(effective_runtime_state_observation_aggregate_digest NOT GLOB '*[^0-9a-f]*'),
    judge_effective_runtime_state_id TEXT NOT NULL
        CHECK(length(judge_effective_runtime_state_id) = 64)
        CHECK(judge_effective_runtime_state_id = lower(judge_effective_runtime_state_id))
        CHECK(judge_effective_runtime_state_id NOT GLOB '*[^0-9a-f]*'),
    judge_effective_runtime_state_join_id TEXT NOT NULL
        CHECK(length(judge_effective_runtime_state_join_id) = 64)
        CHECK(judge_effective_runtime_state_join_id =
            lower(judge_effective_runtime_state_join_id))
        CHECK(judge_effective_runtime_state_join_id NOT GLOB '*[^0-9a-f]*'),
    judge_effective_package_evidence_v2_id TEXT NOT NULL
        CHECK(length(judge_effective_package_evidence_v2_id) = 64)
        CHECK(judge_effective_package_evidence_v2_id =
            lower(judge_effective_package_evidence_v2_id))
        CHECK(judge_effective_package_evidence_v2_id NOT GLOB '*[^0-9a-f]*'),
    first_response_ordinal INTEGER NOT NULL CHECK(first_response_ordinal = 8),
    last_response_ordinal INTEGER NOT NULL
        CHECK(last_response_ordinal = 7 + attempt_count * 9),
    attempt_count INTEGER NOT NULL
        CHECK(attempt_count BETWEEN 2 AND 512 AND attempt_count % 2 = 0),
    cleanup_disposition TEXT NOT NULL CHECK(cleanup_disposition = 'succeeded'),
    runtime_package_revalidation_status TEXT NOT NULL
        CHECK(runtime_package_revalidation_status = 'succeeded'),
    model_package_revalidation_status TEXT NOT NULL
        CHECK(model_package_revalidation_status = 'succeeded'),
    evidence_class TEXT NOT NULL CHECK(evidence_class = 'managed_local_judge_triage'),
    canonical_json BLOB NOT NULL CHECK(length(canonical_json) BETWEEN 1 AND 262144),
    FOREIGN KEY (candidate_judge_plan_id, judge_generation_system_id)
        REFERENCES candidate_judge_plans(
            candidate_judge_plan_id,
            judge_generation_system_id
        ) ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (candidate_judge_plan_id, candidate_judge_schedule_id, attempt_count)
        REFERENCES candidate_judge_schedules(
            candidate_judge_plan_id,
            candidate_judge_schedule_id,
            entry_count
        ) ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (
        judge_request_aggregate_id,
        candidate_judge_plan_id,
        candidate_judge_schedule_id,
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
        candidate_judge_schedule_id,
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
        candidate_judge_schedule_id,
        attempt_count
    ) REFERENCES candidate_judge_observation_batches(
        candidate_judge_observation_batch_id,
        candidate_judge_request_aggregate_id,
        candidate_judge_plan_id,
        candidate_judge_schedule_id,
        entry_count
    ) ON UPDATE RESTRICT ON DELETE RESTRICT,
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
    connection.execute_batch(RECEIPT_SQL)?;
    Ok(())
}
