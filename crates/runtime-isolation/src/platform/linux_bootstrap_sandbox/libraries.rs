use std::{fs::File, os::unix::fs::MetadataExt as _};

use landlock::{AccessFs, BitFlags, make_bitflags};
use rustix::fs::{Mode, OFlags, ResolveFlags, openat2};

use super::{HelperFailure, require_read_only_mount};
use crate::platform::linux_executable::has_elf_magic;

const LOADER: &str = "lib/ld-musl-x86_64.so.1";
const LIBGCC: &str = "usr/lib/libgcc_s.so.1";

pub(super) fn open() -> Result<[(File, BitFlags<AccessFs>); 2], HelperFailure> {
    let root = File::open("/").map_err(|_| HelperFailure::BootstrapRootVerification)?;
    open_at(&root)
}

fn open_at(root: &File) -> Result<[(File, BitFlags<AccessFs>); 2], HelperFailure> {
    // Root preparation has already joined these exact regular payloads and their
    // relative linker aliases to the authenticated Alpine and signed APK bytes.
    // Rust distribution executables name the loader as PT_INTERP; the linker
    // needs the payloads readable. No enclosing host directory is permitted.
    Ok([
        (
            payload(root, LOADER)?,
            make_bitflags!(AccessFs::{Execute | ReadFile}),
        ),
        (payload(root, LIBGCC)?, AccessFs::ReadFile.into()),
    ])
}

fn payload(root: &File, path: &str) -> Result<File, HelperFailure> {
    let file = openat2(
        root,
        path,
        OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::BENEATH
            | ResolveFlags::NO_SYMLINKS
            | ResolveFlags::NO_MAGICLINKS
            | ResolveFlags::NO_XDEV,
    )
    .map(File::from)
    .map_err(|_| HelperFailure::BootstrapRootVerification)?;
    let metadata = file
        .metadata()
        .map_err(|_| HelperFailure::BootstrapRootVerification)?;
    if !metadata.is_file()
        || metadata.nlink() != 1
        || !has_elf_magic(&file).map_err(|_| HelperFailure::BootstrapRootVerification)?
    {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    require_read_only_mount(&file)?;
    Ok(file)
}

#[cfg(test)]
mod tests;
