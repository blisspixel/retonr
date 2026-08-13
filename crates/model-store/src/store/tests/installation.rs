use tempfile::tempdir;

use rewrite_model::{ActivationId, ArtifactRole};
use rewrite_types::Digest;

use super::{ArtifactStateStore, fixture, populate, qualification_id};
use crate::{InstallationWriteDisposition, StoreError, WriteDisposition};

#[test]
fn registers_manifest_and_installation_atomically_and_idempotently() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("artifacts.sqlite3");
    let fixture = fixture();
    let mut store = ArtifactStateStore::open(&path).expect("open store");

    assert_eq!(
        store
            .put_installation(&fixture.manifest, &fixture.installed)
            .expect("register installation"),
        InstallationWriteDisposition {
            manifest: WriteDisposition::Inserted,
            installed: WriteDisposition::Inserted,
            installation: crate::StoredArtifactInstallation {
                installed: fixture.installed.clone(),
                epoch: crate::ArtifactInstallationEpoch::for_test(1).expect("first epoch"),
            },
        }
    );
    assert_eq!(
        store
            .put_installation(&fixture.manifest, &fixture.installed)
            .expect("repeat exact installation"),
        InstallationWriteDisposition {
            manifest: WriteDisposition::AlreadyPresent,
            installed: WriteDisposition::AlreadyPresent,
            installation: crate::StoredArtifactInstallation {
                installed: fixture.installed.clone(),
                epoch: crate::ArtifactInstallationEpoch::for_test(1).expect("first epoch"),
            },
        }
    );
}

#[test]
fn rolls_back_manifest_when_atomic_installation_write_fails() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("artifacts.sqlite3");
    let fixture = fixture();
    let mut store = ArtifactStateStore::open(&path).expect("open store");
    store
        .connection
        .execute_batch(
            "CREATE TRIGGER reject_fixture_install
             BEFORE INSERT ON installed_artifacts
             BEGIN
                 SELECT RAISE(ABORT, 'fixture rejection');
             END;",
        )
        .expect("install failure trigger");

    assert!(matches!(
        store.put_installation(&fixture.manifest, &fixture.installed),
        Err(StoreError::Database(_))
    ));
    assert_eq!(
        store
            .manifest(&fixture.manifest.artifact_id)
            .expect("check rolled back manifest"),
        None
    );
}

#[test]
fn refuses_to_overwrite_malformed_existing_installation_state() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("artifacts.sqlite3");
    let fixture = fixture();
    let mut store = ArtifactStateStore::open(&path).expect("open store");
    store
        .put_installation(&fixture.manifest, &fixture.installed)
        .expect("register fixture");
    store
        .connection
        .execute(
            "UPDATE installed_artifacts SET record_json = 'not-json'
             WHERE artifact_id = ?1",
            [fixture.manifest.artifact_id.digest().as_str()],
        )
        .expect("corrupt installed record");

    assert!(matches!(
        store.put_installation(&fixture.manifest, &fixture.installed),
        Err(StoreError::Serialization(_))
    ));
}

#[test]
fn refuses_dangling_installed_state_instead_of_healing_it() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("artifacts.sqlite3");
    let fixture = fixture();
    let mut store = ArtifactStateStore::open(&path).expect("open store");
    let installed = serde_json::to_string(&fixture.installed).expect("encode installation");
    store
        .connection
        .execute_batch("PRAGMA foreign_keys = OFF;")
        .expect("disable foreign keys for corruption fixture");
    store
        .connection
        .execute(
            "INSERT INTO installed_artifacts
                 (artifact_id, installation_epoch, record_json) VALUES (?1, 1, ?2)",
            rusqlite::params![fixture.manifest.artifact_id.digest().as_str(), installed],
        )
        .expect("insert dangling installation");
    store
        .connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .expect("restore foreign keys");

    assert!(matches!(
        store.put_installation(&fixture.manifest, &fixture.installed),
        Err(StoreError::CorruptRecord)
    ));
    assert_eq!(
        store
            .manifest(&fixture.manifest.artifact_id)
            .expect("manifest remains absent"),
        None
    );
}

#[test]
fn refuses_to_resurrect_a_dangling_active_binding() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("artifacts.sqlite3");
    let fixture = fixture();
    let mut store = ArtifactStateStore::open(&path).expect("open store");
    populate(&mut store, &fixture);
    store
        .activate(
            ActivationId::from_digest(Digest::sha256(b"dangling activation")),
            ArtifactRole::Generation,
            &fixture.installed,
            &qualification_id(&fixture),
        )
        .expect("activate fixture");
    store
        .connection
        .execute_batch("PRAGMA foreign_keys = OFF;")
        .expect("disable foreign keys for corruption fixture");
    store
        .connection
        .execute(
            "DELETE FROM installed_artifacts WHERE artifact_id = ?1",
            [fixture.manifest.artifact_id.digest().as_str()],
        )
        .expect("delete installation under active binding");
    store
        .connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .expect("restore foreign keys");

    assert!(matches!(
        store.put_installation(&fixture.manifest, &fixture.installed),
        Err(StoreError::CorruptRecord)
    ));
    let installed_count: i64 = store
        .connection
        .query_row(
            "SELECT COUNT(*) FROM installed_artifacts WHERE artifact_id = ?1",
            [fixture.manifest.artifact_id.digest().as_str()],
            |row| row.get(0),
        )
        .expect("count installed rows");
    assert_eq!(installed_count, 0);
    assert!(matches!(
        store.recover_active_bindings(|_| true),
        Err(StoreError::MissingRecord | StoreError::InvalidActiveBinding)
    ));
}
