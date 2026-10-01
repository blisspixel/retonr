use super::super::{FileState, RetainedParent};
use rustix::fs::{Mode, OFlags};
use std::os::unix::fs::MetadataExt as _;
use std::{
    fs::{File, Metadata},
    io,
    path::{Component, Path},
};

pub(crate) type FileIdentity = (u64, u64);
pub(crate) type ChangeStamp = (i64, i64);

pub(crate) fn open_followed(path: &Path) -> io::Result<File> {
    rustix::fs::open(
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(io::Error::from)
}

pub(crate) fn open_direct(path: &Path) -> io::Result<(File, Vec<RetainedParent>)> {
    let mut components = path.components().peekable();
    if components.next() != Some(Component::RootDir) {
        return Err(super::super::invalid_input());
    }
    let root = rustix::fs::open(
        "/",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(io::Error::from)?;
    let mut parents = vec![RetainedParent::new(root, std::path::PathBuf::from("/"))?];
    let mut selected = std::path::PathBuf::from("/");
    while let Some(component) = components.next() {
        selected.push(component);
        let mut flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK;
        let is_parent = components.peek().is_some();
        if is_parent {
            flags |= OFlags::DIRECTORY;
        }
        let file = rustix::fs::openat(
            &parents.last().expect("root retained").file,
            component.as_os_str(),
            flags,
            Mode::empty(),
        )
        .map(File::from)
        .map_err(io::Error::from)?;
        if !is_parent {
            return Ok((file, parents));
        }
        parents.push(RetainedParent::new(file, selected.clone())?);
    }
    Err(super::super::invalid_input())
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "Windows file identity requires a fallible handle query"
)]
pub(crate) fn file_identity(_file: &File, metadata: &Metadata) -> io::Result<FileIdentity> {
    Ok((metadata.dev(), metadata.ino()))
}

pub(crate) fn change(metadata: &Metadata) -> ChangeStamp {
    (metadata.ctime(), metadata.ctime_nsec())
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "Windows link counts require a fallible handle query"
)]
pub(crate) fn links(_file: &File, metadata: &Metadata) -> io::Result<u64> {
    Ok(metadata.nlink())
}
pub(crate) fn is_indirect(metadata: &Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "Windows path state requires opening and querying a handle"
)]
pub(crate) fn path_state(_path: &Path, metadata: &Metadata) -> io::Result<FileState> {
    Ok(FileState {
        identity: (metadata.dev(), metadata.ino()),
        length: metadata.len(),
        modified: metadata.modified().ok(),
        links: metadata.nlink(),
        change: (metadata.ctime(), metadata.ctime_nsec()),
    })
}
