use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{File, Metadata},
    io::Read as _,
    os::unix::fs::MetadataExt as _,
};

use rustix::fs::{Dir, Mode, OFlags, ResolveFlags, openat2};
use sha2::{Digest as _, Sha256};

use super::{HelperFailure, require_read_only_mount};

const MAXIMUM_ENTRIES: usize = 262_144;
const MAXIMUM_DIRECTORY_ENTRIES: usize = 65_536;
const MAXIMUM_FILES: usize = 32_768;
const MAXIMUM_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAXIMUM_FILE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAXIMUM_PATH_BYTES: usize = 256 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Stamp {
    device: u64,
    inode: u64,
    mode: u32,
    links: u64,
    pub(super) length: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}

impl Stamp {
    pub(super) fn from_file(file: &File) -> Result<Self, HelperFailure> {
        let metadata = file
            .metadata()
            .map_err(|_| HelperFailure::BootstrapRootVerification)?;
        if !(metadata.is_dir() || metadata.is_file() && metadata.nlink() == 1) {
            return Err(HelperFailure::BootstrapRootVerification);
        }
        Ok(Self::from_metadata(&metadata))
    }

    fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            mode: metadata.mode(),
            links: metadata.nlink(),
            length: metadata.len(),
            modified: (metadata.mtime(), metadata.mtime_nsec()),
            changed: (metadata.ctime(), metadata.ctime_nsec()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum EntryKind {
    Directory,
    File,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct Entry {
    pub(super) kind: EntryKind,
    pub(super) stamp: Stamp,
    pub(super) digest: Option<[u8; 32]>,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct Snapshot {
    root: Stamp,
    pub(super) entries: BTreeMap<String, Entry>,
}

impl Snapshot {
    pub(super) fn acquire(root: &File) -> Result<Self, HelperFailure> {
        require_read_only_mount(root)?;
        inventory(root)
    }
}

fn inventory(root: &File) -> Result<Snapshot, HelperFailure> {
    let root_stamp = Stamp::from_file(root)?;
    require_directory(root)?;
    let mut entries = BTreeMap::new();
    let mut pending = vec![String::new()];
    let mut identities = BTreeSet::new();
    let mut bytes = 0_u64;
    let mut paths = 0_usize;
    let mut file_count = 0_usize;
    while let Some(relative) = pending.pop() {
        let directory = open_directory(root, &relative)?;
        let before = Stamp::from_file(&directory)?;
        let children =
            Dir::read_from(&directory).map_err(|_| HelperFailure::BootstrapRootVerification)?;
        let mut child_count = 0_usize;
        for child in children {
            let child = child.map_err(|_| HelperFailure::BootstrapRootVerification)?;
            let name = child
                .file_name()
                .to_str()
                .map_err(|_| HelperFailure::BootstrapRootVerification)?;
            if matches!(name, "." | "..") {
                continue;
            }
            child_count += 1;
            if child_count > MAXIMUM_DIRECTORY_ENTRIES {
                return Err(HelperFailure::BootstrapRootVerification);
            }
            let path = if relative.is_empty() {
                name.to_owned()
            } else {
                format!("{relative}/{name}")
            };
            validate_path(&path)?;
            paths = paths
                .checked_add(path.len())
                .filter(|total| *total <= MAXIMUM_PATH_BYTES)
                .ok_or(HelperFailure::BootstrapRootVerification)?;
            let mut file = open_relative(root, &path)?;
            let stamp = Stamp::from_file(&file)?;
            if stamp.device != root_stamp.device || !identities.insert((stamp.device, stamp.inode))
            {
                return Err(HelperFailure::BootstrapRootVerification);
            }
            let metadata = file
                .metadata()
                .map_err(|_| HelperFailure::BootstrapRootVerification)?;
            let (kind, digest) = if metadata.is_dir() {
                pending.push(path.clone());
                (EntryKind::Directory, None)
            } else {
                file_count += 1;
                if file_count > MAXIMUM_FILES {
                    return Err(HelperFailure::BootstrapRootVerification);
                }
                bytes = bytes
                    .checked_add(stamp.length)
                    .filter(|total| *total <= MAXIMUM_BYTES)
                    .ok_or(HelperFailure::BootstrapRootVerification)?;
                if stamp.length > MAXIMUM_FILE_BYTES {
                    return Err(HelperFailure::BootstrapRootVerification);
                }
                let (digest, measured) = hash_file(&mut file)?;
                if measured != stamp.length {
                    return Err(HelperFailure::BootstrapRootVerification);
                }
                (EntryKind::File, Some(digest))
            };
            if Stamp::from_file(&file)? != stamp
                || Stamp::from_file(&open_relative(root, &path)?)? != stamp
                || entries
                    .insert(
                        path,
                        Entry {
                            kind,
                            stamp,
                            digest,
                        },
                    )
                    .is_some()
                || entries.len() > MAXIMUM_ENTRIES
            {
                return Err(HelperFailure::BootstrapRootVerification);
            }
        }
        if Stamp::from_file(&directory)? != before {
            return Err(HelperFailure::BootstrapRootVerification);
        }
    }
    if entries.is_empty() || Stamp::from_file(root)? != root_stamp {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    Ok(Snapshot {
        root: root_stamp,
        entries,
    })
}

fn open_directory(root: &File, relative: &str) -> Result<File, HelperFailure> {
    if relative.is_empty() {
        root.try_clone()
            .map_err(|_| HelperFailure::BootstrapRootVerification)
    } else {
        open_relative(root, relative)
    }
}

fn require_directory(root: &File) -> Result<(), HelperFailure> {
    if root
        .metadata()
        .map_err(|_| HelperFailure::BootstrapRootVerification)?
        .is_dir()
    {
        Ok(())
    } else {
        Err(HelperFailure::BootstrapRootVerification)
    }
}

fn validate_path(path: &str) -> Result<(), HelperFailure> {
    if path.is_empty()
        || path.len() > 4096
        || path.contains('\\')
        || path.chars().any(char::is_control)
        || path
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
    {
        Err(HelperFailure::BootstrapRootVerification)
    } else {
        Ok(())
    }
}

pub(super) fn open_relative(root: &File, relative: &str) -> Result<File, HelperFailure> {
    validate_path(relative)?;
    let file = openat2(
        root,
        relative,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
        ResolveFlags::BENEATH
            | ResolveFlags::NO_MAGICLINKS
            | ResolveFlags::NO_SYMLINKS
            | ResolveFlags::NO_XDEV,
    )
    .map(File::from)
    .map_err(|_| HelperFailure::BootstrapRootVerification)?;
    Stamp::from_file(&file)?;
    Ok(file)
}

pub(super) fn create_output(parent: &File, name: &str) -> Result<File, HelperFailure> {
    validate_path(name)?;
    if name.contains('/') {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    openat2(
        parent,
        name,
        OFlags::RDWR
            | OFlags::CREATE
            | OFlags::EXCL
            | OFlags::CLOEXEC
            | OFlags::NOFOLLOW
            | OFlags::NONBLOCK,
        Mode::RUSR | Mode::WUSR,
        ResolveFlags::BENEATH
            | ResolveFlags::NO_MAGICLINKS
            | ResolveFlags::NO_SYMLINKS
            | ResolveFlags::NO_XDEV,
    )
    .map(File::from)
    .map_err(|_| HelperFailure::BootstrapRootPreparation)
}

pub(super) fn hash_file(file: &mut File) -> Result<([u8; 32], u64), HelperFailure> {
    let mut hash = Sha256::new();
    let mut bytes = 0_u64;
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| HelperFailure::BootstrapRootVerification)?;
        if read == 0 {
            return Ok((hash.finalize().into(), bytes));
        }
        bytes = bytes
            .checked_add(read as u64)
            .filter(|total| {
                *total <= MAXIMUM_BYTES + MAXIMUM_PATH_BYTES as u64 + 512 * MAXIMUM_ENTRIES as u64
            })
            .ok_or(HelperFailure::BootstrapRootVerification)?;
        hash.update(&buffer[..read]);
    }
}

#[cfg(test)]
mod tests;
