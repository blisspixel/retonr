use std::collections::BTreeMap;

use rusqlite::{Connection, params};
use tempfile::tempdir;

use super::{reserve_file, schema_five_packages, schema_version};
use crate::{ArtifactStateStore, StoreError, StoreMigrationDisposition};

const PACKAGE_TABLES: [&str; 3] = [
    "model_package_manifests",
    "native_load_observations",
    "runtime_package_manifests",
];
const PREREGISTRATION_TABLES: [&str; 2] = [
    "generation_qualification_operation_policies",
    "generation_qualification_request_projections",
];
const FOUNDATION_TABLES: [&str; 2] = ["effective_package_evidence_v2", "generation_system_records"];

#[test]
fn schema_six_migration_preserves_every_legacy_value_byte_for_byte() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("schema-six.db");
    let backup = directory.path().join("schema-six-backup.db");
    let connection = Connection::open(&source).expect("create schema six");
    crate::schema::create_schema_six_fixture(&connection).expect("create schema six");
    schema_five_packages::seed_every_legacy_table(&connection);
    seed_package_tables(&connection);
    let before = all_schema_six_rows(&connection);
    drop(connection);

    let mut backup_file = reserve_file(&backup);
    let mut session =
        ArtifactStateStore::begin_existing_migration(&source).expect("begin schema-six migration");
    assert_eq!(
        (
            session.schema_status().found,
            session.schema_status().current
        ),
        (6, 10)
    );
    session
        .backup_to(&mut backup_file, 16 * 1024 * 1024, || false)
        .expect("write verified backup");
    let result = session.migrate().expect("migrate schema six");
    assert_eq!(result.disposition, StoreMigrationDisposition::Migrated);

    let backup_connection = Connection::open(&backup).expect("open backup");
    assert_eq!(schema_version(&backup), 6);
    assert_eq!(all_schema_six_rows(&backup_connection), before);
    for table in PREREGISTRATION_TABLES {
        assert!(!schema_five_packages::table_exists(
            &backup_connection,
            table
        ));
    }

    let migrated = Connection::open(&source).expect("open migrated source");
    assert_eq!(schema_version(&source), 10);
    assert_eq!(all_schema_six_rows(&migrated), before);
    for table in PREREGISTRATION_TABLES {
        let count: i64 = migrated
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .expect("count preregistration table");
        assert_eq!(count, 0);
    }
    for table in FOUNDATION_TABLES {
        let count: i64 = migrated
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .expect("count foundation table");
        assert_eq!(count, 0);
    }
    let legacy_binding: Vec<u8> = migrated
        .query_row(
            "SELECT CAST(record_json AS BLOB) FROM active_bindings WHERE role = 'generation'",
            [],
            |row| row.get(0),
        )
        .expect("read legacy generation binding");
    assert_eq!(legacy_binding, br#"{ "legacy": 6 }"#);
}

#[test]
fn schema_six_requires_backup_and_exact_shape() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("backup-required.db");
    let connection = Connection::open(&source).expect("create schema six");
    crate::schema::create_schema_six_fixture(&connection).expect("create schema six");
    schema_five_packages::seed_every_legacy_table(&connection);
    seed_package_tables(&connection);
    let before = all_schema_six_rows(&connection);
    drop(connection);
    let session =
        ArtifactStateStore::begin_existing_migration(&source).expect("begin schema-six migration");
    assert!(matches!(session.migrate(), Err(StoreError::BackupRequired)));
    let unchanged = Connection::open(&source).expect("reopen schema six");
    assert_eq!(schema_version(&source), 6);
    assert_eq!(all_schema_six_rows(&unchanged), before);
    drop(unchanged);

    let connection = Connection::open(&source).expect("corrupt schema six");
    connection
        .execute_batch("CREATE TABLE unexpected_schema_six_state(value BLOB) STRICT;")
        .expect("alter exact shape");
    drop(connection);
    assert!(matches!(
        ArtifactStateStore::begin_existing_migration(&source),
        Err(StoreError::CorruptRecord)
    ));
    assert_eq!(schema_version(&source), 6);
}

#[test]
fn current_schema_enforces_strict_preregistration_identity_bounds_and_pair_relationship() {
    let connection = Connection::open_in_memory().expect("open memory database");
    let mut connection = connection;
    crate::schema::initialize_empty(&mut connection).expect("initialize current schema");
    for table in PREREGISTRATION_TABLES {
        let sql: String = connection
            .query_row(
                "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = ?1",
                [table],
                |row| row.get(0),
            )
            .expect("read exact table SQL");
        assert!(sql.contains("STRICT"));
        assert!(sql.contains("canonical_json BLOB NOT NULL"));
        assert!(sql.contains("NOT GLOB '*[^0-9a-f]*'"));
        if table.ends_with("request_projections") {
            assert!(sql.contains("UNIQUE(operation_policy_id)"));
            assert!(sql.contains("ON UPDATE RESTRICT ON DELETE RESTRICT"));
        }
    }
    assert_policy_constraints(&connection);
    assert_projection_constraints(&connection);
}

fn assert_policy_constraints(connection: &Connection) {
    let id = "a".repeat(64);
    let plan = "b".repeat(64);
    let suite = "c".repeat(64);
    let target = "d".repeat(64);
    let baseline = "e".repeat(64);
    for values in [
        [
            "A".repeat(64),
            plan.clone(),
            suite.clone(),
            target.clone(),
            baseline.clone(),
        ],
        [
            id.clone(),
            plan.clone(),
            suite.clone(),
            target.clone(),
            target.clone(),
        ],
    ] {
        assert!(
            connection
                .execute(
                    "INSERT INTO generation_qualification_operation_policies VALUES
                     (?1, ?2, ?3, ?4, ?5, X'7B7D')",
                    values,
                )
                .is_err()
        );
    }
    assert!(
        connection
            .execute(
                "INSERT INTO generation_qualification_operation_policies VALUES
                 (?1, ?2, ?3, ?4, ?5, ?6)",
                params![&id, &plan, &suite, &target, &baseline, "{}"],
            )
            .is_err()
    );
    for blob in [Vec::new(), vec![b'x'; 16_385]] {
        assert!(
            connection
                .execute(
                    "INSERT INTO generation_qualification_operation_policies VALUES
                     (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![&id, &plan, &suite, &target, &baseline, blob],
                )
                .is_err()
        );
    }
    connection
        .execute(
            "INSERT INTO generation_qualification_operation_policies VALUES
                 (?1, ?2, ?3, ?4, ?5, X'7B7D')",
            [&id, &plan, &suite, &target, &baseline],
        )
        .expect("insert bounded policy fixture");
}

#[expect(
    clippy::too_many_lines,
    reason = "one schema test pins the complete projection table constraint matrix"
)]
fn assert_projection_constraints(connection: &Connection) {
    let policy = "a".repeat(64);
    let plan = "b".repeat(64);
    let suite = "c".repeat(64);
    let target = "d".repeat(64);
    let baseline = "e".repeat(64);
    assert!(
        connection
            .execute(
                "INSERT INTO generation_qualification_request_projections VALUES
                 (?1, ?2, ?3, ?4, ?5, ?6, 1, X'7B7D')",
                [
                    "f".repeat(64),
                    "9".repeat(64),
                    plan.clone(),
                    suite.clone(),
                    target.clone(),
                    baseline.clone()
                ],
            )
            .is_err()
    );
    for count in [0, 1_025] {
        assert!(
            connection
                .execute(
                    "INSERT INTO generation_qualification_request_projections VALUES
                     (?1, ?2, ?3, ?4, ?5, ?6, ?7, X'7B7D')",
                    params![
                        "f".repeat(64),
                        &policy,
                        &plan,
                        &suite,
                        &target,
                        &baseline,
                        count
                    ],
                )
                .is_err()
        );
    }
    assert!(
        connection
            .execute(
                "INSERT INTO generation_qualification_request_projections VALUES
                 (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7)",
                params![
                    "f".repeat(64),
                    &policy,
                    &plan,
                    &suite,
                    &target,
                    &baseline,
                    "{}"
                ],
            )
            .is_err()
    );
    assert!(
        connection
            .execute(
                "INSERT INTO generation_qualification_request_projections VALUES
                 (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7)",
                params![
                    "f".repeat(64),
                    &policy,
                    &plan,
                    &suite,
                    &target,
                    &baseline,
                    vec![b'x'; 4_194_305]
                ],
            )
            .is_err()
    );
    connection
        .execute(
            "INSERT INTO generation_qualification_request_projections VALUES
                 (?1, ?2, ?3, ?4, ?5, ?6, 1, X'7B7D')",
            params!["f".repeat(64), &policy, &plan, &suite, &target, &baseline],
        )
        .expect("insert bounded projection fixture");
    assert!(
        connection
            .execute(
                "INSERT INTO generation_qualification_request_projections VALUES
                 (?1, ?2, ?3, ?4, ?5, ?6, 1, X'7B7D')",
                params!["1".repeat(64), &policy, &plan, &suite, &target, &baseline],
            )
            .is_err()
    );
    assert!(
        connection
            .execute(
                "DELETE FROM generation_qualification_operation_policies
             WHERE operation_policy_id = ?1",
                [&policy],
            )
            .is_err()
    );
    assert!(
        connection
            .execute(
                "UPDATE generation_qualification_operation_policies
             SET operation_policy_id = ?1 WHERE operation_policy_id = ?2",
                ["2".repeat(64), policy],
            )
            .is_err()
    );
}

fn seed_package_tables(connection: &Connection) {
    let artifact_set = "d".repeat(64);
    let runtime = "3".repeat(64);
    let model = "4".repeat(64);
    let observation = "5".repeat(64);
    connection
        .execute_batch(&format!(
            "INSERT INTO runtime_package_manifests VALUES
                 ('{runtime}', '{artifact_set}', NULL, '{{ \"legacy\": 15 }}');
             INSERT INTO model_package_manifests VALUES
                 ('{model}', '{artifact_set}', NULL, '{{ \"legacy\": 16 }}');
             INSERT INTO native_load_observations VALUES
                 ('{observation}', '{runtime}', '{{ \"legacy\": 17 }}');"
        ))
        .expect("seed schema-six package tables");
}

fn all_schema_six_rows(connection: &Connection) -> BTreeMap<String, Vec<Vec<Vec<u8>>>> {
    schema_five_packages::LEGACY_TABLES
        .into_iter()
        .chain(PACKAGE_TABLES)
        .map(|table| {
            (
                table.to_owned(),
                schema_five_packages::table_rows(connection, table),
            )
        })
        .collect()
}
