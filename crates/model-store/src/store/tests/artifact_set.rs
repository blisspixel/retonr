use tempfile::tempdir;

use rewrite_model::{
    ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath,
    InstalledArtifactSet,
};
use rewrite_types::Digest;

use super::ArtifactStateStore;
use crate::{
    ArtifactSetInstallationEpoch, ArtifactSetInstallationWriteDisposition, StoreError,
    StoredArtifactSetInstallation, WriteDisposition, artifact_set_installation::encode_for_test,
};

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

fn first_installation(installed: &InstalledArtifactSet) -> StoredArtifactSetInstallation {
    StoredArtifactSetInstallation {
        installed: installed.clone(),
        epoch: ArtifactSetInstallationEpoch::for_test(1).expect("first epoch"),
    }
}

#[test]
fn atomically_persists_recovers_and_idempotently_selects_exact_state() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("state.db");
    let (manifest, installed) = fixture("weights", "set-root-01");
    let expected = first_installation(&installed);
    {
        let mut store = ArtifactStateStore::open(&path).expect("open store");
        assert_eq!(
            store
                .put_artifact_set_installation(&manifest, &installed)
                .expect("store installation"),
            ArtifactSetInstallationWriteDisposition {
                manifest: WriteDisposition::Inserted,
                installed: WriteDisposition::Inserted,
                installation: expected.clone(),
            }
        );
        assert_eq!(
            store
                .put_artifact_set_installation(&manifest, &installed)
                .expect("repeat exact installation"),
            ArtifactSetInstallationWriteDisposition {
                manifest: WriteDisposition::AlreadyPresent,
                installed: WriteDisposition::AlreadyPresent,
                installation: expected.clone(),
            }
        );
    }
    let store = ArtifactStateStore::open(&path).expect("reopen store");
    assert_eq!(
        store
            .artifact_set_installation(&manifest.artifact_set_id())
            .expect("load installation"),
        Some(expected)
    );
}

#[test]
fn joins_an_existing_manifest_without_rewriting_it() {
    let directory = tempdir().expect("temporary directory");
    let (manifest, installed) = fixture("weights", "set-root-01");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    assert_eq!(
        store
            .put_artifact_set_manifest(&manifest)
            .expect("store manifest"),
        WriteDisposition::Inserted
    );
    let result = store
        .put_artifact_set_installation(&manifest, &installed)
        .expect("join installation");
    assert_eq!(result.manifest, WriteDisposition::AlreadyPresent);
    assert_eq!(result.installed, WriteDisposition::Inserted);
    assert_eq!(result.installation, first_installation(&installed));
}

#[test]
fn conflicting_set_root_fails_without_replacing_current_state() {
    let directory = tempdir().expect("temporary directory");
    let (manifest, installed) = fixture("weights", "set-root-01");
    let replacement =
        InstalledArtifactSet::new(&manifest, "set-root-02").expect("valid replacement");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    store
        .put_artifact_set_installation(&manifest, &installed)
        .expect("store installation");
    assert!(matches!(
        store.put_artifact_set_installation(&manifest, &replacement),
        Err(StoreError::ImmutableConflict)
    ));
    assert_eq!(
        store
            .artifact_set_installation(&manifest.artifact_set_id())
            .expect("load retained installation"),
        Some(first_installation(&installed))
    );
}

#[test]
fn one_set_root_cannot_be_claimed_by_distinct_manifest_identities() {
    let directory = tempdir().expect("temporary directory");
    let (manifest, installed) = fixture("weights", "shared-set-root");
    let (other_manifest, other_installed) = fixture("other", "shared-set-root");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    store
        .put_artifact_set_installation(&manifest, &installed)
        .expect("store first owner");
    assert!(matches!(
        store.put_artifact_set_installation(&other_manifest, &other_installed),
        Err(StoreError::ImmutableConflict)
    ));
    assert_eq!(
        store
            .artifact_set_manifest(&other_manifest.artifact_set_id())
            .expect("other manifest remains absent"),
        None
    );
    assert_eq!(
        store
            .artifact_set_installation(&manifest.artifact_set_id())
            .expect("first owner remains current"),
        Some(first_installation(&installed))
    );
}

#[test]
fn malformed_storage_owner_is_corruption_not_a_normal_conflict() {
    let directory = tempdir().expect("temporary directory");
    let (manifest, installed) = fixture("weights", "shared-set-root");
    let (other_manifest, other_installed) = fixture("other", "shared-set-root");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    store
        .put_artifact_set_installation(&manifest, &installed)
        .expect("store first owner");
    store
        .connection
        .execute(
            "UPDATE installed_artifact_sets SET record_json = 'not-json'
             WHERE artifact_set_id = ?1",
            [manifest.artifact_set_id().digest().as_str()],
        )
        .expect("corrupt first owner");
    assert!(matches!(
        store.put_artifact_set_installation(&other_manifest, &other_installed),
        Err(StoreError::CorruptRecord)
    ));
    assert_eq!(
        store
            .artifact_set_manifest(&other_manifest.artifact_set_id())
            .expect("other manifest remains absent"),
        None
    );
}

#[test]
fn noncanonical_portable_alias_owner_blocks_a_new_claim_as_corruption() {
    let directory = tempdir().expect("temporary directory");
    let (manifest, installed) = fixture("weights", "shared-set-root");
    let (other_manifest, other_installed) = fixture("other", "shared-set-root");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    store
        .put_artifact_set_installation(&manifest, &installed)
        .expect("store first owner");
    store
        .connection
        .execute(
            "UPDATE installed_artifact_sets SET storage_key = 'SHARED-SET-ROOT.'
             WHERE artifact_set_id = ?1",
            [manifest.artifact_set_id().digest().as_str()],
        )
        .expect("corrupt first owner key");
    assert!(matches!(
        store.put_artifact_set_installation(&other_manifest, &other_installed),
        Err(StoreError::CorruptRecord)
    ));
    assert_eq!(
        store
            .artifact_set_manifest(&other_manifest.artifact_set_id())
            .expect("other manifest remains absent"),
        None
    );
}

#[test]
fn multiple_normalized_root_owners_block_an_exact_retry_as_corruption() {
    let directory = tempdir().expect("temporary directory");
    let (manifest, installed) = fixture("weights", "shared-set-root");
    let (other_manifest, other_installed) = fixture("other", "other-set-root");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    store
        .put_artifact_set_installation(&manifest, &installed)
        .expect("store canonical owner");
    store
        .put_artifact_set_manifest(&other_manifest)
        .expect("store alias manifest");
    let alias_record = encode_for_test(
        other_installed,
        ArtifactSetInstallationEpoch::for_test(1).expect("first epoch"),
    )
    .expect("encode alias fixture");
    store
        .connection
        .execute(
            "INSERT INTO installed_artifact_sets
                 (artifact_set_id, storage_key, installation_epoch, record_json)
             VALUES (?1, 'SHARED-SET-ROOT.', 1, ?2)",
            rusqlite::params![
                other_manifest.artifact_set_id().digest().as_str(),
                alias_record
            ],
        )
        .expect("insert raw portable alias");

    assert!(matches!(
        store.put_artifact_set_installation(&manifest, &installed),
        Err(StoreError::CorruptRecord)
    ));
}

#[test]
fn manifest_insert_rolls_back_when_installation_insert_fails() {
    let directory = tempdir().expect("temporary directory");
    let (manifest, installed) = fixture("weights", "set-root-01");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    store
        .connection
        .execute_batch(
            "CREATE TRIGGER reject_set_install
             BEFORE INSERT ON installed_artifact_sets
             BEGIN
                 SELECT RAISE(ABORT, 'fixture rejection');
             END;",
        )
        .expect("create rejection trigger");
    assert!(matches!(
        store.put_artifact_set_installation(&manifest, &installed),
        Err(StoreError::Database(_))
    ));
    assert_eq!(
        store
            .artifact_set_manifest(&manifest.artifact_set_id())
            .expect("check rollback"),
        None
    );
}

#[test]
fn post_insert_drift_is_detected_and_the_transaction_rolls_back() {
    let directory = tempdir().expect("temporary directory");
    let (manifest, installed) = fixture("weights", "set-root-01");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    store
        .connection
        .execute_batch(
            "CREATE TRIGGER corrupt_set_install
             AFTER INSERT ON installed_artifact_sets
             BEGIN
                 UPDATE installed_artifact_sets SET record_json = 'not-json'
                 WHERE artifact_set_id = NEW.artifact_set_id;
             END;",
        )
        .expect("create corruption trigger");
    assert!(matches!(
        store.put_artifact_set_installation(&manifest, &installed),
        Err(StoreError::CorruptRecord)
    ));
    assert_eq!(
        store
            .artifact_set_manifest(&manifest.artifact_set_id())
            .expect("manifest insert rolled back"),
        None
    );
    assert_eq!(
        store
            .artifact_set_installation(&manifest.artifact_set_id())
            .expect("installation insert rolled back"),
        None
    );
}

#[test]
fn mismatched_input_fails_before_durable_state() {
    let directory = tempdir().expect("temporary directory");
    let (manifest, _) = fixture("weights", "set-root-01");
    let (other_manifest, other_installed) = fixture("other", "set-root-02");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    assert!(matches!(
        store.put_artifact_set_installation(&manifest, &other_installed),
        Err(StoreError::InvalidArtifactSetInstallation(_))
    ));
    assert_eq!(
        store
            .artifact_set_manifest(&manifest.artifact_set_id())
            .expect("manifest remains absent"),
        None
    );
    assert_eq!(
        store
            .artifact_set_manifest(&other_manifest.artifact_set_id())
            .expect("other manifest remains absent"),
        None
    );
}

#[test]
fn malformed_noncanonical_and_epoch_drift_fail_closed() {
    for (label, mutation) in [
        (
            "malformed",
            "UPDATE installed_artifact_sets SET record_json = 'not-json'",
        ),
        (
            "noncanonical",
            "UPDATE installed_artifact_sets SET record_json = ' ' || record_json",
        ),
        (
            "epoch",
            "UPDATE installed_artifact_sets SET installation_epoch = 2",
        ),
        (
            "storage-index",
            "UPDATE installed_artifact_sets SET storage_key = 'different-root'",
        ),
    ] {
        let directory = tempdir().expect("temporary directory");
        let (manifest, installed) = fixture(label, "set-root-01");
        let mut store =
            ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
        store
            .put_artifact_set_installation(&manifest, &installed)
            .expect("store installation");
        store.connection.execute(mutation, []).expect("corrupt row");
        assert!(matches!(
            store.artifact_set_installation(&manifest.artifact_set_id()),
            Err(StoreError::CorruptRecord)
        ));
        assert!(matches!(
            store.put_artifact_set_installation(&manifest, &installed),
            Err(StoreError::CorruptRecord)
        ));
    }
}

#[test]
fn cross_set_record_and_dangling_manifest_fail_closed() {
    let directory = tempdir().expect("temporary directory");
    let (manifest, installed) = fixture("weights", "set-root-01");
    let (other_manifest, other_installed) = fixture("other", "set-root-02");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    store
        .put_artifact_set_installation(&manifest, &installed)
        .expect("store installation");
    store
        .put_artifact_set_manifest(&other_manifest)
        .expect("store other manifest");
    let cross_set = encode_for_test(
        other_installed,
        ArtifactSetInstallationEpoch::for_test(1).expect("first epoch"),
    )
    .expect("encode cross-set fixture");
    store
        .connection
        .execute(
            "UPDATE installed_artifact_sets SET record_json = ?1 WHERE artifact_set_id = ?2",
            rusqlite::params![cross_set, manifest.artifact_set_id().digest().as_str()],
        )
        .expect("install cross-set record");
    assert!(matches!(
        store.artifact_set_installation(&manifest.artifact_set_id()),
        Err(StoreError::CorruptRecord)
    ));

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
        .expect("remove required manifest");
    store
        .connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .expect("restore foreign keys");
    assert!(matches!(
        store.artifact_set_installation(&manifest.artifact_set_id()),
        Err(StoreError::CorruptRecord)
    ));
}

#[test]
fn absent_installation_returns_none_and_epoch_bounds_fail_closed() {
    let directory = tempdir().expect("temporary directory");
    let manifest = manifest("weights");
    let store = ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    assert_eq!(
        store
            .artifact_set_installation(&manifest.artifact_set_id())
            .expect("query absent installation"),
        None
    );
    assert!(matches!(
        ArtifactSetInstallationEpoch::for_test(0),
        Err(StoreError::CorruptRecord)
    ));
    assert!(matches!(
        ArtifactSetInstallationEpoch::for_test(u64::MAX),
        Err(StoreError::InstallationEpochExhausted)
    ));
}
