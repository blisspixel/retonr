use std::{ffi::OsString, fs, io};

#[cfg(windows)]
use rewrite_types::CancellationToken;
use tempfile::tempdir;

use super::{ArtifactInventoryError, PinnedDirectory};

#[cfg(windows)]
use super::{FILE_ATTRIBUTE_REPARSE_POINT, windows_indirect};

#[test]
fn staging_name_collision_retries_are_bounded() {
    let directory = tempdir().expect("temporary directory");
    let collision = OsString::from(".import-collision");
    fs::write(directory.path().join(&collision), b"existing").expect("write collision fixture");
    let pinned = PinnedDirectory::open_existing(directory.path()).expect("pin directory");

    let error = pinned
        .create_staging_file_with(|| Ok(collision.clone()))
        .expect_err("repeated collision must exhaust bounded retries");
    assert!(matches!(
        error,
        ArtifactInventoryError::StorageIo(ref source)
            if source.kind() == io::ErrorKind::AlreadyExists
    ));
    assert_eq!(
        fs::read(directory.path().join(collision)).expect("read collision fixture"),
        b"existing"
    );
}

#[cfg(windows)]
#[test]
fn every_windows_reparse_attribute_is_indirect() {
    assert!(windows_indirect(FILE_ATTRIBUTE_REPARSE_POINT));
    assert!(windows_indirect(FILE_ATTRIBUTE_REPARSE_POINT | 0x20));
    assert!(!windows_indirect(0x20));
}

#[cfg(windows)]
#[test]
fn removal_handle_permits_deleting_its_open_file() {
    use fs_at::os::windows::FileExt as _;
    use std::os::windows::io::AsHandle as _;
    use winx::file::AccessMode;

    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("artifact");
    fs::write(&path, b"artifact").expect("write artifact");
    let pinned = PinnedDirectory::open_existing(directory.path()).expect("pin directory");
    let held = pinned
        .open_managed_file_for_removal(
            std::ffi::OsStr::new("artifact"),
            2,
            &CancellationToken::new(),
        )
        .expect("open removal handle")
        .expect("managed file");
    let second = pinned
        .open_managed_file_for_removal(
            std::ffi::OsStr::new("artifact"),
            2,
            &CancellationToken::new(),
        )
        .expect("open second removal handle")
        .expect("second managed file");
    drop(second);
    let access =
        winx::file::query_access_information(held.file.as_handle()).expect("query removal access");
    assert!(access.contains(AccessMode::DELETE));
    let super::mutation::ManagedFile {
        file, fingerprint, ..
    } = held;
    drop(fingerprint);
    file.delete_by_handle()
        .map_err(|(_, error)| error)
        .expect("delete verified handle");
    assert!(!path.exists());
}
