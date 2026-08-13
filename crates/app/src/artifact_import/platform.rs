use std::{
    fs::{self, File, Metadata},
    io,
    path::Path,
};

use super::ArtifactImportError;

#[cfg(unix)]
pub(super) fn set_private_directory_permissions(path: &Path) -> Result<(), ArtifactImportError> {
    use std::os::unix::fs::PermissionsExt as _;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(ArtifactImportError::StorageIo)
}

pub(super) fn sync_directory(path: &Path) -> Result<(), ArtifactImportError> {
    let metadata = fs::symlink_metadata(path).map_err(ArtifactImportError::StorageIo)?;
    if !metadata.is_dir() || is_indirect(&metadata) {
        return Err(ArtifactImportError::UnsafeStorageLayout);
    }
    sync_directory_handle(path)
}

#[cfg(unix)]
fn sync_directory_handle(path: &Path) -> Result<(), ArtifactImportError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(ArtifactImportError::StorageIo)
}

#[cfg(windows)]
fn sync_directory_handle(path: &Path) -> Result<(), ArtifactImportError> {
    use std::os::windows::fs::OpenOptionsExt as _;

    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    let directory = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(ArtifactImportError::StorageIo)?;
    let opened = directory
        .metadata()
        .map_err(ArtifactImportError::StorageIo)?;
    if !opened.is_dir() || is_indirect(&opened) {
        return Err(ArtifactImportError::UnsafeStorageLayout);
    }
    directory.sync_all().map_err(ArtifactImportError::StorageIo)
}

#[cfg(unix)]
pub(super) fn open_readonly_no_follow(path: &Path) -> io::Result<File> {
    use rustix::fs::{Mode, OFlags};

    rustix::fs::open(
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(io::Error::from)
}

#[cfg(windows)]
pub(super) fn open_readonly_no_follow(path: &Path) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt as _;

    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    fs::OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
}

#[cfg(unix)]
pub(super) fn open_lock_file(path: &Path) -> io::Result<File> {
    use rustix::fs::{Mode, OFlags};

    rustix::fs::open(
        path,
        OFlags::RDWR | OFlags::CREATE | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::RUSR | Mode::WUSR,
    )
    .map(File::from)
    .map_err(io::Error::from)
}

#[cfg(windows)]
pub(super) fn open_lock_file(path: &Path) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt as _;

    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
}

pub(super) fn is_indirect(metadata: &Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;

        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(unix)]
    {
        false
    }
}
