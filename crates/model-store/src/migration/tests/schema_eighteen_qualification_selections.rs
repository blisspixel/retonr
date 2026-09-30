use std::path::Path;

use rusqlite::{Connection, params};
use tempfile::tempdir;

use super::super::super::{reserve_file, schema_version};
use crate::{ArtifactStateStore, StoreError, StoreMigrationDisposition};

const SELECTION_TABLE: &str = "generation_qualification_selections";

const ABSENT_AUTHORITY_TABLES: [&str; 2] = [
    "generation_activation_decisions",
    "active_generation_bindings",
];

#[test]
fn schema_nineteen_fresh_database_is_inert_and_adds_only_the_selection_table() {
    let mut current = Connection::open_in_memory().expect("open memory database");
    crate::schema::initialize_empty(&mut current).expect("initialize current schema");
    let version: i64 = current
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read schema version");
    assert_eq!(version, 19);

    let prior = Connection::open_in_memory().expect("open schema eighteen");
    crate::schema::create_schema_eighteen_fixture(&prior).expect("create schema eighteen");
    let mut added = table_names(&current);
    let prior_names = table_names(&prior);
    added.retain(|name| !prior_names.contains(name));
    assert_eq!(added, vec![SELECTION_TABLE.to_owned()]);

    let sql = table_sql(&current, SELECTION_TABLE);
    assert!(sql.contains("STRICT"));
    assert!(!sql.contains("schema_version"));
    assert!(!sql.contains("reason_code"));
    assert!(!sql.contains("sequence"));
    assert!(!sql.contains("status"));
    assert!(!sql.contains("qualified"));
    assert!(!sql.contains("role"));
    assert!(!sql.contains("action"));
    assert!(sql.contains("CHECK(length(canonical_json) BETWEEN 1 AND 16384)"));
    assert!(sql.contains("UNIQUE (generation_qualification_id)"));
    assert_eq!(sql.matches("UNIQUE (").count(), 1);
    assert_eq!(
        sql.matches("ON UPDATE RESTRICT ON DELETE RESTRICT").count(),
        1
    );
    assert!(sql.contains("generation_qualification_records"));
    for absent in [
        "target_generation_system_id",
        "operation_policy_id",
        "generation_qualification_plan_id",
        "generation_attempt_ledger_manifest_id",
        "candidate_judge_join_id",
    ] {
        assert!(!sql.contains(absent), "{absent}");
    }
    assert_eq!(explicit_index_count(&current, SELECTION_TABLE), 0);
    assert_eq!(row_count(&current, SELECTION_TABLE), 0);
    for table in ABSENT_AUTHORITY_TABLES {
        assert!(!table_exists(&current, table), "{table} must stay absent");
    }
    let repeatability = table_sql(&current, "generation_repeatability_result_records");
    assert!(repeatability.contains("terminal_stage = 'candidate_generation_failed'"));
    assert!(!table_sql(&current, "generation_qualification_records").contains(SELECTION_TABLE));
    assert_eq!(foreign_key_violations(&current), 0);
}

#[test]
fn populated_schema_eighteen_migrates_after_verified_byte_preserving_backup() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("schema-eighteen.db");
    let backup = directory.path().join("schema-eighteen-backup.db");
    seed_schema_eighteen(&source);
    let mut backup_file = reserve_file(&backup);
    let mut session = ArtifactStateStore::begin_existing_migration(&source)
        .expect("begin schema-eighteen migration");
    assert_eq!(
        (
            session.schema_status().found,
            session.schema_status().current
        ),
        (18, 19)
    );
    session
        .backup_to(&mut backup_file, 16 * 1024 * 1024, || false)
        .expect("write verified backup");
    let result = session.migrate().expect("migrate schema eighteen");
    assert_eq!((result.from_schema, result.to_schema), (18, 19));
    assert_eq!(result.disposition, StoreMigrationDisposition::Migrated);
    assert_eq!(schema_version(&source), 19);
    assert_eq!(schema_version(&backup), 18);
    let migrated = Connection::open(&source).expect("reopen migrated source");
    assert_eq!(cluster_json(&migrated), b"{\"schema_version\":1}");
    assert!(table_exists(&migrated, SELECTION_TABLE));
    assert_eq!(row_count(&migrated, SELECTION_TABLE), 0);
    assert!(table_exists(&migrated, "generation_qualification_records"));
    let repeatability = table_sql(&migrated, "generation_repeatability_result_records");
    assert!(repeatability.contains("candidate_generation_failed"));
    assert_eq!(foreign_key_violations(&migrated), 0);
    let retained = Connection::open(&backup).expect("reopen schema-eighteen backup");
    assert!(!table_exists(&retained, SELECTION_TABLE));
}

#[test]
fn schema_eighteen_migration_requires_backup_and_rolls_back() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("schema-eighteen-unbacked.db");
    seed_schema_eighteen(&source);
    let session =
        ArtifactStateStore::begin_existing_migration(&source).expect("begin unbacked migration");
    assert!(matches!(session.migrate(), Err(StoreError::BackupRequired)));
    assert_eq!(schema_version(&source), 18);
    let connection = Connection::open(&source).expect("reopen unmigrated source");
    assert!(!table_exists(&connection, SELECTION_TABLE));
}

#[test]
fn inspection_rejects_altered_schema_nineteen_shape() {
    let directory = tempdir().expect("temporary directory");
    let current = directory.path().join("altered-schema-nineteen.db");
    drop(Connection::open(&current).expect("create empty database"));
    drop(
        ArtifactStateStore::open_existing_or_initialize_empty(&current)
            .expect("create current schema"),
    );
    rewrite_table_sql(
        &current,
        SELECTION_TABLE,
        "CHECK(length(canonical_json) BETWEEN 1 AND 16384)",
        "CHECK(length(canonical_json) BETWEEN 1 AND 16383)",
    );
    assert!(matches!(
        ArtifactStateStore::inspect_existing_schema(&current),
        Err(StoreError::CorruptRecord)
    ));
    assert_eq!(schema_version(&current), 19);
}

#[test]
fn selection_checks_reject_illegal_identity_and_qualification_reuse() {
    let connection = open_unchecked();
    assert!(insert(&connection, &row(1, 1)).is_ok());
    assert!(insert(&connection, &row(2, 16_384)).is_ok());
    assert!(insert(&connection, &row(3, 4_096)).is_ok());
    let mut checked = Connection::open_in_memory().expect("open checked database");
    crate::schema::initialize_empty(&mut checked).expect("initialize checked schema");
    assert!(insert(&checked, &row(20, 1)).is_err());
    assert_rejections(&connection);
}

fn assert_rejections(connection: &Connection) {
    assert!(insert(connection, &row(7, 0)).is_err());
    assert!(insert(connection, &row(8, 16_385)).is_err());
    let mut missing = row(9, 1);
    missing.qualification_id = None;
    assert!(insert(connection, &missing).is_err());
    assert_identity_rejections(connection);
}

fn assert_identity_rejections(connection: &Connection) {
    let mut uppercase = digest('c', 10);
    uppercase.replace_range(0..1, "A");
    let mut rejected = row(10, 1);
    rejected.id = uppercase;
    assert!(insert(connection, &rejected).is_err());
    let mut short = row(11, 1);
    short.id.truncate(63);
    assert!(insert(connection, &short).is_err());
    let mut non_hex = row(12, 1);
    non_hex.qualification_id = Some(digest('a', 12).replacen('a', "g", 1));
    assert!(insert(connection, &non_hex).is_err());
    let mut duplicate = row(13, 1);
    duplicate.id = digest('c', 1);
    assert!(insert(connection, &duplicate).is_err());
    let mut same_qualification = row(14, 1);
    same_qualification.qualification_id = Some(digest('a', 1));
    assert!(insert(connection, &same_qualification).is_err());
}

struct SelectionRow {
    id: String,
    qualification_id: Option<String>,
    json_len: usize,
}

fn row(slot: u8, json_len: usize) -> SelectionRow {
    SelectionRow {
        id: digest('c', slot),
        qualification_id: Some(digest('a', slot)),
        json_len,
    }
}

fn insert(connection: &Connection, row: &SelectionRow) -> rusqlite::Result<usize> {
    connection.execute(
        "INSERT INTO generation_qualification_selections (
             generation_qualification_selection_id, generation_qualification_id, canonical_json
         ) VALUES (?1, ?2, ?3)",
        params![row.id, row.qualification_id, vec![b'x'; row.json_len]],
    )
}

fn seed_schema_eighteen(path: &Path) {
    let connection = Connection::open(path).expect("create schema eighteen");
    crate::schema::create_schema_eighteen_fixture(&connection).expect("create schema eighteen");
    connection
        .execute(
            "INSERT INTO generation_cluster_records
                 (generation_cluster_id, canonical_json) VALUES (?1, ?2)",
            params![digest('a', 0), b"{\"schema_version\":1}"],
        )
        .expect("insert retained schema-eighteen row");
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

fn explicit_index_count(connection: &Connection, table: &str) -> i64 {
    connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema
             WHERE type = 'index' AND tbl_name = ?1 AND sql IS NOT NULL",
            [table],
            |row| row.get(0),
        )
        .expect("count explicit indexes")
}

fn table_names(connection: &Connection) -> Vec<String> {
    let mut statement = connection
        .prepare(
            "SELECT name FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'sqlite_%'
             ORDER BY name",
        )
        .expect("prepare table names");
    statement
        .query_map([], |row| row.get(0))
        .expect("query table names")
        .collect::<Result<Vec<_>, _>>()
        .expect("read table names")
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
