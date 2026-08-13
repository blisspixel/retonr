use std::fs;

use rewrite_model::{
    ARTIFACT_MANIFEST_SCHEMA_VERSION, ArtifactId, ArtifactManifest, ArtifactRole, ArtifactSource,
    DeclaredCapabilities, InstalledArtifact, LicenseRecord,
};
use rewrite_model_store::ArtifactStateStore;
use rewrite_types::{CancellationToken, Digest};
use tempfile::{TempDir, tempdir};

use super::{
    ArtifactInventoryError, ArtifactInventoryLimits, ArtifactInventoryService,
    ArtifactInventoryStage, OrphanManifestAssociation, RegisteredArtifactBytes,
};
use crate::{ArtifactImportLimits, OfflineArtifactImportRequest, OfflineArtifactImportService};

mod concurrency;
mod progress;

fn limits() -> ArtifactInventoryLimits {
    ArtifactInventoryLimits {
        maximum_state_entries: 32,
        maximum_storage_entries: 32,
        maximum_artifact_bytes: 1_024,
        maximum_total_verification_bytes: 8_192,
    }
}

fn manifest(bytes: &[u8], label: &str) -> ArtifactManifest {
    let digest = Digest::sha256(bytes);
    ArtifactManifest {
        schema_version: ARTIFACT_MANIFEST_SCHEMA_VERSION,
        artifact_id: ArtifactId::from_digest(digest.clone()),
        source: ArtifactSource {
            origin: format!("fixture/{label}"),
            revision: "sha256:fixture".to_owned(),
        },
        artifact_digest: digest,
        byte_size: u64::try_from(bytes.len()).expect("fixture size is representable"),
        format: "gguf".to_owned(),
        family: "fixture".to_owned(),
        architecture: Some("transformer".to_owned()),
        quantization: Some("q4".to_owned()),
        tokenizer: None,
        licenses: vec![LicenseRecord {
            component: "weights".to_owned(),
            identifier: "Apache-2.0".to_owned(),
            text_digest: Digest::sha256(b"license"),
        }],
        declared_capabilities: DeclaredCapabilities {
            roles: vec![ArtifactRole::Generation],
            languages: vec!["en".to_owned()],
            context_tokens: Some(8_192),
        },
    }
}

fn installed(manifest: &ArtifactManifest) -> InstalledArtifact {
    InstalledArtifact {
        artifact_id: manifest.artifact_id.clone(),
        artifact_digest: manifest.artifact_digest.clone(),
        byte_size: manifest.byte_size,
        storage_key: format!("artifacts/{}", manifest.artifact_digest.as_str()),
    }
}

fn initialized() -> (TempDir, ArtifactStateStore) {
    let directory = tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open artifact state");
    {
        let _service = OfflineArtifactImportService::open(
            directory.path().join("storage"),
            &mut store,
            ArtifactImportLimits {
                maximum_artifact_bytes: 4_096,
            },
        )
        .expect("initialize artifact storage");
    }
    (directory, store)
}

fn storage(directory: &TempDir) -> std::path::PathBuf {
    directory.path().join("storage")
}

fn artifacts(directory: &TempDir) -> std::path::PathBuf {
    storage(directory).join("artifacts")
}

fn write_artifact(directory: &TempDir, digest: &Digest, bytes: &[u8]) {
    fs::write(artifacts(directory).join(digest.as_str()), bytes).expect("write artifact fixture");
}

fn register(store: &mut ArtifactStateStore, value: &ArtifactManifest) {
    store
        .put_installation(value, &installed(value))
        .expect("register artifact fixture");
}

fn import_bytes(
    directory: &TempDir,
    store: &mut ArtifactStateStore,
    bytes: &[u8],
) -> ArtifactManifest {
    let value = manifest(bytes, "imported");
    let source = directory.path().join("source.bin");
    fs::write(&source, bytes).expect("write source fixture");
    let mut service = OfflineArtifactImportService::open(
        storage(directory),
        store,
        ArtifactImportLimits {
            maximum_artifact_bytes: 4_096,
        },
    )
    .expect("open import service");
    service
        .import(
            &OfflineArtifactImportRequest {
                source,
                manifest: value.clone(),
            },
            &CancellationToken::new(),
            |_| {},
        )
        .expect("import fixture");
    value
}

fn inventory(
    directory: &TempDir,
    store: &ArtifactStateStore,
    inventory_limits: ArtifactInventoryLimits,
) -> Result<super::ArtifactInventoryReport, ArtifactInventoryError> {
    ArtifactInventoryService::open(storage(directory), store, inventory_limits)?
        .inventory(&CancellationToken::new(), |_| {})
}

#[test]
fn rejects_invalid_limits_and_uninitialized_storage() {
    let directory = tempdir().expect("temporary directory");
    let store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open artifact state");
    let mut invalid = limits();
    invalid.maximum_state_entries = 0;
    assert!(matches!(
        ArtifactInventoryService::open(directory.path().join("missing"), &store, invalid),
        Err(ArtifactInventoryError::InvalidLimits)
    ));
    assert!(matches!(
        ArtifactInventoryService::open(directory.path().join("missing"), &store, limits()),
        Err(ArtifactInventoryError::StorageNotInitialized)
    ));

    #[cfg(target_pointer_width = "64")]
    {
        let mut invalid = limits();
        invalid.maximum_state_entries =
            usize::try_from(i64::MAX).expect("64-bit usize represents i64 maximum") + 1;
        assert!(matches!(
            ArtifactInventoryService::open(directory.path().join("missing"), &store, invalid),
            Err(ArtifactInventoryError::InvalidLimits)
        ));
    }
}

#[test]
fn empty_inventory_is_read_only_and_does_not_clean_staging() {
    let (directory, store) = initialized();
    let stale = storage(&directory).join(".staging/.import-stale");
    fs::write(&stale, b"stale").expect("write staging fixture");

    let report = inventory(&directory, &store, limits()).expect("inventory empty storage");

    assert!(report.registered.is_empty());
    assert_eq!(report.storage_entry_count, 0);
    assert_eq!(report.verified_bytes, 0);
    assert!(stale.is_file());
}

#[test]
fn reports_registered_missing_size_digest_and_layout_conflicts() {
    let (directory, mut store) = initialized();
    let missing = manifest(b"missing", "missing");
    register(&mut store, &missing);

    let size = manifest(b"expected-size", "size");
    register(&mut store, &size);
    write_artifact(&directory, &size.artifact_digest, b"short");

    let digest = manifest(b"expected-digest", "digest");
    register(&mut store, &digest);
    write_artifact(&directory, &digest.artifact_digest, b"changed-digest!");

    let layout = manifest(b"layout", "layout");
    let mut wrong_layout = installed(&layout);
    wrong_layout.storage_key = format!("models/{}", layout.artifact_digest.as_str());
    store
        .put_installation(&layout, &wrong_layout)
        .expect("register noncanonical application layout");
    write_artifact(&directory, &layout.artifact_digest, b"layout");

    let report = inventory(&directory, &store, limits()).expect("classify registered fixtures");

    assert_eq!(
        status(&report, &missing.artifact_id),
        &RegisteredArtifactBytes::Missing
    );
    assert!(matches!(
        status(&report, &size.artifact_id),
        RegisteredArtifactBytes::SizeConflict { observed_bytes: 5 }
    ));
    assert!(matches!(
        status(&report, &digest.artifact_id),
        RegisteredArtifactBytes::DigestConflict { .. }
    ));
    assert_eq!(
        status(&report, &layout.artifact_id),
        &RegisteredArtifactBytes::StateLayoutConflict
    );
    assert!(report.verified_orphans.iter().any(|item| {
        item.artifact_id == layout.artifact_id
            && matches!(
                &item.manifest,
                OrphanManifestAssociation::MatchingManifest(manifest) if manifest == &layout
            )
    }));
}

fn status<'a>(
    report: &'a super::ArtifactInventoryReport,
    artifact_id: &ArtifactId,
) -> &'a RegisteredArtifactBytes {
    &report
        .registered
        .iter()
        .find(|item| &item.manifest.artifact_id == artifact_id)
        .expect("registered fixture is present")
        .bytes
}

#[test]
fn classifies_uninstalled_entries_without_disclosing_malformed_names() {
    let (directory, store) = initialized();
    fs::write(artifacts(&directory).join("not-a-digest"), b"private name")
        .expect("write malformed entry");
    let empty = Digest::sha256(b"empty-name-fixture");
    fs::write(artifacts(&directory).join(empty.as_str()), b"").expect("write empty fixture");
    let directory_name = Digest::sha256(b"directory-name-fixture");
    fs::create_dir(artifacts(&directory).join(directory_name.as_str()))
        .expect("create nonregular fixture");
    let conflict_name = Digest::sha256(b"claimed");
    fs::write(
        artifacts(&directory).join(conflict_name.as_str()),
        b"changed",
    )
    .expect("write content conflict");
    let oversized = manifest(b"oversized", "oversized");
    write_artifact(&directory, &oversized.artifact_digest, b"oversized");
    let mut inventory_limits = limits();
    inventory_limits.maximum_artifact_bytes = 8;

    let report =
        inventory(&directory, &store, inventory_limits).expect("classify uninstalled fixtures");

    assert_eq!(report.unexpected_entries.malformed_names, 1);
    assert_eq!(report.unexpected_entries.empty_files, 1);
    assert_eq!(report.unexpected_entries.non_regular_entries, 1);
    assert_eq!(report.content_address_conflicts.len(), 1);
    assert_eq!(report.oversized_files.len(), 1);
    assert!(report.verified_orphans.is_empty());
}

#[test]
fn exact_case_is_required_even_on_case_insensitive_storage() {
    let (directory, mut store) = initialized();
    let value = manifest(b"case fixture", "case");
    register(&mut store, &value);
    fs::write(
        artifacts(&directory).join(value.artifact_digest.as_str().to_ascii_uppercase()),
        b"case fixture",
    )
    .expect("write uppercase fixture");

    let report = inventory(&directory, &store, limits()).expect("inspect case fixture");

    assert_eq!(
        status(&report, &value.artifact_id),
        &RegisteredArtifactBytes::Missing
    );
    assert_eq!(report.unexpected_entries.malformed_names, 1);
}

#[test]
fn applies_single_file_state_storage_and_total_hash_limits() {
    let (directory, mut store) = initialized();
    let first = manifest(b"first artifact", "first");
    let second = manifest(b"second artifact", "second");
    register(&mut store, &first);
    register(&mut store, &second);
    write_artifact(&directory, &first.artifact_digest, b"first artifact");
    write_artifact(&directory, &second.artifact_digest, b"second artifact");

    let mut state_limit = limits();
    state_limit.maximum_state_entries = 1;
    assert!(matches!(
        inventory(&directory, &store, state_limit),
        Err(ArtifactInventoryError::StateEntryLimitExceeded)
    ));
    let mut storage_limit = limits();
    storage_limit.maximum_storage_entries = 1;
    assert!(matches!(
        inventory(&directory, &store, storage_limit),
        Err(ArtifactInventoryError::StorageEntryLimitExceeded)
    ));
    let mut total_limit = limits();
    total_limit.maximum_total_verification_bytes = first.byte_size;
    assert!(matches!(
        inventory(&directory, &store, total_limit),
        Err(ArtifactInventoryError::TotalVerificationLimitExceeded)
    ));
    let mut file_limit = limits();
    file_limit.maximum_artifact_bytes = first.byte_size - 1;
    let report = inventory(&directory, &store, file_limit).expect("classify oversized registered");
    assert!(
        report
            .registered
            .iter()
            .all(|item| matches!(item.bytes, RegisteredArtifactBytes::TooLargeToVerify { .. }))
    );
}

#[test]
fn observes_cancellation_before_and_during_inventory() {
    let (directory, store) = initialized();
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    let service = ArtifactInventoryService::open(storage(&directory), &store, limits())
        .expect("open inventory");
    assert!(matches!(
        service.inventory(&cancelled, |_| {}),
        Err(ArtifactInventoryError::Cancelled)
    ));

    let during = CancellationToken::new();
    let signal = during.clone();
    assert!(matches!(
        service.inventory(&during, |item| {
            if item.stage == ArtifactInventoryStage::LoadingState {
                signal.cancel();
            }
        }),
        Err(ArtifactInventoryError::Cancelled)
    ));

    let during_snapshot = CancellationToken::new();
    let signal = during_snapshot.clone();
    assert!(matches!(
        service.inventory(&during_snapshot, |item| {
            if item.stage == ArtifactInventoryStage::FreezingStorage {
                signal.cancel();
            }
        }),
        Err(ArtifactInventoryError::Cancelled)
    ));
}

#[test]
fn reports_registered_nonregular_entry_as_unsafe() {
    let (directory, mut store) = initialized();
    let value = manifest(b"unsafe registered", "unsafe");
    register(&mut store, &value);
    fs::create_dir(artifacts(&directory).join(value.artifact_digest.as_str()))
        .expect("create nonregular registered fixture");

    let report = inventory(&directory, &store, limits()).expect("inspect unsafe fixture");

    assert_eq!(
        status(&report, &value.artifact_id),
        &RegisteredArtifactBytes::UnsafeEntry
    );
}

#[cfg(all(unix, not(target_os = "macos")))]
#[test]
fn malformed_non_utf8_name_remains_aggregated() {
    use std::os::unix::ffi::OsStringExt as _;

    let (directory, store) = initialized();
    let name = std::ffi::OsString::from_vec(vec![0xFF, b'x']);
    fs::write(artifacts(&directory).join(name), b"private").expect("write non-UTF-8 fixture");

    let report = inventory(&directory, &store, limits()).expect("inspect malformed raw name");

    assert_eq!(report.unexpected_entries.malformed_names, 1);
}

#[cfg(unix)]
#[test]
fn reports_uninstalled_symlink_without_following_it() {
    use std::os::unix::fs::symlink;

    let (directory, mut store) = initialized();
    let target = directory.path().join("outside.bin");
    fs::write(&target, b"outside").expect("write symlink target");
    let name = Digest::sha256(b"symlink-name");
    symlink(&target, artifacts(&directory).join(name.as_str())).expect("create symlink fixture");
    let registered = manifest(b"outside", "registered-symlink");
    register(&mut store, &registered);
    symlink(
        &target,
        artifacts(&directory).join(registered.artifact_digest.as_str()),
    )
    .expect("create registered symlink fixture");

    let report = inventory(&directory, &store, limits()).expect("inspect symlink fixture");

    assert_eq!(report.unexpected_entries.indirect_entries, 1);
    assert_eq!(
        status(&report, &registered.artifact_id),
        &RegisteredArtifactBytes::UnsafeEntry
    );
    assert_eq!(fs::read(target).expect("read unchanged target"), b"outside");
}

#[cfg(windows)]
#[test]
fn reports_uninstalled_symlink_without_following_it() {
    use std::{io, os::windows::fs::symlink_file};

    let (directory, mut store) = initialized();
    let target = directory.path().join("outside.bin");
    fs::write(&target, b"outside").expect("write symlink target");
    let name = Digest::sha256(b"symlink-name");
    match symlink_file(&target, artifacts(&directory).join(name.as_str())) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => return,
        Err(error) => panic!("create symlink fixture: {error}"),
    }
    let registered = manifest(b"outside", "registered-symlink");
    register(&mut store, &registered);
    match symlink_file(
        &target,
        artifacts(&directory).join(registered.artifact_digest.as_str()),
    ) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => return,
        Err(error) => panic!("create registered symlink fixture: {error}"),
    }

    let report = inventory(&directory, &store, limits()).expect("inspect symlink fixture");

    assert_eq!(report.unexpected_entries.indirect_entries, 1);
    assert_eq!(
        status(&report, &registered.artifact_id),
        &RegisteredArtifactBytes::UnsafeEntry
    );
    assert_eq!(fs::read(target).expect("read unchanged target"), b"outside");
}

#[test]
fn rejects_unsafe_managed_boundaries() {
    let (directory, store) = initialized();
    fs::remove_dir(artifacts(&directory)).expect("remove empty artifacts directory");
    fs::write(artifacts(&directory), b"not a directory").expect("replace boundary with file");

    match ArtifactInventoryService::open(storage(&directory), &store, limits()) {
        Err(ArtifactInventoryError::UnsafeStorageLayout) => {}
        Err(error) => panic!("unexpected inventory error: {error:?}"),
        Ok(_) => panic!("unsafe boundary was accepted"),
    }
}
