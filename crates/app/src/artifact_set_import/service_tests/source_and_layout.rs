use std::fs;

use tempfile::tempdir;

use super::*;

#[test]
fn a_file_occupying_the_set_root_name_is_a_conflict() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source");
    let storage = directory.path().join("storage");
    write_source(&source);
    let request = request(&source);
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.sqlite3")).expect("state store");
    let mut service =
        OfflineArtifactSetImportService::open(&storage, &mut store, limits()).expect("service");
    fs::write(
        storage.join("sets").join(storage_key(&request.manifest)),
        b"not-a-set-root",
    )
    .expect("occupy set-root name with a file");

    assert!(matches!(
        service.import(&request, &CancellationToken::new(), |_| {}),
        Err(ArtifactSetImportError::StorageConflict)
    ));
    assert_eq!(
        fs::read(storage.join("sets").join(storage_key(&request.manifest)))
            .expect("read occupying file"),
        b"not-a-set-root"
    );
    assert_no_state(&store, &request.manifest);
}

#[test]
fn cancelled_prepublication_verification_cleans_staging() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source");
    let storage = directory.path().join("storage");
    write_source(&source);
    let request = request(&source);
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.sqlite3")).expect("state store");
    let mut service =
        OfflineArtifactSetImportService::open(&storage, &mut store, limits()).expect("service");
    let cancellation = CancellationToken::new();
    let callback_token = cancellation.clone();
    let error = service
        .import(&request, &cancellation, |event| {
            if event.stage == ArtifactSetImportStage::PublishingTree {
                callback_token.cancel();
            }
        })
        .expect_err("pre-publication cancellation must stop import");

    assert!(
        matches!(error, ArtifactSetImportError::Cancelled),
        "unexpected publishing-stage result: {error:?}"
    );
    assert!(
        fs::read_dir(storage.join(".set-staging"))
            .expect("staging directory")
            .next()
            .is_none()
    );
    assert!(
        fs::read_dir(storage.join("sets"))
            .expect("sets directory")
            .next()
            .is_none()
    );
    assert_no_state(&store, &request.manifest);
}

#[cfg(windows)]
#[test]
fn managed_sets_junction_is_not_adopted() {
    use std::os::windows::fs::symlink_dir;

    let directory = tempdir().expect("temporary directory");
    let storage = directory.path().join("storage");
    let outside = directory.path().join("outside-sets");
    fs::create_dir(&storage).expect("storage root");
    fs::create_dir(&outside).expect("external sets target");
    fs::write(outside.join("winner"), b"external").expect("external sentinel");
    match symlink_dir(&outside, storage.join("sets")) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return,
        Err(error) => panic!("create managed sets junction: {error}"),
    }
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.sqlite3")).expect("state store");

    assert!(matches!(
        OfflineArtifactSetImportService::open(&storage, &mut store, limits()),
        Err(ArtifactSetImportError::UnsafeStorageLayout)
    ));
    assert_eq!(
        fs::read(outside.join("winner")).expect("read external sentinel"),
        b"external"
    );
}

#[cfg(windows)]
#[test]
fn source_directory_junction_is_rejected_without_following() {
    use std::os::windows::fs::symlink_dir;

    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source");
    let outside = directory.path().join("outside");
    write_source(&source);
    fs::create_dir(&outside).expect("outside directory");
    fs::rename(source.join("model"), outside.join("model")).expect("move nested source");
    match symlink_dir(outside.join("model"), source.join("model")) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return,
        Err(error) => panic!("create source directory junction: {error}"),
    }
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.sqlite3")).expect("state store");
    let mut service = OfflineArtifactSetImportService::open(
        directory.path().join("storage"),
        &mut store,
        limits(),
    )
    .expect("service");

    assert!(matches!(
        service.import(&request(&source), &CancellationToken::new(), |_| {}),
        Err(ArtifactSetImportError::UnsafeSourceTree)
    ));
    assert_eq!(
        fs::read(outside.join("model/weights.bin")).expect("read junction target"),
        b"weights"
    );
    assert_no_state(&store, &request(&source).manifest);
}
