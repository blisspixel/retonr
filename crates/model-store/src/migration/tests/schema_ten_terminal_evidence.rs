use rusqlite::{Connection, params};
use tempfile::tempdir;

use super::{reserve_file, schema_version};
use crate::{ArtifactStateStore, StoreError, StoreMigrationDisposition};

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

const TERMINAL_EVIDENCE_TABLES: [&str; 9] = [
    "generation_qualification_platform_evidence",
    "generation_qualification_license_evidence",
    "generation_attempt_ledger_manifests",
    "generation_repeatability_result_records",
    "generation_repeatability_evidence_manifests",
    "generation_resource_evidence_manifests",
    "generation_human_adjudication_evidence_manifests",
    "generation_qualification_operation_receipts",
    "generation_qualification_phase_interruption_records",
];

#[test]
fn schema_eleven_fresh_database_is_version_11_and_keeps_schema_ten_tables() {
    let connection = Connection::open_in_memory().expect("open memory database");
    let mut connection = connection;
    crate::schema::initialize_empty(&mut connection).expect("initialize current schema");
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read schema version");
    assert_eq!(version, 11);
    for table in TERMINAL_CLOSURE_TABLES
        .iter()
        .chain(TERMINAL_EVIDENCE_TABLES.iter())
    {
        let sql = table_sql(&connection, table);
        assert!(sql.contains("STRICT"), "{table} must be strict");
        assert!(
            sql.contains("NOT GLOB '*[^0-9a-f]*'"),
            "{table} must bind hex ids"
        );
    }
    let violations: i64 = connection
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .expect("check foreign keys");
    assert_eq!(violations, 0);
}

#[test]
fn populated_schema_ten_migrates_after_verified_byte_preserving_backup() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("schema-ten.db");
    let backup = directory.path().join("schema-ten-backup.db");
    seed_schema_ten(&source);
    let mut backup_file = reserve_file(&backup);
    let mut session =
        ArtifactStateStore::begin_existing_migration(&source).expect("begin schema-ten migration");
    assert_eq!(
        (
            session.schema_status().found,
            session.schema_status().current
        ),
        (10, 11)
    );
    session
        .backup_to(&mut backup_file, 16 * 1024 * 1024, || false)
        .expect("write verified backup");
    let result = session.migrate().expect("migrate schema ten");
    assert_eq!(result.disposition, StoreMigrationDisposition::Migrated);
    assert_eq!((result.from_schema, result.to_schema), (10, 11));

    assert_eq!(schema_version(&backup), 10);
    let backup_connection = Connection::open(&backup).expect("open backup");
    assert_eq!(cluster_json(&backup_connection), b"{\"schema_version\":1}");
    for table in TERMINAL_EVIDENCE_TABLES {
        assert!(!table_exists(&backup_connection, table));
    }

    assert_eq!(schema_version(&source), 11);
    let migrated = Connection::open(&source).expect("open migrated source");
    assert_eq!(cluster_json(&migrated), b"{\"schema_version\":1}");
    for table in TERMINAL_EVIDENCE_TABLES {
        let count: i64 = migrated
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .expect("count new terminal-evidence table");
        assert_eq!(count, 0);
    }
    let violations: i64 = migrated
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .expect("check migrated foreign keys");
    assert_eq!(violations, 0);
}

#[test]
fn schema_ten_migration_requires_backup_and_rolls_back() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("schema-ten-unbacked.db");
    seed_schema_ten(&source);
    let session =
        ArtifactStateStore::begin_existing_migration(&source).expect("begin unbacked migration");
    assert!(matches!(session.migrate(), Err(StoreError::BackupRequired)));
    assert_eq!(schema_version(&source), 10);
    let connection = Connection::open(&source).expect("reopen unmigrated source");
    assert_eq!(cluster_json(&connection), b"{\"schema_version\":1}");
    for table in TERMINAL_EVIDENCE_TABLES {
        assert!(!table_exists(&connection, table));
    }
}

#[test]
fn inspection_rejects_altered_schema_ten_shape() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("altered-schema-ten.db");
    let connection = Connection::open(&path).expect("create schema ten");
    crate::schema::create_schema_ten_fixture(&connection).expect("create schema ten");
    connection
        .execute_batch("PRAGMA writable_schema = ON;")
        .expect("enable shape mutation");
    let changed = connection
        .execute(
            "UPDATE sqlite_schema
             SET sql = replace(sql, 'BETWEEN 1 AND 8193', 'BETWEEN 1 AND 8194')
             WHERE name = 'generation_evidence_bundle_storage'",
            [],
        )
        .expect("alter schema-ten table shape");
    assert_eq!(changed, 1);
    connection
        .execute_batch("PRAGMA writable_schema = OFF;")
        .expect("disable shape mutation");
    drop(connection);
    assert!(matches!(
        ArtifactStateStore::inspect_existing_schema(&path),
        Err(StoreError::CorruptRecord)
    ));
    assert_eq!(schema_version(&path), 10);
}

#[test]
fn schema_eleven_phase_manifest_counts_match_status() {
    let connection = open_unchecked();
    assert!(insert_manifest(&connection, 'a', "skipped", 0).is_ok());
    assert!(insert_manifest(&connection, 'b', "skipped", 1).is_err());
    assert!(insert_manifest(&connection, 'c', "passed", 0).is_err());
    assert!(insert_manifest(&connection, 'd', "failed", 0).is_err());
    assert!(insert_manifest(&connection, 'e', "passed", 1).is_ok());
}

#[test]
fn schema_eleven_stores_only_candidate_generation_failure() {
    let connection = open_unchecked();
    assert!(insert_result(&connection, 'a', "candidate_generation_failed").is_ok());
    for (identity, stage) in [
        ('b', "deterministic_failed"),
        ('c', "judge_failed"),
        ('d', "passed"),
        ('e', "CandidateGenerationFailed"),
    ] {
        assert!(
            insert_result(&connection, identity, stage).is_err(),
            "accepted repeatability stage {stage}"
        );
    }
}

#[test]
fn schema_eleven_interruption_rejects_a_completed_receipt() {
    let connection = open_unchecked();
    assert!(insert_interruption(&connection, 'a', "cancelled", "cancelled").is_ok());
    assert!(insert_interruption(&connection, 'b', "cancelled", "cleanup_failed").is_err());
    assert!(insert_interruption(&connection, 'c', "failed", "cleanup_failed").is_ok());
    assert!(insert_interruption(&connection, 'd', "completed", "cancelled").is_err());
}

fn seed_schema_ten(path: &std::path::Path) {
    let connection = Connection::open(path).expect("create schema ten");
    crate::schema::create_schema_ten_fixture(&connection).expect("create schema ten");
    connection
        .execute(
            "INSERT INTO generation_cluster_records
                 (generation_cluster_id, canonical_json) VALUES (?1, ?2)",
            params![digest('a'), b"{\"schema_version\":1}"],
        )
        .expect("insert retained schema-ten row");
}

fn cluster_json(connection: &Connection) -> Vec<u8> {
    connection
        .query_row(
            "SELECT canonical_json FROM generation_cluster_records
             WHERE generation_cluster_id = ?1",
            [digest('a')],
            |row| row.get(0),
        )
        .expect("read retained cluster row")
}

fn open_unchecked() -> Connection {
    let mut connection = Connection::open_in_memory().expect("open memory database");
    crate::schema::initialize_empty(&mut connection).expect("initialize current schema");
    connection
        .execute_batch("PRAGMA foreign_keys = OFF;")
        .expect("disable foreign keys");
    connection
}

fn insert_manifest(
    connection: &Connection,
    identity: char,
    status: &str,
    evidence_item_count: i64,
) -> rusqlite::Result<usize> {
    connection.execute(
        "INSERT INTO generation_repeatability_evidence_manifests (
             generation_repeatability_evidence_manifest_id,
             generation_system_id,
             generation_qualification_plan_id,
             generation_suite_manifest_id,
             evidence_item_count,
             status,
             canonical_json
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            digest(identity),
            digest(identity),
            digest('2'),
            digest('3'),
            evidence_item_count,
            status,
            b"{}"
        ],
    )
}

fn insert_result(connection: &Connection, identity: char, stage: &str) -> rusqlite::Result<usize> {
    connection.execute(
        "INSERT INTO generation_repeatability_result_records (
             generation_repeatability_result_id,
             generation_system_id,
             generation_qualification_plan_id,
             generation_suite_manifest_id,
             generation_repetition_id,
             generation_attempt_ledger_manifest_id,
             terminal_stage,
             candidate_generation_receipt_set_id,
             candidate_deterministic_evaluation_id,
             candidate_judge_join_id,
             canonical_json
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, NULL, NULL, ?8)",
        params![
            digest(identity),
            digest('1'),
            digest('2'),
            digest('3'),
            digest(identity),
            digest('5'),
            stage,
            b"{}"
        ],
    )
}

fn insert_interruption(
    connection: &Connection,
    identity: char,
    terminal_status: &str,
    reason: &str,
) -> rusqlite::Result<usize> {
    connection.execute(
        "INSERT INTO generation_qualification_phase_interruption_records (
             generation_qualification_phase_interruption_record_id,
             generation_qualification_operation_receipt_id,
             operation_policy_id,
             target_generation_system_id,
             generation_qualification_plan_id,
             generation_suite_manifest_id,
             phase,
             checkpoint,
             planned_candidate_attempt_id,
             reason,
             terminal_status,
             canonical_json
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'attempt_ledger', 'evidence_compilation', NULL, ?7, ?8, ?9)",
        params![
            digest(identity),
            digest(identity),
            digest(identity),
            digest('3'),
            digest('4'),
            digest('5'),
            reason,
            terminal_status,
            b"{}"
        ],
    )
}

fn table_sql(connection: &Connection, table: &str) -> String {
    connection
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = ?1",
            [table],
            |row| row.get(0),
        )
        .expect("read table SQL")
}

fn table_exists(connection: &Connection, table: &str) -> bool {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = ?1)",
            [table],
            |row| row.get(0),
        )
        .expect("inspect table existence")
}

fn digest(character: char) -> String {
    character.to_string().repeat(64)
}
