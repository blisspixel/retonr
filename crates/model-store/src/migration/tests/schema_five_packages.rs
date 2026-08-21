use std::collections::BTreeMap;

use rusqlite::{Connection, types::ValueRef};
use tempfile::tempdir;

use super::{reserve_file, schema_version};
use crate::{ArtifactStateStore, StoreMigrationDisposition};

const LEGACY_TABLES: [&str; 14] = [
    "activation_decisions",
    "active_bindings",
    "artifact_manifests",
    "artifact_removals",
    "artifact_set_manifests",
    "artifact_set_removals",
    "effective_package_evidence",
    "effective_runtime_states",
    "installed_artifact_sets",
    "installed_artifacts",
    "qualification_invalidations",
    "qualification_records",
    "qualification_v2_records",
    "runtime_build_identities",
];

const NEW_TABLES: [&str; 3] = [
    "model_package_manifests",
    "native_load_observations",
    "runtime_package_manifests",
];

#[test]
fn verified_wal_backup_and_schema_five_migration_preserve_all_legacy_rows() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("schema-five.db");
    let backup = directory.path().join("schema-five-backup.db");
    let setup = Connection::open(&source).expect("create schema-five fixture");
    crate::schema::create_schema_five_fixture(&setup).expect("create schema five");
    drop(setup);

    let writer = Connection::open(&source).expect("open WAL writer");
    writer
        .pragma_update(None, "journal_mode", "WAL")
        .expect("enable WAL");
    writer
        .pragma_update(None, "wal_autocheckpoint", 0)
        .expect("disable automatic checkpoint");
    seed_every_legacy_table(&writer);
    let before = all_legacy_rows(&writer);
    assert!(source.with_extension("db-wal").exists());

    let mut backup_file = reserve_file(&backup);
    let mut session = ArtifactStateStore::begin_existing_migration(&source)
        .expect("begin schema-five migration over WAL");
    assert_eq!(
        (
            session.schema_status().found,
            session.schema_status().current
        ),
        (5, 6)
    );
    session
        .backup_to(&mut backup_file, 16 * 1024 * 1024, || false)
        .expect("write verified backup");
    let result = session.migrate().expect("migrate schema five");
    assert_eq!(result.disposition, StoreMigrationDisposition::Migrated);
    drop(writer);

    let backup_connection = Connection::open(&backup).expect("open retained backup");
    assert_eq!(schema_version(&backup), 5);
    assert_eq!(all_legacy_rows(&backup_connection), before);
    for table in NEW_TABLES {
        assert!(!table_exists(&backup_connection, table));
    }

    let migrated = Connection::open(&source).expect("open migrated source");
    assert_eq!(schema_version(&source), 6);
    assert_eq!(all_legacy_rows(&migrated), before);
    for table in NEW_TABLES {
        let count: i64 = migrated
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .expect("count new table");
        assert_eq!(count, 0, "{table} must start empty");
    }
}

#[test]
fn schema_five_migration_requires_a_completed_verified_backup() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("backup-required.db");
    let connection = Connection::open(&source).expect("create schema-five fixture");
    crate::schema::create_schema_five_fixture(&connection).expect("create schema five");
    seed_every_legacy_table(&connection);
    let before = all_legacy_rows(&connection);
    drop(connection);

    let session =
        ArtifactStateStore::begin_existing_migration(&source).expect("begin schema-five migration");
    assert!(matches!(
        session.migrate(),
        Err(crate::StoreError::BackupRequired)
    ));
    let unchanged = Connection::open(&source).expect("reopen unchanged schema five");
    assert_eq!(schema_version(&source), 5);
    assert_eq!(all_legacy_rows(&unchanged), before);
    for table in NEW_TABLES {
        assert!(!table_exists(&unchanged, table));
    }
}

#[test]
fn compatibility_opens_require_explicit_schema_five_migration_without_mutation() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("explicit-schema-five.db");
    let connection = Connection::open(&source).expect("create schema-five fixture");
    crate::schema::create_schema_five_fixture(&connection).expect("create schema five");
    seed_every_legacy_table(&connection);
    let before = all_legacy_rows(&connection);
    drop(connection);

    for result in [
        ArtifactStateStore::open(&source),
        ArtifactStateStore::open_or_create_and_migrate(&source),
        ArtifactStateStore::open_existing_and_migrate(&source),
    ] {
        assert!(matches!(
            result,
            Err(crate::StoreError::MigrationRequired {
                found: 5,
                current: 6
            })
        ));
        let unchanged = Connection::open(&source).expect("reopen unchanged schema five");
        assert_eq!(schema_version(&source), 5);
        assert_eq!(all_legacy_rows(&unchanged), before);
        for table in NEW_TABLES {
            assert!(!table_exists(&unchanged, table));
        }
    }
}

#[test]
fn fresh_schema_six_has_the_exact_new_strict_tables_and_hex_checks() {
    let connection = Connection::open_in_memory().expect("open memory database");
    let mut connection = connection;
    crate::schema::initialize_empty(&mut connection).expect("initialize exact schema");
    crate::schema::validate_schema_shape(&connection).expect("validate exact schema shape");
    let names = NEW_TABLES
        .iter()
        .map(|name| {
            let sql: String = connection
                .query_row(
                    "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = ?1",
                    [name],
                    |row| row.get(0),
                )
                .expect("load table SQL");
            assert!(sql.contains("STRICT"));
            assert!(sql.contains("NOT GLOB '*[^0-9a-f]*'"));
            if name.ends_with("package_manifests") {
                assert!(sql.contains("source_artifact_set_id TEXT"));
                assert!(sql.contains("FOREIGN KEY(source_artifact_set_id)"));
            }
            *name
        })
        .collect::<Vec<_>>();
    assert_eq!(names, NEW_TABLES);
}

fn seed_every_legacy_table(connection: &Connection) {
    let artifact = "a".repeat(64);
    let qualification = "b".repeat(64);
    let activation = "c".repeat(64);
    let artifact_set = "d".repeat(64);
    let build = "e".repeat(64);
    let state = "f".repeat(64);
    let package = "1".repeat(64);
    let qualification_v2 = "2".repeat(64);
    connection
        .execute_batch(&format!(
            "INSERT INTO artifact_manifests VALUES ('{artifact}', '{{ \"legacy\": 1 }}');
             INSERT INTO installed_artifacts VALUES
                 ('{artifact}', 9, '{{ \"legacy\": 2 }}');
             INSERT INTO qualification_records VALUES
                 ('{qualification}', '{artifact}', '{{ \"legacy\": 3 }}');
             INSERT INTO qualification_invalidations VALUES
                 (7, '{qualification}', 'legacy-reason', '{{ \"legacy\": 4 }}');
             INSERT INTO activation_decisions VALUES
                 ('{activation}', 'generation', '{{ \"legacy\": 5 }}');
             INSERT INTO active_bindings VALUES
                 ('generation', '{artifact}', '{qualification}', '{{ \"legacy\": 6 }}');
             INSERT INTO artifact_removals VALUES
                 ('{artifact}', 9, 'prepared', '{{ \"legacy\": 7 }}');
             INSERT INTO artifact_set_manifests VALUES
                 ('{artifact_set}', '{{ \"legacy\": 8 }}');
             INSERT INTO runtime_build_identities VALUES
                 ('{build}', '{{ \"legacy\": 9 }}');
             INSERT INTO effective_runtime_states VALUES
                 ('{state}', '{build}', '{{ \"legacy\": 10 }}');
             INSERT INTO effective_package_evidence VALUES
                 ('{package}', '{artifact_set}', '{build}', '{state}',
                  '{{ \"legacy\": 11 }}');
             INSERT INTO qualification_v2_records VALUES
                 ('{qualification_v2}', '{artifact_set}', '{package}', '{build}', '{state}',
                  '{{ \"legacy\": 12 }}');
             INSERT INTO installed_artifact_sets VALUES
                 ('{artifact_set}', 'legacy/set', 4, '{{ \"legacy\": 13 }}');
             INSERT INTO artifact_set_removals VALUES
                 ('{artifact_set}', 4, 'completed', '{{ \"legacy\": 14 }}');"
        ))
        .expect("seed every legacy table");
}

fn all_legacy_rows(connection: &Connection) -> BTreeMap<String, Vec<Vec<Vec<u8>>>> {
    LEGACY_TABLES
        .into_iter()
        .map(|table| (table.to_owned(), table_rows(connection, table)))
        .collect()
}

fn table_rows(connection: &Connection, table: &str) -> Vec<Vec<Vec<u8>>> {
    let mut statement = connection
        .prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))
        .expect("prepare legacy snapshot");
    let column_count = statement.column_count();
    let rows = statement
        .query_map([], |row| {
            (0..column_count)
                .map(|index| row.get_ref(index).map(encode_value))
                .collect::<Result<Vec<_>, _>>()
        })
        .expect("query legacy snapshot");
    rows.map(|row| row.expect("read legacy snapshot")).collect()
}

fn encode_value(value: ValueRef<'_>) -> Vec<u8> {
    let mut encoded = Vec::new();
    match value {
        ValueRef::Null => encoded.push(0),
        ValueRef::Integer(value) => {
            encoded.push(1);
            encoded.extend_from_slice(&value.to_be_bytes());
        }
        ValueRef::Real(value) => {
            encoded.push(2);
            encoded.extend_from_slice(&value.to_bits().to_be_bytes());
        }
        ValueRef::Text(value) => {
            encoded.push(3);
            encoded.extend_from_slice(value);
        }
        ValueRef::Blob(value) => {
            encoded.push(4);
            encoded.extend_from_slice(value);
        }
    }
    encoded
}

fn table_exists(connection: &Connection, name: &str) -> bool {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = ?1)",
            [name],
            |row| row.get(0),
        )
        .expect("inspect table")
}
