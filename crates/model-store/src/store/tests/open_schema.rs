use rusqlite::Connection;
use tempfile::tempdir;

use super::ArtifactStateStore;
use crate::StoreError;

#[test]
fn compatibility_open_refuses_corrupt_older_schema_without_inspecting_its_shape() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("corrupt-schema-two.db");
    let connection = Connection::open(&path).expect("create schema-two fixture");
    crate::schema::create_schema_two_fixture(&connection).expect("create schema two");
    connection
        .execute_batch(
            "CREATE TRIGGER unexpected_v2_trigger
             AFTER INSERT ON artifact_manifests
             BEGIN
                 DELETE FROM artifact_manifests WHERE artifact_id = NEW.artifact_id;
             END;",
        )
        .expect("corrupt schema two");
    drop(connection);

    assert!(matches!(
        ArtifactStateStore::open_existing_and_migrate(&path),
        Err(StoreError::MigrationRequired {
            found: 2,
            current: 6
        })
    ));
    let connection = Connection::open(&path).expect("reopen rejected schema");
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read unchanged version");
    assert_eq!(version, 2);
    let v3_table_exists: bool = connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM sqlite_schema WHERE name = 'artifact_set_manifests'
             )",
            [],
            |row| row.get(0),
        )
        .expect("inspect rejected migration");
    assert!(!v3_table_exists);
}

#[test]
fn current_version_with_missing_or_altered_schema_is_corrupt() {
    let directory = tempdir().expect("temporary directory");
    let missing = directory.path().join("missing-schema.db");
    let connection = Connection::open(&missing).expect("create database");
    connection
        .pragma_update(None, "user_version", 6)
        .expect("set current version");
    drop(connection);
    assert!(matches!(
        ArtifactStateStore::open_existing_read_only(&missing),
        Err(StoreError::CorruptRecord)
    ));

    let altered = directory.path().join("altered-schema.db");
    drop(ArtifactStateStore::open(&altered).expect("create current state"));
    let connection = Connection::open(&altered).expect("reopen state");
    connection
        .execute_batch("DROP INDEX invalidations_by_qualification;")
        .expect("remove required index");
    drop(connection);
    assert!(matches!(
        ArtifactStateStore::open_existing_writable_exact(&altered),
        Err(StoreError::CorruptRecord)
    ));

    let wrong_index = directory.path().join("wrong-index.db");
    drop(ArtifactStateStore::open(&wrong_index).expect("create current state"));
    let connection = Connection::open(&wrong_index).expect("reopen state");
    connection
        .execute_batch(
            "DROP INDEX invalidations_by_qualification;
             CREATE INDEX invalidations_by_qualification
                 ON qualification_invalidations(sequence, qualification_id);",
        )
        .expect("replace index with wrong columns");
    drop(connection);
    assert!(matches!(
        ArtifactStateStore::open_existing_read_only(&wrong_index),
        Err(StoreError::CorruptRecord)
    ));

    let lax = directory.path().join("lax-current.db");
    let connection = Connection::open(&lax).expect("create lax state");
    connection
        .execute_batch(
            "CREATE TABLE artifact_manifests (
                 artifact_id TEXT,
                 record_json TEXT
             );
             PRAGMA user_version = 6;",
        )
        .expect("create lax current-version state");
    drop(connection);
    assert!(matches!(
        ArtifactStateStore::open_existing_read_only(&lax),
        Err(StoreError::CorruptRecord)
    ));
}

#[test]
fn current_schema_rejects_spoofed_constraints_and_unexpected_objects() {
    let directory = tempdir().expect("temporary directory");
    let spoofed_check = directory.path().join("spoofed-check.db");
    drop(ArtifactStateStore::open(&spoofed_check).expect("create current state"));
    rewrite_schema_sql(
        &spoofed_check,
        "artifact_manifests",
        "CHECK(length(artifact_id) = 64)",
        "CHECK(1 OR length(artifact_id) = 64)",
    );
    assert_corrupt(&spoofed_check);

    let changed_foreign_key = directory.path().join("changed-foreign-key.db");
    drop(ArtifactStateStore::open(&changed_foreign_key).expect("create current state"));
    rewrite_schema_sql(
        &changed_foreign_key,
        "installed_artifacts",
        "REFERENCES artifact_manifests(artifact_id)",
        "REFERENCES artifact_manifests(artifact_id) ON DELETE CASCADE",
    );
    assert_corrupt(&changed_foreign_key);

    let changed_v3_foreign_key = directory.path().join("changed-v3-foreign-key.db");
    drop(ArtifactStateStore::open(&changed_v3_foreign_key).expect("create current state"));
    rewrite_schema_sql(
        &changed_v3_foreign_key,
        "effective_runtime_states",
        "REFERENCES runtime_build_identities(runtime_build_id)",
        "REFERENCES runtime_build_identities(runtime_build_id) ON DELETE CASCADE",
    );
    assert_corrupt(&changed_v3_foreign_key);

    let changed_literal = directory.path().join("changed-literal.db");
    drop(ArtifactStateStore::open(&changed_literal).expect("create current state"));
    rewrite_schema_sql(
        &changed_literal,
        "artifact_removals",
        "'prepared', 'completed'",
        "'PREPARED', 'completed'",
    );
    assert_corrupt(&changed_literal);

    let merged_tokens = directory.path().join("merged-tokens.db");
    drop(ArtifactStateStore::open(&merged_tokens).expect("create current state"));
    rewrite_schema_sql(
        &merged_tokens,
        "artifact_manifests",
        "TEXT PRIMARY KEY",
        "TEXTPRIMARY KEY",
    );
    assert_schema_rejected(&merged_tokens);

    let non_ascii_separator = directory.path().join("non-ascii-separator.db");
    drop(ArtifactStateStore::open(&non_ascii_separator).expect("create current state"));
    rewrite_schema_sql(
        &non_ascii_separator,
        "artifact_manifests",
        "TEXT PRIMARY KEY",
        "TEXT\u{00a0}PRIMARY KEY",
    );
    assert_schema_rejected(&non_ascii_separator);

    let trigger = directory.path().join("unexpected-trigger.db");
    drop(ArtifactStateStore::open(&trigger).expect("create current state"));
    let connection = Connection::open(&trigger).expect("reopen state");
    connection
        .execute_batch(
            "CREATE TRIGGER erase_installation
             AFTER INSERT ON installed_artifacts
             BEGIN
                 DELETE FROM installed_artifacts WHERE artifact_id = NEW.artifact_id;
             END;",
        )
        .expect("add unexpected trigger");
    drop(connection);
    assert_corrupt(&trigger);
}

fn rewrite_schema_sql(path: &std::path::Path, object: &str, from: &str, to: &str) {
    let connection = Connection::open(path).expect("reopen state");
    connection
        .execute_batch("PRAGMA writable_schema = ON;")
        .expect("enable schema fixture mutation");
    let changed = connection
        .execute(
            "UPDATE sqlite_schema SET sql = replace(sql, ?1, ?2) WHERE name = ?3",
            (from, to, object),
        )
        .expect("mutate schema fixture");
    assert_eq!(changed, 1);
    connection
        .execute_batch("PRAGMA writable_schema = OFF;")
        .expect("disable schema fixture mutation");
}

fn assert_corrupt(path: &std::path::Path) {
    assert!(matches!(
        ArtifactStateStore::open_existing_read_only(path),
        Err(StoreError::CorruptRecord)
    ));
}

fn assert_schema_rejected(path: &std::path::Path) {
    assert!(ArtifactStateStore::open_existing_read_only(path).is_err());
}
