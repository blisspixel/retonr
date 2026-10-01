//! Platform-specific safe regular-file opens and object identities.

#[cfg(not(any(unix, windows)))]
use super::{FileState, RetainedParent};
#[cfg(not(any(unix, windows)))]
use std::{
    fs::{File, Metadata},
    io,
    path::Path,
};

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(unix)]
pub(super) use unix::*;
#[cfg(windows)]
pub(super) use windows::*;

#[cfg(not(any(unix, windows)))]
mod unsupported {
    use super::*;
    pub(crate) type FileIdentity = ();
    pub(crate) type ChangeStamp = ();
    pub(crate) fn open_followed(path: &Path) -> io::Result<File> {
        File::open(path)
    }
    pub(crate) fn open_direct(_path: &Path) -> io::Result<(File, Vec<RetainedParent>)> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "direct file input is unsupported",
        ))
    }
    pub(crate) fn file_identity(_file: &File, _metadata: &Metadata) -> io::Result<FileIdentity> {
        Ok(())
    }
    pub(crate) fn change(_metadata: &Metadata) -> ChangeStamp {}
    pub(crate) fn links(_file: &File, _metadata: &Metadata) -> io::Result<u64> {
        Ok(1)
    }
    pub(crate) fn is_indirect(metadata: &Metadata) -> bool {
        metadata.file_type().is_symlink()
    }
    pub(crate) fn path_state(path: &Path, _metadata: &Metadata) -> io::Result<FileState> {
        super::super::file_state(&File::open(path)?)
    }
}
#[cfg(not(any(unix, windows)))]
pub(super) use unsupported::*;
