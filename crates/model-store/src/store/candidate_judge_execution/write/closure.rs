use rusqlite::{Connection, params};

use super::super::codec::PreparedCohort;
use super::super::derive;
use super::super::text;
use super::insert;
use crate::{StoreResult, WriteDisposition};

pub(super) fn insert_receipt(
    connection: &Connection,
    prepared: &PreparedCohort<'_>,
) -> StoreResult<WriteDisposition> {
    let value = &prepared.records.receipt;
    insert(
        connection,
        "INSERT OR IGNORE INTO managed_local_judge_receipts (
            managed_local_judge_receipt_id, schema_version, candidate_judge_plan_id,
            candidate_judge_schedule_id, judge_generation_system_id,
            judge_runtime_admission_join_id, judge_managed_generation_path_id,
            judge_frozen_external_component_set_id, judge_runtime_package_manifest_id,
            judge_runtime_build_id, judge_model_package_manifest_id, judge_model_artifact_id,
            judge_runtime_installation_generation, judge_model_installation_generation,
            managed_preflight_digest, retained_session_preflight_digest,
            judge_request_aggregate_id, judge_response_aggregate_id, judge_observation_batch_id,
            residency_receipt_aggregate_digest, process_observation_aggregate_digest,
            native_load_observation_aggregate_digest, connection_observation_aggregate_digest,
            effective_runtime_state_observation_aggregate_digest,
            judge_effective_runtime_state_id, judge_effective_runtime_state_join_id,
            judge_effective_package_evidence_v2_id, first_response_ordinal, last_response_ordinal,
            attempt_count, cleanup_disposition, runtime_package_revalidation_status,
            model_package_revalidation_status, evidence_class, canonical_json
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18,
            ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34, ?35
        )",
        params![
            text::digest_text(value.managed_local_judge_receipt_id().digest()),
            i64::from(value.schema_version()),
            text::digest_text(value.candidate_judge_plan_id().digest()),
            text::digest_text(value.candidate_judge_schedule_id().digest()),
            text::digest_text(value.judge_generation_system_id().digest()),
            text::digest_text(value.judge_runtime_admission_join_id().digest()),
            text::digest_text(value.judge_managed_generation_path_id().digest()),
            text::digest_text(value.judge_frozen_external_component_set_id().digest()),
            text::digest_text(value.judge_runtime_package_manifest_id().digest()),
            text::digest_text(value.judge_runtime_build_id().digest()),
            text::digest_text(value.judge_model_package_manifest_id().digest()),
            text::digest_text(value.judge_model_artifact_id().digest()),
            derive::sql_u64(value.judge_runtime_installation_generation())?,
            derive::sql_u64(value.judge_model_installation_generation())?,
            text::digest_text(value.managed_preflight_digest()),
            text::digest_text(value.retained_session_preflight_digest()),
            text::digest_text(value.judge_request_aggregate_id().digest()),
            text::digest_text(value.judge_response_aggregate_id().digest()),
            text::digest_text(value.judge_observation_batch_id().digest()),
            text::digest_text(value.residency_receipt_aggregate_digest()),
            text::digest_text(value.process_observation_aggregate_digest()),
            text::digest_text(value.native_load_observation_aggregate_digest()),
            text::digest_text(value.connection_observation_aggregate_digest()),
            text::digest_text(value.effective_runtime_state_observation_aggregate_digest()),
            text::digest_text(value.judge_effective_runtime_state_id().digest()),
            text::digest_text(value.judge_effective_runtime_state_join_id().digest()),
            text::digest_text(value.judge_effective_package_evidence_v2_id().digest()),
            prepared.closed.first_ordinal,
            prepared.closed.last_ordinal,
            prepared.closed.attempt_count,
            text::receipt_success(value.cleanup_disposition()),
            text::receipt_success(value.runtime_package_revalidation_status()),
            text::receipt_success(value.model_package_revalidation_status()),
            text::receipt_evidence(value.evidence_class()),
            &prepared.encoded.receipt,
        ],
    )
}

pub(super) fn insert_join(
    connection: &Connection,
    prepared: &PreparedCohort<'_>,
) -> StoreResult<WriteDisposition> {
    let value = &prepared.records.join;
    insert(
        connection,
        "INSERT OR IGNORE INTO candidate_judge_join_records (
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
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18,
            ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34,
            ?35, ?36
        )",
        params![
            text::digest_text(value.candidate_judge_join_id().digest()),
            i64::from(value.schema_version()),
            text::digest_text(value.candidate_judge_plan_id().digest()),
            text::digest_text(value.candidate_receipt_pair_set_id().digest()),
            text::digest_text(value.candidate_a_receipt_set_id().digest()),
            text::digest_text(value.candidate_b_receipt_set_id().digest()),
            text::digest_text(value.candidate_a_generation_system_id().digest()),
            text::digest_text(value.candidate_b_generation_system_id().digest()),
            text::digest_text(value.judge_generation_system_id().digest()),
            text::digest_text(value.deterministic_evaluation_id().digest()),
            text::digest_text(value.case_material_set_digest()),
            text::digest_text(value.suite_pair_digest()),
            text::digest_text(value.judge_schedule_id().digest()),
            text::digest_text(value.managed_local_judge_receipt_id().digest()),
            text::digest_text(value.judge_request_aggregate_id().digest()),
            text::digest_text(value.judge_response_aggregate_id().digest()),
            text::digest_text(value.judge_observation_batch_id().digest()),
            prepared.closed.attempt_count,
            text::digest_text(value.judge_runtime_admission_join_id().digest()),
            text::digest_text(value.judge_managed_generation_path_id().digest()),
            text::digest_text(value.judge_frozen_external_component_set_id().digest()),
            text::digest_text(value.judge_runtime_package_manifest_id().digest()),
            text::digest_text(value.judge_runtime_build_id().digest()),
            text::digest_text(value.judge_effective_runtime_state_id().digest()),
            text::digest_text(value.judge_effective_runtime_state_join_id().digest()),
            text::digest_text(value.judge_model_package_manifest_id().digest()),
            text::digest_text(value.judge_model_artifact_id().digest()),
            text::digest_text(value.judge_effective_package_evidence_v2_id().digest()),
            derive::sql_u64(value.judge_runtime_installation_generation())?,
            derive::sql_u64(value.judge_model_installation_generation())?,
            text::digest_text(value.triage_report_digest()),
            text::join_evidence(value.evidence_class()),
            0_i64,
            0_i64,
            0_i64,
            &prepared.encoded.join,
        ],
    )
}
