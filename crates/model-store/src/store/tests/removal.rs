use rusqlite::Connection;
use tempfile::tempdir;

use rewrite_model::{ActivationId, ArtifactRole};
use rewrite_types::Digest;

use super::{ArtifactStateStore, fixture, populate, qualification_id};
use crate::{
    ArtifactRemovalPhase, RemovalCompletionDisposition, RemovalPreparationDisposition, StoreError,
};

#[test]
fn migrates_schema_one_installations_to_first_epoch() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("state.db");
    let fixture = fixture();
    let connection = Connection::open(&path).expect("open legacy database");
    connection
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE artifact_manifests (
                 artifact_id TEXT PRIMARY KEY NOT NULL CHECK(length(artifact_id) = 64),
                 record_json TEXT NOT NULL
                     CHECK(length(CAST(record_json AS BLOB)) <= 1048576)
             ) STRICT;
             CREATE TABLE installed_artifacts (
                 artifact_id TEXT PRIMARY KEY NOT NULL CHECK(length(artifact_id) = 64),
                 record_json TEXT NOT NULL
                     CHECK(length(CAST(record_json AS BLOB)) <= 1048576),
                 FOREIGN KEY(artifact_id) REFERENCES artifact_manifests(artifact_id)
             ) STRICT;
             CREATE TABLE qualification_records (
                 qualification_id TEXT PRIMARY KEY NOT NULL CHECK(length(qualification_id) = 64),
                 artifact_id TEXT NOT NULL CHECK(length(artifact_id) = 64),
                 record_json TEXT NOT NULL
                     CHECK(length(CAST(record_json AS BLOB)) <= 1048576),
                 FOREIGN KEY(artifact_id) REFERENCES artifact_manifests(artifact_id)
             ) STRICT;
             CREATE TABLE qualification_invalidations (
                 sequence INTEGER PRIMARY KEY,
                 qualification_id TEXT NOT NULL CHECK(length(qualification_id) = 64),
                 reason_code TEXT NOT NULL CHECK(length(reason_code) BETWEEN 1 AND 64),
                 record_json TEXT NOT NULL
                     CHECK(length(CAST(record_json AS BLOB)) <= 1048576),
                 FOREIGN KEY(qualification_id)
                     REFERENCES qualification_records(qualification_id)
             ) STRICT;
             CREATE INDEX invalidations_by_qualification
                 ON qualification_invalidations(qualification_id, sequence);
             CREATE TABLE activation_decisions (
                 activation_id TEXT PRIMARY KEY NOT NULL CHECK(length(activation_id) = 64),
                 role TEXT NOT NULL CHECK(length(role) BETWEEN 1 AND 64),
                 record_json TEXT NOT NULL
                     CHECK(length(CAST(record_json AS BLOB)) <= 1048576)
             ) STRICT;
             CREATE TABLE active_bindings (
                 role TEXT PRIMARY KEY NOT NULL CHECK(length(role) BETWEEN 1 AND 64),
                 artifact_id TEXT NOT NULL CHECK(length(artifact_id) = 64),
                 qualification_id TEXT NOT NULL CHECK(length(qualification_id) = 64),
                 record_json TEXT NOT NULL
                     CHECK(length(CAST(record_json AS BLOB)) <= 1048576),
                 FOREIGN KEY(artifact_id) REFERENCES installed_artifacts(artifact_id),
                 FOREIGN KEY(qualification_id)
                     REFERENCES qualification_records(qualification_id)
             ) STRICT;
             PRAGMA user_version = 1;",
        )
        .expect("create legacy schema");
    connection
        .execute(
            "INSERT INTO artifact_manifests (artifact_id, record_json) VALUES (?1, ?2)",
            rusqlite::params![
                fixture.manifest.artifact_id.digest().as_str(),
                serde_json::to_string(&fixture.manifest).expect("encode manifest")
            ],
        )
        .expect("insert legacy manifest");
    connection
        .execute(
            "INSERT INTO installed_artifacts (artifact_id, record_json) VALUES (?1, ?2)",
            rusqlite::params![
                fixture.installed.artifact_id.digest().as_str(),
                serde_json::to_string(&fixture.installed).expect("encode installation")
            ],
        )
        .expect("insert legacy installation");
    drop(connection);

    let store =
        ArtifactStateStore::open_existing_and_migrate(&path).expect("migrate existing schema");
    let selection = store
        .artifact_removal_state(&fixture.installed.artifact_id)
        .expect("load migrated state")
        .0
        .expect("migrated installation");
    assert_eq!(selection.epoch.get(), 1);
    drop(store);
    ArtifactStateStore::open_existing_writable_exact(&path)
        .expect("migrated schema reopens through the exact path");
}

#[test]
fn prepares_completes_and_reinstalls_with_a_new_epoch() {
    let directory = tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    let fixture = fixture();
    let first = store
        .put_installation(&fixture.manifest, &fixture.installed)
        .expect("register installation")
        .installation;
    assert_eq!(
        store
            .prepare_artifact_removal(&first)
            .expect("prepare removal"),
        RemovalPreparationDisposition::Prepared
    );
    assert_eq!(
        store
            .prepare_artifact_removal(&first)
            .expect("repeat prepared removal"),
        RemovalPreparationDisposition::AlreadyPrepared
    );
    assert_eq!(
        store
            .complete_artifact_removal(&first)
            .expect("complete removal"),
        RemovalCompletionDisposition::Completed
    );
    assert_eq!(
        store
            .complete_artifact_removal(&first)
            .expect("repeat completed removal"),
        RemovalCompletionDisposition::AlreadyCompleted
    );
    let second = store
        .put_installation(&fixture.manifest, &fixture.installed)
        .expect("reinstall")
        .installation;
    assert_eq!(second.epoch.get(), first.epoch.get() + 1);
    store
        .put_qualification(&fixture.qualification)
        .expect("store qualification for reinstalled generation");
    let binding = store
        .activate(
            ActivationId::from_digest(Digest::sha256(b"reinstalled activation")),
            ArtifactRole::Generation,
            &fixture.installed,
            &qualification_id(&fixture),
        )
        .expect("activate reinstalled generation");
    assert_eq!(
        store
            .prepare_artifact_removal(&first)
            .expect("old retry remains complete despite current active generation"),
        RemovalPreparationDisposition::AlreadyCompleted
    );
    assert_eq!(
        store
            .artifact_removal_state(&fixture.installed.artifact_id)
            .expect("load reinstalled state")
            .0,
        Some(second)
    );
    assert_eq!(
        store
            .active_binding(ArtifactRole::Generation, |_| true)
            .expect("recover current active binding"),
        Some(binding)
    );
}

#[test]
fn pending_removal_blocks_installation_and_activation() {
    let directory = tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    let fixture = fixture();
    populate(&mut store, &fixture);
    let selection = store
        .artifact_removal_state(&fixture.installed.artifact_id)
        .expect("load state")
        .0
        .expect("installed selection");
    store
        .prepare_artifact_removal(&selection)
        .expect("prepare removal");
    assert!(matches!(
        store.put_installation(&fixture.manifest, &fixture.installed),
        Err(StoreError::RemovalPending)
    ));
    assert!(matches!(
        store.activate(
            ActivationId::from_digest(Digest::sha256(b"blocked activation")),
            ArtifactRole::Generation,
            &fixture.installed,
            &qualification_id(&fixture),
        ),
        Err(StoreError::RemovalPending)
    ));
}

#[test]
fn active_selection_cannot_be_prepared() {
    let directory = tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    let fixture = fixture();
    populate(&mut store, &fixture);
    store
        .activate(
            ActivationId::from_digest(Digest::sha256(b"active selection")),
            ArtifactRole::Generation,
            &fixture.installed,
            &qualification_id(&fixture),
        )
        .expect("activate");
    let selection = store
        .artifact_removal_state(&fixture.installed.artifact_id)
        .expect("load state")
        .0
        .expect("installed selection");
    assert!(matches!(
        store.prepare_artifact_removal(&selection),
        Err(StoreError::ActiveArtifact)
    ));
}

#[test]
fn inventory_retains_validated_pending_removal_state() {
    let directory = tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    let fixture = fixture();
    let selection = store
        .put_installation(&fixture.manifest, &fixture.installed)
        .expect("register installation")
        .installation;
    store
        .prepare_artifact_removal(&selection)
        .expect("prepare removal");

    let state = store.artifact_inventory(1).expect("inventory");
    assert_eq!(state[0].installed, None);
    assert_eq!(
        state[0].removal.as_ref().map(|value| value.phase),
        Some(ArtifactRemovalPhase::Prepared)
    );
}

#[test]
fn epoch_overflow_fails_closed() {
    let directory = tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    let fixture = fixture();
    store
        .put_installation(&fixture.manifest, &fixture.installed)
        .expect("register installation");
    let selection = store
        .artifact_removal_state(&fixture.installed.artifact_id)
        .expect("load first generation")
        .0
        .expect("installed selection");
    store
        .prepare_artifact_removal(&selection)
        .expect("prepare first generation");
    store
        .complete_artifact_removal(&selection)
        .expect("complete first generation");
    store
        .connection
        .execute(
            "UPDATE artifact_removals
             SET installation_epoch = ?2,
                 record_json = json_set(record_json, '$.installation_epoch', ?2)
             WHERE artifact_id = ?1",
            rusqlite::params![fixture.installed.artifact_id.digest().as_str(), i64::MAX],
        )
        .expect("set maximum completed epoch");
    assert!(matches!(
        store.put_installation(&fixture.manifest, &fixture.installed),
        Err(StoreError::InstallationEpochExhausted)
    ));
}

#[test]
fn prepare_and_complete_failures_preserve_the_prior_transaction_state() {
    let directory = tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    let fixture = fixture();
    let selection = store
        .put_installation(&fixture.manifest, &fixture.installed)
        .expect("register installation")
        .installation;
    store
        .connection
        .execute_batch(
            "CREATE TEMP TRIGGER fail_preparation_delete
             BEFORE DELETE ON installed_artifacts
             BEGIN SELECT RAISE(ABORT, 'injected preparation failure'); END;",
        )
        .expect("create preparation fault");
    assert!(store.prepare_artifact_removal(&selection).is_err());
    store
        .connection
        .execute_batch("DROP TRIGGER fail_preparation_delete;")
        .expect("remove preparation fault");
    let (installed, removal) = store
        .artifact_removal_state(&fixture.installed.artifact_id)
        .expect("inspect rolled back preparation");
    assert_eq!(installed, Some(selection.clone()));
    assert_eq!(removal, None);

    store
        .prepare_artifact_removal(&selection)
        .expect("prepare removal");
    store
        .connection
        .execute_batch(
            "CREATE TEMP TRIGGER fail_completion_update
             BEFORE UPDATE ON artifact_removals
             BEGIN SELECT RAISE(ABORT, 'injected completion failure'); END;",
        )
        .expect("create completion fault");
    assert!(store.complete_artifact_removal(&selection).is_err());
    let (installed, removal) = store
        .artifact_removal_state(&fixture.installed.artifact_id)
        .expect("inspect preserved preparation");
    assert_eq!(installed, None);
    assert_eq!(
        removal.map(|value| value.phase),
        Some(ArtifactRemovalPhase::Prepared)
    );
}

#[test]
fn corrupt_epoch_sequences_fail_before_activation_or_recovery() {
    let directory = tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    let fixture = fixture();
    populate(&mut store, &fixture);
    let selection = store
        .artifact_removal_state(&fixture.installed.artifact_id)
        .expect("load selection")
        .0
        .expect("installed selection");
    store
        .prepare_artifact_removal(&selection)
        .expect("prepare removal");
    store
        .complete_artifact_removal(&selection)
        .expect("complete removal");
    store
        .put_installation(&fixture.manifest, &fixture.installed)
        .expect("reinstall");
    store
        .activate(
            ActivationId::from_digest(Digest::sha256(b"valid second activation")),
            ArtifactRole::Generation,
            &fixture.installed,
            &qualification_id(&fixture),
        )
        .expect("activate valid second generation");
    store
        .connection
        .execute(
            "UPDATE installed_artifacts SET installation_epoch = 3 WHERE artifact_id = ?1",
            [fixture.installed.artifact_id.digest().as_str()],
        )
        .expect("inject skipped generation");
    assert!(matches!(
        store.activate(
            ActivationId::from_digest(Digest::sha256(b"invalid epoch activation")),
            ArtifactRole::Generation,
            &fixture.installed,
            &qualification_id(&fixture),
        ),
        Err(StoreError::CorruptRecord)
    ));
    assert!(matches!(
        store.recover_active_bindings(|_| true),
        Err(StoreError::CorruptRecord)
    ));
}
