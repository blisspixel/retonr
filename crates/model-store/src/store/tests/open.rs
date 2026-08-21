use std::fs;

use rusqlite::Connection;
use tempfile::tempdir;

use super::{ArtifactStateStore, fixture};
use crate::StoreError;

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
                    current: 6
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
                    current: 6
                }
            )
        },
        "schema-two-read-only.db",
    );
    assert_read_only_schema_rejection(
        3,
        |error| {
            matches!(
                error,
                StoreError::MigrationRequired {
                    found: 3,
                    current: 6
                }
            )
        },
        "schema-three-read-only.db",
    );
    assert_read_only_schema_rejection(
        4,
        |error| {
            matches!(
                error,
                StoreError::MigrationRequired {
                    found: 4,
                    current: 6
                }
            )
        },
        "schema-four-read-only.db",
    );
    assert_read_only_schema_rejection(
        5,
        |error| {
            matches!(
                error,
                StoreError::MigrationRequired {
                    found: 5,
                    current: 6
                }
            )
        },
        "schema-five-read-only.db",
    );
}

#[test]
fn read_only_open_rejects_future_schema_without_mutation() {
    assert_read_only_schema_rejection(
        7,
        |error| matches!(error, StoreError::UnsupportedSchema(7)),
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
            current: 6
        })
    ));
    assert_eq!(fs::read(&path).expect("reread legacy state"), before);
    assert!(matches!(
        ArtifactStateStore::open_existing_and_migrate(&path),
        Err(StoreError::MigrationRequired {
            found: 1,
            current: 6
        })
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
fn compatibility_opens_reject_schema_two_without_rewriting_v1_records() {
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
    let before = fs::read(&path).expect("read schema two before rejected opens");

    for result in [
        ArtifactStateStore::open(&path),
        ArtifactStateStore::open_or_create_and_migrate(&path),
        ArtifactStateStore::open_existing_and_migrate(&path),
    ] {
        assert!(matches!(
            result,
            Err(StoreError::MigrationRequired {
                found: 2,
                current: 6
            })
        ));
        assert_eq!(fs::read(&path).expect("reread rejected schema two"), before);
    }

    let unchanged = Connection::open(&path).expect("reopen unchanged schema two");
    let version: i64 = unchanged
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read unchanged version");
    assert_eq!(version, 2);
    let stored_qualification: String = unchanged
        .query_row(
            "SELECT record_json FROM qualification_records WHERE qualification_id = ?1",
            [qualification_id.digest().as_str()],
            |row| row.get(0),
        )
        .expect("load unchanged qualification bytes");
    assert_eq!(stored_qualification, qualification_json);
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
