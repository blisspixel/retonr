use tempfile::tempdir;

use rewrite_model::{
    ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath,
    InstalledArtifactSet,
};
use rewrite_types::Digest;

use super::{ArtifactStateStore, exclusive_lifecycle_lock};
use crate::{
    ArtifactRemovalPhase, RemovalCompletionDisposition, RemovalPreparationDisposition, StoreError,
};

fn member(path: &str, bytes: &[u8]) -> ArtifactSetMember {
    ArtifactSetMember::new(
        ArtifactId::from_digest(Digest::sha256(bytes)),
        u64::try_from(bytes.len()).expect("fixture size fits u64"),
        ArtifactSetRelativePath::new(path).expect("valid fixture path"),
    )
}

fn fixture(label: &str) -> (ArtifactSetManifest, InstalledArtifactSet) {
    let manifest = ArtifactSetManifest::new(vec![
        member("config.json", b"{}"),
        member("model/weights.bin", label.as_bytes()),
    ])
    .expect("valid artifact-set manifest");
    let installed =
        InstalledArtifactSet::new(&manifest, "set-root-01").expect("valid installed set");
    (manifest, installed)
}

#[test]
fn prepares_completes_and_increments_generation_on_reinstall() {
    let directory = tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    let (manifest, installed) = fixture("weights");
    let first = store
        .put_artifact_set_installation(&manifest, &installed)
        .expect("install first generation")
        .installation;
    assert_eq!(first.epoch.get(), 1);

    assert_eq!(
        store
            .prepare_artifact_set_removal(&exclusive_lifecycle_lock(), &first)
            .expect("prepare first removal"),
        RemovalPreparationDisposition::Prepared
    );
    assert_eq!(
        store
            .prepare_artifact_set_removal(&exclusive_lifecycle_lock(), &first)
            .expect("repeat prepare"),
        RemovalPreparationDisposition::AlreadyPrepared
    );
    assert!(matches!(
        store.put_artifact_set_installation(&manifest, &installed),
        Err(StoreError::RemovalPending)
    ));
    assert_eq!(
        store
            .pending_artifact_set_removals(8)
            .expect("list prepared")
            .as_slice(),
        std::slice::from_ref(&first)
    );

    assert_eq!(
        store
            .complete_artifact_set_removal(&exclusive_lifecycle_lock(), &first)
            .expect("complete first removal"),
        RemovalCompletionDisposition::Completed
    );
    assert_eq!(
        store
            .complete_artifact_set_removal(&exclusive_lifecycle_lock(), &first)
            .expect("repeat complete"),
        RemovalCompletionDisposition::AlreadyCompleted
    );
    assert!(
        store
            .pending_artifact_set_removals(8)
            .expect("completed journals are omitted")
            .is_empty()
    );

    let second = store
        .put_artifact_set_installation(&manifest, &installed)
        .expect("reinstall after completed removal")
        .installation;
    assert!(second.epoch > first.epoch);
    let (current, removal) = store
        .artifact_set_removal_state(&manifest.artifact_set_id())
        .expect("inspect reinstalled state");
    assert_eq!(current.as_ref(), Some(&second));
    assert_eq!(
        removal.as_ref().map(|value| value.phase),
        Some(ArtifactRemovalPhase::Completed)
    );
}

#[test]
fn stale_generation_cannot_prepare_or_complete_a_reinstall() {
    let directory = tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    let (manifest, installed) = fixture("stale");
    let first = store
        .put_artifact_set_installation(&manifest, &installed)
        .expect("install")
        .installation;
    store
        .prepare_artifact_set_removal(&exclusive_lifecycle_lock(), &first)
        .expect("prepare");
    store
        .complete_artifact_set_removal(&exclusive_lifecycle_lock(), &first)
        .expect("complete");
    let second = store
        .put_artifact_set_installation(&manifest, &installed)
        .expect("reinstall")
        .installation;

    assert!(matches!(
        store.prepare_artifact_set_removal(&exclusive_lifecycle_lock(), &first),
        Err(StoreError::StaleInstallation)
    ));
    assert!(matches!(
        store.complete_artifact_set_removal(&exclusive_lifecycle_lock(), &first),
        Err(StoreError::StaleInstallation)
    ));
    assert_eq!(
        store
            .artifact_set_installation(&manifest.artifact_set_id())
            .expect("current generation remains"),
        Some(second)
    );
}

#[test]
fn pending_set_removals_reject_zero_and_overflow_limits() {
    let directory = tempdir().expect("temporary directory");
    let store = ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    assert!(matches!(
        store.pending_artifact_set_removals(0),
        Err(StoreError::InvalidLimit)
    ));
}
