use std::{fs::File, io};

#[cfg(unix)]
pub(super) fn sync_directory(directory: &File) -> io::Result<()> {
    directory.sync_all()
}

#[cfg(windows)]
pub(super) fn sync_directory(directory: &File) -> io::Result<()> {
    use std::path::Path;

    use cap_fs_ext::OpenOptionsFollowExt as _;
    use cap_primitives::fs::{FollowSymlinks, OpenOptions, OpenOptionsExt as _};

    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_SHARE_READ: u32 = 0x0000_0001;
    const FILE_SHARE_WRITE: u32 = 0x0000_0002;
    const FILE_SHARE_DELETE: u32 = 0x0000_0004;
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .follow(FollowSymlinks::No);
    cap_primitives::fs::open(directory, Path::new("."), &options)?.sync_all()
}
