use std::path::Path;

use rusqlite::{Connection, params};
use tempfile::tempdir;

use super::{reserve_file, schema_version};
use crate::{ArtifactStateStore, StoreError, StoreMigrationDisposition};

const TERMINAL_TABLE: &str = "generation_repeatability_terminal_result_records";

const ABSENT_AUTHORITY_TABLES: [&str; 5] = [
    "generation_qualification_records",
    "generation_qualification_invalidations",
    "generation_qualification_selections",
    "generation_activation_decisions",
    "active_generation_bindings",
];

#[test]
fn schema_sixteen_fresh_database_is_inert_and_adds_only_the_terminal_result_table() {
    let current = Connection::open_in_memory().expect("open memory database");
    crate::schema::create_schema_sixteen_fixture(&current).expect("create schema sixteen");
    let version: i64 = current
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read schema version");
    assert_eq!(version, 16);

    let prior = Connection::open_in_memory().expect("open schema fifteen");
    crate::schema::create_schema_fifteen_fixture(&prior).expect("create schema fifteen");
    let mut added = table_names(&current);
    let prior_names = table_names(&prior);
    added.retain(|name| !prior_names.contains(name));
    assert_eq!(added, vec![TERMINAL_TABLE.to_owned()]);

    let sql = table_sql(&current, TERMINAL_TABLE);
    assert!(sql.contains("STRICT"));
    assert!(!sql.contains("schema_version"));
    assert!(sql.contains("CHECK(length(canonical_json) BETWEEN 1 AND 16384)"));
    assert!(sql.contains("'deterministic_failed', 'judge_failed', 'passed'"));
    assert!(!sql.contains("candidate_generation_failed"));
    assert!(sql.contains("candidate_judge_join_id IS NULL"));
    assert!(sql.contains("terminal_stage = 'passed' AND candidate_judge_join_id IS NOT NULL"));
    assert!(!sql.contains("candidate_judge_join_id TEXT NOT NULL"));
    assert_eq!(sql.matches("UNIQUE (").count(), 2);
    assert_eq!(
        sql.matches("ON UPDATE RESTRICT ON DELETE RESTRICT").count(),
        5
    );
    assert_eq!(explicit_index_count(&current, TERMINAL_TABLE), 0);
    assert_eq!(row_count(&current, TERMINAL_TABLE), 0);
    for table in ABSENT_AUTHORITY_TABLES {
        assert!(!table_exists(&current, table), "{table} must stay absent");
    }
    let repeatability = table_sql(&current, "generation_repeatability_result_records");
    assert!(repeatability.contains("terminal_stage = 'candidate_generation_failed'"));
    assert!(repeatability.contains("candidate_generation_receipt_set_id IS NULL"));
    assert!(repeatability.contains("candidate_deterministic_evaluation_id IS NULL"));
    assert!(repeatability.contains("candidate_judge_join_id IS NULL"));
    for parent in [
        "candidate_generation_receipt_sets",
        "candidate_deterministic_evaluation_records",
        "candidate_judge_join_records",
    ] {
        assert!(!table_sql(&current, parent).contains(TERMINAL_TABLE));
    }
    assert_eq!(foreign_key_violations(&current), 0);
}

#[test]
fn populated_schema_fifteen_migrates_after_verified_byte_preserving_backup() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("schema-fifteen.db");
    let backup = directory.path().join("schema-fifteen-backup.db");
    seed_schema_fifteen(&source);
    let mut backup_file = reserve_file(&backup);
    let mut session = ArtifactStateStore::begin_existing_migration(&source)
        .expect("begin schema-fifteen migration");
    assert_eq!(
        (
            session.schema_status().found,
            session.schema_status().current
        ),
        (15, 19)
    );
    session
        .backup_to(&mut backup_file, 16 * 1024 * 1024, || false)
        .expect("write verified backup");
    let result = session.migrate().expect("migrate schema fifteen");
    assert_eq!((result.from_schema, result.to_schema), (15, 19));
    assert_eq!(result.disposition, StoreMigrationDisposition::Migrated);
    assert_eq!(schema_version(&source), 19);
    assert_eq!(schema_version(&backup), 15);
    let migrated = Connection::open(&source).expect("reopen migrated source");
    assert_eq!(cluster_json(&migrated), b"{\"schema_version\":1}");
    assert!(table_exists(&migrated, TERMINAL_TABLE));
    assert_eq!(row_count(&migrated, TERMINAL_TABLE), 0);
    assert!(table_exists(
        &migrated,
        "candidate_deterministic_evaluation_records"
    ));
    let repeatability = table_sql(&migrated, "generation_repeatability_result_records");
    assert!(repeatability.contains("candidate_generation_failed"));
    assert!(repeatability.contains("candidate_deterministic_evaluation_id IS NULL"));
    assert_eq!(foreign_key_violations(&migrated), 0);
    let retained = Connection::open(&backup).expect("reopen schema-fifteen backup");
    assert!(!table_exists(&retained, TERMINAL_TABLE));
}

#[test]
fn schema_fifteen_migration_requires_backup_and_rolls_back() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("schema-fifteen-unbacked.db");
    seed_schema_fifteen(&source);
    let session =
        ArtifactStateStore::begin_existing_migration(&source).expect("begin unbacked migration");
    assert!(matches!(session.migrate(), Err(StoreError::BackupRequired)));
    assert_eq!(schema_version(&source), 15);
    let connection = Connection::open(&source).expect("reopen unmigrated source");
    assert!(!table_exists(&connection, TERMINAL_TABLE));
}

#[test]
fn inspection_rejects_altered_schema_sixteen_shape() {
    let directory = tempdir().expect("temporary directory");
    let current = directory.path().join("altered-schema-sixteen.db");
    drop(Connection::open(&current).expect("create empty database"));
    drop(
        ArtifactStateStore::open_existing_or_initialize_empty(&current)
            .expect("create current schema"),
    );
    rewrite_table_sql(
        &current,
        TERMINAL_TABLE,
        "'deterministic_failed', 'judge_failed', 'passed'",
        "'passed'",
    );
    assert!(matches!(
        ArtifactStateStore::inspect_existing_schema(&current),
        Err(StoreError::CorruptRecord)
    ));
    assert_eq!(schema_version(&current), 19);
}

#[test]
fn terminal_result_checks_reject_generation_failure_and_illegal_stage_pairs() {
    let connection = open_unchecked();
    assert!(
        insert(
            &connection,
            &row([1, 1, 1, 1], "deterministic_failed", [true, true, false], 1)
        )
        .is_ok()
    );
    assert!(
        insert(
            &connection,
            &row([2, 2, 2, 2], "judge_failed", [true, true, false], 16_384)
        )
        .is_ok()
    );
    assert!(
        insert(
            &connection,
            &row([3, 3, 3, 3], "passed", [true, true, true], 1)
        )
        .is_ok()
    );
    assert!(
        insert(
            &connection,
            &row([4, 1, 4, 4], "deterministic_failed", [true, true, false], 1)
        )
        .is_ok()
    );
    assert_rejections(&connection);
}

fn assert_rejections(connection: &Connection) {
    assert!(
        insert(
            connection,
            &row(
                [5, 5, 5, 5],
                "candidate_generation_failed",
                [false, false, false],
                1
            )
        )
        .is_err()
    );
    assert!(
        insert(
            connection,
            &row([6, 6, 6, 6], "passed", [true, true, false], 1)
        )
        .is_err()
    );
    assert!(
        insert(
            connection,
            &row([7, 7, 7, 7], "deterministic_failed", [true, true, true], 1)
        )
        .is_err()
    );
    assert!(
        insert(
            connection,
            &row([8, 8, 8, 8], "judge_failed", [true, true, true], 1)
        )
        .is_err()
    );
    assert!(
        insert(
            connection,
            &row([9, 9, 9, 9], "deterministic_failed", [true, true, false], 0)
        )
        .is_err()
    );
    assert!(
        insert(
            connection,
            &row([0, 0, 0, 0], "judge_failed", [true, true, false], 16_385)
        )
        .is_err()
    );
    assert_identity_rejections(connection);
}

fn assert_identity_rejections(connection: &Connection) {
    let mut uppercase = digest('a', 10);
    uppercase.replace_range(0..1, "A");
    let mut rejected = row(
        [10, 10, 10, 10],
        "deterministic_failed",
        [true, true, false],
        1,
    );
    rejected.id = uppercase;
    assert!(insert(connection, &rejected).is_err());
    let mut short = row(
        [11, 11, 11, 11],
        "deterministic_failed",
        [true, true, false],
        1,
    );
    short.id.truncate(63);
    assert!(insert(connection, &short).is_err());
    assert!(
        insert(
            connection,
            &row(
                [12, 12, 12, 12],
                "deterministic_failed",
                [false, true, false],
                1
            )
        )
        .is_err()
    );
    assert!(
        insert(
            connection,
            &row([13, 13, 13, 13], "passed", [true, false, true], 1)
        )
        .is_err()
    );
    assert!(
        insert(
            connection,
            &row([1, 14, 14, 14], "passed", [true, true, true], 1)
        )
        .is_err()
    );
    assert!(
        insert(
            connection,
            &row([15, 1, 1, 15], "passed", [true, true, true], 1)
        )
        .is_err()
    );
    assert!(
        insert(
            connection,
            &row([16, 16, 1, 1], "judge_failed", [true, true, false], 1)
        )
        .is_err()
    );
}

struct TerminalRow {
    id: String,
    plan: String,
    repetition: String,
    ledger: String,
    stage: &'static str,
    receipt: Option<String>,
    evaluation: Option<String>,
    join: Option<String>,
    json_len: usize,
}

fn row(slots: [u8; 4], stage: &'static str, parents: [bool; 3], json_len: usize) -> TerminalRow {
    TerminalRow {
        id: digest('c', slots[0]),
        plan: digest('d', slots[1]),
        repetition: digest('e', slots[2]),
        ledger: digest('f', slots[3]),
        stage,
        receipt: parents[0].then(|| digest('0', slots[0])),
        evaluation: parents[1].then(|| digest('1', slots[0])),
        join: parents[2].then(|| digest('2', slots[0])),
        json_len,
    }
}

fn insert(connection: &Connection, row: &TerminalRow) -> rusqlite::Result<usize> {
    connection.execute(
        "INSERT INTO generation_repeatability_terminal_result_records (
             generation_repeatability_result_id, generation_system_id,
             generation_qualification_plan_id, generation_suite_manifest_id,
             generation_repetition_id, generation_attempt_ledger_manifest_id, terminal_stage,
             candidate_generation_receipt_set_id, candidate_deterministic_evaluation_id,
             candidate_judge_join_id, canonical_json
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            row.id,
            digest('a', 1),
            row.plan,
            digest('b', 1),
            row.repetition,
            row.ledger,
            row.stage,
            row.receipt,
            row.evaluation,
            row.join,
            vec![b'x'; row.json_len],
        ],
    )
}

fn seed_schema_fifteen(path: &Path) {
    let connection = Connection::open(path).expect("create schema fifteen");
    crate::schema::create_schema_fifteen_fixture(&connection).expect("create schema fifteen");
    connection
        .execute(
            "INSERT INTO generation_cluster_records
                 (generation_cluster_id, canonical_json) VALUES (?1, ?2)",
            params![digest('a', 0), b"{\"schema_version\":1}"],
        )
        .expect("insert retained schema-fifteen row");
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
