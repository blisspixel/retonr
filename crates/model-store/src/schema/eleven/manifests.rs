use rusqlite::Connection;

use crate::StoreResult;

pub(super) fn create(connection: &Connection) -> StoreResult<()> {
    create_attempt_ledger(connection)?;
    create_repeatability_results(connection)?;
    create_repeatability_manifest(connection)?;
    create_resource_manifest(connection)?;
    create_human_manifest(connection)
}

fn create_attempt_ledger(connection: &Connection) -> StoreResult<()> {
    connection.execute_batch(&phase_manifest_sql(
        "generation_attempt_ledger_manifests",
        "generation_attempt_ledger_manifest_id",
    ))?;
    Ok(())
}

fn create_repeatability_results(connection: &Connection) -> StoreResult<()> {
    connection.execute_batch(
        "CREATE TABLE generation_repeatability_result_records (
             generation_repeatability_result_id TEXT PRIMARY KEY NOT NULL
                 CHECK(length(generation_repeatability_result_id) = 64)
                 CHECK(generation_repeatability_result_id =
                     lower(generation_repeatability_result_id))
                 CHECK(generation_repeatability_result_id NOT GLOB '*[^0-9a-f]*'),
             generation_system_id TEXT NOT NULL
                 CHECK(length(generation_system_id) = 64)
                 CHECK(generation_system_id = lower(generation_system_id))
                 CHECK(generation_system_id NOT GLOB '*[^0-9a-f]*'),
             generation_qualification_plan_id TEXT NOT NULL
                 CHECK(length(generation_qualification_plan_id) = 64)
                 CHECK(generation_qualification_plan_id = lower(generation_qualification_plan_id))
                 CHECK(generation_qualification_plan_id NOT GLOB '*[^0-9a-f]*'),
             generation_suite_manifest_id TEXT NOT NULL
                 CHECK(length(generation_suite_manifest_id) = 64)
                 CHECK(generation_suite_manifest_id = lower(generation_suite_manifest_id))
                 CHECK(generation_suite_manifest_id NOT GLOB '*[^0-9a-f]*'),
             generation_repetition_id TEXT NOT NULL
                 CHECK(length(generation_repetition_id) = 64)
                 CHECK(generation_repetition_id = lower(generation_repetition_id))
                 CHECK(generation_repetition_id NOT GLOB '*[^0-9a-f]*'),
             generation_attempt_ledger_manifest_id TEXT NOT NULL
                 CHECK(length(generation_attempt_ledger_manifest_id) = 64)
                 CHECK(generation_attempt_ledger_manifest_id =
                     lower(generation_attempt_ledger_manifest_id))
                 CHECK(generation_attempt_ledger_manifest_id NOT GLOB '*[^0-9a-f]*'),
             terminal_stage TEXT NOT NULL
                 CHECK(terminal_stage = 'candidate_generation_failed'),
             candidate_generation_receipt_set_id TEXT
                 CHECK(candidate_generation_receipt_set_id IS NULL),
             candidate_deterministic_evaluation_id TEXT
                 CHECK(candidate_deterministic_evaluation_id IS NULL),
             candidate_judge_join_id TEXT
                 CHECK(candidate_judge_join_id IS NULL),
             canonical_json BLOB NOT NULL CHECK(length(canonical_json) BETWEEN 1 AND 16384),
             UNIQUE (generation_qualification_plan_id, generation_repetition_id),
             UNIQUE (
                 generation_attempt_ledger_manifest_id,
                 generation_repetition_id
             ),
             FOREIGN KEY (
                 generation_attempt_ledger_manifest_id,
                 generation_qualification_plan_id,
                 generation_system_id,
                 generation_suite_manifest_id
             ) REFERENCES generation_attempt_ledger_manifests(
                 generation_attempt_ledger_manifest_id,
                 generation_qualification_plan_id,
                 generation_system_id,
                 generation_suite_manifest_id
             ) ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (
                 generation_qualification_plan_id,
                 generation_repetition_id
             ) REFERENCES generation_qualification_plan_repetitions(
                 generation_qualification_plan_id,
                 generation_repetition_id
             ) ON UPDATE RESTRICT ON DELETE RESTRICT
         ) STRICT;",
    )?;
    Ok(())
}

fn create_repeatability_manifest(connection: &Connection) -> StoreResult<()> {
    connection.execute_batch(&phase_manifest_sql(
        "generation_repeatability_evidence_manifests",
        "generation_repeatability_evidence_manifest_id",
    ))?;
    Ok(())
}

fn create_resource_manifest(connection: &Connection) -> StoreResult<()> {
    connection.execute_batch(&phase_manifest_sql(
        "generation_resource_evidence_manifests",
        "generation_resource_evidence_manifest_id",
    ))?;
    Ok(())
}

fn create_human_manifest(connection: &Connection) -> StoreResult<()> {
    connection.execute_batch(&phase_manifest_sql(
        "generation_human_adjudication_evidence_manifests",
        "generation_human_adjudication_evidence_manifest_id",
    ))?;
    Ok(())
}

fn phase_manifest_sql(table: &str, identifier: &str) -> String {
    format!(
        "CREATE TABLE {table} (
             {identifier} TEXT PRIMARY KEY NOT NULL
                 CHECK(length({identifier}) = 64)
                 CHECK({identifier} = lower({identifier}))
                 CHECK({identifier} NOT GLOB '*[^0-9a-f]*'),
             generation_system_id TEXT NOT NULL
                 CHECK(length(generation_system_id) = 64)
                 CHECK(generation_system_id = lower(generation_system_id))
                 CHECK(generation_system_id NOT GLOB '*[^0-9a-f]*'),
             generation_qualification_plan_id TEXT NOT NULL
                 CHECK(length(generation_qualification_plan_id) = 64)
                 CHECK(generation_qualification_plan_id = lower(generation_qualification_plan_id))
                 CHECK(generation_qualification_plan_id NOT GLOB '*[^0-9a-f]*'),
             generation_suite_manifest_id TEXT NOT NULL
                 CHECK(length(generation_suite_manifest_id) = 64)
                 CHECK(generation_suite_manifest_id = lower(generation_suite_manifest_id))
                 CHECK(generation_suite_manifest_id NOT GLOB '*[^0-9a-f]*'),
             evidence_item_count INTEGER NOT NULL
                 CHECK(evidence_item_count BETWEEN 0 AND 1024),
             status TEXT NOT NULL CHECK(status IN ('passed', 'failed', 'skipped')),
             canonical_json BLOB NOT NULL CHECK(length(canonical_json) BETWEEN 1 AND 16384),
             CHECK(
                 (status = 'skipped' AND evidence_item_count = 0)
                 OR (status IN ('passed', 'failed')
                     AND evidence_item_count BETWEEN 1 AND 1024)
             ),
             UNIQUE (generation_qualification_plan_id, generation_system_id),
             UNIQUE (
                 {identifier},
                 generation_qualification_plan_id,
                 generation_system_id,
                 generation_suite_manifest_id
             ),
             FOREIGN KEY (
                 generation_qualification_plan_id,
                 generation_system_id
             ) REFERENCES generation_qualification_plan_systems(
                 generation_qualification_plan_id,
                 generation_system_id
             ) ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (generation_qualification_plan_id)
                 REFERENCES generation_qualification_plans(generation_qualification_plan_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (generation_suite_manifest_id)
                 REFERENCES generation_suite_manifests(generation_suite_manifest_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (generation_system_id)
                 REFERENCES generation_system_records(generation_system_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT
         ) STRICT;"
    )
}
