use rusqlite::{Connection, params};
use tempfile::tempdir;

use super::{reserve_file, schema_version};
use crate::{ArtifactStateStore, StoreError, StoreMigrationDisposition};
use inserts::{join, legal_plan, plan, receipt, result, schedule};

#[path = "schema_eleven_judge_execution_inserts.rs"]
mod inserts;

const JUDGE_EXECUTION_TABLES: [&str; 7] = [
    "candidate_judge_plans",
    "candidate_judge_schedules",
    "candidate_judge_request_aggregates",
    "candidate_judge_response_aggregates",
    "candidate_judge_observation_batches",
    "managed_local_judge_receipts",
    "candidate_judge_join_records",
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

const ABSENT_AUTHORITY_TABLES: [&str; 4] = [
    "generation_qualification_records",
    "generation_qualification_invalidations",
    "generation_activation_decisions",
    "active_generation_bindings",
];

#[test]
fn schema_twelve_fresh_database_is_inert_and_keeps_schema_eleven_tables() {
    let connection = Connection::open_in_memory().expect("open memory database");
    crate::schema::create_schema_twelve_fixture(&connection).expect("create schema twelve");
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read schema version");
    assert_eq!(version, 12);
    for table in [
        "generation_resource_attempt_result_records",
        "generation_resource_policy_denial_records",
        "generation_human_adjudication_policy_denial_records",
    ] {
        assert!(
            !table_exists(&connection, table),
            "{table} must stay absent"
        );
    }
    for table in JUDGE_EXECUTION_TABLES
        .iter()
        .chain(TERMINAL_EVIDENCE_TABLES.iter())
    {
        assert!(table_exists(&connection, table), "{table} must remain");
    }
    for table in JUDGE_EXECUTION_TABLES {
        let sql = table_sql(&connection, table);
        assert!(sql.contains("STRICT"), "{table} must be strict");
        assert!(
            sql.contains("NOT GLOB '*[^0-9a-f]*'"),
            "{table} must bind hex ids"
        );
    }
    assert_eq!(foreign_key_violations(&connection), 0);
}

#[test]
fn populated_schema_eleven_migrates_after_verified_byte_preserving_backup() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("schema-eleven.db");
    let backup = directory.path().join("schema-eleven-backup.db");
    seed_schema_eleven(&source);
    let mut backup_file = reserve_file(&backup);
    let mut session = ArtifactStateStore::begin_existing_migration(&source)
        .expect("begin schema-eleven migration");
    assert_eq!(
        (
            session.schema_status().found,
            session.schema_status().current
        ),
        (11, 16)
    );
    session
        .backup_to(&mut backup_file, 16 * 1024 * 1024, || false)
        .expect("write verified backup");
    let result = session.migrate().expect("migrate schema eleven");
    assert_eq!(result.disposition, StoreMigrationDisposition::Migrated);
    assert_eq!((result.from_schema, result.to_schema), (11, 16));

    assert_eq!(schema_version(&backup), 11);
    let backup_connection = Connection::open(&backup).expect("open backup");
    assert_eq!(cluster_json(&backup_connection), b"{\"schema_version\":1}");
    for table in JUDGE_EXECUTION_TABLES {
        assert!(!table_exists(&backup_connection, table));
    }

    assert_eq!(schema_version(&source), 16);
    let migrated = Connection::open(&source).expect("open migrated source");
    assert_eq!(cluster_json(&migrated), b"{\"schema_version\":1}");
    for table in JUDGE_EXECUTION_TABLES {
        assert_eq!(row_count(&migrated, table), 0);
    }
    assert_eq!(row_count(&migrated, "qualification_records"), 0);
    assert_eq!(row_count(&migrated, "qualification_v2_records"), 0);
    for table in ABSENT_AUTHORITY_TABLES {
        assert!(!table_exists(&migrated, table), "{table} must stay absent");
    }
    assert_eq!(foreign_key_violations(&migrated), 0);
}

#[test]
fn schema_eleven_migration_requires_backup_and_rolls_back() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("schema-eleven-unbacked.db");
    seed_schema_eleven(&source);
    let session =
        ArtifactStateStore::begin_existing_migration(&source).expect("begin unbacked migration");
    assert!(matches!(session.migrate(), Err(StoreError::BackupRequired)));
    assert_eq!(schema_version(&source), 11);
    let connection = Connection::open(&source).expect("reopen unmigrated source");
    assert_eq!(cluster_json(&connection), b"{\"schema_version\":1}");
    for table in JUDGE_EXECUTION_TABLES {
        assert!(!table_exists(&connection, table));
    }
}

#[test]
fn inspection_rejects_altered_schema_twelve_and_schema_eleven_shapes() {
    let directory = tempdir().expect("temporary directory");
    let current = directory.path().join("altered-schema-twelve.db");
    drop(Connection::open(&current).expect("create empty database"));
    drop(
        ArtifactStateStore::open_existing_or_initialize_empty(&current)
            .expect("create current schema"),
    );
    rewrite_table_sql(
        &current,
        "candidate_judge_plans",
        "CHECK(order_policy = 'both_orders')",
        "CHECK(order_policy = 'either_order')",
    );
    assert!(matches!(
        ArtifactStateStore::inspect_existing_schema(&current),
        Err(StoreError::CorruptRecord)
    ));
    assert_eq!(schema_version(&current), 16);

    let schema_eleven = directory.path().join("altered-schema-eleven.db");
    let connection = Connection::open(&schema_eleven).expect("create schema eleven");
    crate::schema::create_schema_eleven_fixture(&connection).expect("create schema eleven");
    drop(connection);
    rewrite_table_sql(
        &schema_eleven,
        "generation_repeatability_result_records",
        "CHECK(terminal_stage = 'candidate_generation_failed')",
        "CHECK(terminal_stage = 'passed')",
    );
    assert!(matches!(
        ArtifactStateStore::inspect_existing_schema(&schema_eleven),
        Err(StoreError::CorruptRecord)
    ));
    assert_eq!(schema_version(&schema_eleven), 11);
}

#[test]
fn schema_twelve_plan_checks_reject_open_authority_and_bad_bounds() {
    let connection = open_unchecked();
    assert!(legal_plan(&connection, 'a').is_ok());
    assert!(plan(&connection, 'b', "forward", 1, "1", 'c', 256).is_err());
    assert!(plan(&connection, 'c', "both_orders", 2, "1", 'c', 256).is_err());
    assert!(plan(&connection, 'd', "both_orders", 1, "1", 'a', 256).is_err());
    assert!(plan(&connection, 'e', "both_orders", 1, "01", 'c', 256).is_err());
    assert!(plan(&connection, 'f', "both_orders", 1, "x", 'c', 256).is_err());
    assert!(plan(&connection, 'g', "both_orders", 1, "", 'c', 256).is_err());
    assert!(plan(&connection, 'h', "both_orders", 1, "1", 'c', 255).is_err());
}

#[test]
fn schema_twelve_schedule_rejects_odd_and_oversized_entry_counts() {
    let connection = open_unchecked();
    assert!(schedule(&connection, 'a', 1, 2, "7").is_ok());
    assert!(schedule(&connection, 'b', 1, 3, "7").is_err());
    assert!(schedule(&connection, 'c', 256, 513, "7").is_err());
}

#[test]
fn schema_twelve_receipt_accepts_only_succeeded_triage_closure() {
    let connection = open_unchecked();
    assert!(receipt(&connection, 'a', "succeeded", "succeeded", "succeeded").is_ok());
    assert!(receipt(&connection, 'b', "failed", "succeeded", "succeeded").is_err());
    assert!(receipt(&connection, 'c', "succeeded", "failed", "succeeded").is_err());
    assert!(receipt(&connection, 'd', "succeeded", "succeeded", "failed").is_err());
    assert!(receipt(&connection, 'e', "succeeded", "succeeded", "succeeded_live").is_err());
}

#[test]
fn schema_twelve_join_cannot_claim_proof_or_qualification() {
    let connection = open_unchecked();
    assert!(join(&connection, 'a', "managed_local_judge_triage", 0, 0, 0).is_ok());
    assert!(join(&connection, 'b', "qualified", 0, 0, 0).is_err());
    assert!(join(&connection, 'c', "managed_local_judge_triage", 1, 0, 0).is_err());
    assert!(join(&connection, 'd', "managed_local_judge_triage", 0, 1, 0).is_err());
    assert!(join(&connection, 'e', "managed_local_judge_triage", 0, 0, 1).is_err());
}

#[test]
fn schema_twelve_repeatability_stays_candidate_generation_failure() {
    let connection = open_unchecked();
    assert!(
        result(
            &connection,
            'a',
            "candidate_generation_failed",
            None,
            None,
            None
        )
        .is_ok()
    );
    for (identity, stage) in [
        ('b', "deterministic_failed"),
        ('c', "judge_failed"),
        ('d', "passed"),
        ('e', "CandidateGenerationFailed"),
    ] {
        assert!(
            result(&connection, identity, stage, None, None, None).is_err(),
            "accepted repeatability stage {stage}"
        );
    }
    let populated = digest('9');
    assert!(
        result(
            &connection,
            'f',
            "candidate_generation_failed",
            Some(&populated),
            None,
            None
        )
        .is_err()
    );
    assert!(
        result(
            &connection,
            'g',
            "candidate_generation_failed",
            None,
            Some(&populated),
            None
        )
        .is_err()
    );
    assert!(
        result(
            &connection,
            'h',
            "candidate_generation_failed",
            None,
            None,
            Some(&populated)
        )
        .is_err()
    );
}

fn seed_schema_eleven(path: &std::path::Path) {
    let connection = Connection::open(path).expect("create schema eleven");
    crate::schema::create_schema_eleven_fixture(&connection).expect("create schema eleven");
    connection
        .execute(
            "INSERT INTO generation_cluster_records
                 (generation_cluster_id, canonical_json) VALUES (?1, ?2)",
            params![digest('a'), b"{\"schema_version\":1}"],
        )
        .expect("insert retained schema-eleven row");
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

fn rewrite_table_sql(path: &std::path::Path, table: &str, from: &str, to: &str) {
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

fn digest(character: char) -> String {
    character.to_string().repeat(64)
}
