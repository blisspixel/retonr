use std::{
    fs::File,
    io::{Read as _, Seek as _, SeekFrom, Write as _},
    os::unix::fs::MetadataExt as _,
};

use rustix::fs::{Dir, Mode, OFlags, ResolveFlags, openat2};
use sha2::{Digest as _, Sha256};

use super::{HelperFailure, recipe};
use crate::platform::linux_executable::has_elf_magic;

const MAXIMUM_PROGRAM_BYTES: u64 = 128 * 1024 * 1024;
const RESOLUTION: ResolveFlags = ResolveFlags::BENEATH
    .union(ResolveFlags::NO_MAGICLINKS)
    .union(ResolveFlags::NO_SYMLINKS)
    .union(ResolveFlags::NO_XDEV);

pub(super) fn open_regular(
    root: &File,
    path: &str,
    executable: bool,
) -> Result<File, HelperFailure> {
    let file = openat2(
        root,
        path,
        OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
        RESOLUTION,
    )
    .map(File::from)
    .map_err(|_| HelperFailure::BootstrapRootVerification)?;
    let metadata = file
        .metadata()
        .map_err(|_| HelperFailure::BootstrapRootVerification)?;
    if !metadata.is_file()
        || metadata.nlink() != 1
        || (executable
            && (metadata.mode() & 0o111 == 0
                || metadata.len() == 0
                || metadata.len() > MAXIMUM_PROGRAM_BYTES))
    {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    Ok(file)
}

pub(super) fn publish(target: &File, output: &File) -> Result<(), HelperFailure> {
    for entry in Dir::read_from(output).map_err(|_| HelperFailure::BootstrapRootVerification)? {
        let entry = entry.map_err(|_| HelperFailure::BootstrapRootVerification)?;
        if !matches!(entry.file_name().to_bytes(), b"." | b"..") {
            return Err(HelperFailure::BootstrapRootVerification);
        }
    }
    let candidates = recipe::BUILDS
        .map(|build| {
            let file = open_regular(target, &build.output(), true)?;
            if !has_elf_magic(&file).map_err(|_| HelperFailure::BootstrapRootVerification)? {
                return Err(HelperFailure::BootstrapRootVerification);
            }
            Ok(file)
        })
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    for (build, source) in recipe::BUILDS.into_iter().zip(candidates) {
        copy(&source, output, build.program)?;
    }
    output
        .sync_all()
        .map_err(|_| HelperFailure::BootstrapRootPreparation)
}

fn copy(source: &File, output: &File, name: &str) -> Result<(), HelperFailure> {
    let before = stamp(source)?;
    let mut source = source
        .try_clone()
        .map_err(|_| HelperFailure::BootstrapRootVerification)?;
    source
        .seek(SeekFrom::Start(0))
        .map_err(|_| HelperFailure::BootstrapRootVerification)?;
    let mut destination = openat2(
        output,
        name,
        OFlags::RDWR
            | OFlags::CREATE
            | OFlags::EXCL
            | OFlags::NONBLOCK
            | OFlags::NOFOLLOW
            | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o755),
        RESOLUTION,
    )
    .map(File::from)
    .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    let mut hasher = Sha256::new();
    let mut bytes = 0_u64;
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = source
            .read(&mut buffer)
            .map_err(|_| HelperFailure::BootstrapRootVerification)?;
        if read == 0 {
            break;
        }
        bytes = bytes
            .checked_add(u64::try_from(read).map_err(|_| HelperFailure::BootstrapRootVerification)?)
            .filter(|bytes| *bytes <= before.bytes)
            .ok_or(HelperFailure::BootstrapRootVerification)?;
        hasher.update(&buffer[..read]);
        destination
            .write_all(&buffer[..read])
            .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    }
    if bytes != before.bytes || stamp(&source)? != before {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    destination
        .sync_all()
        .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    destination
        .seek(SeekFrom::Start(0))
        .map_err(|_| HelperFailure::BootstrapRootVerification)?;
    let mut readback = Sha256::new();
    loop {
        let read = destination
            .read(&mut buffer)
            .map_err(|_| HelperFailure::BootstrapRootVerification)?;
        if read == 0 {
            break;
        }
        readback.update(&buffer[..read]);
    }
    if readback.finalize() != hasher.finalize()
        || destination
            .metadata()
            .map_err(|_| HelperFailure::BootstrapRootVerification)?
            .len()
            != bytes
        || stamp(&source)? != before
    {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    Ok(())
}

#[derive(Eq, PartialEq)]
struct Stamp {
    device: u64,
    inode: u64,
    mode: u32,
    links: u64,
    bytes: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}

fn stamp(file: &File) -> Result<Stamp, HelperFailure> {
    let metadata = file
        .metadata()
        .map_err(|_| HelperFailure::BootstrapRootVerification)?;
    Ok(Stamp {
        device: metadata.dev(),
        inode: metadata.ino(),
        mode: metadata.mode(),
        links: metadata.nlink(),
        bytes: metadata.len(),
        modified: (metadata.mtime(), metadata.mtime_nsec()),
        changed: (metadata.ctime(), metadata.ctime_nsec()),
    })
}

#[cfg(test)]
mod tests;
