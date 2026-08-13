use std::{fs, io, path::Path};

use rewrite_model::{
    ARTIFACT_MANIFEST_SCHEMA_VERSION, ArtifactId, ArtifactManifest, ArtifactRole, ArtifactSource,
    DeclaredCapabilities, LicenseRecord,
};
use rewrite_model_store::{ArtifactStateStore, StoreError, WriteDisposition};
use rewrite_types::{CancellationToken, Digest};
use tempfile::tempdir;

use super::{
    ArtifactImportError, ArtifactImportLimits, ArtifactImportProgress, ArtifactImportResult,
    ArtifactImportStage, COPY_BUFFER_BYTES, LOCK_FILE, OfflineArtifactImportRequest,
    OfflineArtifactImportService, storage_key,
};

const ARTIFACT_BYTES: &[u8] = b"verified local model artifact";

const fn limits() -> ArtifactImportLimits {
    ArtifactImportLimits {
        maximum_artifact_bytes: 64 * 1024 * 1024,
    }
}

fn manifest(bytes: &[u8]) -> ArtifactManifest {
    let digest = Digest::sha256(bytes);
    ArtifactManifest {
        schema_version: ARTIFACT_MANIFEST_SCHEMA_VERSION,
        artifact_id: ArtifactId::from_digest(digest.clone()),
        source: ArtifactSource {
            origin: "fixture/local-model".to_owned(),
            revision: "fixture-revision-1".to_owned(),
        },
        artifact_digest: digest,
        byte_size: u64::try_from(bytes.len()).expect("fixture length fits u64"),
        format: "gguf".to_owned(),
        family: "fixture".to_owned(),
        architecture: Some("transformer".to_owned()),
        quantization: Some("q4".to_owned()),
        tokenizer: None,
        licenses: vec![LicenseRecord {
            component: "weights".to_owned(),
            identifier: "Apache-2.0".to_owned(),
            text_digest: Digest::sha256(b"fixture license"),
        }],
        declared_capabilities: DeclaredCapabilities {
            roles: vec![ArtifactRole::Generation],
            languages: vec!["en".to_owned()],
            context_tokens: Some(8_192),
        },
    }
}

fn run_import(
    service: &mut OfflineArtifactImportService<'_>,
    request: &OfflineArtifactImportRequest,
) -> Result<ArtifactImportResult, ArtifactImportError> {
    service.import(request, &CancellationToken::new(), |_| {})
}

#[cfg(unix)]
fn create_directory_link(target: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn create_directory_link(target: &Path, link: &Path) -> io::Result<()> {
    std::os::windows::fs::symlink_dir(target, link)
}

#[test]
fn imports_without_mutating_source_and_repeats_idempotently() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.gguf");
    let storage = directory.path().join("managed");
    fs::write(&source, ARTIFACT_BYTES).expect("write source fixture");
    let expected_manifest = manifest(ARTIFACT_BYTES);
    let request = OfflineArtifactImportRequest {
        source: source.clone(),
        manifest: expected_manifest.clone(),
    };
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");
    let mut service = OfflineArtifactImportService::open(&storage, &mut store, limits())
        .expect("open import service");

    let mut progress = Vec::new();
    let first = service
        .import(&request, &CancellationToken::new(), |event| {
            progress.push(event);
        })
        .expect("import exact artifact");
    assert_eq!(first.state.manifest, WriteDisposition::Inserted);
    assert_eq!(first.state.installed, WriteDisposition::Inserted);
    assert_eq!(
        fs::read(&source).expect("read preserved source"),
        ARTIFACT_BYTES
    );
    assert_eq!(
        fs::read(storage.join(&first.installed.storage_key)).expect("read stored artifact"),
        ARTIFACT_BYTES
    );
    assert_eq!(
        progress.first().map(|event| event.stage),
        Some(ArtifactImportStage::InspectingSource)
    );
    assert_eq!(
        progress.last(),
        Some(&ArtifactImportProgress {
            stage: ArtifactImportStage::Complete,
            completed_bytes: expected_manifest.byte_size,
            total_bytes: expected_manifest.byte_size,
        })
    );

    let mut repeated_progress = Vec::new();
    let repeated = service
        .import(&request, &CancellationToken::new(), |event| {
            repeated_progress.push(event);
        })
        .expect("repeat exact import");
    assert_eq!(repeated.installed, first.installed);
    assert_eq!(repeated.state.manifest, WriteDisposition::AlreadyPresent);
    assert_eq!(repeated.state.installed, WriteDisposition::AlreadyPresent);
    assert!(
        repeated_progress
            .iter()
            .any(|event| event.stage == ArtifactImportStage::VerifyingExistingFile)
    );
    assert!(
        repeated_progress
            .iter()
            .any(|event| event.stage == ArtifactImportStage::VerifyingSource)
    );
    assert!(repeated_progress.iter().all(|event| !matches!(
        event.stage,
        ArtifactImportStage::StagingAndVerifying | ArtifactImportStage::CommittingFile
    )));
}

#[test]
fn rejects_wrong_size_and_digest_without_changing_source() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.gguf");
    let storage = directory.path().join("managed");
    fs::write(&source, ARTIFACT_BYTES).expect("write source fixture");
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");
    let mut service = OfflineArtifactImportService::open(&storage, &mut store, limits())
        .expect("open import service");

    let short_manifest = manifest(b"other bytes");
    let size_error = service
        .import(
            &OfflineArtifactImportRequest {
                source: source.clone(),
                manifest: short_manifest,
            },
            &CancellationToken::new(),
            |_| {},
        )
        .expect_err("wrong size must fail");
    assert!(matches!(size_error, ArtifactImportError::SizeMismatch));

    let mut wrong_digest = manifest(ARTIFACT_BYTES);
    wrong_digest.artifact_digest = Digest::sha256(b"different content same len");
    wrong_digest.artifact_id = ArtifactId::from_digest(wrong_digest.artifact_digest.clone());
    let digest_error = service
        .import(
            &OfflineArtifactImportRequest {
                source: source.clone(),
                manifest: wrong_digest,
            },
            &CancellationToken::new(),
            |_| {},
        )
        .expect_err("wrong digest must fail");
    assert!(matches!(digest_error, ArtifactImportError::DigestMismatch));
    assert_eq!(
        fs::read(source).expect("read original source"),
        ARTIFACT_BYTES
    );
    assert_eq!(
        fs::read_dir(storage.join(".staging"))
            .expect("read staging directory")
            .count(),
        0
    );
}

#[test]
fn rejects_invalid_and_exceeded_resource_limits_before_copying() {
    let directory = tempdir().expect("temporary directory");
    let storage = directory.path().join("managed");
    let source = directory.path().join("source.gguf");
    fs::write(&source, ARTIFACT_BYTES).expect("write source fixture");
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");
    let Err(invalid) = OfflineArtifactImportService::open(
        &storage,
        &mut store,
        ArtifactImportLimits {
            maximum_artifact_bytes: 0,
        },
    ) else {
        panic!("zero byte ceiling must fail");
    };
    assert!(matches!(invalid, ArtifactImportError::InvalidLimits));

    let mut service = OfflineArtifactImportService::open(
        &storage,
        &mut store,
        ArtifactImportLimits {
            maximum_artifact_bytes: 1,
        },
    )
    .expect("open bounded import service");
    let error = run_import(
        &mut service,
        &OfflineArtifactImportRequest {
            source,
            manifest: manifest(ARTIFACT_BYTES),
        },
    )
    .expect_err("oversized manifest must fail before copying");
    assert!(matches!(
        error,
        ArtifactImportError::ArtifactTooLarge { .. }
    ));
    assert_eq!(
        fs::read_dir(storage.join("artifacts"))
            .expect("read empty artifact directory")
            .count(),
        0
    );
}

#[test]
fn state_conflict_leaves_only_a_verified_unregistered_file() {
    let directory = tempdir().expect("temporary directory");
    let storage = directory.path().join("managed");
    let source = directory.path().join("source.gguf");
    fs::write(&source, ARTIFACT_BYTES).expect("write source fixture");
    let expected_manifest = manifest(ARTIFACT_BYTES);
    let mut conflicting_manifest = expected_manifest.clone();
    conflicting_manifest.family = "conflicting-fixture".to_owned();
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");
    store
        .put_manifest(&conflicting_manifest)
        .expect("store conflicting immutable record");
    let mut service = OfflineArtifactImportService::open(&storage, &mut store, limits())
        .expect("open import service");

    let error = run_import(
        &mut service,
        &OfflineArtifactImportRequest {
            source,
            manifest: expected_manifest.clone(),
        },
    )
    .expect_err("state conflict must not report successful import");
    assert!(matches!(
        error,
        ArtifactImportError::State(StoreError::ImmutableConflict)
    ));
    assert_eq!(
        fs::read(storage.join(storage_key(&expected_manifest.artifact_digest)))
            .expect("read verified orphan"),
        ARTIFACT_BYTES
    );
}

#[test]
fn rejects_directory_source_and_conflicting_final_bytes() {
    let directory = tempdir().expect("temporary directory");
    let storage = directory.path().join("managed");
    let expected_manifest = manifest(ARTIFACT_BYTES);
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");
    let mut service = OfflineArtifactImportService::open(&storage, &mut store, limits())
        .expect("open import service");
    let directory_error = service
        .import(
            &OfflineArtifactImportRequest {
                source: directory.path().to_path_buf(),
                manifest: expected_manifest.clone(),
            },
            &CancellationToken::new(),
            |_| {},
        )
        .expect_err("directory source must fail");
    assert!(matches!(
        directory_error,
        ArtifactImportError::SourceNotRegular
    ));

    let source = directory.path().join("source.gguf");
    fs::write(&source, ARTIFACT_BYTES).expect("write source fixture");
    let final_path = storage.join(storage_key(&expected_manifest.artifact_digest));
    let artifact_id = expected_manifest.artifact_id.clone();
    fs::create_dir_all(final_path.parent().expect("final parent")).expect("create final parent");
    fs::write(&final_path, b"conflicting bytes").expect("write conflicting final bytes");
    let conflict = service
        .import(
            &OfflineArtifactImportRequest {
                source,
                manifest: expected_manifest,
            },
            &CancellationToken::new(),
            |_| {},
        )
        .expect_err("conflicting final bytes must fail");
    assert!(matches!(conflict, ArtifactImportError::StorageConflict));
    drop(service);
    assert_eq!(
        store.manifest(&artifact_id).expect("check absent state"),
        None
    );
}

#[test]
fn rejects_non_file_destination_as_unsafe_storage() {
    let directory = tempdir().expect("temporary directory");
    let storage = directory.path().join("managed");
    let source = directory.path().join("source.gguf");
    fs::write(&source, ARTIFACT_BYTES).expect("write source fixture");
    let expected_manifest = manifest(ARTIFACT_BYTES);
    let destination = storage.join(storage_key(&expected_manifest.artifact_digest));
    fs::create_dir_all(&destination).expect("create invalid destination directory");
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");
    let mut service = OfflineArtifactImportService::open(&storage, &mut store, limits())
        .expect("open import service");

    let error = run_import(
        &mut service,
        &OfflineArtifactImportRequest {
            source,
            manifest: expected_manifest,
        },
    )
    .expect_err("non-file destination must fail as unsafe storage");
    assert!(matches!(error, ArtifactImportError::UnsafeStorageLayout));
}

#[test]
fn rejects_non_file_lock_path_as_unsafe_storage() {
    let directory = tempdir().expect("temporary directory");
    let storage = directory.path().join("managed");
    fs::create_dir_all(storage.join(LOCK_FILE)).expect("create invalid lock directory");
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");

    let Err(error) = OfflineArtifactImportService::open(&storage, &mut store, limits()) else {
        panic!("non-file lock path must fail as unsafe storage");
    };
    assert!(matches!(error, ArtifactImportError::UnsafeStorageLayout));
}

#[test]
fn rejects_staging_directory_replaced_with_an_indirect_path() {
    let directory = tempdir().expect("temporary directory");
    let storage = directory.path().join("managed");
    let source = directory.path().join("source.gguf");
    let redirected = directory.path().join("redirected");
    fs::write(&source, ARTIFACT_BYTES).expect("write source fixture");
    fs::create_dir(&redirected).expect("create redirected directory");
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");
    let mut service = OfflineArtifactImportService::open(&storage, &mut store, limits())
        .expect("open import service");
    let staging = storage.join(".staging");
    fs::remove_dir(&staging).expect("remove original staging directory");
    if let Err(error) = create_directory_link(&redirected, &staging) {
        if cfg!(windows) && error.kind() == io::ErrorKind::PermissionDenied {
            return;
        }
        panic!("replace staging with directory link: {error}");
    }

    let error = run_import(
        &mut service,
        &OfflineArtifactImportRequest {
            source,
            manifest: manifest(ARTIFACT_BYTES),
        },
    )
    .expect_err("indirect staging directory must fail before writing");
    assert!(matches!(error, ArtifactImportError::UnsafeStorageLayout));
    assert_eq!(
        fs::read_dir(&redirected)
            .expect("read redirected directory")
            .count(),
        0
    );
}

#[test]
fn recovers_only_owned_staging_files_and_excludes_concurrent_service() {
    let directory = tempdir().expect("temporary directory");
    let storage = directory.path().join("managed");
    let staging = storage.join(".staging");
    fs::create_dir_all(&staging).expect("create staging fixture");
    let stale = staging.join(".import-stale");
    let unrelated = staging.join("operator-note");
    fs::write(&stale, b"partial").expect("write stale fixture");
    fs::write(&unrelated, b"keep").expect("write unrelated fixture");
    let mut first_store = ArtifactStateStore::open(&directory.path().join("first.sqlite3"))
        .expect("open first state store");
    let service = OfflineArtifactImportService::open(&storage, &mut first_store, limits())
        .expect("open first service");
    assert!(!stale.exists());
    assert!(unrelated.exists());

    let mut second_store = ArtifactStateStore::open(&directory.path().join("second.sqlite3"))
        .expect("open second state store");
    let Err(error) = OfflineArtifactImportService::open(&storage, &mut second_store, limits())
    else {
        panic!("second service must not share import ownership");
    };
    assert!(matches!(error, ArtifactImportError::StorageInUse));
    drop(service);
}

#[test]
fn cancellation_removes_staging_and_registers_no_state() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("large.gguf");
    let storage = directory.path().join("managed");
    let bytes = vec![b'x'; COPY_BUFFER_BYTES + 1];
    fs::write(&source, &bytes).expect("write cancellable source fixture");
    let expected_manifest = manifest(&bytes);
    let artifact_id = expected_manifest.artifact_id.clone();
    let request = OfflineArtifactImportRequest {
        source,
        manifest: expected_manifest,
    };
    let cancellation = CancellationToken::new();
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");
    let mut service = OfflineArtifactImportService::open(&storage, &mut store, limits())
        .expect("open import service");

    let error = service
        .import(&request, &cancellation, |event| {
            if event.stage == ArtifactImportStage::StagingAndVerifying && event.completed_bytes > 0
            {
                cancellation.cancel();
            }
        })
        .expect_err("mid-copy cancellation must stop import");
    assert!(matches!(error, ArtifactImportError::Cancelled));
    assert_eq!(
        fs::read_dir(storage.join(".staging"))
            .expect("read recovered staging")
            .count(),
        0
    );
    assert_eq!(
        fs::read_dir(storage.join("artifacts"))
            .expect("read final artifact directory")
            .count(),
        0
    );
    drop(service);
    assert_eq!(
        store.manifest(&artifact_id).expect("check absent state"),
        None
    );
}

#[cfg(unix)]
#[test]
fn rejects_symbolic_link_source() {
    use std::os::unix::fs::symlink;

    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.gguf");
    let link = directory.path().join("linked.gguf");
    fs::write(&source, ARTIFACT_BYTES).expect("write source fixture");
    symlink(&source, &link).expect("create source symlink");
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");
    let mut service =
        OfflineArtifactImportService::open(directory.path().join("managed"), &mut store, limits())
            .expect("open import service");
    let error = service
        .import(
            &OfflineArtifactImportRequest {
                source: link,
                manifest: manifest(ARTIFACT_BYTES),
            },
            &CancellationToken::new(),
            |_| {},
        )
        .expect_err("symlink source must fail");
    assert!(matches!(error, ArtifactImportError::IndirectSource));
}

#[cfg(windows)]
#[test]
fn rejects_symbolic_link_source() {
    use std::os::windows::fs::symlink_file;

    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.gguf");
    let link = directory.path().join("linked.gguf");
    fs::write(&source, ARTIFACT_BYTES).expect("write source fixture");
    if let Err(error) = symlink_file(&source, &link) {
        if error.kind() == std::io::ErrorKind::PermissionDenied {
            return;
        }
        panic!("create source symbolic link: {error}");
    }
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");
    let mut service =
        OfflineArtifactImportService::open(directory.path().join("managed"), &mut store, limits())
            .expect("open import service");
    let error = service
        .import(
            &OfflineArtifactImportRequest {
                source: link,
                manifest: manifest(ARTIFACT_BYTES),
            },
            &CancellationToken::new(),
            |_| {},
        )
        .expect_err("symbolic link source must fail");
    assert!(matches!(error, ArtifactImportError::IndirectSource));
}
