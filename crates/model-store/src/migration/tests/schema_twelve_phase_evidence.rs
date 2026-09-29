use std::path::Path;

use rusqlite::{Connection, params};
use tempfile::tempdir;

use super::{reserve_file, schema_version};
use crate::{ArtifactStateStore, StoreError, StoreMigrationDisposition};

const PHASE_EVIDENCE_TABLES: [&str; 3] = [
    "generation_resource_attempt_result_records",
    "generation_resource_policy_denial_records",
    "generation_human_adjudication_policy_denial_records",
];

const PRIOR_TABLES: [&str; 2] = [
    "candidate_judge_plans",
    "generation_repeatability_result_records",
];

const ABSENT_AUTHORITY_TABLES: [&str; 4] = [
    "generation_qualification_records",
    "generation_qualification_invalidations",
    "generation_activation_decisions",
    "active_generation_bindings",
];

#[test]
fn schema_thirteen_fresh_database_is_inert_and_keeps_prior_tables() {
    let connection = Connection::open_in_memory().expect("open memory database");
    crate::schema::create_schema_thirteen_fixture(&connection).expect("create schema thirteen");
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read schema version");
    assert_eq!(version, 13);
    for table in PHASE_EVIDENCE_TABLES.iter().chain(PRIOR_TABLES.iter()) {
        let sql = table_sql(&connection, table);
        assert!(sql.contains("STRICT"), "{table} must be strict");
    }
    let repeatability = table_sql(&connection, "generation_repeatability_result_records");
    assert!(repeatability.contains("terminal_stage = 'candidate_generation_failed'"));
    for table in ABSENT_AUTHORITY_TABLES {
        assert!(
            !table_exists(&connection, table),
            "{table} must stay absent"
        );
    }
    assert!(!table_exists(
        &connection,
        "candidate_generation_receipt_sets"
    ));
    assert_eq!(foreign_key_violations(&connection), 0);
}

#[test]
fn populated_schema_twelve_migrates_after_verified_byte_preserving_backup() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("schema-twelve.db");
    let backup = directory.path().join("schema-twelve-backup.db");
    seed_schema_twelve(&source);
    let mut backup_file = reserve_file(&backup);
    let mut session = ArtifactStateStore::begin_existing_migration(&source)
        .expect("begin schema-twelve migration");
    assert_eq!(
        (
            session.schema_status().found,
            session.schema_status().current
        ),
        (12, 15)
    );
    session
        .backup_to(&mut backup_file, 16 * 1024 * 1024, || false)
        .expect("write verified backup");
    let result = session.migrate().expect("migrate schema twelve");
    assert_eq!((result.from_schema, result.to_schema), (12, 15));
    assert_eq!(result.disposition, StoreMigrationDisposition::Migrated);
    assert_eq!(schema_version(&source), 15);
    assert_eq!(schema_version(&backup), 12);
    let migrated = Connection::open(&source).expect("reopen migrated source");
    assert_eq!(cluster_json(&migrated), b"{\"schema_version\":1}");
    for table in PHASE_EVIDENCE_TABLES {
        assert!(table_exists(&migrated, table), "{table} must be created");
        assert_eq!(row_count(&migrated, table), 0);
    }
    assert_eq!(foreign_key_violations(&migrated), 0);
}

#[test]
fn schema_twelve_migration_requires_backup_and_rolls_back() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("schema-twelve-unbacked.db");
    seed_schema_twelve(&source);
    let session =
        ArtifactStateStore::begin_existing_migration(&source).expect("begin unbacked migration");
    assert!(matches!(session.migrate(), Err(StoreError::BackupRequired)));
    assert_eq!(schema_version(&source), 12);
    let connection = Connection::open(&source).expect("reopen unmigrated source");
    for table in PHASE_EVIDENCE_TABLES {
        assert!(!table_exists(&connection, table));
    }
}

#[test]
fn inspection_rejects_altered_schema_thirteen_shape() {
    let directory = tempdir().expect("temporary directory");
    let current = directory.path().join("altered-schema-thirteen.db");
    drop(Connection::open(&current).expect("create empty database"));
    drop(
        ArtifactStateStore::open_existing_or_initialize_empty(&current)
            .expect("create current schema"),
    );
    rewrite_table_sql(
        &current,
        "generation_resource_attempt_result_records",
        "CHECK(observation_profile = 'managed_ollama_v0_32_15_linux_v1')",
        "CHECK(observation_profile = 'other')",
    );
    assert!(matches!(
        ArtifactStateStore::inspect_existing_schema(&current),
        Err(StoreError::CorruptRecord)
    ));
    assert_eq!(schema_version(&current), 15);
}

#[test]
fn phase_evidence_checks_reject_open_profiles_kinds_and_bounds() {
    let connection = open_unchecked();
    assert!(resource(&connection, 'a', 1, "managed_ollama_v0_32_15_linux_v1", 16).is_ok());
    assert!(resource(&connection, 'b', 2, "managed_ollama_v0_32_15_linux_v1", 16).is_err());
    assert!(resource(&connection, 'c', 1, "other", 16).is_err());
    assert!(
        resource(
            &connection,
            'd',
            1,
            "managed_ollama_v0_32_15_linux_v1",
            16_385
        )
        .is_err()
    );
    assert!(resource(&connection, 'e', 1, "managed_ollama_v0_32_15_linux_v1", 0).is_err());
    assert!(
        denial(
            &connection,
            'f',
            "generation_resource_policy_denial_records",
            "generation_resource_policy_denial_record_id",
            "resource_policy_denial",
            "policy_source_denied",
            8
        )
        .is_ok()
    );
    assert!(
        denial(
            &connection,
            '0',
            "generation_resource_policy_denial_records",
            "generation_resource_policy_denial_record_id",
            "other",
            "policy_source_denied",
            8
        )
        .is_err()
    );
    assert!(
        denial(
            &connection,
            '1',
            "generation_resource_policy_denial_records",
            "generation_resource_policy_denial_record_id",
            "resource_policy_denial",
            "other",
            8
        )
        .is_err()
    );
    assert!(
        denial(
            &connection,
            '2',
            "generation_resource_policy_denial_records",
            "generation_resource_policy_denial_record_id",
            "resource_policy_denial",
            "policy_source_denied",
            4_097
        )
        .is_err()
    );
    assert!(
        denial(
            &connection,
            '3',
            "generation_human_adjudication_policy_denial_records",
            "generation_human_adjudication_policy_denial_record_id",
            "human_adjudication_policy_denial",
            "policy_source_denied",
            8
        )
        .is_ok()
    );
    assert!(
        denial(
            &connection,
            '4',
            "generation_human_adjudication_policy_denial_records",
            "generation_human_adjudication_policy_denial_record_id",
            "resource_policy_denial",
            "policy_source_denied",
            8
        )
        .is_err()
    );
}

fn resource(
    connection: &Connection,
    label: char,
    schema_version_value: i64,
    profile: &str,
    json_len: usize,
) -> rusqlite::Result<usize> {
    connection.execute(
        "INSERT INTO generation_resource_attempt_result_records (
             generation_resource_attempt_result_id, schema_version, generation_system_id,
             generation_qualification_plan_id, generation_suite_manifest_id, generation_case_id,
             generation_repetition_id, planned_candidate_attempt_id,
             candidate_generation_attempt_record_id, candidate_generation_receipt_id,
             resource_policy_digest, observation_profile, canonical_json
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            digest(label, 0),
            schema_version_value,
            digest(label, 1),
            digest(label, 2),
            digest(label, 3),
            digest(label, 4),
            digest(label, 5),
            digest(label, 6),
            digest(label, 7),
            digest(label, 8),
            digest(label, 9),
            profile,
            vec![b'x'; json_len],
        ],
    )
}

fn denial(
    connection: &Connection,
    label: char,
    table: &str,
    id_column: &str,
    kind: &str,
    reason: &str,
    json_len: usize,
) -> rusqlite::Result<usize> {
    connection.execute(
        &format!(
            "INSERT INTO {table} (
                 {id_column}, schema_version, record_kind, generation_system_id,
                 generation_qualification_plan_id, generation_suite_manifest_id,
                 phase_policy_digest, reason, canonical_json
             ) VALUES (?1, 1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
        ),
        params![
            digest(label, 0),
            kind,
            digest(label, 1),
            digest(label, 2),
            digest(label, 3),
            digest(label, 4),
            reason,
            vec![b'x'; json_len],
        ],
    )
}

fn seed_schema_twelve(path: &Path) {
    let connection = Connection::open(path).expect("create schema twelve");
    crate::schema::create_schema_twelve_fixture(&connection).expect("create schema twelve");
    connection
        .execute(
            "INSERT INTO generation_cluster_records
                 (generation_cluster_id, canonical_json) VALUES (?1, ?2)",
            params![digest('a', 0), b"{\"schema_version\":1}"],
        )
        .expect("insert retained schema-twelve row");
}

fn cluster_json(connection: &Connection) -> Vec<u8> {
    connection
        .query_row(
            "SELECT canonical_json FROM generation_cluster_records
             WHERE generation_cluster_id = ?1",
            [digest('a', 0)],
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

fn rewrite_table_sql(path: &Path, table: &str, from: &str, to: &str) {
    let connection = Connection::open(path).expect("reopen schema");
    connection
        .execute_batch("PRAGMA writable_schema = ON;")
        .expect("enable shape mutation");
    let changed = connection
        .execute(
            "UPDATE sqlite_schema SET sql = replace(sql, ?1, ?2) WHERE name = ?3",
            params![from, to, table],
        )
        .expect("alter table shape");
    assert_eq!(changed, 1);
    connection
        .execute_batch("PRAGMA writable_schema = OFF;")
        .expect("disable shape mutation");
}

fn row_count(connection: &Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("count table")
}

fn foreign_key_violations(connection: &Connection) -> i64 {
    connection
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .expect("check foreign keys")
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

fn digest(label: char, slot: u8) -> String {
    let mut value = format!("{slot:02x}{label}").repeat(21);
    value.push('0');
    value
}
