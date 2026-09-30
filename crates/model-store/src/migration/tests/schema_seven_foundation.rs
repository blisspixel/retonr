use std::collections::BTreeMap;

use rusqlite::Connection;
use tempfile::tempdir;

use super::{reserve_file, schema_five_packages, schema_version};
use crate::{ArtifactStateStore, StoreError, StoreMigrationDisposition};

const FOUNDATION_TABLES: [&str; 2] = ["effective_package_evidence_v2", "generation_system_records"];
const PLAN_FOUNDATION_TABLES: [&str; 12] = [
    "candidate_selection_policies",
    "generation_case_manifests",
    "generation_cluster_records",
    "generation_deterministic_case_contracts",
    "generation_qualification_plan_attempts",
    "generation_qualification_plan_repetitions",
    "generation_qualification_plan_systems",
    "generation_qualification_plans",
    "generation_repetition_records",
    "generation_suite_cases",
    "generation_suite_manifests",
    "planned_candidate_attempts",
];
const PHASE_EVIDENCE_TABLES: [&str; 3] = [
    "generation_human_adjudication_policy_denial_records",
    "generation_resource_attempt_result_records",
    "generation_resource_policy_denial_records",
];
const RECEIPT_SET_TABLES: [&str; 1] = ["candidate_generation_receipt_sets"];
const TERMINAL_RESULT_TABLES: [&str; 1] = ["generation_repeatability_terminal_result_records"];
const EVALUATION_TABLES: [&str; 1] = ["candidate_deterministic_evaluation_records"];
const JUDGE_EXECUTION_TABLES: [&str; 7] = [
    "candidate_judge_join_records",
    "managed_local_judge_receipts",
    "candidate_judge_observation_batches",
    "candidate_judge_response_aggregates",
    "candidate_judge_request_aggregates",
    "candidate_judge_schedules",
    "candidate_judge_plans",
];
const TERMINAL_EVIDENCE_TABLES: [&str; 9] = [
    "generation_qualification_phase_interruption_records",
    "generation_qualification_operation_receipts",
    "generation_repeatability_result_records",
    "generation_human_adjudication_evidence_manifests",
    "generation_resource_evidence_manifests",
    "generation_repeatability_evidence_manifests",
    "generation_attempt_ledger_manifests",
    "generation_qualification_license_evidence",
    "generation_qualification_platform_evidence",
];
const TERMINAL_CLOSURE_TABLES: [&str; 8] = [
    "candidate_generation_attempt_precursors",
    "managed_candidate_generation_evidence",
    "candidate_generation_cleanup_records",
    "generation_evidence_bundles",
    "generation_evidence_bundle_storage",
    "generation_evidence_bundle_readbacks",
    "candidate_generation_receipts",
    "candidate_generation_attempt_records",
];

#[test]
fn schema_seven_migration_preserves_every_legacy_value_and_verified_backup() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("schema-seven.db");
    let backup = directory.path().join("schema-seven-backup.db");
    let connection = Connection::open(&source).expect("create schema seven");
    crate::schema::create_schema_seven_fixture(&connection).expect("create schema seven");
    seed_schema_seven_rows(&connection);
    let before = all_rows(&connection);
    drop(connection);

    let mut backup_file = reserve_file(&backup);
    let session = ArtifactStateStore::begin_existing_migration(&source)
        .expect("begin migration without backup");
    assert!(matches!(session.migrate(), Err(StoreError::BackupRequired)));
    let mut session =
        ArtifactStateStore::begin_existing_migration(&source).expect("begin backed migration");
    assert_eq!(
        (
            session.schema_status().found,
            session.schema_status().current
        ),
        (7, 16)
    );
    session
        .backup_to(&mut backup_file, 16 * 1024 * 1024, || false)
        .expect("write verified backup");
    let result = session.migrate().expect("migrate schema seven");
    assert_eq!(result.disposition, StoreMigrationDisposition::Migrated);

    let backup_connection = Connection::open(&backup).expect("open backup");
    assert_eq!(schema_version(&backup), 7);
    assert_eq!(all_rows(&backup_connection), before);
    for table in FOUNDATION_TABLES
        .iter()
        .chain(PLAN_FOUNDATION_TABLES.iter())
        .chain(TERMINAL_CLOSURE_TABLES.iter())
        .chain(TERMINAL_EVIDENCE_TABLES.iter())
        .chain(JUDGE_EXECUTION_TABLES.iter())
        .chain(PHASE_EVIDENCE_TABLES.iter())
        .chain(RECEIPT_SET_TABLES.iter())
        .chain(EVALUATION_TABLES.iter())
        .chain(TERMINAL_RESULT_TABLES.iter())
    {
        assert!(!schema_five_packages::table_exists(
            &backup_connection,
            table
        ));
    }

    let migrated = Connection::open(&source).expect("open migrated source");
    assert_eq!(schema_version(&source), 16);
    assert_eq!(all_rows(&migrated), before);
    for table in FOUNDATION_TABLES
        .iter()
        .chain(PLAN_FOUNDATION_TABLES.iter())
        .chain(TERMINAL_CLOSURE_TABLES.iter())
        .chain(TERMINAL_EVIDENCE_TABLES.iter())
        .chain(JUDGE_EXECUTION_TABLES.iter())
        .chain(PHASE_EVIDENCE_TABLES.iter())
        .chain(RECEIPT_SET_TABLES.iter())
        .chain(EVALUATION_TABLES.iter())
        .chain(TERMINAL_RESULT_TABLES.iter())
    {
        let count: i64 = migrated
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .expect("count new table");
        assert_eq!(count, 0);
    }
}

#[test]
fn schema_seven_requires_exact_shape_before_backup_authority() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("altered-seven.db");
    let connection = Connection::open(&source).expect("create schema seven");
    crate::schema::create_schema_seven_fixture(&connection).expect("create schema seven");
    connection
        .execute_batch("CREATE TABLE unexpected_schema_seven_state(value BLOB) STRICT;")
        .expect("alter schema seven");
    drop(connection);
    assert!(matches!(
        ArtifactStateStore::begin_existing_migration(&source),
        Err(StoreError::CorruptRecord)
    ));
    assert_eq!(schema_version(&source), 7);
}

#[test]
fn schema_eight_foundation_tables_are_strict_bounded_and_recursively_bound() {
    let connection = Connection::open_in_memory().expect("open memory database");
    crate::schema::create_schema_eight_fixture(&connection).expect("initialize schema eight");
    for table in FOUNDATION_TABLES {
        let sql: String = connection
            .query_row(
                "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = ?1",
                [table],
                |row| row.get(0),
            )
            .expect("read table SQL");
        assert!(sql.contains("STRICT"));
        assert!(sql.contains("canonical_json BLOB NOT NULL"));
        assert!(sql.contains("NOT GLOB '*[^0-9a-f]*'"));
        assert!(sql.contains("ON UPDATE RESTRICT ON DELETE RESTRICT"));
        if table == "effective_package_evidence_v2" {
            assert!(sql.contains("length(canonical_json) BETWEEN 1 AND 2097152"));
            assert!(sql.contains("member_count BETWEEN 1 AND 1024"));
        } else {
            assert!(sql.contains("length(canonical_json) BETWEEN 1 AND 16384"));
            assert!(sql.contains("model_artifact_id TEXT NOT NULL"));
            assert!(sql.contains("REFERENCES effective_package_evidence_v2"));
        }
    }
}

fn seed_schema_seven_rows(connection: &Connection) {
    let policy = "a".repeat(64);
    let plan = "b".repeat(64);
    let suite = "c".repeat(64);
    let target = "d".repeat(64);
    let baseline = "e".repeat(64);
    connection
        .execute(
            "INSERT INTO generation_qualification_operation_policies VALUES
             (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                &policy,
                &plan,
                &suite,
                &target,
                &baseline,
                br#"{ "legacy": 18 }"#,
            ],
        )
        .expect("seed policy");
    connection
        .execute(
            "INSERT INTO generation_qualification_request_projections VALUES
             (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7)",
            rusqlite::params![
                "f".repeat(64),
                &policy,
                &plan,
                &suite,
                &target,
                &baseline,
                br#"{ "legacy": 19 }"#,
            ],
        )
        .expect("seed projection");
}

fn all_rows(connection: &Connection) -> BTreeMap<String, Vec<Vec<Vec<u8>>>> {
    let mut statement = connection
        .prepare(
            "SELECT name FROM sqlite_schema
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%'
             ORDER BY name",
        )
        .expect("prepare table inventory");
    let tables = statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query table inventory")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect table inventory");
    tables
        .into_iter()
        .filter(|table| {
            !FOUNDATION_TABLES.contains(&table.as_str())
                && !PLAN_FOUNDATION_TABLES.contains(&table.as_str())
                && !TERMINAL_CLOSURE_TABLES.contains(&table.as_str())
                && !TERMINAL_EVIDENCE_TABLES.contains(&table.as_str())
                && !JUDGE_EXECUTION_TABLES.contains(&table.as_str())
                && !PHASE_EVIDENCE_TABLES.contains(&table.as_str())
                && !RECEIPT_SET_TABLES.contains(&table.as_str())
                && !EVALUATION_TABLES.contains(&table.as_str())
                && !TERMINAL_RESULT_TABLES.contains(&table.as_str())
        })
        .map(|table| {
            let rows = schema_five_packages::table_rows(connection, &table);
            (table, rows)
        })
        .collect()
}
