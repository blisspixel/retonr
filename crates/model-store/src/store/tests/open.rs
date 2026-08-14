use std::fs;

use rusqlite::Connection;
use tempfile::tempdir;

use super::{ArtifactStateStore, fixture};
use crate::StoreError;

#[test]
fn newer_schema_is_rejected_without_migration() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("future.db");
    let connection = Connection::open(&path).expect("create database");
    connection
        .pragma_update(None, "user_version", 4)
        .expect("set future version");
    drop(connection);
    assert!(matches!(
        ArtifactStateStore::open(&path),
        Err(StoreError::UnsupportedSchema(4))
    ));
}

#[test]
fn explicit_existing_opens_never_create_missing_state() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("missing.db");
    let before = directory_entries(directory.path());

    assert!(matches!(
        ArtifactStateStore::open_existing_read_only(&path),
        Err(StoreError::NotInitialized)
    ));
    assert!(!path.exists());
    assert_eq!(directory_entries(directory.path()), before);
    assert!(matches!(
        ArtifactStateStore::open_existing_and_migrate(&path),
        Err(StoreError::NotInitialized)
    ));
    assert!(!path.exists());
    assert_eq!(directory_entries(directory.path()), before);
}

#[cfg(unix)]
#[test]
fn ancestor_symlink_is_resolved_but_final_symlink_is_rejected() {
    use std::os::unix::fs::symlink;

    let directory = tempdir().expect("temporary directory");
    let target = directory.path().join("target");
    let alias = directory.path().join("alias");
    fs::create_dir(&target).expect("create target directory");
    symlink(&target, &alias).expect("create ancestor symlink");

    let aliased_state = alias.join("state.db");
    drop(
        ArtifactStateStore::open_or_create_and_migrate(&aliased_state)
            .expect("create through ancestor alias"),
    );
    ArtifactStateStore::open_existing_read_only(&aliased_state)
        .expect("read through ancestor alias");
    ArtifactStateStore::open_existing_writable_exact(&aliased_state)
        .expect("write through ancestor alias");

    let final_alias = alias.join("final.db");
    symlink(target.join("state.db"), &final_alias).expect("create final symlink");
    assert!(matches!(
        ArtifactStateStore::open_existing_read_only(&final_alias),
        Err(StoreError::Database(_))
    ));
}

#[test]
fn read_only_open_rejects_legacy_schema_without_mutation() {
    assert_read_only_schema_rejection(
        1,
        |error| {
            matches!(
                error,
                StoreError::MigrationRequired {
                    found: 1,
                    current: 3
                }
            )
        },
        "legacy.db",
    );
    assert_read_only_schema_rejection(
        2,
        |error| {
            matches!(
                error,
                StoreError::MigrationRequired {
                    found: 2,
                    current: 3
                }
            )
        },
        "schema-two-read-only.db",
    );
}

#[test]
fn read_only_open_rejects_future_schema_without_mutation() {
    assert_read_only_schema_rejection(
        4,
        |error| matches!(error, StoreError::UnsupportedSchema(4)),
        "future-read-only.db",
    );
}

#[test]
fn exact_schema_read_only_open_cannot_write() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("current.db");
    drop(
        ArtifactStateStore::open_or_create_and_migrate(&path).expect("create current-schema state"),
    );
    let before = fs::read(&path).expect("read state before read-only open");
    let store =
        ArtifactStateStore::open_existing_read_only(&path).expect("open current schema read-only");
    assert!(matches!(
        store.put_manifest(&fixture().manifest),
        Err(StoreError::Database(_))
    ));
    drop(store);
    assert_eq!(
        fs::read(&path).expect("read state after read-only open"),
        before
    );
}

#[test]
fn exact_schema_writable_open_never_migrates_legacy_state() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("legacy-writable.db");
    let connection = Connection::open(&path).expect("create database");
    connection
        .execute_batch("CREATE TABLE marker (value INTEGER NOT NULL) STRICT;")
        .expect("create marker");
    connection
        .pragma_update(None, "user_version", 1)
        .expect("set legacy version");
    drop(connection);
    let before = fs::read(&path).expect("read legacy state");
    assert!(matches!(
        ArtifactStateStore::open_existing_writable_exact(&path),
        Err(StoreError::MigrationRequired {
            found: 1,
            current: 3
        })
    ));
    assert_eq!(fs::read(&path).expect("reread legacy state"), before);
    assert!(matches!(
        ArtifactStateStore::open_existing_and_migrate(&path),
        Err(StoreError::CorruptRecord)
    ));
    assert_eq!(fs::read(&path).expect("reread rejected state"), before);
}

#[test]
fn interrupted_empty_initialization_can_resume_but_arbitrary_version_zero_cannot() {
    let directory = tempdir().expect("temporary directory");
    let empty = directory.path().join("empty.db");
    drop(Connection::open(&empty).expect("create empty SQLite file"));
    drop(
        ArtifactStateStore::open_existing_or_initialize_empty(&empty)
            .expect("resume empty initialization"),
    );
    ArtifactStateStore::open_existing_writable_exact(&empty)
        .expect("resumed state uses current schema");

    let arbitrary = directory.path().join("arbitrary.db");
    let connection = Connection::open(&arbitrary).expect("create arbitrary state");
    connection
        .execute_batch("CREATE TABLE foreign_state (value TEXT) STRICT;")
        .expect("create arbitrary table");
    drop(connection);
    assert!(matches!(
        ArtifactStateStore::open_existing_or_initialize_empty(&arbitrary),
        Err(StoreError::MigrationRequired { found: 0, .. })
    ));
    assert!(matches!(
        ArtifactStateStore::open_existing_and_migrate(&arbitrary),
        Err(StoreError::MigrationRequired { found: 0, .. })
    ));
    assert!(matches!(
        ArtifactStateStore::open_or_create_and_migrate(&arbitrary),
        Err(StoreError::MigrationRequired { found: 0, .. })
    ));
}

#[test]
fn migrates_schema_two_without_rewriting_v1_records() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("schema-two.db");
    let fixture = fixture();
    let qualification_id = fixture
        .qualification
        .qualification_id()
        .expect("qualification id");
    let manifest_json = serde_json::to_string(&fixture.manifest).expect("manifest JSON");
    let qualification_json =
        serde_json::to_string(&fixture.qualification).expect("qualification JSON");
    let connection = Connection::open(&path).expect("create schema-two fixture");
    crate::schema::create_schema_two_fixture(&connection).expect("create schema two");
    connection
        .execute(
            "INSERT INTO artifact_manifests (artifact_id, record_json) VALUES (?1, ?2)",
            rusqlite::params![
                fixture.manifest.artifact_id.digest().as_str(),
                manifest_json
            ],
        )
        .expect("insert legacy manifest");
    connection
        .execute(
            "INSERT INTO qualification_records
                 (qualification_id, artifact_id, record_json) VALUES (?1, ?2, ?3)",
            rusqlite::params![
                qualification_id.digest().as_str(),
                fixture.manifest.artifact_id.digest().as_str(),
                qualification_json
            ],
        )
        .expect("insert legacy qualification");
    drop(connection);

    let store = ArtifactStateStore::open_existing_and_migrate(&path).expect("migrate schema two");
    assert_eq!(
        store
            .manifest(&fixture.manifest.artifact_id)
            .expect("load manifest"),
        Some(fixture.manifest.clone())
    );
    let version: i64 = store
        .connection()
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read migrated version");
    assert_eq!(version, 3);
    let stored_qualification: String = store
        .connection()
        .query_row(
            "SELECT record_json FROM qualification_records WHERE qualification_id = ?1",
            [qualification_id.digest().as_str()],
            |row| row.get(0),
        )
        .expect("load unchanged qualification bytes");
    assert_eq!(stored_qualification, qualification_json);
}

#[test]
fn corrupt_schema_two_is_rejected_without_partial_migration() {
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
        Err(StoreError::CorruptRecord)
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
        .pragma_update(None, "user_version", 3)
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
             PRAGMA user_version = 3;",
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

#[cfg(unix)]
#[test]
fn state_open_rejects_a_final_symbolic_link() {
    use std::os::unix::fs::symlink;

    let directory = tempdir().expect("temporary directory");
    let target = directory.path().join("target.db");
    drop(
        ArtifactStateStore::open_or_create_and_migrate(&target)
            .expect("create current-schema target"),
    );
    let indirect = directory.path().join("indirect.db");
    symlink(&target, &indirect).expect("create state symlink");
    assert!(matches!(
        ArtifactStateStore::open_existing_read_only(&indirect),
        Err(StoreError::Database(_))
    ));
}

fn assert_read_only_schema_rejection(
    version: i64,
    expected: impl FnOnce(&StoreError) -> bool,
    name: &str,
) {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join(name);
    let connection = Connection::open(&path).expect("create schema fixture");
    connection
        .execute_batch("CREATE TABLE marker (value INTEGER NOT NULL) STRICT;")
        .expect("create marker table");
    connection
        .pragma_update(None, "user_version", version)
        .expect("set fixture schema version");
    drop(connection);
    let before = fs::read(&path).expect("read state before rejected open");
    let before_entries = directory_entries(directory.path());

    let error = ArtifactStateStore::open_existing_read_only(&path)
        .err()
        .expect("schema must be rejected");
    assert!(expected(&error));
    assert_eq!(
        fs::read(&path).expect("read state after rejected open"),
        before
    );
    assert_eq!(directory_entries(directory.path()), before_entries);
    let connection = Connection::open(&path).expect("reopen unchanged fixture");
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .expect("read unchanged schema version"),
        version
    );
    let marker_exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name = 'marker')",
            [],
            |row| row.get(0),
        )
        .expect("read marker table");
    assert!(marker_exists);
}

fn directory_entries(path: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut entries = fs::read_dir(path)
        .expect("read fixture directory")
        .map(|entry| entry.expect("read fixture entry").path())
        .collect::<Vec<_>>();
    entries.sort();
    entries
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
