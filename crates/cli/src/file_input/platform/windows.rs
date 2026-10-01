use super::super::{FileState, RetainedParent};
use std::os::windows::fs::{MetadataExt as _, OpenOptionsExt as _};
use std::{
    fs::{File, Metadata, OpenOptions},
    io,
    path::{Path, PathBuf},
};

pub(crate) type FileIdentity = (u64, u64);
pub(crate) type ChangeStamp = (u64, u64);
const OPEN_REPARSE: u32 = 0x0020_0000;
const BACKUP_SEMANTICS: u32 = 0x0200_0000;
const SHARE_READ_WRITE: u32 = 0x0000_0003;

pub(crate) fn open_followed(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(BACKUP_SEMANTICS)
        .open(path)
}

pub(crate) fn open_direct(path: &Path) -> io::Result<(File, Vec<RetainedParent>)> {
    let mut components = path.components().peekable();
    let mut selected = PathBuf::new();
    let mut parents = Vec::new();
    while let Some(component) = components.next() {
        selected.push(component);
        if matches!(component, std::path::Component::Prefix(_)) {
            continue;
        }
        let file = OpenOptions::new()
            .read(true)
            .share_mode(SHARE_READ_WRITE)
            .custom_flags(OPEN_REPARSE | BACKUP_SEMANTICS)
            .open(&selected)?;
        let metadata = file.metadata()?;
        if is_indirect(&metadata) {
            return Err(super::super::invalid_input());
        }
        if components.peek().is_none() {
            return Ok((file, parents));
        }
        if !metadata.is_dir() {
            return Err(super::super::invalid_input());
        }
        parents.push(RetainedParent::new(file, selected.clone())?);
    }
    Err(super::super::invalid_input())
}

pub(crate) fn file_identity(file: &File, _metadata: &Metadata) -> io::Result<FileIdentity> {
    let information = winx::winapi_util::file::information(file)?;
    Ok((information.volume_serial_number(), information.file_index()))
}

pub(crate) fn change(metadata: &Metadata) -> ChangeStamp {
    (metadata.creation_time(), metadata.last_write_time())
}

pub(crate) fn links(file: &File, _metadata: &Metadata) -> io::Result<u64> {
    winx::winapi_util::file::information(file).map(|information| information.number_of_links())
}

pub(crate) fn is_indirect(metadata: &Metadata) -> bool {
    metadata.file_attributes() & 0x0000_0400 != 0
}

pub(crate) fn path_state(path: &Path, _metadata: &Metadata) -> io::Result<FileState> {
    super::super::file_state(&open_followed(path)?)
}
