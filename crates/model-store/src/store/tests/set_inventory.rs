use rewrite_model::{
    ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath,
    InstalledArtifactSet,
};
use rewrite_types::Digest;
use tempfile::tempdir;

use super::ArtifactStateStore;
use crate::StoreError;

fn member(path: &str, bytes: &[u8]) -> ArtifactSetMember {
    ArtifactSetMember::new(
        ArtifactId::from_digest(Digest::sha256(bytes)),
        u64::try_from(bytes.len()).expect("fixture size fits u64"),
        ArtifactSetRelativePath::new(path).expect("valid fixture path"),
    )
}

fn manifest(label: &str) -> ArtifactSetManifest {
    ArtifactSetManifest::new(vec![
        member("config/empty.json", b""),
        member("model/weights.gguf", label.as_bytes()),
    ])
    .expect("valid artifact-set manifest")
}

fn fixture(label: &str, storage_key: &str) -> (ArtifactSetManifest, InstalledArtifactSet) {
    let manifest = manifest(label);
    let installed =
        InstalledArtifactSet::new(&manifest, storage_key).expect("valid installed set root");
    (manifest, installed)
}

#[test]
fn lists_validated_set_state_in_identity_order() {
    let directory = tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open artifact state");
    let (second, second_installed) = fixture("second-set", "set-root-02");
    let (first, first_installed) = fixture("first-set", "set-root-01");
    store
        .put_artifact_set_installation(&second, &second_installed)
        .expect("register second set");
    store
        .put_artifact_set_manifest(&first)
        .expect("register first manifest only");
    store
        .put_artifact_set_installation(&first, &first_installed)
        .expect("register first set");

    let states = store.artifact_set_inventory(2).expect("list set state");
    assert_eq!(states.len(), 2);
    assert!(
        states[0].manifest.artifact_set_id().digest().as_str()
            < states[1].manifest.artifact_set_id().digest().as_str()
    );
    assert!(states.iter().any(|item| {
        item.manifest == first
            && item
                .installed
                .as_ref()
                .is_some_and(|value| value.installed == first_installed)
    }));
    assert!(states.iter().any(|item| {
        item.manifest == second
            && item
                .installed
                .as_ref()
                .is_some_and(|value| value.installed == second_installed)
    }));
}

#[test]
fn includes_manifest_only_set_state() {
    let directory = tempdir().expect("temporary directory");
    let store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open artifact state");
    let (manifest, _) = fixture("manifest-only", "set-root-01");
    store
        .put_artifact_set_manifest(&manifest)
        .expect("register set manifest");

    let states = store
        .artifact_set_inventory(1)
        .expect("list manifest-only set");
    assert_eq!(states.len(), 1);
    assert_eq!(states[0].manifest, manifest);
    assert!(states[0].installed.is_none());
}

#[test]
fn rejects_invalid_and_exceeded_set_inventory_limits() {
    let directory = tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open artifact state");
    let (first, first_installed) = fixture("first-set", "set-root-01");
    let (second, second_installed) = fixture("second-set", "set-root-02");
    store
        .put_artifact_set_installation(&first, &first_installed)
        .expect("register first set");
    store
        .put_artifact_set_installation(&second, &second_installed)
        .expect("register second set");

    assert!(matches!(
        store.artifact_set_inventory(0),
        Err(StoreError::InvalidLimit)
    ));
    assert!(matches!(
        store.artifact_set_inventory(1),
        Err(StoreError::InventoryLimitExceeded)
    ));
}

#[test]
fn rejects_installed_set_without_a_manifest() {
    let directory = tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open artifact state");
    let (manifest, installed) = fixture("orphan-install", "set-root-01");
    store
        .put_artifact_set_installation(&manifest, &installed)
        .expect("register set");
    store
        .connection
        .execute_batch("PRAGMA foreign_keys = OFF;")
        .expect("disable test foreign keys");
    store
        .connection
        .execute(
            "DELETE FROM artifact_set_manifests WHERE artifact_set_id = ?1",
            [manifest.artifact_set_id().digest().as_str()],
        )
        .expect("drop set manifest");
    store
        .connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .expect("restore foreign keys");

    assert!(matches!(
        store.artifact_set_inventory(1),
        Err(StoreError::MissingRecord | StoreError::CorruptRecord)
    ));
}
