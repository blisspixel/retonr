use std::path::Path;

use rusqlite::{Connection, params};
use tempfile::tempdir;

use super::{reserve_file, schema_version};
use crate::{ArtifactStateStore, StoreError, StoreMigrationDisposition};

const EVALUATION_TABLE: &str = "candidate_deterministic_evaluation_records";

const ABSENT_AUTHORITY_TABLES: [&str; 4] = [
    "generation_qualification_records",
    "generation_qualification_invalidations",
    "generation_activation_decisions",
    "active_generation_bindings",
];

#[test]
fn schema_fifteen_fresh_database_is_inert_and_adds_only_the_evaluation_table() {
    let mut current = Connection::open_in_memory().expect("open memory database");
    crate::schema::initialize_empty(&mut current).expect("initialize current schema");
    let version: i64 = current
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read schema version");
    assert_eq!(version, 15);

    let prior = Connection::open_in_memory().expect("open schema fourteen");
    crate::schema::create_schema_fourteen_fixture(&prior).expect("create schema fourteen");
    let mut added = table_names(&current);
    let prior_names = table_names(&prior);
    added.retain(|name| !prior_names.contains(name));
    assert_eq!(added, vec![EVALUATION_TABLE.to_owned()]);

    let sql = table_sql(&current, EVALUATION_TABLE);
    assert!(sql.contains("STRICT"));
    assert!(sql.contains("CHECK(schema_version = 1)"));
    assert!(sql.contains("CHECK(length(canonical_json) BETWEEN 1 AND 16384)"));
    assert!(sql.contains("CHECK(candidate_a_receipt_set_id != candidate_b_receipt_set_id)"));
    assert!(!sql.contains("UNIQUE ("));
    assert_eq!(
        sql.matches("ON UPDATE RESTRICT ON DELETE RESTRICT").count(),
        2
    );
    assert_eq!(explicit_index_count(&current, EVALUATION_TABLE), 0);
    assert_eq!(row_count(&current, EVALUATION_TABLE), 0);
    for table in ABSENT_AUTHORITY_TABLES {
        assert!(!table_exists(&current, table), "{table} must stay absent");
    }
    let repeatability = table_sql(&current, "generation_repeatability_result_records");
    assert!(repeatability.contains("terminal_stage = 'candidate_generation_failed'"));
    assert!(repeatability.contains("candidate_generation_receipt_set_id IS NULL"));
    assert!(repeatability.contains("candidate_deterministic_evaluation_id IS NULL"));
    let receipt_sets = table_sql(&current, "candidate_generation_receipt_sets");
    assert!(!receipt_sets.contains(EVALUATION_TABLE));
    let join = table_sql(&current, "candidate_judge_join_records");
    assert!(!join.contains(EVALUATION_TABLE));
    assert_eq!(foreign_key_violations(&current), 0);
}

#[test]
fn populated_schema_fourteen_migrates_after_verified_byte_preserving_backup() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("schema-fourteen.db");
    let backup = directory.path().join("schema-fourteen-backup.db");
    seed_schema_fourteen(&source);
    let mut backup_file = reserve_file(&backup);
    let mut session = ArtifactStateStore::begin_existing_migration(&source)
        .expect("begin schema-fourteen migration");
    assert_eq!(
        (
            session.schema_status().found,
            session.schema_status().current
        ),
        (14, 15)
    );
    session
        .backup_to(&mut backup_file, 16 * 1024 * 1024, || false)
        .expect("write verified backup");
    let result = session.migrate().expect("migrate schema fourteen");
    assert_eq!((result.from_schema, result.to_schema), (14, 15));
    assert_eq!(result.disposition, StoreMigrationDisposition::Migrated);
    assert_eq!(schema_version(&source), 15);
    assert_eq!(schema_version(&backup), 14);
    let migrated = Connection::open(&source).expect("reopen migrated source");
    assert_eq!(cluster_json(&migrated), b"{\"schema_version\":1}");
    assert!(table_exists(&migrated, EVALUATION_TABLE));
    assert_eq!(row_count(&migrated, EVALUATION_TABLE), 0);
    assert!(table_exists(&migrated, "candidate_generation_receipt_sets"));
    let repeatability = table_sql(&migrated, "generation_repeatability_result_records");
    assert!(repeatability.contains("candidate_generation_receipt_set_id IS NULL"));
    assert_eq!(foreign_key_violations(&migrated), 0);
    let retained = Connection::open(&backup).expect("reopen schema-fourteen backup");
    assert!(!table_exists(&retained, EVALUATION_TABLE));
}

#[test]
fn schema_fourteen_migration_requires_backup_and_rolls_back() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("schema-fourteen-unbacked.db");
    seed_schema_fourteen(&source);
    let session =
        ArtifactStateStore::begin_existing_migration(&source).expect("begin unbacked migration");
    assert!(matches!(session.migrate(), Err(StoreError::BackupRequired)));
    assert_eq!(schema_version(&source), 14);
    let connection = Connection::open(&source).expect("reopen unmigrated source");
    assert!(!table_exists(&connection, EVALUATION_TABLE));
}

#[test]
fn inspection_rejects_altered_schema_fifteen_shape() {
    let directory = tempdir().expect("temporary directory");
    let current = directory.path().join("altered-schema-fifteen.db");
    drop(Connection::open(&current).expect("create empty database"));
    drop(
        ArtifactStateStore::open_existing_or_initialize_empty(&current)
            .expect("create current schema"),
    );
    rewrite_table_sql(
        &current,
        EVALUATION_TABLE,
        "CHECK(length(canonical_json) BETWEEN 1 AND 16384)",
        "CHECK(length(canonical_json) BETWEEN 1 AND 1024)",
    );
    assert!(matches!(
        ArtifactStateStore::inspect_existing_schema(&current),
        Err(StoreError::CorruptRecord)
    ));
    assert_eq!(schema_version(&current), 15);
}

#[test]
fn evaluation_checks_reject_bad_identity_bounds_and_duplicate_pairs() {
    let connection = open_unchecked();
    assert!(insert_evaluation(&connection, &digest('a', 0), 'a', 1, 1).is_ok());
    assert!(insert_evaluation(&connection, &digest('b', 0), 'b', 1, 16_384).is_ok());
    assert!(insert_evaluation(&connection, &digest('c', 0), 'c', 2, 1).is_err());
    assert!(insert_evaluation(&connection, &digest('d', 0), 'd', 1, 0).is_err());
    assert!(insert_evaluation(&connection, &digest('e', 0), 'e', 1, 16_385).is_err());
    let mut uppercase = digest('0', 0);
    uppercase.replace_range(0..1, "A");
    assert!(insert_evaluation(&connection, &uppercase, '0', 1, 1).is_err());
    let short = digest('1', 0);
    assert!(insert_evaluation(&connection, &short[..63], '1', 1, 1).is_err());
    assert!(
        insert_pair(
            &connection,
            &digest('2', 0),
            &digest('2', 2),
            &digest('2', 2)
        )
        .is_err()
    );
    assert!(insert_evaluation(&connection, &digest('3', 0), '3', 1, 1).is_ok());
    assert!(insert_evaluation(&connection, &digest('3', 0), '4', 1, 1).is_err());
    assert!(insert_evaluation(&connection, &digest('4', 0), '3', 1, 1).is_ok());
}

fn insert_evaluation(
    connection: &Connection,
    id: &str,
    scope: char,
    schema_version_value: i64,
    json_len: usize,
) -> rusqlite::Result<usize> {
    connection.execute(
        "INSERT INTO candidate_deterministic_evaluation_records (
             candidate_deterministic_evaluation_id, schema_version,
             candidate_a_receipt_set_id, candidate_b_receipt_set_id, canonical_json
         ) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            id,
            schema_version_value,
            digest(scope, 2),
            digest(scope, 3),
            vec![b'x'; json_len],
        ],
    )
}

fn insert_pair(
    connection: &Connection,
    id: &str,
    receipt_a: &str,
    receipt_b: &str,
) -> rusqlite::Result<usize> {
    connection.execute(
        "INSERT INTO candidate_deterministic_evaluation_records (
             candidate_deterministic_evaluation_id, schema_version,
             candidate_a_receipt_set_id, candidate_b_receipt_set_id, canonical_json
         ) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, 1_i64, receipt_a, receipt_b, vec![b'x'; 1]],
    )
}

fn seed_schema_fourteen(path: &Path) {
    let connection = Connection::open(path).expect("create schema fourteen");
    crate::schema::create_schema_fourteen_fixture(&connection).expect("create schema fourteen");
    connection
        .execute(
            "INSERT INTO generation_cluster_records
                 (generation_cluster_id, canonical_json) VALUES (?1, ?2)",
            params![digest('a', 0), b"{\"schema_version\":1}"],
        )
        .expect("insert retained schema-fourteen row");
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
