use std::fs;

use rewrite_model::{
    ARTIFACT_MANIFEST_SCHEMA_VERSION, ArtifactId, ArtifactManifest, ArtifactRole, ArtifactSource,
    DeclaredCapabilities, LicenseRecord,
};
use rewrite_model_store::{ArtifactStateStore, RemovalPreparationDisposition};
use rewrite_types::{CancellationToken, Digest};
use tempfile::tempdir;

use super::*;
use crate::{
    ArtifactImportLimits, ArtifactInventoryLimits, ArtifactRemovalDisposition,
    ArtifactRemovalError, ArtifactRemovalLimits, ArtifactRemovalRecoveryError,
    OfflineArtifactImportRequest, RegisteredArtifactBytes,
};

const ARTIFACT_BYTES: &[u8] = b"repository facade artifact";

fn manifest() -> ArtifactManifest {
    let digest = Digest::sha256(ARTIFACT_BYTES);
    ArtifactManifest {
        schema_version: ARTIFACT_MANIFEST_SCHEMA_VERSION,
        artifact_id: ArtifactId::from_digest(digest.clone()),
        source: ArtifactSource {
            origin: "fixture/model".to_owned(),
            revision: "fixture-revision".to_owned(),
        },
        artifact_digest: digest,
        byte_size: u64::try_from(ARTIFACT_BYTES.len()).expect("fixture size fits u64"),
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

const fn import_limits() -> ArtifactImportLimits {
    ArtifactImportLimits {
        maximum_artifact_bytes: 1024 * 1024,
        maximum_storage_entries: 64,
    }
}

const fn inventory_limits() -> ArtifactInventoryLimits {
    ArtifactInventoryLimits {
        maximum_state_entries: 64,
        maximum_storage_entries: 64,
        maximum_artifact_bytes: 1024 * 1024,
        maximum_total_verification_bytes: 16 * 1024 * 1024,
    }
}

const fn removal_limits() -> ArtifactRemovalLimits {
    ArtifactRemovalLimits {
        maximum_artifact_bytes: 1024 * 1024,
        maximum_storage_entries: 64,
    }
}

fn imported_repository() -> (
    tempfile::TempDir,
    ArtifactRepository,
    ArtifactRepositoryImportResult,
) {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.gguf");
    fs::write(&source, ARTIFACT_BYTES).expect("write artifact source");
    let repository =
        ArtifactRepository::new(directory.path().join("data")).expect("derive repository");
    let result = repository
        .import(
            &OfflineArtifactImportRequest {
                source,
                manifest: manifest(),
            },
            import_limits(),
            &CancellationToken::new(),
        )
        .expect("import artifact");
    (directory, repository, result)
}

#[test]
fn missing_inventory_creates_nothing() {
    let directory = tempdir().expect("temporary directory");
    let data = directory.path().join("missing");
    let repository = ArtifactRepository::new(&data).expect("derive repository");
    let error = repository
        .inventory(inventory_limits(), &CancellationToken::new())
        .expect_err("missing repository must fail");
    assert!(matches!(error, ArtifactRepositoryError::NotInitialized));
    assert!(!data.exists());
}

#[test]
fn import_refuses_a_nonempty_uninitialized_selected_root() {
    let directory = tempdir().expect("temporary directory");
    let data = directory.path().join("existing-data");
    fs::create_dir(&data).expect("create existing root");
    let sentinel = data.join("do-not-touch.txt");
    fs::write(&sentinel, b"preserve me").expect("write sentinel");
    let source = directory.path().join("source.gguf");
    fs::write(&source, ARTIFACT_BYTES).expect("write artifact source");
    let repository = ArtifactRepository::new(&data).expect("derive repository");
    assert!(matches!(
        repository.import(
            &OfflineArtifactImportRequest {
                source,
                manifest: manifest(),
            },
            import_limits(),
            &CancellationToken::new(),
        ),
        Err(ArtifactRepositoryError::UnsafeDataDirectory)
    ));
    assert_eq!(fs::read(sentinel).expect("read sentinel"), b"preserve me");
    assert_eq!(fs::read_dir(data).expect("read existing root").count(), 1);
}

#[test]
fn import_returns_generation_and_inventory_is_read_only() {
    let (_directory, repository, imported) = imported_repository();
    assert_eq!(
        imported.disposition,
        ArtifactRepositoryImportDisposition::Imported
    );
    assert_eq!(imported.key.installation_generation(), 1);
    let data_before = directory_snapshot(&repository.data_directory);

    let report = repository
        .inventory(inventory_limits(), &CancellationToken::new())
        .expect("inventory repository");
    assert_eq!(report.registered.len(), 1);
    assert_eq!(
        report.registered[0].installation.artifact_id(),
        imported.key.artifact_id()
    );
    assert_eq!(
        report.registered[0].installation.installation_generation(),
        1
    );
    assert_eq!(
        report.registered[0].bytes,
        RegisteredArtifactBytes::Verified
    );
    assert_eq!(directory_snapshot(&repository.data_directory), data_before);
}

#[test]
fn old_generation_cannot_remove_a_reinstall() {
    let (directory, repository, first) = imported_repository();
    let removed = repository
        .remove(&first.key, removal_limits(), &CancellationToken::new())
        .expect("remove first generation");
    assert_eq!(removed.disposition, ArtifactRemovalDisposition::Removed);

    let second = repository
        .import(
            &OfflineArtifactImportRequest {
                source: directory.path().join("source.gguf"),
                manifest: manifest(),
            },
            import_limits(),
            &CancellationToken::new(),
        )
        .expect("reinstall artifact");
    assert_eq!(second.key.installation_generation(), 2);
    let stale = repository
        .remove(&first.key, removal_limits(), &CancellationToken::new())
        .expect("completed old generation is idempotent");
    assert_eq!(
        stale.disposition,
        ArtifactRemovalDisposition::AlreadyRemoved
    );

    let report = repository
        .inventory(inventory_limits(), &CancellationToken::new())
        .expect("inventory reinstalled artifact");
    assert_eq!(
        report.registered[0].installation.installation_generation(),
        2
    );
    assert_eq!(
        report.registered[0].bytes,
        RegisteredArtifactBytes::Verified
    );
}

#[test]
fn prepared_removal_requires_explicit_exact_recovery() {
    let (_directory, repository, imported) = imported_repository();
    let mut store = ArtifactStateStore::open_existing_and_migrate(&repository.state_database())
        .expect("open state to simulate abrupt exit after preparation");
    let (selection, _) = store
        .artifact_removal_state(imported.key.artifact_id())
        .expect("load installation selection");
    assert_eq!(
        crate::artifact_storage::test_support::prepare_artifact_removal(
            &repository.managed_storage(),
            &mut store,
            &selection.expect("installed selection"),
        )
        .expect("prepare removal"),
        RemovalPreparationDisposition::Prepared
    );
    drop(store);

    let error = repository
        .remove(&imported.key, removal_limits(), &CancellationToken::new())
        .expect_err("fresh removal must not hide recovery");
    assert!(matches!(
        error,
        ArtifactRepositoryError::RemovalRecoveryPending { ref key } if key == &imported.key
    ));
    let recovered = repository
        .recover_removal(&imported.key, removal_limits())
        .expect("recover exact prepared generation");
    assert_eq!(recovered.disposition, ArtifactRemovalDisposition::Recovered);
}

#[test]
fn pending_operation_inspection_is_read_only_bounded_and_cancellable() {
    let (_directory, repository, imported) = imported_repository();
    let mut store = ArtifactStateStore::open_existing_and_migrate(&repository.state_database())
        .expect("open state to prepare removal");
    let (selection, _) = store
        .artifact_removal_state(imported.key.artifact_id())
        .expect("load installed selection");
    crate::artifact_storage::test_support::prepare_artifact_removal(
        &repository.managed_storage(),
        &mut store,
        &selection.expect("installed selection"),
    )
    .expect("prepare removal");
    drop(store);
    fs::remove_file(
        repository
            .managed_storage()
            .join("artifacts")
            .join(imported.key.artifact_id().digest().as_str()),
    )
    .expect("remove artifact bytes before state-only inspection");
    let data_before = directory_snapshot(&repository.data_directory);

    let pending = repository
        .pending_operations(1, &CancellationToken::new())
        .expect("inspect pending operations");
    assert_eq!(pending.artifact_removals, vec![imported.key.clone()]);
    assert_eq!(directory_snapshot(&repository.data_directory), data_before);
    assert!(matches!(
        repository.pending_operations(0, &CancellationToken::new()),
        Err(ArtifactRepositoryError::InvalidLimits)
    ));
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(matches!(
        repository.pending_operations(1, &cancelled),
        Err(ArtifactRepositoryError::Cancelled)
    ));
}

#[test]
fn installation_key_rejects_nonpositive_and_unrepresentable_generations() {
    let artifact_id = manifest().artifact_id;
    assert!(matches!(
        ArtifactInstallationKey::new(artifact_id.clone(), 0),
        Err(ArtifactRepositoryError::InvalidInstallationGeneration)
    ));
    assert!(matches!(
        ArtifactInstallationKey::new(artifact_id, u64::MAX),
        Err(ArtifactRepositoryError::InvalidInstallationGeneration)
    ));
}

#[test]
fn post_preparation_failure_keeps_its_recovery_key_across_a_boundary_failure() {
    let key = ArtifactInstallationKey::new(manifest().artifact_id, 1).expect("valid key");
    let mapped = map_removal_error(
        &key,
        ArtifactRemovalError::RecoveryRequired(ArtifactRemovalRecoveryError::Storage),
    );
    let result: Result<(), _> = finish_operation(
        Err(mapped),
        Err(ArtifactRepositoryError::UnsafeDataDirectory),
    );
    assert!(matches!(
        result,
        Err(ArtifactRepositoryError::RemovalRecoveryRequired { key: actual, .. })
            if actual == key
    ));
}

#[test]
fn boundary_failure_overrides_a_merely_observed_pending_removal() {
    let key = ArtifactInstallationKey::new(manifest().artifact_id, 1).expect("valid key");
    let result: Result<(), _> = finish_operation(
        Err(ArtifactRepositoryError::RemovalRecoveryPending { key }),
        Err(ArtifactRepositoryError::UnsafeDataDirectory),
    );
    assert!(matches!(
        result,
        Err(ArtifactRepositoryError::UnsafeDataDirectory)
    ));
}

#[test]
fn a_state_alias_added_after_pinning_invalidates_the_guard() {
    let (_directory, repository, _imported) = imported_repository();
    let mut guard = repository
        .pin_data_directory(RepositoryLockMode::ExistingShared)
        .expect("pin repository");
    guard.pin_state_database().expect("pin state database");
    guard.recheck().expect("initial boundary is coherent");
    let alias = repository.data_directory.join("state-alias.sqlite3");
    fs::hard_link(repository.state_database(), alias).expect("create state hard link");
    assert!(matches!(
        guard.recheck(),
        Err(ArtifactRepositoryError::UnsafeDataDirectory)
    ));
}

#[cfg(unix)]
#[test]
fn import_preserves_permissions_on_an_existing_selected_root() {
    use std::os::unix::fs::PermissionsExt;

    let directory = tempdir().expect("temporary directory");
    let data = directory.path().join("existing-data");
    fs::create_dir(&data).expect("create existing root");
    fs::set_permissions(&data, fs::Permissions::from_mode(0o755)).expect("set fixture mode");
    let source = directory.path().join("source.gguf");
    fs::write(&source, ARTIFACT_BYTES).expect("write artifact source");
    let repository = ArtifactRepository::new(&data).expect("derive repository");
    repository
        .import(
            &OfflineArtifactImportRequest {
                source,
                manifest: manifest(),
            },
            import_limits(),
            &CancellationToken::new(),
        )
        .expect("import into existing root");
    assert_eq!(
        fs::metadata(data)
            .expect("read root metadata")
            .permissions()
            .mode()
            & 0o777,
        0o755
    );
}

fn directory_snapshot(path: &std::path::Path) -> Vec<(std::path::PathBuf, u64)> {
    let mut entries = fs::read_dir(path)
        .expect("read repository directory")
        .map(|entry| {
            let entry = entry.expect("read repository entry");
            let metadata = entry.metadata().expect("read repository metadata");
            (entry.path(), metadata.len())
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    entries
}
