use std::{ffi::OsStr, fs::File, io};

#[cfg(test)]
use std::cell::Cell;

#[cfg(windows)]
use std::path::Path;

#[cfg(test)]
thread_local! {
    static FAIL_NO_REPLACE_RENAME_ONCE: Cell<bool> = const { Cell::new(false) };
}

#[cfg(test)]
pub(in crate::artifact_storage::tree) fn inject_no_replace_rename_failure_once() {
    FAIL_NO_REPLACE_RENAME_ONCE.with(|fail| fail.set(true));
}

#[cfg(test)]
fn take_no_replace_rename_failure() -> bool {
    FAIL_NO_REPLACE_RENAME_ONCE.with(|fail| fail.replace(false))
}

#[cfg(not(test))]
const fn take_no_replace_rename_failure() -> bool {
    false
}

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
    options
        .read(true)
        .follow(false)
        .open_dir_at(parent, Path::new(name))
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
    if take_no_replace_rename_failure() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "injected no-replace rename failure",
        ));
    }
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
    if take_no_replace_rename_failure() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "injected no-replace rename failure",
        ));
    }
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
    if take_no_replace_rename_failure() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "injected no-replace rename failure",
        ));
    }
    let source_parent_path = winx::file::get_file_path(source_parent)?;
    let destination_parent_path = winx::file::get_file_path(destination_parent)?;
    let source_path = source_parent_path.join(source_name);
    let destination_path = destination_parent_path.join(destination_name);
    let source = source_path.to_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "source path is not valid Unicode",
        )
    })?;
    let destination = destination_path.to_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "destination path is not valid Unicode",
        )
    })?;
    // Omitting REPLACE_EXISTING is the Windows kernel's no-replace contract.
    // COPY_ALLOWED is also omitted so a cross-volume move cannot become copy-and-delete.
    winsafe::MoveFileEx(
        source,
        Some(destination),
        winsafe::co::MOVEFILE::WRITE_THROUGH,
    )
    .map_err(|error| {
        let raw = i32::from_ne_bytes(u32::from(error).to_ne_bytes());
        io::Error::from_raw_os_error(raw)
    })
}
