use std::fs;

use rewrite_model::{ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath};
use rewrite_model_store::ArtifactStateStore;
use rewrite_types::{CancellationToken, Digest};
use tempfile::{TempDir, tempdir};

use super::{
    ArtifactSetReconciliationError, ArtifactSetReconciliationLimits,
    ArtifactSetReconciliationRequest, ArtifactSetReconciliationService,
    ArtifactSetReconciliationStage,
};
use crate::{
    ArtifactImportLimits, ArtifactReconciliationDisposition, ArtifactSetImportLimits,
    OfflineArtifactImportService, OfflineArtifactSetImportRequest,
    artifact_set_import::{
        OfflineArtifactSetImportService, SET_STORAGE_KEY_PREFIX, SETS_DIRECTORY,
    },
};

fn limits() -> ArtifactSetReconciliationLimits {
    ArtifactSetReconciliationLimits {
        maximum_members: 16,
        maximum_member_bytes: 1_024,
        maximum_total_bytes: 4_096,
        maximum_tree_entries: 32,
        maximum_storage_entries: 16,
    }
}

fn import_limits() -> ArtifactSetImportLimits {
    ArtifactSetImportLimits {
        maximum_members: 16,
        maximum_member_bytes: 1_024,
        maximum_total_bytes: 4_096,
        maximum_tree_entries: 32,
        maximum_storage_entries: 16,
        maximum_staging_entries: 16,
    }
}

fn member(path: &str, bytes: &[u8]) -> ArtifactSetMember {
    ArtifactSetMember::new(
        ArtifactId::from_digest(Digest::sha256(bytes)),
        u64::try_from(bytes.len()).expect("fixture size"),
        ArtifactSetRelativePath::new(path).expect("fixture path"),
    )
}

fn manifest(label: &str) -> ArtifactSetManifest {
    ArtifactSetManifest::new(vec![
        member("config.json", b"{}"),
        member("model/weights.bin", label.as_bytes()),
    ])
    .expect("valid set manifest")
}

fn initialized() -> (TempDir, ArtifactStateStore) {
    let directory = tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open artifact state");
    {
        let _service = OfflineArtifactImportService::open(
            storage(&directory),
            &mut store,
            ArtifactImportLimits {
                maximum_artifact_bytes: 4_096,
                maximum_storage_entries: 32,
            },
        )
        .expect("initialize artifact storage");
    }
    (directory, store)
}

fn storage(directory: &TempDir) -> std::path::PathBuf {
    directory.path().join("storage")
}

fn set_root(directory: &TempDir, manifest: &ArtifactSetManifest) -> std::path::PathBuf {
    storage(directory).join(SETS_DIRECTORY).join(format!(
        "{SET_STORAGE_KEY_PREFIX}{}",
        manifest.artifact_set_id().digest().as_str()
    ))
}

fn write_source(root: &std::path::Path, label: &str) {
    fs::create_dir_all(root.join("model")).expect("source tree");
    fs::write(root.join("config.json"), b"{}").expect("config source");
    fs::write(root.join("model/weights.bin"), label.as_bytes()).expect("weights source");
}

fn import_set(
    directory: &TempDir,
    store: &mut ArtifactStateStore,
    label: &str,
) -> ArtifactSetManifest {
    let value = manifest(label);
    let source = directory.path().join(format!("source-{label}"));
    write_source(&source, label);
    let mut service =
        OfflineArtifactSetImportService::open(storage(directory), store, import_limits())
            .expect("open set import");
    service
        .import(
            &OfflineArtifactSetImportRequest {
                source_root: source,
                manifest: value.clone(),
            },
            &CancellationToken::new(),
            |_| {},
        )
        .expect("import set fixture");
    value
}

fn drop_installation(directory: &TempDir, manifest: &ArtifactSetManifest) {
    rusqlite::Connection::open(directory.path().join("state.db"))
        .expect("open state fixture")
        .execute(
            "DELETE FROM installed_artifact_sets WHERE artifact_set_id = ?1",
            [manifest.artifact_set_id().digest().as_str()],
        )
        .expect("drop installed set record");
}

fn reconcile(
    directory: &TempDir,
    store: &mut ArtifactStateStore,
    manifest: ArtifactSetManifest,
) -> Result<super::ArtifactSetReconciliationResult, ArtifactSetReconciliationError> {
    ArtifactSetReconciliationService::open_existing(storage(directory), store, limits())?.reconcile(
        &ArtifactSetReconciliationRequest { manifest },
        &CancellationToken::new(),
        |_| {},
    )
}

#[test]
fn rejects_invalid_limits_and_uninitialized_storage() {
    let directory = tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open artifact state");
    let mut invalid = limits();
    invalid.maximum_members = 0;
    assert!(matches!(
        ArtifactSetReconciliationService::open_existing(
            directory.path().join("missing"),
            &mut store,
            invalid
        ),
        Err(ArtifactSetReconciliationError::InvalidLimits)
    ));
    assert!(matches!(
        ArtifactSetReconciliationService::open_existing(
            directory.path().join("missing"),
            &mut store,
            limits()
        ),
        Err(ArtifactSetReconciliationError::StorageNotInitialized)
    ));
}

#[test]
fn registers_a_verified_orphan_set_without_copying_bytes() {
    let (directory, mut store) = initialized();
    let value = import_set(&directory, &mut store, "orphan-set");
    drop_installation(&directory, &value);
    let weights = set_root(&directory, &value).join("model/weights.bin");
    let before = fs::read(&weights).expect("read managed member");

    let result = reconcile(&directory, &mut store, value.clone()).expect("register orphan set");

    assert_eq!(
        result.disposition,
        ArtifactReconciliationDisposition::Registered
    );
    assert_eq!(result.installation.epoch.get(), 1);
    assert_eq!(result.installed.artifact_set_id(), &value.artifact_set_id());
    assert_eq!(fs::read(weights).expect("bytes unchanged"), before);
}

#[test]
fn already_registered_exact_state_is_idempotent() {
    let (directory, mut store) = initialized();
    let value = import_set(&directory, &mut store, "present-set");
    let result = reconcile(&directory, &mut store, value).expect("confirm exact set");
    assert_eq!(
        result.disposition,
        ArtifactReconciliationDisposition::AlreadyRegistered
    );
}

#[test]
fn missing_set_root_and_digest_conflict_fail_without_registration() {
    let (directory, mut store) = initialized();
    let missing = manifest("missing-set");
    assert!(matches!(
        reconcile(&directory, &mut store, missing),
        Err(ArtifactSetReconciliationError::OrphanNotFound)
    ));

    let value = import_set(&directory, &mut store, "conflict-set");
    drop_installation(&directory, &value);
    fs::write(
        set_root(&directory, &value).join("model/weights.bin"),
        b"changed",
    )
    .expect("corrupt member");
    assert!(matches!(
        reconcile(&directory, &mut store, value.clone()),
        Err(ArtifactSetReconciliationError::StorageConflict)
    ));
    assert!(
        store
            .artifact_set_installation(&value.artifact_set_id())
            .expect("read installation")
            .is_none()
    );
}

#[test]
fn observes_cancellation_before_registration() {
    let (directory, mut store) = initialized();
    let value = import_set(&directory, &mut store, "cancel-set");
    drop_installation(&directory, &value);
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    let mut service =
        ArtifactSetReconciliationService::open_existing(storage(&directory), &mut store, limits())
            .expect("open set reconcile");
    assert!(matches!(
        service.reconcile(
            &ArtifactSetReconciliationRequest { manifest: value },
            &cancelled,
            |_| {}
        ),
        Err(ArtifactSetReconciliationError::Cancelled)
    ));
}

#[test]
fn cancellation_during_verify_is_observed() {
    let (directory, mut store) = initialized();
    let value = import_set(&directory, &mut store, "cancel-verify");
    drop_installation(&directory, &value);
    let during = CancellationToken::new();
    let signal = during.clone();
    let mut service =
        ArtifactSetReconciliationService::open_existing(storage(&directory), &mut store, limits())
            .expect("open set reconcile");
    assert!(matches!(
        service.reconcile(
            &ArtifactSetReconciliationRequest { manifest: value },
            &during,
            |item| {
                if item.stage == ArtifactSetReconciliationStage::InspectingSelection {
                    signal.cancel();
                }
            }
        ),
        Err(ArtifactSetReconciliationError::Cancelled)
    ));
}
