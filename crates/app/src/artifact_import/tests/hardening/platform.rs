use std::fs;

use rewrite_model_store::ArtifactStateStore;
use rewrite_types::CancellationToken;
use tempfile::tempdir;

use super::super::{
    ARTIFACT_BYTES, ArtifactImportError, ArtifactImportStage, OfflineArtifactImportRequest,
    OfflineArtifactImportService, limits, manifest,
};

#[cfg(unix)]
#[test]
fn root_replacement_from_final_callback_cannot_escape_pinned_storage() {
    use std::os::unix::fs::symlink;

    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.gguf");
    let storage = directory.path().join("managed");
    let moved = directory.path().join("managed-held");
    let redirected = directory.path().join("redirected");
    fs::write(&source, ARTIFACT_BYTES).expect("write source fixture");
    fs::create_dir(&redirected).expect("create redirected directory");
    fs::write(redirected.join("sentinel"), b"outside").expect("write outside sentinel");
    let expected_manifest = manifest(ARTIFACT_BYTES);
    let artifact_id = expected_manifest.artifact_id.clone();
    let request = OfflineArtifactImportRequest {
        source: source.clone(),
        manifest: expected_manifest,
    };
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");
    let mut service = OfflineArtifactImportService::open(&storage, &mut store, limits())
        .expect("open import service");

    let error = service
        .import(&request, &CancellationToken::new(), |event| {
            if event.stage == ArtifactImportStage::CommittingFile && !moved.exists() {
                fs::rename(&storage, &moved).expect("move held storage root");
                symlink(&redirected, &storage).expect("redirect original root path");
            }
        })
        .expect_err("replaced storage root must fail before registration");
    assert!(matches!(
        error,
        ArtifactImportError::StorageChanged | ArtifactImportError::UnsafeStorageLayout
    ));
    assert_eq!(
        fs::read(redirected.join("sentinel")).expect("read outside sentinel"),
        b"outside"
    );
    assert_eq!(
        fs::read(&source).expect("read preserved source"),
        ARTIFACT_BYTES
    );
    drop(service);
    assert_eq!(
        store.manifest(&artifact_id).expect("check absent state"),
        None
    );
}

#[cfg(windows)]
#[test]
fn windows_handles_block_root_replacement_before_path_backed_commit() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.gguf");
    let ancestor = directory.path().join("container");
    let storage = ancestor.join("managed");
    let moved = ancestor.join("managed-held");
    let moved_ancestor = directory.path().join("container-held");
    fs::write(&source, ARTIFACT_BYTES).expect("write source fixture");
    let request = OfflineArtifactImportRequest {
        source,
        manifest: manifest(ARTIFACT_BYTES),
    };
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");
    let mut service = OfflineArtifactImportService::open(&storage, &mut store, limits())
        .expect("open import service");
    let mut replacement_blocked = false;
    let mut ancestor_replacement_blocked = false;

    service
        .import(&request, &CancellationToken::new(), |event| {
            if event.stage == ArtifactImportStage::CommittingFile {
                let error = fs::rename(&storage, &moved)
                    .expect_err("held Windows root must reject replacement");
                replacement_blocked = error.kind() == std::io::ErrorKind::PermissionDenied
                    || error.raw_os_error() == Some(32);
                let error = fs::rename(&ancestor, &moved_ancestor)
                    .expect_err("held Windows ancestor must reject replacement");
                ancestor_replacement_blocked = error.kind() == std::io::ErrorKind::PermissionDenied
                    || error.raw_os_error() == Some(32);
            }
        })
        .expect("import succeeds after replacement is blocked");
    assert!(replacement_blocked);
    assert!(ancestor_replacement_blocked);
}

#[cfg(unix)]
#[test]
fn recovery_rejects_reserved_symlink_without_partial_cleanup() {
    use std::os::unix::fs::symlink;

    assert_recovery_rejects_reserved_symlink(|target, link| {
        symlink(target, link).expect("create reserved staging symlink");
    });
}

#[cfg(windows)]
#[test]
fn recovery_rejects_reserved_symlink_without_partial_cleanup() {
    use std::os::windows::fs::symlink_file;

    let result =
        assert_recovery_rejects_reserved_symlink(|target, link| symlink_file(target, link));
    if let Err(error) = &result
        && crate::symlink_test_support::skip_unavailable_link(
            "recovery_rejects_reserved_symlink_without_partial_cleanup",
            error,
        )
    {
        return;
    }
    result.expect("create and reject reserved staging symlink");
}

#[cfg(unix)]
fn assert_recovery_rejects_reserved_symlink(
    create_link: impl FnOnce(&std::path::Path, &std::path::Path),
) {
    let directory = tempdir().expect("temporary directory");
    let storage = directory.path().join("managed");
    let staging = storage.join(".staging");
    let target = directory.path().join("outside-target");
    let regular = staging.join(".import-regular");
    fs::create_dir_all(&staging).expect("create staging directory");
    fs::write(&target, b"outside").expect("write outside target");
    fs::write(&regular, b"retained").expect("write retained staging file");
    create_link(&target, &staging.join(".import-link"));
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");

    let Err(error) =
        OfflineArtifactImportService::open(&storage, &mut store, super::super::limits())
    else {
        panic!("reserved symlink must fail recovery");
    };
    assert!(matches!(error, ArtifactImportError::UnsafeStorageLayout));
    assert_eq!(fs::read(target).expect("read outside target"), b"outside");
    assert_eq!(
        fs::read(regular).expect("read retained staging"),
        b"retained"
    );
}

#[cfg(windows)]
fn assert_recovery_rejects_reserved_symlink(
    create_link: impl FnOnce(&std::path::Path, &std::path::Path) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let directory = tempdir().expect("temporary directory");
    let storage = directory.path().join("managed");
    let staging = storage.join(".staging");
    let target = directory.path().join("outside-target");
    let regular = staging.join(".import-regular");
    fs::create_dir_all(&staging).expect("create staging directory");
    fs::write(&target, b"outside").expect("write outside target");
    fs::write(&regular, b"retained").expect("write retained staging file");
    create_link(&target, &staging.join(".import-link"))?;
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");

    let Err(error) =
        OfflineArtifactImportService::open(&storage, &mut store, super::super::limits())
    else {
        panic!("reserved symlink must fail recovery");
    };
    assert!(matches!(error, ArtifactImportError::UnsafeStorageLayout));
    assert_eq!(fs::read(target).expect("read outside target"), b"outside");
    assert_eq!(
        fs::read(regular).expect("read retained staging"),
        b"retained"
    );
    Ok(())
}

#[cfg(windows)]
#[test]
fn rejects_symbolic_link_source() {
    use std::os::windows::fs::symlink_file;

    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.gguf");
    let link = directory.path().join("linked.gguf");
    fs::write(&source, super::super::ARTIFACT_BYTES).expect("write source fixture");
    if let Err(error) = symlink_file(&source, &link) {
        if crate::symlink_test_support::skip_unavailable_link(
            "rejects_symbolic_link_source",
            &error,
        ) {
            return;
        }
        panic!("create source symbolic link: {error}");
    }
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");
    let mut service = OfflineArtifactImportService::open(
        directory.path().join("managed"),
        &mut store,
        super::super::limits(),
    )
    .expect("open import service");
    let error = service
        .import(
            &OfflineArtifactImportRequest {
                source: link,
                manifest: super::super::manifest(super::super::ARTIFACT_BYTES),
            },
            &CancellationToken::new(),
            |_| {},
        )
        .expect_err("symbolic link source must fail");
    assert!(matches!(error, ArtifactImportError::IndirectSource));
}
