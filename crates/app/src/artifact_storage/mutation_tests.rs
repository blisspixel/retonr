use std::{ffi::OsString, fs, io};

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
