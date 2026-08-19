use std::fs;

use rewrite_model::{ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath};
use rewrite_model_store::{ArtifactRemovalPhase, ArtifactStateStore};
use rewrite_types::{CancellationToken, Digest};
use tempfile::{TempDir, tempdir};

use super::{
    ArtifactSetRemovalError, ArtifactSetRemovalLimits, ArtifactSetRemovalRequest,
    ArtifactSetRemovalService, ArtifactSetRemovalStage,
};
use crate::{
    ArtifactImportLimits, ArtifactRemovalDisposition, ArtifactSetImportLimits,
    OfflineArtifactImportService, OfflineArtifactSetImportRequest,
    artifact_set_import::{
        OfflineArtifactSetImportService, SET_STORAGE_KEY_PREFIX, SETS_DIRECTORY,
    },
};

fn limits() -> ArtifactSetRemovalLimits {
    ArtifactSetRemovalLimits {
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
) -> ArtifactSetRemovalRequest {
    let value = manifest(label);
    let source = directory.path().join(format!("source-{label}"));
    write_source(&source, label);
    let mut service =
        OfflineArtifactSetImportService::open(storage(directory), store, import_limits())
            .expect("open set import");
    let imported = service
        .import(
            &OfflineArtifactSetImportRequest {
                source_root: source,
                manifest: value,
            },
            &CancellationToken::new(),
            |_| {},
        )
        .expect("import set fixture");
    ArtifactSetRemovalRequest {
        selection: imported.state.installation,
    }
}

#[test]
fn rejects_invalid_limits_and_uninitialized_storage() {
    let directory = tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open artifact state");
    let mut invalid = limits();
    invalid.maximum_members = 0;
    assert!(matches!(
        ArtifactSetRemovalService::open_existing(
            directory.path().join("missing"),
            &mut store,
            invalid
        ),
        Err(ArtifactSetRemovalError::InvalidLimits)
    ));
    assert!(matches!(
        ArtifactSetRemovalService::open_existing(
            directory.path().join("missing"),
            &mut store,
            limits()
        ),
        Err(ArtifactSetRemovalError::StorageNotInitialized)
    ));
}

#[test]
fn removes_exact_inactive_tree_and_retries_without_touching_reinstall() {
    let (directory, mut store) = initialized();
    let request = import_set(&directory, &mut store, "remove-set");
    let root = set_root(&directory, &store_manifest(&store, &request));
    let mut progress = Vec::new();
    let result =
        ArtifactSetRemovalService::open_existing(storage(&directory), &mut store, limits())
            .expect("open removal")
            .remove(&request, &CancellationToken::new(), |value| {
                progress.push(value);
            })
            .expect("remove set");
    assert_eq!(result.disposition, ArtifactRemovalDisposition::Removed);
    assert!(!root.exists());
    assert_eq!(
        progress.iter().map(|value| value.stage).collect::<Vec<_>>(),
        vec![
            ArtifactSetRemovalStage::InspectingSelection,
            ArtifactSetRemovalStage::VerifyingInactiveTree,
            ArtifactSetRemovalStage::VerifyingInactiveTree,
            ArtifactSetRemovalStage::PreparingRemoval,
        ]
    );
    let state = store
        .artifact_set_removal_state(request.selection.installed.artifact_set_id())
        .expect("inspect completed state");
    assert_eq!(state.0, None);
    assert_eq!(
        state.1.as_ref().map(|value| value.phase),
        Some(ArtifactRemovalPhase::Completed)
    );

    write_source(&directory.path().join("reinstall-source"), "remove-set");
    let reinstall_manifest = store_manifest(&store, &request);
    let reinstalled =
        OfflineArtifactSetImportService::open(storage(&directory), &mut store, import_limits())
            .expect("open reimport")
            .import(
                &OfflineArtifactSetImportRequest {
                    source_root: directory.path().join("reinstall-source"),
                    manifest: reinstall_manifest,
                },
                &CancellationToken::new(),
                |_| {},
            )
            .expect("reinstall set")
            .state
            .installation;
    assert!(reinstalled.epoch > request.selection.epoch);
    let retry = ArtifactSetRemovalService::open_existing(storage(&directory), &mut store, limits())
        .expect("open old retry")
        .remove(&request, &CancellationToken::new(), |_| {})
        .expect("old retry is complete");
    assert_eq!(
        retry.disposition,
        ArtifactRemovalDisposition::AlreadyRemoved
    );
    assert_eq!(
        fs::read(set_root(&directory, &store_manifest(&store, &request)).join("model/weights.bin"))
            .expect("new tree remains"),
        b"remove-set"
    );
}

#[test]
fn resumes_prepared_set_removal_after_state_reopen() {
    for remove_before_retry in [false, true] {
        let (directory, mut store) = initialized();
        let request = import_set(&directory, &mut store, "recover-set");
        let root = set_root(&directory, &store_manifest(&store, &request));
        crate::artifact_storage::test_support::prepare_artifact_set_removal(
            &storage(&directory),
            &mut store,
            &request.selection,
        )
        .expect("simulate crash after preparation");
        drop(store);
        if remove_before_retry {
            fs::remove_dir_all(&root).expect("simulate crash after tree deletion");
        }
        let mut store = ArtifactStateStore::open(&directory.path().join("state.db"))
            .expect("reopen durable state");
        let result =
            ArtifactSetRemovalService::open_existing(storage(&directory), &mut store, limits())
                .expect("open recovery")
                .remove(&request, &CancellationToken::new(), |_| {})
                .expect("recover set removal");
        assert_eq!(result.disposition, ArtifactRemovalDisposition::Recovered);
        assert!(!root.exists());
    }
}

#[test]
fn cancellation_before_preparation_leaves_the_tree() {
    let (directory, mut store) = initialized();
    let request = import_set(&directory, &mut store, "cancel-set");
    let root = set_root(&directory, &store_manifest(&store, &request));
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let error = ArtifactSetRemovalService::open_existing(storage(&directory), &mut store, limits())
        .expect("open removal")
        .remove(&request, &cancellation, |_| {})
        .expect_err("cancelled before preparation");
    assert!(matches!(error, ArtifactSetRemovalError::Cancelled));
    assert!(root.exists());
    assert_eq!(
        store
            .artifact_set_installation(request.selection.installed.artifact_set_id())
            .expect("installation remains"),
        Some(request.selection)
    );
}

fn store_manifest(
    store: &ArtifactStateStore,
    request: &ArtifactSetRemovalRequest,
) -> ArtifactSetManifest {
    store
        .artifact_set_manifest(request.selection.installed.artifact_set_id())
        .expect("load fixture manifest")
        .expect("fixture manifest exists")
}
