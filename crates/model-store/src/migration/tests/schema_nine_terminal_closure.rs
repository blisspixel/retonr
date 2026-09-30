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

const CANONICAL_TABLE_BOUNDS: [(&str, usize); 7] = [
    ("candidate_generation_attempt_precursors", 16_384),
    ("managed_candidate_generation_evidence", 65_536),
    ("candidate_generation_cleanup_records", 16_384),
    ("generation_evidence_bundles", 4_194_304),
    ("generation_evidence_bundle_readbacks", 16_384),
    ("candidate_generation_receipts", 16_384),
    ("candidate_generation_attempt_records", 16_384),
];

#[test]
fn schema_ten_terminal_closure_has_exact_strict_bounded_shape() {
    let mut connection = Connection::open_in_memory().expect("open memory database");
    crate::schema::initialize_empty(&mut connection).expect("initialize schema ten");

    for table in TERMINAL_CLOSURE_TABLES {
        let sql = table_sql(&connection, table);
        assert!(sql.contains("STRICT"), "{table} must be strict");
        assert!(sql.contains("NOT GLOB '*[^0-9a-f]*'"));
    }
    for (table, maximum) in CANONICAL_TABLE_BOUNDS {
        let sql = table_sql(&connection, table);
        assert!(sql.contains(&format!(
            "canonical_json BLOB NOT NULL CHECK(length(canonical_json) BETWEEN 1 AND {maximum})"
        )));
    }

    assert_eq!(
        table_columns(&connection, "generation_evidence_bundle_storage"),
        vec![
            (
                "candidate_generation_evidence_bundle_id".to_owned(),
                "TEXT".to_owned()
            ),
            (
                "generation_qualification_plan_id".to_owned(),
                "TEXT".to_owned()
            ),
            ("planned_candidate_attempt_id".to_owned(), "TEXT".to_owned()),
            ("storage_root_id".to_owned(), "TEXT".to_owned()),
            ("relative_reference".to_owned(), "TEXT".to_owned()),
            ("maximum_tree_entries".to_owned(), "INTEGER".to_owned()),
            ("maximum_tree_depth".to_owned(), "INTEGER".to_owned()),
            ("maximum_aggregate_bytes".to_owned(), "INTEGER".to_owned()),
        ]
    );
    let storage_sql = table_sql(&connection, "generation_evidence_bundle_storage");
    assert!(storage_sql.contains("CHECK(maximum_tree_entries BETWEEN 1 AND 8193)"));
    assert!(storage_sql.contains("CHECK(maximum_tree_depth BETWEEN 1 AND 256)"));
    assert!(storage_sql.contains("CHECK(maximum_aggregate_bytes BETWEEN 1 AND 268435456)"));

    let violations: i64 = connection
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .expect("check schema-ten foreign keys");
    assert_eq!(violations, 0);
}

#[test]
fn schema_ten_enforces_precursor_terminal_and_storage_boundaries() {
    let mut connection = Connection::open_in_memory().expect("open memory database");
    crate::schema::initialize_empty(&mut connection).expect("initialize schema ten");
    connection
        .pragma_update(None, "foreign_keys", false)
        .expect("isolate local constraints");

    assert_precursor_and_terminal_boundaries(&connection);
    assert_storage_boundaries(&connection);
}

fn assert_precursor_and_terminal_boundaries(connection: &Connection) {
    let plan_id = digest('1');
    let attempt_id = digest('2');
    let precursor_id = digest('3');
    let binding_id = digest('4');
    connection
        .execute(
            "INSERT INTO candidate_generation_attempt_precursors (
                 candidate_generation_attempt_precursor_id,
                 generation_qualification_plan_id,
                 planned_candidate_attempt_id,
                 structured_request_binding_id,
                 canonical_json
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![precursor_id, plan_id, attempt_id, binding_id, b"{}"],
        )
        .expect("insert valid precursor shape");
    assert!(
        connection
            .execute(
                "INSERT INTO candidate_generation_attempt_precursors (
                     candidate_generation_attempt_precursor_id,
                     generation_qualification_plan_id,
                     planned_candidate_attempt_id,
                     structured_request_binding_id,
                     canonical_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![digest('5'), plan_id, attempt_id, binding_id, b"{}"],
            )
            .is_err()
    );
    assert!(
        connection
            .execute(
                "INSERT INTO candidate_generation_attempt_precursors (
                     candidate_generation_attempt_precursor_id,
                     generation_qualification_plan_id,
                     planned_candidate_attempt_id,
                     structured_request_binding_id,
                     canonical_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![digest('6'), digest('6'), digest('6'), digest('6'), "{}"],
            )
            .is_err()
    );
    assert!(
        connection
            .execute(
                "INSERT INTO candidate_generation_attempt_precursors (
                     candidate_generation_attempt_precursor_id,
                     generation_qualification_plan_id,
                     planned_candidate_attempt_id,
                     structured_request_binding_id,
                     canonical_json
                 ) VALUES (?1, ?2, ?3, ?4, zeroblob(16385))",
                params![digest('7'), digest('7'), digest('7'), digest('7')],
            )
            .is_err()
    );

    insert_failed_attempt(connection, '8', '9', 'a').expect("insert terminal failure");
    assert!(insert_failed_attempt(connection, 'b', '9', 'a').is_err());
    assert!(
        connection
            .execute(
                "INSERT INTO candidate_generation_attempt_records (
                     candidate_generation_attempt_record_id,
                     generation_qualification_plan_id,
                     planned_candidate_attempt_id,
                     outcome,
                     candidate_generation_attempt_precursor_id,
                     candidate_generation_receipt_id,
                     canonical_json
                 ) VALUES (?1, ?2, ?3, 'completed', NULL, NULL, ?4)",
                params![digest('c'), digest('c'), digest('c'), b"{}"],
            )
            .is_err()
    );
    assert!(
        connection
            .execute(
                "INSERT INTO candidate_generation_attempt_records (
                     candidate_generation_attempt_record_id,
                     generation_qualification_plan_id,
                     planned_candidate_attempt_id,
                     outcome,
                     candidate_generation_attempt_precursor_id,
                     candidate_generation_receipt_id,
                     canonical_json
                 ) VALUES (?1, ?2, ?3, 'failed', NULL, ?4, ?5)",
                params![digest('d'), digest('d'), digest('d'), digest('d'), b"{}"],
            )
            .is_err()
    );
}

fn assert_storage_boundaries(connection: &Connection) {
    let valid_bundle_id = digest('e');
    insert_storage(
        connection,
        &valid_bundle_id,
        &canonical_storage_reference(&valid_bundle_id),
        8_193,
        256,
        268_435_456,
    )
    .expect("insert valid storage reference");
    for invalid in [
        "",
        "/absolute",
        "trailing/",
        "a//b",
        "a/../b",
        "a\\b",
        "a:b",
    ] {
        assert!(
            insert_storage(connection, &digest('f'), invalid, 8_193, 256, 268_435_456).is_err(),
            "accepted invalid relative reference {invalid:?}"
        );
    }
    for (entries, depth, bytes) in [
        (0, 256, 268_435_456),
        (8_194, 256, 268_435_456),
        (8_193, 0, 268_435_456),
        (8_193, 257, 268_435_456),
        (8_193, 256, 0),
        (8_193, 256, 268_435_457),
    ] {
        let bundle_id = digest('f');
        assert!(
            insert_storage(
                connection,
                &bundle_id,
                &canonical_storage_reference(&bundle_id),
                entries,
                depth,
                bytes
            )
            .is_err()
        );
    }
    let root_test_bundle_id = digest('f');
    assert!(
        connection
            .execute(
                "INSERT INTO generation_evidence_bundle_storage (
                     candidate_generation_evidence_bundle_id,
                     generation_qualification_plan_id,
                     planned_candidate_attempt_id,
                     storage_root_id,
                     relative_reference,
                     maximum_tree_entries,
                     maximum_tree_depth,
                     maximum_aggregate_bytes
                 ) VALUES (?1, ?2, ?3, 'ABC', ?4, 1, 1, 1)",
                params![
                    root_test_bundle_id,
                    digest('1'),
                    digest('2'),
                    canonical_storage_reference(&root_test_bundle_id)
                ],
            )
            .is_err()
    );
}

#[test]
fn populated_schema_nine_migrates_after_verified_byte_preserving_backup() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("schema-nine.db");
    let backup = directory.path().join("schema-nine-backup.db");
    let connection = Connection::open(&source).expect("create schema-nine fixture");
    crate::schema::create_schema_nine_fixture(&connection).expect("create schema nine");
    connection
        .execute(
            "INSERT INTO generation_cluster_records
                 (generation_cluster_id, canonical_json) VALUES (?1, ?2)",
            params![digest('a'), b"{\"schema_version\":1}"],
        )
        .expect("insert retained schema-nine row");
    drop(connection);
    let mut backup_file = reserve_file(&backup);
    let mut session =
        ArtifactStateStore::begin_existing_migration(&source).expect("begin schema-nine migration");
    assert_eq!(
        (
            session.schema_status().found,
            session.schema_status().current
        ),
        (9, 17)
    );
    session
        .backup_to(&mut backup_file, 16 * 1024 * 1024, || false)
        .expect("write verified backup");
    let result = session.migrate().expect("migrate schema nine");
    assert_eq!(result.disposition, StoreMigrationDisposition::Migrated);

    assert_eq!(schema_version(&backup), 9);
    let backup_connection = Connection::open(&backup).expect("open backup");
    let backed_up: Vec<u8> = backup_connection
        .query_row(
            "SELECT canonical_json FROM generation_cluster_records
             WHERE generation_cluster_id = ?1",
            [digest('a')],
            |row| row.get(0),
        )
        .expect("read byte-preserved backup row");
    assert_eq!(backed_up, b"{\"schema_version\":1}");
    for table in TERMINAL_CLOSURE_TABLES {
        assert!(!table_exists(&backup_connection, table));
    }

    assert_eq!(schema_version(&source), 17);
    let migrated = Connection::open(&source).expect("open migrated source");
    let retained: Vec<u8> = migrated
        .query_row(
            "SELECT canonical_json FROM generation_cluster_records
             WHERE generation_cluster_id = ?1",
            [digest('a')],
            |row| row.get(0),
        )
        .expect("read retained schema-nine row");
    assert_eq!(retained, b"{\"schema_version\":1}");
    for table in TERMINAL_CLOSURE_TABLES {
        let count: i64 = migrated
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .expect("count new terminal-closure table");
        assert_eq!(count, 0);
    }
}

#[test]
fn inspection_rejects_schema_ten_with_altered_terminal_shape() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("altered-schema-ten.db");
    drop(ArtifactStateStore::open(&path).expect("create schema ten"));
    let connection = Connection::open(&path).expect("reopen schema ten");
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
        .expect("alter terminal table shape");
    assert_eq!(changed, 1);
    connection
        .execute_batch("PRAGMA writable_schema = OFF;")
        .expect("disable shape mutation");
    drop(connection);

    assert!(matches!(
        ArtifactStateStore::inspect_existing_schema(&path),
        Err(StoreError::CorruptRecord)
    ));
}

fn table_sql(connection: &Connection, table: &str) -> String {
    connection
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = ?1",
            [table],
            |row| row.get(0),
        )
        .expect("read terminal table SQL")
}

fn table_columns(connection: &Connection, table: &str) -> Vec<(String, String)> {
    let mut statement = connection
        .prepare(&format!(
            "SELECT name, type FROM pragma_table_info('{table}') ORDER BY cid"
        ))
        .expect("prepare table column query");
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("query table columns")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect table columns")
}

fn insert_failed_attempt(
    connection: &Connection,
    record: char,
    plan: char,
    attempt: char,
) -> rusqlite::Result<usize> {
    connection.execute(
        "INSERT INTO candidate_generation_attempt_records (
             candidate_generation_attempt_record_id,
             generation_qualification_plan_id,
             planned_candidate_attempt_id,
             outcome,
             candidate_generation_attempt_precursor_id,
             candidate_generation_receipt_id,
             canonical_json
         ) VALUES (?1, ?2, ?3, 'failed', NULL, NULL, ?4)",
        params![digest(record), digest(plan), digest(attempt), b"{}"],
    )
}

fn insert_storage(
    connection: &Connection,
    bundle_id: &str,
    reference: &str,
    maximum_tree_entries: i64,
    maximum_tree_depth: i64,
    maximum_aggregate_bytes: i64,
) -> rusqlite::Result<usize> {
    connection.execute(
        "INSERT INTO generation_evidence_bundle_storage (
             candidate_generation_evidence_bundle_id,
             generation_qualification_plan_id,
             planned_candidate_attempt_id,
             storage_root_id,
             relative_reference,
             maximum_tree_entries,
             maximum_tree_depth,
             maximum_aggregate_bytes
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            bundle_id,
            digest('1'),
            digest('2'),
            digest('0'),
            reference,
            maximum_tree_entries,
            maximum_tree_depth,
            maximum_aggregate_bytes
        ],
    )
}

fn canonical_storage_reference(bundle_id: &str) -> String {
    format!("bundles/v1/{}/{}/{}", digest('1'), digest('2'), bundle_id)
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
