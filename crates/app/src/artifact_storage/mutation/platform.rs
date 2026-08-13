use std::{
    ffi::{OsStr, OsString},
    fmt::Write as _,
    fs::File,
    io,
};

#[cfg(windows)]
use std::path::Path;

use super::EntryStatus;
use crate::artifact_inventory::ArtifactInventoryError;
#[cfg(windows)]
use crate::artifact_storage::windows_indirect;
use crate::artifact_storage::{PinnedDirectory, map_active_error, map_initial_error};

#[cfg(unix)]
pub(super) fn remove_verified_file(
    directory: &PinnedDirectory,
    name: &OsStr,
    held: File,
) -> Result<(), ArtifactInventoryError> {
    directory.remove_file(name)?;
    drop(held);
    Ok(())
}

#[cfg(windows)]
pub(super) fn remove_verified_file(
    _directory: &PinnedDirectory,
    _name: &OsStr,
    held: File,
) -> Result<(), ArtifactInventoryError> {
    use fs_at::os::windows::FileExt as _;

    held.delete_by_handle()
        .map_err(|(_, error)| ArtifactInventoryError::StorageIo(error))
}

#[cfg(unix)]
pub(super) fn inspect_entry(
    parent: &File,
    name: &OsStr,
) -> Result<Option<EntryStatus>, ArtifactInventoryError> {
    use rustix::fs::{AtFlags, FileType};

    let stat = match rustix::fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => stat,
        Err(rustix::io::Errno::NOENT) => return Ok(None),
        Err(error) => return Err(map_active_error(io::Error::from(error))),
    };
    let file_type = FileType::from_raw_mode(stat.st_mode);
    Ok(Some(EntryStatus {
        direct_regular_file: file_type == FileType::RegularFile,
        indirect: file_type == FileType::Symlink,
        byte_size: u64::try_from(stat.st_size)
            .map_err(|_| ArtifactInventoryError::ConcurrentModification)?,
    }))
}

#[cfg(windows)]
pub(super) fn inspect_entry(
    parent: &File,
    name: &OsStr,
) -> Result<Option<EntryStatus>, ArtifactInventoryError> {
    use cap_primitives::fs::{FollowSymlinks, MetadataExt as _};

    let metadata = match cap_primitives::fs::stat(parent, Path::new(name), FollowSymlinks::No) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(map_active_error(error)),
    };
    let indirect = windows_indirect(metadata.file_attributes());
    Ok(Some(EntryStatus {
        direct_regular_file: metadata.is_file() && !indirect,
        indirect,
        byte_size: metadata.len(),
    }))
}

pub(super) fn random_staging_name(prefix: &str) -> Result<OsString, ArtifactInventoryError> {
    let mut random = [0_u8; 16];
    getrandom::fill(&mut random).map_err(|_| {
        ArtifactInventoryError::StorageIo(io::Error::other(
            "could not generate a staging file name",
        ))
    })?;
    let mut name = String::with_capacity(prefix.len() + random.len() * 2);
    name.push_str(prefix);
    for byte in random {
        write!(&mut name, "{byte:02x}").map_err(|_| {
            ArtifactInventoryError::StorageIo(io::Error::other(
                "could not format a staging file name",
            ))
        })?;
    }
    Ok(OsString::from(name))
}

#[cfg(unix)]
pub(super) fn create_directory(parent: &File, name: &OsStr) -> Result<(), ArtifactInventoryError> {
    use rustix::fs::Mode;

    match rustix::fs::mkdirat(parent, name, Mode::RUSR | Mode::WUSR | Mode::XUSR) {
        Ok(()) | Err(rustix::io::Errno::EXIST) => Ok(()),
        Err(error) => Err(map_initial_error(io::Error::from(error))),
    }
}

#[cfg(unix)]
pub(super) fn create_directory_exclusive(
    parent: &File,
    name: &OsStr,
) -> Result<(), ArtifactInventoryError> {
    use rustix::fs::Mode;

    rustix::fs::mkdirat(parent, name, Mode::RUSR | Mode::WUSR | Mode::XUSR)
        .map_err(io::Error::from)
        .map_err(map_initial_error)
}

#[cfg(windows)]
pub(super) fn create_directory(parent: &File, name: &OsStr) -> Result<(), ArtifactInventoryError> {
    let options = cap_primitives::fs::DirOptions::new();
    match cap_primitives::fs::create_dir(parent, Path::new(name), &options) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(map_initial_error(error)),
    }
}

#[cfg(windows)]
pub(super) fn create_directory_exclusive(
    parent: &File,
    name: &OsStr,
) -> Result<(), ArtifactInventoryError> {
    let options = cap_primitives::fs::DirOptions::new();
    cap_primitives::fs::create_dir(parent, Path::new(name), &options).map_err(map_initial_error)
}

#[cfg(unix)]
pub(super) fn set_private_permissions(directory: &File) -> Result<(), ArtifactInventoryError> {
    use rustix::fs::Mode;

    rustix::fs::fchmod(directory, Mode::RUSR | Mode::WUSR | Mode::XUSR)
        .map_err(io::Error::from)
        .map_err(ArtifactInventoryError::StorageIo)
}

#[cfg(unix)]
pub(super) fn open_or_create_file(parent: &File, name: &OsStr) -> io::Result<File> {
    use rustix::fs::{Mode, OFlags};
    rustix::fs::openat(
        parent,
        name,
        OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )
    .map(File::from)
    .map_err(io::Error::from)
}

#[cfg(windows)]
pub(super) fn open_or_create_file(parent: &File, name: &OsStr) -> io::Result<File> {
    use cap_fs_ext::OpenOptionsFollowExt as _;
    use cap_primitives::fs::{FollowSymlinks, OpenOptions, OpenOptionsExt as _};
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(true)
        .create(true)
        .share_mode(0x0000_0001 | 0x0000_0002)
        .follow(FollowSymlinks::No);
    cap_primitives::fs::open(parent, Path::new(name), &options)
}

#[cfg(unix)]
pub(super) fn create_new_file(parent: &File, name: &OsStr) -> io::Result<File> {
    use rustix::fs::{Mode, OFlags};
    rustix::fs::openat(
        parent,
        name,
        OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )
    .map(File::from)
    .map_err(io::Error::from)
}

#[cfg(windows)]
pub(super) fn create_new_file(parent: &File, name: &OsStr) -> io::Result<File> {
    use cap_fs_ext::OpenOptionsFollowExt as _;
    use cap_primitives::fs::{FollowSymlinks, OpenOptions, OpenOptionsExt as _};
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(true)
        .create_new(true)
        .share_mode(0x0000_0001 | 0x0000_0002 | 0x0000_0004)
        .follow(FollowSymlinks::No);
    cap_primitives::fs::open(parent, Path::new(name), &options)
}

#[cfg(unix)]
pub(super) fn hard_link(
    source_parent: &File,
    source_name: &OsStr,
    destination_parent: &File,
    destination_name: &OsStr,
) -> io::Result<()> {
    rustix::fs::linkat(
        source_parent,
        source_name,
        destination_parent,
        destination_name,
        rustix::fs::AtFlags::empty(),
    )
    .map_err(io::Error::from)
}

#[cfg(windows)]
pub(super) fn hard_link(
    source_parent: &File,
    source_name: &OsStr,
    destination_parent: &File,
    destination_name: &OsStr,
) -> io::Result<()> {
    cap_primitives::fs::hard_link(
        source_parent,
        Path::new(source_name),
        destination_parent,
        Path::new(destination_name),
    )
}

#[cfg(unix)]
pub(super) fn remove_file(parent: &File, name: &OsStr) -> io::Result<()> {
    rustix::fs::unlinkat(parent, name, rustix::fs::AtFlags::empty()).map_err(io::Error::from)
}

#[cfg(windows)]
pub(super) fn remove_file(parent: &File, name: &OsStr) -> io::Result<()> {
    cap_primitives::fs::remove_file(parent, Path::new(name))
}
