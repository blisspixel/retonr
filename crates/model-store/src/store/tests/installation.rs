use tempfile::tempdir;

use super::{ArtifactStateStore, fixture};
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
        }
    );
    assert_eq!(
        store
            .put_installation(&fixture.manifest, &fixture.installed)
            .expect("repeat exact installation"),
        InstallationWriteDisposition {
            manifest: WriteDisposition::AlreadyPresent,
            installed: WriteDisposition::AlreadyPresent,
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
