use std::{
    collections::BTreeMap,
    ffi::CString,
    fs::{self, File, OpenOptions},
    io::Write as _,
    os::{
        fd::AsRawFd as _,
        unix::fs::{FileExt as _, MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _},
    },
    path::Path,
    time::Instant,
};

use rewrite_types::Digest as DomainDigest;
use rustix::{
    fs::{Mode, OFlags, ResolveFlags, StatVfsMountFlags, fstatvfs, openat2},
    mount::{MountFlags, MountPropagationFlags, mount, mount_bind, mount_change, mount_remount},
};
use sha2::{Digest as _, Sha256};

use crate::{
    ControlledBuildInputFile, MAXIMUM_CONTROLLED_BUILD_INPUT_BYTES,
    MAXIMUM_CONTROLLED_BUILD_WORKSPACE_BYTES, MAXIMUM_CONTROLLED_BUILD_WORKSPACE_INODES,
};

use super::linux_helper_setup::HelperFailure;

const BUILD_ROOT: &str = "/tmp/retonr-controlled-build";
const SNAPSHOT_BUFFER_BYTES: usize = 64 * 1024;
const MAXIMUM_EXPANDED_PATH_BYTES: usize = 16 * 1024 * 1024;
pub(super) const BUILD_INPUT_ROOT: &str = "/tmp/retonr-controlled-build/input";
pub(super) const BUILD_OUTPUT_ROOT: &str = "/tmp/retonr-controlled-build/output";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExpectedKind {
    Directory,
    RegularFile,
}

pub(super) fn establish(
    output: &File,
    input_files: &[ControlledBuildInputFile],
    snapshot_deadline: Instant,
) -> Result<(), HelperFailure> {
    require_directory(output)?;
    require_empty(output)?;
    mount_change(
        "/",
        MountPropagationFlags::PRIVATE | MountPropagationFlags::REC,
    )
    .map_err(|_| HelperFailure::FilesystemAliasPropagation)?;
    mount_private_tmpfs()?;
    let expected = expected_tree(input_files)?;
    create_private_input(&expected, input_files, snapshot_deadline)?;
    let input = open_private_input()?;
    let private_output = open_private_output()?;
    validate(&input, output)?;
    require_empty(&private_output)?;
    Ok(())
}

fn mount_private_tmpfs() -> Result<(), HelperFailure> {
    let options = CString::new(format!(
        "size={MAXIMUM_CONTROLLED_BUILD_WORKSPACE_BYTES},nr_inodes={MAXIMUM_CONTROLLED_BUILD_WORKSPACE_INODES},mode=0700"
    ))
    .map_err(|_| HelperFailure::FilesystemIsolationSetup)?;
    mount(
        "tmpfs",
        "/tmp",
        "tmpfs",
        MountFlags::NODEV | MountFlags::NOSUID,
        Some(options.as_c_str()),
    )
    .map_err(|_| HelperFailure::FilesystemIsolationSetup)?;
    fs::create_dir(BUILD_ROOT).map_err(|_| HelperFailure::FilesystemAliasWorkspace)?;
    fs::create_dir(BUILD_INPUT_ROOT).map_err(|_| HelperFailure::FilesystemAliasInput)?;
    fs::create_dir(BUILD_OUTPUT_ROOT).map_err(|_| HelperFailure::FilesystemAliasOutput)
}

fn create_private_input(
    expected: &BTreeMap<String, ExpectedKind>,
    input_files: &[ControlledBuildInputFile],
    snapshot_deadline: Instant,
) -> Result<(), HelperFailure> {
    input_files.iter().try_fold(0_u64, |total, input| {
        total
            .checked_add(input.expected_bytes())
            .filter(|bytes| *bytes <= MAXIMUM_CONTROLLED_BUILD_INPUT_BYTES)
            .ok_or(HelperFailure::ControlledBuildObjectMismatch)
    })?;
    for (relative, kind) in expected {
        let path = Path::new(BUILD_INPUT_ROOT).join(relative);
        match kind {
            ExpectedKind::Directory => {
                fs::create_dir(&path).map_err(|_| HelperFailure::FilesystemAliasInput)?;
            }
            ExpectedKind::RegularFile => {}
        }
    }
    for input in input_files {
        copy_retained_file(input, snapshot_deadline)?;
    }
    for input in input_files {
        let reopened = open_private_regular(input.relative_path())?;
        require_snapshot_file(input, &reopened)?;
    }
    for (relative, kind) in expected.iter().rev() {
        if *kind == ExpectedKind::Directory {
            fs::set_permissions(
                Path::new(BUILD_INPUT_ROOT).join(relative),
                fs::Permissions::from_mode(0o555),
            )
            .map_err(|_| HelperFailure::FilesystemAliasInputPermission)?;
        }
    }
    fs::set_permissions(BUILD_INPUT_ROOT, fs::Permissions::from_mode(0o555))
        .map_err(|_| HelperFailure::FilesystemAliasInputPermission)?;
    mount_bind(BUILD_INPUT_ROOT, BUILD_INPUT_ROOT)
        .map_err(|_| HelperFailure::FilesystemAliasInputPermission)?;
    mount_remount(
        BUILD_INPUT_ROOT,
        MountFlags::BIND | MountFlags::RDONLY | MountFlags::NODEV | MountFlags::NOSUID,
        "",
    )
    .map_err(|_| HelperFailure::FilesystemAliasInputPermission)?;
    require_read_only_mount(&open_private_input()?)
}

fn copy_retained_file(
    input: &ControlledBuildInputFile,
    snapshot_deadline: Instant,
) -> Result<(), HelperFailure> {
    require_snapshot_time(snapshot_deadline)?;
    let before = RetainedObjectStamp::read(
        &input
            .file()
            .metadata()
            .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?,
    );
    if !before.regular_file || before.links != 1 || before.bytes != input.expected_bytes() {
        return Err(HelperFailure::ControlledBuildObjectMismatch);
    }
    let path = Path::new(BUILD_INPUT_ROOT).join(input.relative_path());
    let mut copied = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o400)
        .open(path)
        .map_err(|_| HelperFailure::FilesystemAliasInput)?;
    copy_exact_bytes(
        input.file(),
        &mut copied,
        input.expected_bytes(),
        input.expected_digest(),
        snapshot_deadline,
        |_| {},
    )?;
    let after = RetainedObjectStamp::read(
        &input
            .file()
            .metadata()
            .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?,
    );
    if before != after {
        return Err(HelperFailure::ControlledBuildObjectMismatch);
    }
    let normalized_mode = if before.mode & 0o111 == 0 {
        0o444
    } else {
        0o555
    };
    copied
        .set_permissions(fs::Permissions::from_mode(normalized_mode))
        .map_err(|_| HelperFailure::FilesystemAliasInputPermission)?;
    copied
        .sync_all()
        .map_err(|_| HelperFailure::FilesystemAliasInput)?;
    drop(copied);
    Ok(())
}

fn copy_exact_bytes(
    source: &File,
    destination: &mut File,
    expected_bytes: u64,
    expected_digest: &DomainDigest,
    snapshot_deadline: Instant,
    mut after_chunk: impl FnMut(u64),
) -> Result<(), HelperFailure> {
    let mut hasher = Sha256::new();
    let mut offset = 0_u64;
    let mut buffer = vec![0_u8; SNAPSHOT_BUFFER_BYTES];
    while offset < expected_bytes {
        require_snapshot_time(snapshot_deadline)?;
        let remaining = expected_bytes - offset;
        let limit = usize::try_from(remaining.min(SNAPSHOT_BUFFER_BYTES as u64))
            .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?;
        let read = source
            .read_at(&mut buffer[..limit], offset)
            .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?;
        if read == 0 || read > limit {
            return Err(HelperFailure::ControlledBuildObjectMismatch);
        }
        destination
            .write_all(&buffer[..read])
            .map_err(|_| HelperFailure::FilesystemAliasInput)?;
        hasher.update(&buffer[..read]);
        offset = offset
            .checked_add(u64::try_from(read).map_err(|_| HelperFailure::InvalidLaunch)?)
            .ok_or(HelperFailure::ControlledBuildObjectMismatch)?;
        after_chunk(offset);
    }
    require_snapshot_time(snapshot_deadline)?;
    let mut trailing = [0_u8; 1];
    if source
        .read_at(&mut trailing, expected_bytes)
        .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?
        != 0
    {
        return Err(HelperFailure::ControlledBuildObjectMismatch);
    }
    let digest = DomainDigest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?;
    if digest != *expected_digest {
        return Err(HelperFailure::ControlledBuildObjectMismatch);
    }
    destination
        .sync_data()
        .map_err(|_| HelperFailure::FilesystemAliasInput)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RetainedObjectStamp {
    device: u64,
    inode: u64,
    mode: u32,
    links: u64,
    user: u32,
    group: u32,
    bytes: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
    regular_file: bool,
}

impl RetainedObjectStamp {
    fn read(metadata: &fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            mode: metadata.mode(),
            links: metadata.nlink(),
            user: metadata.uid(),
            group: metadata.gid(),
            bytes: metadata.len(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
            regular_file: metadata.is_file(),
        }
    }
}

fn require_snapshot_time(deadline: Instant) -> Result<(), HelperFailure> {
    if Instant::now() < deadline {
        Ok(())
    } else {
        Err(HelperFailure::ControlledBuildSnapshotTimeout)
    }
}

fn expected_tree(
    input_files: &[ControlledBuildInputFile],
) -> Result<BTreeMap<String, ExpectedKind>, HelperFailure> {
    let mut expected = BTreeMap::new();
    let mut expanded_path_bytes = 0_usize;
    for input in input_files {
        let components = input.relative_path().split('/').collect::<Vec<_>>();
        let mut prefix = String::new();
        for component in &components[..components.len().saturating_sub(1)] {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(component);
            expanded_path_bytes = expanded_path_bytes
                .checked_add(prefix.len())
                .filter(|bytes| *bytes <= MAXIMUM_EXPANDED_PATH_BYTES)
                .ok_or(HelperFailure::ControlledBuildObjectMismatch)?;
            match expected.insert(prefix.clone(), ExpectedKind::Directory) {
                Some(ExpectedKind::RegularFile) => {
                    return Err(HelperFailure::ControlledBuildObjectMismatch);
                }
                Some(ExpectedKind::Directory) | None => {}
            }
            require_tree_bound(expected.len())?;
        }
        if expected
            .insert(input.relative_path().to_owned(), ExpectedKind::RegularFile)
            .is_some()
        {
            return Err(HelperFailure::ControlledBuildObjectMismatch);
        }
        require_tree_bound(expected.len())?;
    }
    Ok(expected)
}

fn require_tree_bound(entries: usize) -> Result<(), HelperFailure> {
    if u64::try_from(entries).unwrap_or(u64::MAX) <= MAXIMUM_CONTROLLED_BUILD_WORKSPACE_INODES {
        Ok(())
    } else {
        Err(HelperFailure::ControlledBuildObjectMismatch)
    }
}

pub(super) fn open_private_regular(relative: &str) -> Result<File, HelperFailure> {
    let root = open_private_input()?;
    openat2(
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
    .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)
}

fn require_snapshot_file(
    declaration: &ControlledBuildInputFile,
    snapshot: &File,
) -> Result<(), HelperFailure> {
    let metadata = snapshot
        .metadata()
        .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?;
    if metadata.is_file()
        && metadata.nlink() == 1
        && metadata.len() == declaration.expected_bytes()
        && metadata.mode() & 0o222 == 0
    {
        Ok(())
    } else {
        Err(HelperFailure::ControlledBuildObjectMismatch)
    }
}

pub(super) fn require_read_only_mount(file: &File) -> Result<(), HelperFailure> {
    let flags = fstatvfs(file)
        .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?
        .f_flag;
    if flags.contains(StatVfsMountFlags::RDONLY) {
        Ok(())
    } else {
        Err(HelperFailure::ControlledBuildObjectMismatch)
    }
}

pub(super) fn validate(input: &File, host_output: &File) -> Result<(), HelperFailure> {
    let alias_input = open_private_input()?;
    let private_output = open_private_output()?;
    require_same_directory(input, &alias_input)?;
    require_distinct_directories(input, &private_output)?;
    require_distinct_directories(input, host_output)?;
    require_distinct_directories(&private_output, host_output)
}

pub(super) fn validate_private(input: &File) -> Result<(), HelperFailure> {
    let alias_input = open_private_input()?;
    let private_output = open_private_output()?;
    require_same_directory(input, &alias_input)?;
    require_distinct_directories(input, &private_output)
}

pub(super) fn open_private_input() -> Result<File, HelperFailure> {
    File::open(BUILD_INPUT_ROOT).map_err(|_| HelperFailure::FilesystemAliasInputNotFound)
}

pub(super) fn open_private_output() -> Result<File, HelperFailure> {
    File::open(BUILD_OUTPUT_ROOT).map_err(|_| HelperFailure::FilesystemAliasOutput)
}

fn require_directory(directory: &File) -> Result<(), HelperFailure> {
    let metadata = directory
        .metadata()
        .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?;
    if metadata.is_dir() {
        Ok(())
    } else {
        Err(HelperFailure::ControlledBuildObjectMismatch)
    }
}

fn require_same_directory(left: &File, right: &File) -> Result<(), HelperFailure> {
    let left = left
        .metadata()
        .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?;
    let right = right
        .metadata()
        .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?;
    if left.is_dir() && right.is_dir() && left.dev() == right.dev() && left.ino() == right.ino() {
        Ok(())
    } else {
        Err(HelperFailure::ControlledBuildObjectMismatch)
    }
}

fn require_distinct_directories(left: &File, right: &File) -> Result<(), HelperFailure> {
    let left = left
        .metadata()
        .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?;
    let right = right
        .metadata()
        .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?;
    if left.is_dir() && right.is_dir() && (left.dev() != right.dev() || left.ino() != right.ino()) {
        Ok(())
    } else {
        Err(HelperFailure::ControlledBuildObjectMismatch)
    }
}

fn require_empty(directory: &File) -> Result<(), HelperFailure> {
    let path = format!("/proc/self/fd/{}", directory.as_raw_fd());
    if fs::read_dir(path)
        .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?
        .next()
        .is_none()
    {
        Ok(())
    } else {
        Err(HelperFailure::ControlledBuildOutputNotEmpty)
    }
}

#[cfg(test)]
mod tests;
