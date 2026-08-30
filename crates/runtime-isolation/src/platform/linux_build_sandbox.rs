use std::{
    fs::{self, File, OpenOptions},
    io::{Read as _, Write as _},
    os::fd::AsRawFd as _,
    os::unix::fs::{FileTypeExt as _, MetadataExt as _},
    path::PathBuf,
};

use landlock::{
    ABI, Access, AccessFs, CompatLevel, Compatible, PathBeneath, Ruleset, RulesetAttr,
    RulesetCreatedAttr, RulesetStatus, make_bitflags,
};
use rustix::fs::{AtFlags, Mode, OFlags, ResolveFlags, openat, openat2, unlinkat};

use super::linux_build_mount::{BUILD_INPUT_ROOT, BUILD_OUTPUT_ROOT, require_read_only_mount};
use super::linux_helper_setup::HelperFailure;

const SELECTED_LANDLOCK_ABI: ABI = ABI::V3;
const SELECTED_LANDLOCK_ABI_NUMBER: u32 = 3;
const MAXIMUM_INPUT_TREE_ENTRIES: usize = 262_144;

pub(super) fn validate_objects(
    program: &File,
    input_root: &File,
    private_output: &File,
    program_relative_path: &str,
) -> Result<(), HelperFailure> {
    let program_metadata = program
        .metadata()
        .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?;
    let input_metadata = input_root
        .metadata()
        .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?;
    let private_output_metadata = private_output
        .metadata()
        .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?;
    if !program_metadata.is_file()
        || !input_metadata.is_dir()
        || !private_output_metadata.is_dir()
        || same_object(&input_metadata, &private_output_metadata)
    {
        return Err(HelperFailure::ControlledBuildObjectMismatch);
    }
    require_read_only_mount(input_root)?;
    validate_input_mount_closure(input_root)?;
    let reopened = openat2(
        input_root,
        program_relative_path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
        ResolveFlags::BENEATH
            | ResolveFlags::NO_MAGICLINKS
            | ResolveFlags::NO_SYMLINKS
            | ResolveFlags::NO_XDEV,
    )
    .map(File::from)
    .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?;
    let reopened_metadata = reopened
        .metadata()
        .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?;
    if !same_object(&program_metadata, &reopened_metadata)
        || program_metadata.len() != reopened_metadata.len()
    {
        return Err(HelperFailure::ControlledBuildObjectMismatch);
    }
    let output_path = format!("/proc/self/fd/{}", private_output.as_raw_fd());
    if fs::read_dir(output_path)
        .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?
        .next()
        .is_some()
    {
        return Err(HelperFailure::ControlledBuildOutputNotEmpty);
    }
    Ok(())
}

fn validate_input_mount_closure(input_root: &File) -> Result<(), HelperFailure> {
    let mut directories = vec![(
        input_root
            .try_clone()
            .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?,
        PathBuf::new(),
    )];
    let mut entries = 0_usize;
    while let Some((directory, prefix)) = directories.pop() {
        let path = format!("/proc/self/fd/{}", directory.as_raw_fd());
        for raw in fs::read_dir(path).map_err(|_| HelperFailure::ControlledBuildObjectMismatch)? {
            let raw = raw.map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?;
            entries = entries
                .checked_add(1)
                .filter(|count| *count <= MAXIMUM_INPUT_TREE_ENTRIES)
                .ok_or(HelperFailure::ControlledBuildObjectMismatch)?;
            let relative = prefix.join(raw.file_name());
            let directory = openat2(
                input_root,
                &relative,
                OFlags::RDONLY
                    | OFlags::CLOEXEC
                    | OFlags::NOFOLLOW
                    | OFlags::NONBLOCK
                    | OFlags::DIRECTORY,
                Mode::empty(),
                ResolveFlags::BENEATH
                    | ResolveFlags::NO_MAGICLINKS
                    | ResolveFlags::NO_SYMLINKS
                    | ResolveFlags::NO_XDEV,
            );
            if let Ok(directory) = directory {
                directories.push((File::from(directory), relative));
                continue;
            }
            let opened = openat2(
                input_root,
                &relative,
                OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
                Mode::empty(),
                ResolveFlags::BENEATH
                    | ResolveFlags::NO_MAGICLINKS
                    | ResolveFlags::NO_SYMLINKS
                    | ResolveFlags::NO_XDEV,
            )
            .map(File::from)
            .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?;
            let metadata = opened
                .metadata()
                .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?;
            if !metadata.is_file() || metadata.nlink() != 1 {
                return Err(HelperFailure::ControlledBuildObjectMismatch);
            }
            require_read_only_mount(&opened)?;
        }
    }
    Ok(())
}

pub(super) fn install_and_probe(
    input_root: &File,
    output_root: &File,
    program_relative_path: &str,
) -> Result<u32, HelperFailure> {
    let read_access = make_bitflags!(AccessFs::{Execute | ReadFile | ReadDir});
    let write_access = AccessFs::from_all(SELECTED_LANDLOCK_ABI);
    let input = input_root
        .try_clone()
        .map_err(|_| HelperFailure::FilesystemIsolationSetup)?;
    let output = output_root
        .try_clone()
        .map_err(|_| HelperFailure::FilesystemIsolationSetup)?;
    let dev_null = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/null")
        .map_err(|_| HelperFailure::FilesystemIsolationSetup)?;
    let dev_null_metadata = dev_null
        .metadata()
        .map_err(|_| HelperFailure::FilesystemIsolationSetup)?;
    if !dev_null_metadata.file_type().is_char_device() {
        return Err(HelperFailure::FilesystemIsolationSetup);
    }
    let dev_null_access = make_bitflags!(AccessFs::{ReadFile | WriteFile});
    let ruleset = Ruleset::default()
        .handle_access(write_access)
        .map_err(|_| HelperFailure::FilesystemIsolationSetup)?
        .set_compatibility(CompatLevel::HardRequirement)
        .create()
        .map_err(|_| HelperFailure::FilesystemIsolationUnavailable)?
        .add_rule(PathBeneath::new(input, read_access))
        .map_err(|_| HelperFailure::FilesystemIsolationSetup)?
        .add_rule(PathBeneath::new(output, write_access))
        .map_err(|_| HelperFailure::FilesystemIsolationSetup)?
        .add_rule(PathBeneath::new(dev_null, dev_null_access))
        .map_err(|_| HelperFailure::FilesystemIsolationSetup)?
        .set_compatibility(CompatLevel::HardRequirement);
    let status = ruleset
        .restrict_self()
        .map_err(|_| HelperFailure::FilesystemIsolationSetup)?;
    if status.ruleset != RulesetStatus::FullyEnforced || !status.no_new_privs {
        return Err(HelperFailure::FilesystemIsolationUnavailable);
    }
    probe_denied_host_reads()?;
    probe_allowed_dev_null()?;
    probe_allowed_input(input_root, program_relative_path)?;
    probe_allowed_output(output_root)?;
    probe_mounted_aliases(program_relative_path)?;
    Ok(SELECTED_LANDLOCK_ABI_NUMBER)
}

fn probe_mounted_aliases(program_relative_path: &str) -> Result<(), HelperFailure> {
    let input = std::path::Path::new(BUILD_INPUT_ROOT).join(program_relative_path);
    File::open(input).map_err(|_| HelperFailure::FilesystemIsolationBehavior)?;
    let canary =
        std::path::Path::new(BUILD_OUTPUT_ROOT).join(".rewrite-filesystem-mounted-alias-canary");
    fs::write(&canary, b"ok").map_err(|_| HelperFailure::FilesystemIsolationBehavior)?;
    fs::remove_file(canary).map_err(|_| HelperFailure::FilesystemIsolationBehavior)
}

fn probe_denied_host_reads() -> Result<(), HelperFailure> {
    let root_denied = fs::read_dir("/")
        .err()
        .is_some_and(|error| error.kind() == std::io::ErrorKind::PermissionDenied);
    let proc_denied = File::open("/proc/self/status")
        .err()
        .is_some_and(|error| error.kind() == std::io::ErrorKind::PermissionDenied);
    let etc_denied = File::open("/etc/passwd")
        .err()
        .is_some_and(|error| error.kind() == std::io::ErrorKind::PermissionDenied);
    let other_device_denied = File::open("/dev/zero")
        .err()
        .is_some_and(|error| error.kind() == std::io::ErrorKind::PermissionDenied);
    if root_denied && proc_denied && etc_denied && other_device_denied {
        Ok(())
    } else {
        Err(HelperFailure::FilesystemIsolationBehavior)
    }
}

fn probe_allowed_dev_null() -> Result<(), HelperFailure> {
    let mut dev_null = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/null")
        .map_err(|_| HelperFailure::FilesystemIsolationBehavior)?;
    dev_null
        .write_all(b"controlled-build-canary")
        .map_err(|_| HelperFailure::FilesystemIsolationBehavior)?;
    let mut byte = [0_u8; 1];
    let read = dev_null
        .read(&mut byte)
        .map_err(|_| HelperFailure::FilesystemIsolationBehavior)?;
    if read == 0 {
        Ok(())
    } else {
        Err(HelperFailure::FilesystemIsolationBehavior)
    }
}

fn probe_allowed_input(input_root: &File, path: &str) -> Result<(), HelperFailure> {
    openat(
        input_root,
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map(drop)
    .map_err(|_| HelperFailure::FilesystemIsolationBehavior)
}

fn probe_allowed_output(output_root: &File) -> Result<(), HelperFailure> {
    const CANARY: &str = ".rewrite-filesystem-isolation-canary";
    let descriptor = openat(
        output_root,
        CANARY,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(|_| HelperFailure::FilesystemIsolationBehavior)?;
    let mut file = File::from(descriptor);
    file.write_all(b"ok")
        .map_err(|_| HelperFailure::FilesystemIsolationBehavior)?;
    drop(file);
    unlinkat(output_root, CANARY, AtFlags::empty())
        .map_err(|_| HelperFailure::FilesystemIsolationBehavior)
}

fn same_object(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;

    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn landlock_access_masks_keep_input_read_only() {
        let read = make_bitflags!(AccessFs::{Execute | ReadFile | ReadDir});
        let write = AccessFs::from_all(SELECTED_LANDLOCK_ABI);
        let dev_null = make_bitflags!(AccessFs::{ReadFile | WriteFile});
        assert!(!read.contains(AccessFs::WriteFile));
        assert!(!read.contains(AccessFs::MakeReg));
        assert!(write.contains(AccessFs::Truncate));
        assert!(write.contains(read));
        assert!(!dev_null.contains(AccessFs::ReadDir));
        assert!(!dev_null.contains(AccessFs::MakeReg));
    }
}
