use std::{ffi::OsStr, fs::File, io};

#[cfg(windows)]
use std::path::Path;

#[cfg(unix)]
pub(super) fn open_directory_for_publish(parent: &File, name: &OsStr) -> io::Result<File> {
    use rustix::fs::{Mode, OFlags};

    rustix::fs::openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(io::Error::from)
}

#[cfg(windows)]
pub(super) fn open_directory_for_publish(parent: &File, name: &OsStr) -> io::Result<File> {
    let mut options = fs_at::OpenOptions::default();
    options.read(true).open_dir_at(parent, Path::new(name))
}

#[cfg(unix)]
pub(super) fn open_directory_for_cleanup(parent: &File, name: &OsStr) -> io::Result<File> {
    open_directory_for_publish(parent, name)
}

#[cfg(windows)]
pub(super) fn open_directory_for_cleanup(parent: &File, name: &OsStr) -> io::Result<File> {
    use cap_fs_ext::OpenOptionsFollowExt as _;
    use cap_primitives::fs::{FollowSymlinks, OpenOptions, OpenOptionsExt as _};
    use winx::file::AccessMode;

    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_SHARE_READ: u32 = 0x0000_0001;
    const FILE_SHARE_WRITE: u32 = 0x0000_0002;
    const FILE_SHARE_DELETE: u32 = 0x0000_0004;
    let mut options = OpenOptions::new();
    options
        .read(true)
        .access_mode((AccessMode::GENERIC_READ | AccessMode::DELETE).bits())
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .follow(FollowSymlinks::No);
    cap_primitives::fs::open(parent, Path::new(name), &options)
}

#[cfg(unix)]
pub(super) fn remove_verified_directory(parent: &File, name: &OsStr, held: File) -> io::Result<()> {
    rustix::fs::unlinkat(parent, name, rustix::fs::AtFlags::REMOVEDIR).map_err(io::Error::from)?;
    drop(held);
    Ok(())
}

#[cfg(windows)]
pub(super) fn remove_verified_directory(
    _parent: &File,
    _name: &OsStr,
    held: File,
) -> io::Result<()> {
    use fs_at::os::windows::FileExt as _;

    held.delete_by_handle().map_err(|(_, error)| error)
}

#[cfg(any(target_os = "linux", target_os = "android", target_os = "macos"))]
pub(super) fn rename_directory_no_replace(
    source_parent: &File,
    source_name: &OsStr,
    destination_parent: &File,
    destination_name: &OsStr,
) -> io::Result<()> {
    rustix::fs::renameat_with(
        source_parent,
        source_name,
        destination_parent,
        destination_name,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(io::Error::from)
}

#[cfg(all(
    unix,
    not(any(target_os = "linux", target_os = "android", target_os = "macos"))
))]
pub(super) fn rename_directory_no_replace(
    _source_parent: &File,
    _source_name: &OsStr,
    _destination_parent: &File,
    _destination_name: &OsStr,
) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "atomic no-replace directory publication is unavailable",
    ))
}

#[cfg(windows)]
pub(super) fn rename_directory_no_replace(
    source_parent: &File,
    source_name: &OsStr,
    destination_parent: &File,
    destination_name: &OsStr,
) -> io::Result<()> {
    cap_primitives::fs::rename(
        source_parent,
        Path::new(source_name),
        destination_parent,
        Path::new(destination_name),
    )
}
