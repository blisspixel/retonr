use std::{fs, io, path::Path};

use rewrite_model_store::ArtifactStateStore;
use tempfile::tempdir;

use super::{
    ARTIFACT_BYTES, ArtifactImportError, LOCK_FILE, OfflineArtifactImportRequest,
    OfflineArtifactImportService, limits, manifest, run_import, storage_key,
};

#[cfg(unix)]
fn create_directory_link(target: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn create_directory_link(target: &Path, link: &Path) -> io::Result<()> {
    std::os::windows::fs::symlink_dir(target, link)
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
fn rejects_non_directory_artifact_storage_as_unsafe() {
    let directory = tempdir().expect("temporary directory");
    let storage = directory.path().join("managed");
    fs::create_dir(&storage).expect("create storage root");
    fs::write(storage.join("artifacts"), b"not a directory")
        .expect("write invalid artifact storage entry");
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");

    let Err(error) = OfflineArtifactImportService::open(&storage, &mut store, limits()) else {
        panic!("non-directory artifact storage must fail as unsafe");
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
