use std::path::Path;

use rusqlite::{Connection, params};
use tempfile::tempdir;

use super::{reserve_file, schema_version};
use crate::{ArtifactStateStore, StoreError, StoreMigrationDisposition};

const RECEIPT_SET_TABLE: &str = "candidate_generation_receipt_sets";

const ABSENT_AUTHORITY_TABLES: [&str; 4] = [
    "generation_qualification_records",
    "generation_qualification_invalidations",
    "generation_activation_decisions",
    "active_generation_bindings",
];

#[test]
fn schema_fourteen_fresh_database_is_inert_and_adds_only_the_receipt_set_table() {
    let current = Connection::open_in_memory().expect("open schema fourteen");
    crate::schema::create_schema_fourteen_fixture(&current).expect("create schema fourteen");
    let version: i64 = current
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read schema version");
    assert_eq!(version, 14);

    let prior = Connection::open_in_memory().expect("open schema thirteen");
    crate::schema::create_schema_thirteen_fixture(&prior).expect("create schema thirteen");
    let mut added = table_names(&current);
    let prior_names = table_names(&prior);
    added.retain(|name| !prior_names.contains(name));
    assert_eq!(added, vec![RECEIPT_SET_TABLE.to_owned()]);

    let sql = table_sql(&current, RECEIPT_SET_TABLE);
    assert!(sql.contains("STRICT"));
    assert!(sql.contains("CHECK(schema_version = 1)"));
    assert!(sql.contains("CHECK(entry_count BETWEEN 1 AND 256)"));
    assert!(sql.contains("CHECK(length(canonical_json) BETWEEN 1 AND 4194304)"));
    assert_eq!(
        sql.matches("ON UPDATE RESTRICT ON DELETE RESTRICT").count(),
        7
    );
    let compact = sql.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(compact.contains(
        "UNIQUE ( generation_qualification_plan_id, generation_repetition_id, generation_system_id )"
    ));
    assert_eq!(explicit_index_count(&current, RECEIPT_SET_TABLE), 0);
    assert_eq!(row_count(&current, RECEIPT_SET_TABLE), 0);
    for table in ABSENT_AUTHORITY_TABLES {
        assert!(!table_exists(&current, table), "{table} must stay absent");
    }
    let repeatability = table_sql(&current, "generation_repeatability_result_records");
    assert!(repeatability.contains("terminal_stage = 'candidate_generation_failed'"));
    assert!(repeatability.contains("candidate_generation_receipt_set_id IS NULL"));
    let join = table_sql(&current, "candidate_judge_join_records");
    assert!(!join.contains(RECEIPT_SET_TABLE));
    assert!(!table_exists(
        &current,
        "candidate_deterministic_evaluation_records"
    ));
    assert_eq!(foreign_key_violations(&current), 0);
}

#[test]
fn populated_schema_thirteen_migrates_after_verified_byte_preserving_backup() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("schema-thirteen.db");
    let backup = directory.path().join("schema-thirteen-backup.db");
    seed_schema_thirteen(&source);
    let mut backup_file = reserve_file(&backup);
    let mut session = ArtifactStateStore::begin_existing_migration(&source)
        .expect("begin schema-thirteen migration");
    assert_eq!(
        (
            session.schema_status().found,
            session.schema_status().current
        ),
        (13, 17)
    );
    session
        .backup_to(&mut backup_file, 16 * 1024 * 1024, || false)
        .expect("write verified backup");
    let result = session.migrate().expect("migrate schema thirteen");
    assert_eq!((result.from_schema, result.to_schema), (13, 17));
    assert_eq!(result.disposition, StoreMigrationDisposition::Migrated);
    assert_eq!(schema_version(&source), 17);
    assert_eq!(schema_version(&backup), 13);
    let migrated = Connection::open(&source).expect("reopen migrated source");
    assert_eq!(cluster_json(&migrated), b"{\"schema_version\":1}");
    assert!(table_exists(&migrated, RECEIPT_SET_TABLE));
    assert_eq!(row_count(&migrated, RECEIPT_SET_TABLE), 0);
    let repeatability = table_sql(&migrated, "generation_repeatability_result_records");
    assert!(repeatability.contains("candidate_generation_receipt_set_id IS NULL"));
    assert_eq!(foreign_key_violations(&migrated), 0);
    let retained = Connection::open(&backup).expect("reopen schema-thirteen backup");
    assert!(!table_exists(&retained, RECEIPT_SET_TABLE));
}

#[test]
fn schema_thirteen_migration_requires_backup_and_rolls_back() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("schema-thirteen-unbacked.db");
    seed_schema_thirteen(&source);
    let session =
        ArtifactStateStore::begin_existing_migration(&source).expect("begin unbacked migration");
    assert!(matches!(session.migrate(), Err(StoreError::BackupRequired)));
    assert_eq!(schema_version(&source), 13);
    let connection = Connection::open(&source).expect("reopen unmigrated source");
    assert!(!table_exists(&connection, RECEIPT_SET_TABLE));
}

#[test]
fn inspection_rejects_altered_schema_fourteen_shape() {
    let directory = tempdir().expect("temporary directory");
    let current = directory.path().join("altered-schema-fourteen.db");
    drop(Connection::open(&current).expect("create empty database"));
    drop(
        ArtifactStateStore::open_existing_or_initialize_empty(&current)
            .expect("create current schema"),
    );
    rewrite_table_sql(
        &current,
        RECEIPT_SET_TABLE,
        "CHECK(entry_count BETWEEN 1 AND 256)",
        "CHECK(entry_count BETWEEN 1 AND 16)",
    );
    assert!(matches!(
        ArtifactStateStore::inspect_existing_schema(&current),
        Err(StoreError::CorruptRecord)
    ));
    assert_eq!(schema_version(&current), 17);
}

#[test]
fn receipt_set_checks_reject_bad_identity_counts_and_bounds() {
    let connection = open_unchecked();
    assert!(insert_receipt_set(&connection, &digest('a', 0), 'a', 1, 1, 1).is_ok());
    assert!(insert_receipt_set(&connection, &digest('b', 0), 'b', 1, 256, 1).is_ok());
    assert!(insert_receipt_set(&connection, &digest('c', 0), 'c', 2, 1, 1).is_err());
    assert!(insert_receipt_set(&connection, &digest('d', 0), 'd', 1, 0, 1).is_err());
    assert!(insert_receipt_set(&connection, &digest('e', 0), 'e', 1, 257, 1).is_err());
    assert!(insert_receipt_set(&connection, &digest('f', 0), 'f', 1, 1, 0).is_err());
    assert!(insert_receipt_set(&connection, &digest('0', 0), '0', 1, 1, 4_194_305).is_err());
    let mut uppercase = digest('1', 0);
    uppercase.replace_range(0..1, "A");
    assert!(insert_receipt_set(&connection, &uppercase, '1', 1, 1, 1).is_err());
    let short = digest('2', 0);
    assert!(insert_receipt_set(&connection, &short[..63], '2', 1, 1, 1).is_err());
    assert!(insert_receipt_set(&connection, &digest('3', 0), '3', 1, 1, 1).is_ok());
    assert!(insert_receipt_set(&connection, &digest('4', 0), '3', 1, 1, 1).is_err());
}

fn insert_receipt_set(
    connection: &Connection,
    id: &str,
    scope: char,
    schema_version_value: i64,
    entry_count: i64,
    json_len: usize,
) -> rusqlite::Result<usize> {
    connection.execute(
        "INSERT INTO candidate_generation_receipt_sets (
             candidate_generation_receipt_set_id, schema_version,
             generation_qualification_plan_id, generation_suite_manifest_id,
             generation_repetition_id, generation_system_id, candidate_selection_policy_id,
             entry_count, canonical_json
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            id,
            schema_version_value,
            digest(scope, 1),
            digest(scope, 2),
            digest(scope, 3),
            digest(scope, 4),
            digest(scope, 5),
            entry_count,
            vec![b'x'; json_len],
        ],
    )
}

fn seed_schema_thirteen(path: &Path) {
    let connection = Connection::open(path).expect("create schema thirteen");
    crate::schema::create_schema_thirteen_fixture(&connection).expect("create schema thirteen");
    connection
        .execute(
            "INSERT INTO generation_cluster_records
                 (generation_cluster_id, canonical_json) VALUES (?1, ?2)",
            params![digest('a', 0), b"{\"schema_version\":1}"],
        )
        .expect("insert retained schema-thirteen row");
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
