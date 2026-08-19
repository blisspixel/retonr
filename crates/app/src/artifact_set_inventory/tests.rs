use std::fs;

use rewrite_model::{
    ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath,
    InstalledArtifactSet,
};
use rewrite_model_store::ArtifactStateStore;
use rewrite_types::{CancellationToken, Digest};
use tempfile::{TempDir, tempdir};

use super::{
    ArtifactSetInventoryError, ArtifactSetInventoryLimits, ArtifactSetInventoryService,
    ArtifactSetInventoryStage, RegisteredArtifactSetBytes,
};
use crate::{
    ArtifactImportLimits, ArtifactSetImportLimits, OfflineArtifactImportRequest,
    OfflineArtifactImportService, OfflineArtifactSetImportRequest,
    artifact_set_import::{
        OfflineArtifactSetImportService, SET_STORAGE_KEY_PREFIX, SETS_DIRECTORY,
    },
};

fn inventory_limits() -> ArtifactSetInventoryLimits {
    ArtifactSetInventoryLimits {
        maximum_state_entries: 32,
        maximum_storage_entries: 32,
        maximum_members: 16,
        maximum_member_bytes: 1_024,
        maximum_tree_entries: 32,
        maximum_total_verification_bytes: 8_192,
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

fn sets(directory: &TempDir) -> std::path::PathBuf {
    storage(directory).join(SETS_DIRECTORY)
}

fn set_root(directory: &TempDir, manifest: &ArtifactSetManifest) -> std::path::PathBuf {
    sets(directory).join(format!(
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

fn inventory(
    directory: &TempDir,
    store: &ArtifactStateStore,
    limits: ArtifactSetInventoryLimits,
) -> Result<super::ArtifactSetInventoryReport, ArtifactSetInventoryError> {
    ArtifactSetInventoryService::open(storage(directory), store, limits)?
        .inventory(&CancellationToken::new(), |_| {})
}

#[test]
fn rejects_invalid_limits_and_uninitialized_storage() {
    let directory = tempdir().expect("temporary directory");
    let store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open artifact state");
    let mut invalid = inventory_limits();
    invalid.maximum_state_entries = 0;
    assert!(matches!(
        ArtifactSetInventoryService::open(directory.path().join("missing"), &store, invalid),
        Err(ArtifactSetInventoryError::InvalidLimits)
    ));
    assert!(matches!(
        ArtifactSetInventoryService::open(
            directory.path().join("missing"),
            &store,
            inventory_limits()
        ),
        Err(ArtifactSetInventoryError::StorageNotInitialized)
    ));
}

#[test]
fn empty_set_inventory_after_single_file_init_is_read_only() {
    let (directory, store) = initialized();
    let stale = storage(&directory).join(".staging/.import-stale");
    fs::write(&stale, b"stale").expect("write staging fixture");
    let artifact = storage(&directory).join("artifacts").join("not-a-set");
    fs::write(&artifact, b"single-file-only").expect("write single-file fixture");

    let report = inventory(&directory, &store, inventory_limits()).expect("empty set inventory");

    assert!(report.registered.is_empty());
    assert_eq!(report.storage_entry_count, 0);
    assert_eq!(report.verified_bytes, 0);
    assert!(stale.is_file());
    assert_eq!(
        fs::read(artifact).expect("single-file bytes remain"),
        b"single-file-only"
    );
}

#[test]
fn reports_a_verified_imported_set_without_granting_authority() {
    let (directory, mut store) = initialized();
    let value = import_set(&directory, &mut store, "imported-set");

    let report = inventory(&directory, &store, inventory_limits()).expect("inventory imported set");

    assert_eq!(report.registered.len(), 1);
    assert_eq!(
        report.registered[0].manifest.artifact_set_id(),
        value.artifact_set_id()
    );
    assert_eq!(
        report.registered[0].installation.installation_generation(),
        1
    );
    assert_eq!(
        report.registered[0].bytes,
        RegisteredArtifactSetBytes::Verified
    );
    assert_eq!(report.verified_bytes, value.total_byte_size());
    assert!(report.manifest_only.is_empty());
    assert!(report.verified_orphans.is_empty());
}

#[test]
fn classifies_missing_digest_layout_and_tree_conflicts() {
    let (directory, mut store) = initialized();
    let missing = manifest("missing-set");
    store
        .put_artifact_set_installation(
            &missing,
            &InstalledArtifactSet::new(
                &missing,
                format!(
                    "{SET_STORAGE_KEY_PREFIX}{}",
                    missing.artifact_set_id().digest().as_str()
                ),
            )
            .expect("canonical installed set"),
        )
        .expect("register missing set");

    let digest = import_set(&directory, &mut store, "digest-set");
    fs::write(
        set_root(&directory, &digest).join("model/weights.bin"),
        b"DIGEST-set",
    )
    .expect("corrupt member digest");

    let extra = import_set(&directory, &mut store, "tree-set");
    fs::write(set_root(&directory, &extra).join("extra.bin"), b"extra").expect("add extra member");

    let layout = manifest("layout-set");
    store
        .put_artifact_set_installation(
            &layout,
            &InstalledArtifactSet::new(&layout, "set-root-noncanonical")
                .expect("noncanonical installed set"),
        )
        .expect("register layout conflict");

    let report =
        inventory(&directory, &store, inventory_limits()).expect("classify registered sets");

    assert_eq!(
        status(&report, &missing.artifact_set_id()),
        &RegisteredArtifactSetBytes::Missing
    );
    assert_eq!(
        status(&report, &digest.artifact_set_id()),
        &RegisteredArtifactSetBytes::MemberDigestConflict
    );
    assert_eq!(
        status(&report, &extra.artifact_set_id()),
        &RegisteredArtifactSetBytes::TreeMismatch
    );
    assert_eq!(
        status(&report, &layout.artifact_set_id()),
        &RegisteredArtifactSetBytes::StateLayoutConflict
    );
}

#[test]
fn reports_manifest_only_verified_orphan_and_unregistered_root() {
    let (directory, mut store) = initialized();
    let orphan = import_set(&directory, &mut store, "orphan-set");
    rusqlite::Connection::open(directory.path().join("state.db"))
        .expect("open state fixture")
        .execute(
            "DELETE FROM installed_artifact_sets WHERE artifact_set_id = ?1",
            [orphan.artifact_set_id().digest().as_str()],
        )
        .expect("drop installed set record");

    let only = manifest("manifest-only");
    store
        .put_artifact_set_manifest(&only)
        .expect("register manifest-only set");

    fs::create_dir_all(sets(&directory).join(format!(
        "{SET_STORAGE_KEY_PREFIX}{}",
        Digest::sha256(b"unregistered-root").as_str()
    )))
    .expect("create unregistered set root");
    fs::write(sets(&directory).join("not-a-set-root"), b"private").expect("malformed name");

    let report =
        inventory(&directory, &store, inventory_limits()).expect("classify uninstalled sets");

    assert_eq!(report.manifest_only, vec![only]);
    assert_eq!(report.verified_orphans.len(), 1);
    assert_eq!(
        report.verified_orphans[0].artifact_set_id,
        orphan.artifact_set_id()
    );
    assert_eq!(report.unexpected_entries.unregistered_roots, 1);
    assert_eq!(report.unexpected_entries.malformed_names, 1);
    assert!(report.registered.is_empty());
}

#[test]
fn applies_state_storage_and_total_hash_limits() {
    let (directory, mut store) = initialized();
    let first = import_set(&directory, &mut store, "first-set");
    let second = import_set(&directory, &mut store, "second-set");

    let mut state_limit = inventory_limits();
    state_limit.maximum_state_entries = 1;
    assert!(matches!(
        inventory(&directory, &store, state_limit),
        Err(ArtifactSetInventoryError::StateEntryLimitExceeded)
    ));
    let mut storage_limit = inventory_limits();
    storage_limit.maximum_storage_entries = 1;
    assert!(matches!(
        inventory(&directory, &store, storage_limit),
        Err(ArtifactSetInventoryError::StorageEntryLimitExceeded)
    ));
    let mut total_limit = inventory_limits();
    total_limit.maximum_total_verification_bytes =
        first.total_byte_size() + second.total_byte_size() - 1;
    assert!(matches!(
        inventory(&directory, &store, total_limit),
        Err(ArtifactSetInventoryError::TotalVerificationLimitExceeded)
    ));
    let mut member_limit = inventory_limits();
    member_limit.maximum_member_bytes = 1;
    let report = inventory(&directory, &store, member_limit).expect("classify oversized sets");
    assert!(report.registered.iter().all(|item| matches!(
        item.bytes,
        RegisteredArtifactSetBytes::TooLargeToVerify { .. }
    )));
}

#[test]
fn observes_cancellation_before_and_during_set_inventory() {
    let (directory, store) = initialized();
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    let service =
        ArtifactSetInventoryService::open(storage(&directory), &store, inventory_limits())
            .expect("open set inventory");
    assert!(matches!(
        service.inventory(&cancelled, |_| {}),
        Err(ArtifactSetInventoryError::Cancelled)
    ));

    let during = CancellationToken::new();
    let signal = during.clone();
    assert!(matches!(
        service.inventory(&during, |item| {
            if item.stage == ArtifactSetInventoryStage::LoadingState {
                signal.cancel();
            }
        }),
        Err(ArtifactSetInventoryError::Cancelled)
    ));
}

fn status<'a>(
    report: &'a super::ArtifactSetInventoryReport,
    artifact_set_id: &rewrite_model::ArtifactSetId,
) -> &'a RegisteredArtifactSetBytes {
    &report
        .registered
        .iter()
        .find(|item| item.manifest.artifact_set_id() == *artifact_set_id)
        .expect("registered fixture is present")
        .bytes
}

#[test]
fn single_file_import_bytes_are_ignored() {
    let (directory, mut store) = initialized();
    let _set = import_set(&directory, &mut store, "keep-set");
    let source = directory.path().join("file.bin");
    fs::write(&source, b"single-file-bytes").expect("write single-file source");
    let digest = Digest::sha256(b"single-file-bytes");
    let file_manifest = {
        use rewrite_model::{
            ARTIFACT_MANIFEST_SCHEMA_VERSION, ArtifactManifest, ArtifactRole, ArtifactSource,
            DeclaredCapabilities, LicenseRecord,
        };
        ArtifactManifest {
            schema_version: ARTIFACT_MANIFEST_SCHEMA_VERSION,
            artifact_id: ArtifactId::from_digest(digest.clone()),
            source: ArtifactSource {
                origin: "fixture/file".to_owned(),
                revision: "fixture".to_owned(),
            },
            artifact_digest: digest,
            byte_size: 17,
            format: "gguf".to_owned(),
            family: "fixture".to_owned(),
            architecture: None,
            quantization: None,
            tokenizer: None,
            licenses: vec![LicenseRecord {
                component: "weights".to_owned(),
                identifier: "Apache-2.0".to_owned(),
                text_digest: Digest::sha256(b"license"),
            }],
            declared_capabilities: DeclaredCapabilities {
                roles: vec![ArtifactRole::Generation],
                languages: vec!["en".to_owned()],
                context_tokens: None,
            },
        }
    };
    let mut service = OfflineArtifactImportService::open(
        storage(&directory),
        &mut store,
        ArtifactImportLimits {
            maximum_artifact_bytes: 4_096,
            maximum_storage_entries: 32,
        },
    )
    .expect("open file import");
    service
        .import(
            &OfflineArtifactImportRequest {
                source,
                manifest: file_manifest,
            },
            &CancellationToken::new(),
            |_| {},
        )
        .expect("import single file");
    drop(service);

    let report =
        inventory(&directory, &store, inventory_limits()).expect("ignore single-file bytes");
    assert_eq!(report.registered.len(), 1);
    assert_eq!(report.storage_entry_count, 1);
    assert_eq!(report.unexpected_entries.malformed_names, 0);
}
