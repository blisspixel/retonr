use rusqlite::Connection;

use crate::StoreResult;

pub(super) fn create(connection: &Connection) -> StoreResult<()> {
    create_receipt(connection)?;
    create_interruption(connection)
}

fn create_receipt(connection: &Connection) -> StoreResult<()> {
    connection.execute_batch(RECEIPT_SQL)?;
    Ok(())
}

fn create_interruption(connection: &Connection) -> StoreResult<()> {
    connection.execute_batch(INTERRUPTION_SQL)?;
    Ok(())
}

const RECEIPT_SQL: &str = "CREATE TABLE generation_qualification_operation_receipts (
             generation_qualification_operation_receipt_id TEXT PRIMARY KEY NOT NULL
                 CHECK(length(generation_qualification_operation_receipt_id) = 64)
                 CHECK(generation_qualification_operation_receipt_id =
                     lower(generation_qualification_operation_receipt_id))
                 CHECK(generation_qualification_operation_receipt_id NOT GLOB '*[^0-9a-f]*'),
             operation_policy_id TEXT NOT NULL
                 CHECK(length(operation_policy_id) = 64)
                 CHECK(operation_policy_id = lower(operation_policy_id))
                 CHECK(operation_policy_id NOT GLOB '*[^0-9a-f]*'),
             request_projection_id TEXT NOT NULL
                 CHECK(length(request_projection_id) = 64)
                 CHECK(request_projection_id = lower(request_projection_id))
                 CHECK(request_projection_id NOT GLOB '*[^0-9a-f]*'),
             generation_qualification_plan_id TEXT NOT NULL
                 CHECK(length(generation_qualification_plan_id) = 64)
                 CHECK(generation_qualification_plan_id = lower(generation_qualification_plan_id))
                 CHECK(generation_qualification_plan_id NOT GLOB '*[^0-9a-f]*'),
             generation_suite_manifest_id TEXT NOT NULL
                 CHECK(length(generation_suite_manifest_id) = 64)
                 CHECK(generation_suite_manifest_id = lower(generation_suite_manifest_id))
                 CHECK(generation_suite_manifest_id NOT GLOB '*[^0-9a-f]*'),
             target_generation_system_id TEXT NOT NULL
                 CHECK(length(target_generation_system_id) = 64)
                 CHECK(target_generation_system_id = lower(target_generation_system_id))
                 CHECK(target_generation_system_id NOT GLOB '*[^0-9a-f]*'),
             baseline_generation_system_id TEXT NOT NULL
                 CHECK(length(baseline_generation_system_id) = 64)
                 CHECK(baseline_generation_system_id = lower(baseline_generation_system_id))
                 CHECK(baseline_generation_system_id NOT GLOB '*[^0-9a-f]*'),
             generation_qualification_platform_evidence_id TEXT NOT NULL
                 CHECK(length(generation_qualification_platform_evidence_id) = 64)
                 CHECK(generation_qualification_platform_evidence_id =
                     lower(generation_qualification_platform_evidence_id))
                 CHECK(generation_qualification_platform_evidence_id NOT GLOB '*[^0-9a-f]*'),
             generation_qualification_license_evidence_id TEXT NOT NULL
                 CHECK(length(generation_qualification_license_evidence_id) = 64)
                 CHECK(generation_qualification_license_evidence_id =
                     lower(generation_qualification_license_evidence_id))
                 CHECK(generation_qualification_license_evidence_id NOT GLOB '*[^0-9a-f]*'),
             generation_attempt_ledger_manifest_id TEXT NOT NULL
                 CHECK(length(generation_attempt_ledger_manifest_id) = 64)
                 CHECK(generation_attempt_ledger_manifest_id =
                     lower(generation_attempt_ledger_manifest_id))
                 CHECK(generation_attempt_ledger_manifest_id NOT GLOB '*[^0-9a-f]*'),
             generation_repeatability_evidence_manifest_id TEXT NOT NULL
                 CHECK(length(generation_repeatability_evidence_manifest_id) = 64)
                 CHECK(generation_repeatability_evidence_manifest_id =
                     lower(generation_repeatability_evidence_manifest_id))
                 CHECK(generation_repeatability_evidence_manifest_id NOT GLOB '*[^0-9a-f]*'),
             generation_resource_evidence_manifest_id TEXT NOT NULL
                 CHECK(length(generation_resource_evidence_manifest_id) = 64)
                 CHECK(generation_resource_evidence_manifest_id =
                     lower(generation_resource_evidence_manifest_id))
                 CHECK(generation_resource_evidence_manifest_id NOT GLOB '*[^0-9a-f]*'),
             generation_human_adjudication_evidence_manifest_id TEXT NOT NULL
                 CHECK(length(generation_human_adjudication_evidence_manifest_id) = 64)
                 CHECK(generation_human_adjudication_evidence_manifest_id =
                     lower(generation_human_adjudication_evidence_manifest_id))
                 CHECK(generation_human_adjudication_evidence_manifest_id NOT GLOB '*[^0-9a-f]*'),
             elapsed_nanoseconds INTEGER NOT NULL
                 CHECK(elapsed_nanoseconds BETWEEN 0 AND 9223372036854775807),
             peak_concurrent_attempts INTEGER NOT NULL
                 CHECK(peak_concurrent_attempts IN (0, 1)),
             terminal_status TEXT NOT NULL CHECK(terminal_status IN (
                 'completed', 'cancelled', 'deadline_exceeded', 'failed'
             )),
             finalization_status TEXT NOT NULL CHECK(finalization_status IN (
                 'not_required', 'passed', 'failed'
             )),
             canonical_json BLOB NOT NULL CHECK(length(canonical_json) BETWEEN 1 AND 16384),
             CHECK(target_generation_system_id <> baseline_generation_system_id),
             UNIQUE (operation_policy_id),
             CHECK(
                 (peak_concurrent_attempts = 0
                     AND finalization_status = 'not_required'
                     AND terminal_status IN (
                         'completed', 'cancelled', 'deadline_exceeded', 'failed'
                     ))
                 OR (peak_concurrent_attempts = 1
                     AND terminal_status = 'completed'
                     AND finalization_status = 'passed')
                 OR (peak_concurrent_attempts = 1
                     AND terminal_status IN ('cancelled', 'deadline_exceeded', 'failed')
                     AND finalization_status IN ('passed', 'failed'))
             ),
             UNIQUE (
                 generation_qualification_operation_receipt_id,
                 terminal_status
             ),
             UNIQUE (
                 generation_qualification_operation_receipt_id,
                 operation_policy_id,
                 generation_qualification_plan_id,
                 generation_suite_manifest_id,
                 target_generation_system_id
             ),
             UNIQUE (
                 generation_qualification_operation_receipt_id,
                 operation_policy_id,
                 generation_qualification_plan_id,
                 generation_suite_manifest_id,
                 target_generation_system_id,
                 baseline_generation_system_id
             ),
             FOREIGN KEY (
                 operation_policy_id,
                 generation_qualification_plan_id,
                 generation_suite_manifest_id,
                 target_generation_system_id,
                 baseline_generation_system_id
             ) REFERENCES generation_qualification_operation_policies(
                 operation_policy_id,
                 generation_qualification_plan_id,
                 suite_manifest_id,
                 target_generation_system_id,
                 baseline_generation_system_id
             ) ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (request_projection_id)
                 REFERENCES generation_qualification_request_projections(request_projection_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (
                 generation_qualification_platform_evidence_id,
                 operation_policy_id,
                 request_projection_id,
                 target_generation_system_id
             ) REFERENCES generation_qualification_platform_evidence(
                 generation_qualification_platform_evidence_id,
                 operation_policy_id,
                 request_projection_id,
                 target_generation_system_id
             ) ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (
                 generation_qualification_license_evidence_id,
                 operation_policy_id,
                 request_projection_id,
                 target_generation_system_id
             ) REFERENCES generation_qualification_license_evidence(
                 generation_qualification_license_evidence_id,
                 operation_policy_id,
                 request_projection_id,
                 target_generation_system_id
             ) ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (
                 generation_attempt_ledger_manifest_id,
                 generation_qualification_plan_id,
                 target_generation_system_id,
                 generation_suite_manifest_id
             ) REFERENCES generation_attempt_ledger_manifests(
                 generation_attempt_ledger_manifest_id,
                 generation_qualification_plan_id,
                 generation_system_id,
                 generation_suite_manifest_id
             ) ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (
                 generation_repeatability_evidence_manifest_id,
                 generation_qualification_plan_id,
                 target_generation_system_id,
                 generation_suite_manifest_id
             ) REFERENCES generation_repeatability_evidence_manifests(
                 generation_repeatability_evidence_manifest_id,
                 generation_qualification_plan_id,
                 generation_system_id,
                 generation_suite_manifest_id
             ) ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (
                 generation_resource_evidence_manifest_id,
                 generation_qualification_plan_id,
                 target_generation_system_id,
                 generation_suite_manifest_id
             ) REFERENCES generation_resource_evidence_manifests(
                 generation_resource_evidence_manifest_id,
                 generation_qualification_plan_id,
                 generation_system_id,
                 generation_suite_manifest_id
             ) ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (
                 generation_human_adjudication_evidence_manifest_id,
                 generation_qualification_plan_id,
                 target_generation_system_id,
                 generation_suite_manifest_id
             ) REFERENCES generation_human_adjudication_evidence_manifests(
                 generation_human_adjudication_evidence_manifest_id,
                 generation_qualification_plan_id,
                 generation_system_id,
                 generation_suite_manifest_id
             ) ON UPDATE RESTRICT ON DELETE RESTRICT
         ) STRICT;";

const INTERRUPTION_SQL: &str = "CREATE TABLE generation_qualification_phase_interruption_records (
             generation_qualification_phase_interruption_record_id TEXT PRIMARY KEY NOT NULL
                 CHECK(length(generation_qualification_phase_interruption_record_id) = 64)
                 CHECK(generation_qualification_phase_interruption_record_id =
                     lower(generation_qualification_phase_interruption_record_id))
                 CHECK(generation_qualification_phase_interruption_record_id
                     NOT GLOB '*[^0-9a-f]*'),
             generation_qualification_operation_receipt_id TEXT NOT NULL
                 CHECK(length(generation_qualification_operation_receipt_id) = 64)
                 CHECK(generation_qualification_operation_receipt_id =
                     lower(generation_qualification_operation_receipt_id))
                 CHECK(generation_qualification_operation_receipt_id NOT GLOB '*[^0-9a-f]*'),
             operation_policy_id TEXT NOT NULL
                 CHECK(length(operation_policy_id) = 64)
                 CHECK(operation_policy_id = lower(operation_policy_id))
                 CHECK(operation_policy_id NOT GLOB '*[^0-9a-f]*'),
             target_generation_system_id TEXT NOT NULL
                 CHECK(length(target_generation_system_id) = 64)
                 CHECK(target_generation_system_id = lower(target_generation_system_id))
                 CHECK(target_generation_system_id NOT GLOB '*[^0-9a-f]*'),
             generation_qualification_plan_id TEXT NOT NULL
                 CHECK(length(generation_qualification_plan_id) = 64)
                 CHECK(generation_qualification_plan_id = lower(generation_qualification_plan_id))
                 CHECK(generation_qualification_plan_id NOT GLOB '*[^0-9a-f]*'),
             generation_suite_manifest_id TEXT NOT NULL
                 CHECK(length(generation_suite_manifest_id) = 64)
                 CHECK(generation_suite_manifest_id = lower(generation_suite_manifest_id))
                 CHECK(generation_suite_manifest_id NOT GLOB '*[^0-9a-f]*'),
             phase TEXT NOT NULL CHECK(phase IN (
                 'attempt_ledger', 'repeatability', 'resource_evidence', 'human_adjudication'
             )),
             checkpoint TEXT NOT NULL CHECK(checkpoint IN (
                 'before_phase',
                 'evidence_acquisition',
                 'evidence_compilation',
                 'manifest_compilation',
                 'final_authority_revalidation',
                 'mandatory_finalization'
             )),
             planned_candidate_attempt_id TEXT
                 CHECK(planned_candidate_attempt_id IS NULL OR (
                     length(planned_candidate_attempt_id) = 64
                     AND planned_candidate_attempt_id = lower(planned_candidate_attempt_id)
                     AND planned_candidate_attempt_id NOT GLOB '*[^0-9a-f]*'
                 )),
             reason TEXT NOT NULL CHECK(reason IN (
                 'cancelled',
                 'deadline_exceeded',
                 'authority_drift',
                 'required_observation_missing',
                 'required_observation_invalid',
                 'measurement_overflow',
                 'evidence_compilation_failed',
                 'cleanup_failed'
             )),
             terminal_status TEXT NOT NULL CHECK(terminal_status IN (
                 'cancelled', 'deadline_exceeded', 'failed'
             )),
             canonical_json BLOB NOT NULL CHECK(length(canonical_json) BETWEEN 1 AND 16384),
             CHECK(
                 (reason = 'cancelled' AND terminal_status = 'cancelled')
                 OR (reason = 'deadline_exceeded' AND terminal_status = 'deadline_exceeded')
                 OR (reason IN (
                     'authority_drift',
                     'required_observation_missing',
                     'required_observation_invalid',
                     'measurement_overflow',
                     'evidence_compilation_failed',
                     'cleanup_failed'
                 ) AND terminal_status = 'failed')
             ),
             CHECK(
                 checkpoint <> 'before_phase' OR planned_candidate_attempt_id IS NULL
             ),
             UNIQUE (operation_policy_id),
             UNIQUE (generation_qualification_operation_receipt_id),
             FOREIGN KEY (
                 generation_qualification_operation_receipt_id,
                 terminal_status
             ) REFERENCES generation_qualification_operation_receipts(
                 generation_qualification_operation_receipt_id,
                 terminal_status
             ) ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (
                 generation_qualification_operation_receipt_id,
                 operation_policy_id,
                 generation_qualification_plan_id,
                 generation_suite_manifest_id,
                 target_generation_system_id
             ) REFERENCES generation_qualification_operation_receipts(
                 generation_qualification_operation_receipt_id,
                 operation_policy_id,
                 generation_qualification_plan_id,
                 generation_suite_manifest_id,
                 target_generation_system_id
             ) ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (
                 generation_qualification_plan_id,
                 planned_candidate_attempt_id
             ) REFERENCES generation_qualification_plan_attempts(
                 generation_qualification_plan_id,
                 planned_candidate_attempt_id
             ) ON UPDATE RESTRICT ON DELETE RESTRICT
         ) STRICT;";
