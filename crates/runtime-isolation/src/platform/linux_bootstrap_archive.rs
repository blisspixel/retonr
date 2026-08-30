use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek as _, SeekFrom, Write as _},
    os::{
        fd::AsRawFd as _,
        unix::fs::{OpenOptionsExt as _, PermissionsExt as _},
        unix::process::CommandExt as _,
    },
    path::Path,
    process::{Child, Command, Stdio},
};

use rewrite_types::Digest as DomainDigest;
use rustix::fs::{Mode, OFlags, ResolveFlags, openat2};

use super::linux_helper_setup::HelperFailure;

mod plan;

use plan::{ArchivePlan, EntryKind};

const TAR_BLOCK_BYTES: u64 = 512;
const COPY_BUFFER_BYTES: usize = 64 * 1024;
const MAXIMUM_PATH_BYTES: usize = 4_096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Compression {
    None,
    Gzip,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ArchiveLimits {
    pub(super) entries: usize,
    pub(super) file_bytes: u64,
    pub(super) total_bytes: u64,
    pub(super) stream_bytes: u64,
    pub(super) links: usize,
    pub(super) link_bytes: usize,
    pub(super) link_depth: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LinkPolicy {
    Reject,
    AlpineMinirootfs,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ArchiveSummary {
    pub(super) entries: usize,
    pub(super) regular_bytes: u64,
    pub(super) link_plan_digest: DomainDigest,
    pub(super) deferred_proc_mtab: bool,
}

pub(super) fn extract(
    busybox: &File,
    archive: &File,
    compression: Compression,
    destination: &Path,
    required_root: Option<&str>,
    link_policy: LinkPolicy,
    limits: ArchiveLimits,
) -> Result<ArchiveSummary, HelperFailure> {
    require_empty_directory(destination)?;
    let temporary_path = destination.join(".retonr-validated-archive");
    let mut temporary = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary_path)
        .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    let result = (|| {
        decompress(busybox, archive, compression, &mut temporary, limits)?;
        let plan = plan::validate(&mut temporary, required_root, link_policy, limits)?;
        extract_plan(
            &mut temporary,
            destination,
            required_root,
            link_policy,
            limits,
            &plan,
        )?;
        Ok(ArchiveSummary {
            entries: plan.entries.len(),
            regular_bytes: plan.regular_bytes,
            link_plan_digest: plan.link_plan_digest,
            deferred_proc_mtab: plan.deferred_proc_mtab,
        })
    })();
    drop(temporary);
    fs::remove_file(&temporary_path).map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    result
}

fn decompress(
    busybox: &File,
    archive: &File,
    compression: Compression,
    destination: &mut File,
    limits: ArchiveLimits,
) -> Result<(), HelperFailure> {
    let (applet, decompress_argument) = match compression {
        Compression::None => ("cat", None),
        Compression::Gzip => ("gzip", Some("-dc")),
    };
    let mut command = Command::new(format!("/proc/self/fd/{}", busybox.as_raw_fd()));
    command.arg0("busybox");
    command.arg(applet);
    if let Some(argument) = decompress_argument {
        command.arg(argument);
    }
    let mut archive_input = archive
        .try_clone()
        .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    archive_input
        .seek(SeekFrom::Start(0))
        .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    command
        .stdin(Stdio::from(archive_input))
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command
        .spawn()
        .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    let result = copy_bounded_child_output(&mut child, destination, limits.stream_bytes);
    finish_child(child, result)
}

fn copy_bounded_child_output(
    child: &mut Child,
    destination: &mut File,
    maximum_bytes: u64,
) -> Result<(), HelperFailure> {
    let mut output = child
        .stdout
        .take()
        .ok_or(HelperFailure::BootstrapRootPreparation)?;
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    let mut copied = 0_u64;
    loop {
        let read = output
            .read(&mut buffer)
            .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
        if read == 0 {
            break;
        }
        copied = copied
            .checked_add(u64::try_from(read).map_err(|_| HelperFailure::BootstrapRootPreparation)?)
            .filter(|bytes| *bytes <= maximum_bytes)
            .ok_or(HelperFailure::BootstrapRootVerification)?;
        destination
            .write_all(&buffer[..read])
            .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    }
    destination
        .sync_data()
        .map_err(|_| HelperFailure::BootstrapRootPreparation)
}

fn finish_child(mut child: Child, result: Result<(), HelperFailure>) -> Result<(), HelperFailure> {
    if result.is_err() {
        let _ = child.kill();
    }
    let status = child
        .wait()
        .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    result?;
    if status.success() {
        Ok(())
    } else {
        Err(HelperFailure::BootstrapRootVerification)
    }
}

fn extract_plan(
    archive: &mut File,
    destination: &Path,
    required_root: Option<&str>,
    link_policy: LinkPolicy,
    limits: ArchiveLimits,
    plan: &ArchivePlan,
) -> Result<(), HelperFailure> {
    let root = File::open(destination).map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    create_directories(destination, plan)?;
    extract_regular_entries(archive, &root, required_root, link_policy, limits, plan)?;
    materialize_hardlinks(&root, plan)?;
    create_symlinks(destination, plan)?;
    validate_symlinks(destination, plan)?;
    finalize_directory_modes(destination, plan)
}

fn create_directories(destination: &Path, plan: &ArchivePlan) -> Result<(), HelperFailure> {
    for entry in plan.entries.iter().filter(|entry| {
        entry.kind == EntryKind::Directory && !entry.relative.as_os_str().is_empty()
    }) {
        let path = destination.join(&entry.relative);
        fs::create_dir(&path).map_err(|_| HelperFailure::BootstrapRootPreparation)?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    }
    Ok(())
}

fn extract_regular_entries(
    archive: &mut File,
    root: &File,
    required_root: Option<&str>,
    link_policy: LinkPolicy,
    limits: ArchiveLimits,
    plan: &ArchivePlan,
) -> Result<(), HelperFailure> {
    archive
        .seek(SeekFrom::Start(0))
        .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    let mut tar = tar::Archive::new(&mut *archive);
    let mut entries = tar
        .entries()
        .map_err(|_| HelperFailure::BootstrapRootVerification)?;
    for expected in &plan.entries {
        let raw = entries
            .next()
            .ok_or(HelperFailure::BootstrapRootVerification)?;
        let mut entry = raw.map_err(|_| HelperFailure::BootstrapRootVerification)?;
        if plan::plan_entry(&entry, required_root, link_policy, limits)? != *expected {
            return Err(HelperFailure::BootstrapRootVerification);
        }
        if expected.kind == EntryKind::Regular {
            copy_regular_entry(root, expected, &mut entry)?;
        }
    }
    if entries.next().is_some() {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    Ok(())
}

fn copy_regular_entry<R: Read>(
    root: &File,
    expected: &plan::PlannedEntry,
    entry: &mut tar::Entry<'_, R>,
) -> Result<(), HelperFailure> {
    let mode = Mode::from_bits(expected.mode).ok_or(HelperFailure::BootstrapRootVerification)?;
    let mut output = create_regular(root, &expected.relative, mode)?;
    let copied =
        std::io::copy(entry, &mut output).map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    if copied != expected.bytes {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    output
        .set_permissions(fs::Permissions::from_mode(expected.mode))
        .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    output
        .sync_data()
        .map_err(|_| HelperFailure::BootstrapRootPreparation)
}

fn create_regular(root: &File, path: &Path, mode: Mode) -> Result<File, HelperFailure> {
    openat2(
        root,
        path,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        mode,
        ResolveFlags::BENEATH
            | ResolveFlags::NO_MAGICLINKS
            | ResolveFlags::NO_SYMLINKS
            | ResolveFlags::NO_XDEV,
    )
    .map(File::from)
    .map_err(|_| HelperFailure::BootstrapRootPreparation)
}

fn materialize_hardlinks(root: &File, plan: &ArchivePlan) -> Result<(), HelperFailure> {
    for entry in plan
        .entries
        .iter()
        .filter(|entry| entry.kind == EntryKind::Hardlink)
    {
        let link = entry
            .link
            .as_ref()
            .ok_or(HelperFailure::BootstrapRootVerification)?;
        let mut source = openat2(
            root,
            &link.resolved_target,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::empty(),
            ResolveFlags::BENEATH
                | ResolveFlags::NO_MAGICLINKS
                | ResolveFlags::NO_SYMLINKS
                | ResolveFlags::NO_XDEV,
        )
        .map(File::from)
        .map_err(|_| HelperFailure::BootstrapRootVerification)?;
        let mode = Mode::from_bits(entry.mode).ok_or(HelperFailure::BootstrapRootVerification)?;
        let mut output = create_regular(root, &entry.relative, mode)?;
        std::io::copy(&mut source, &mut output)
            .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
        output
            .set_permissions(fs::Permissions::from_mode(entry.mode))
            .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
        output
            .sync_data()
            .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    }
    Ok(())
}

fn create_symlinks(destination: &Path, plan: &ArchivePlan) -> Result<(), HelperFailure> {
    for entry in plan
        .entries
        .iter()
        .filter(|entry| entry.kind == EntryKind::Symlink)
    {
        let link = entry
            .link
            .as_ref()
            .ok_or(HelperFailure::BootstrapRootVerification)?;
        std::os::unix::fs::symlink(&link.stored_target, destination.join(&entry.relative))
            .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    }
    Ok(())
}

fn validate_symlinks(destination: &Path, plan: &ArchivePlan) -> Result<(), HelperFailure> {
    for entry in plan
        .entries
        .iter()
        .filter(|entry| entry.kind == EntryKind::Symlink)
    {
        let link = entry
            .link
            .as_ref()
            .ok_or(HelperFailure::BootstrapRootVerification)?;
        let path = destination.join(&entry.relative);
        let link_type = fs::symlink_metadata(&path)
            .map_err(|_| HelperFailure::BootstrapRootVerification)?
            .file_type();
        let resolved_type = if link.deferred_proc_mtab {
            None
        } else {
            Some(
                fs::metadata(&path)
                    .map_err(|_| HelperFailure::BootstrapRootVerification)?
                    .file_type(),
            )
        };
        if !link_type.is_symlink()
            || fs::read_link(&path).map_err(|_| HelperFailure::BootstrapRootVerification)?
                != link.stored_target
            || resolved_type.is_some_and(|kind| !kind.is_file() && !kind.is_dir())
        {
            return Err(HelperFailure::BootstrapRootVerification);
        }
    }
    Ok(())
}

fn finalize_directory_modes(destination: &Path, plan: &ArchivePlan) -> Result<(), HelperFailure> {
    for entry in plan.entries.iter().rev().filter(|entry| {
        entry.kind == EntryKind::Directory && !entry.relative.as_os_str().is_empty()
    }) {
        fs::set_permissions(
            destination.join(&entry.relative),
            fs::Permissions::from_mode(entry.mode),
        )
        .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    }
    Ok(())
}

fn require_empty_directory(path: &Path) -> Result<(), HelperFailure> {
    if !path
        .metadata()
        .map_err(|_| HelperFailure::BootstrapRootPreparation)?
        .is_dir()
        || fs::read_dir(path)
            .map_err(|_| HelperFailure::BootstrapRootPreparation)?
            .next()
            .is_some()
    {
        Err(HelperFailure::BootstrapRootVerification)
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "linux_bootstrap_archive/tests.rs"]
mod tests;
