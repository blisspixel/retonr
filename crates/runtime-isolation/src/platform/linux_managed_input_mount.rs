use std::{
    collections::BTreeSet,
    ffi::CString,
    fs::{self, File},
    os::unix::fs::{MetadataExt as _, PermissionsExt as _},
    path::{Path, PathBuf},
};

use rustix::{
    fs::{StatVfsMountFlags, fstatvfs},
    mount::{MountFlags, mount, mount_remount},
};

use crate::contract::{
    RetainedRuntimeInputDeclaration, RetainedRuntimeInputMember, runtime_input_file_digest,
    runtime_input_layout_digest,
};
use crate::{
    IsolationError, IsolationResult, MANAGED_RUNTIME_INPUT_ROOT_V1,
    MAXIMUM_MANAGED_RUNTIME_INPUT_BYTES, MAXIMUM_MANAGED_RUNTIME_INPUT_FILES,
    ManagedRuntimeInputEvidence, NamespaceIdentity,
};

use super::{
    linux_helper_setup::HelperFailure,
    linux_managed_mount::{mount_options, mounts_are_private, read_bounded_mount_info},
};

const SCRATCH_MOUNT_BYTES: u64 = 1024 * 1024;
const SCRATCH_MOUNT_INODES: u64 = 512;
const EMPTY_INPUT_MOUNT_BYTES: u64 = 1024 * 1024;
const INPUT_MOUNT_BYTES: u64 = MAXIMUM_MANAGED_RUNTIME_INPUT_BYTES;
const INPUT_MOUNT_INODES: u64 = 4_096;

mod materialize;

pub(super) fn stage(inputs: &[RetainedRuntimeInputMember]) -> Result<(), HelperFailure> {
    validate_order_and_bounds(inputs)
}

pub(super) fn establish(inputs: &[RetainedRuntimeInputMember]) -> Result<(), HelperFailure> {
    require_scratch_mount()?;
    fs::create_dir(MANAGED_RUNTIME_INPUT_ROOT_V1)
        .map_err(|_| HelperFailure::RuntimeInputBoundarySetup)?;
    let input_mount_bytes = input_mount_bytes(inputs.len());
    let options = CString::new(format!(
        "size={input_mount_bytes},nr_inodes={INPUT_MOUNT_INODES},mode=0700"
    ))
    .map_err(|_| HelperFailure::RuntimeInputBoundarySetup)?;
    mount(
        "tmpfs",
        MANAGED_RUNTIME_INPUT_ROOT_V1,
        "tmpfs",
        MountFlags::NODEV | MountFlags::NOSUID | MountFlags::NOEXEC,
        Some(options.as_c_str()),
    )
    .map_err(|_| HelperFailure::RuntimeInputBoundarySetup)?;
    let directories = expected_directories(inputs.iter().map(|input| &input.declaration))?;
    for directory in &directories {
        fs::create_dir(input_path(directory))
            .map_err(|_| HelperFailure::RuntimeInputBoundarySetup)?;
    }
    for input in inputs {
        let target = input_path(&input.declaration.relative_alias);
        materialize::member(input, &target)?;
    }
    seal_directories(&directories)?;
    mount_remount(
        MANAGED_RUNTIME_INPUT_ROOT_V1,
        MountFlags::RDONLY | MountFlags::NOSUID | MountFlags::NODEV | MountFlags::NOEXEC,
        "",
    )
    .map_err(|_| HelperFailure::RuntimeInputBoundarySetup)?;
    observe(inputs.iter().map(|input| &input.declaration))?;
    Ok(())
}

pub(super) fn observe<'a>(
    declarations: impl IntoIterator<Item = &'a RetainedRuntimeInputDeclaration>,
) -> Result<ManagedRuntimeInputEvidence, HelperFailure> {
    let declarations = declarations.into_iter().collect::<Vec<_>>();
    validate_tree(
        &declarations,
        Path::new(MANAGED_RUNTIME_INPUT_ROOT_V1),
        None,
    )?;
    let mount_info = read_bounded_mount_info("/proc/self/mountinfo")
        .map_err(|()| HelperFailure::RuntimeInputBoundaryBehavior)?;
    if !mounts_are_private(&mount_info) || !mount_flags_are_exact(&mount_info, &declarations) {
        return Err(HelperFailure::RuntimeInputBoundaryBehavior);
    }
    let total_bytes = total_bytes(&declarations)?;
    let member_count = u32::try_from(declarations.len())
        .map_err(|_| HelperFailure::RuntimeInputBoundaryBehavior)?;
    Ok(ManagedRuntimeInputEvidence::verified(
        identity(Path::new("/tmp"))?,
        identity(Path::new(MANAGED_RUNTIME_INPUT_ROOT_V1))?,
        member_count,
        total_bytes,
        runtime_input_layout_digest(declarations),
    ))
}

pub(super) fn reobserve(
    target_pid: u32,
    expected: &ManagedRuntimeInputEvidence,
    declarations: &[RetainedRuntimeInputDeclaration],
    cancellation: &rewrite_types::CancellationToken,
) -> IsolationResult<()> {
    let target_root = PathBuf::from(format!("/proc/{target_pid}/root"));
    let input_root = target_root.join(MANAGED_RUNTIME_INPUT_ROOT_V1.trim_start_matches('/'));
    let declaration_refs = declarations.iter().collect::<Vec<_>>();
    validate_tree(&declaration_refs, &input_root, Some(cancellation)).map_err(|_| {
        if cancellation.is_cancelled() {
            IsolationError::Cancelled
        } else {
            IsolationError::EvidenceChanged
        }
    })?;
    let mount_info = read_bounded_mount_info(&format!("/proc/{target_pid}/mountinfo"))
        .map_err(|()| IsolationError::EvidenceChanged)?;
    let observed = ManagedRuntimeInputEvidence::verified(
        identity(&target_root.join("tmp")).map_err(|_| IsolationError::EvidenceChanged)?,
        identity(&input_root).map_err(|_| IsolationError::EvidenceChanged)?,
        u32::try_from(declarations.len()).map_err(|_| IsolationError::EvidenceChanged)?,
        total_bytes(&declaration_refs).map_err(|_| IsolationError::EvidenceChanged)?,
        runtime_input_layout_digest(declarations),
    );
    if &observed != expected
        || !mounts_are_private(&mount_info)
        || !mount_flags_are_exact(&mount_info, &declaration_refs)
    {
        return Err(IsolationError::EvidenceChanged);
    }
    Ok(())
}

fn validate_order_and_bounds(inputs: &[RetainedRuntimeInputMember]) -> Result<(), HelperFailure> {
    if inputs.len() > MAXIMUM_MANAGED_RUNTIME_INPUT_FILES
        || inputs
            .windows(2)
            .any(|pair| pair[0].declaration.relative_alias >= pair[1].declaration.relative_alias)
    {
        return Err(HelperFailure::RuntimeInputObjectMismatch);
    }
    let declarations = inputs
        .iter()
        .map(|input| &input.declaration)
        .collect::<Vec<_>>();
    total_bytes(&declarations)?;
    for input in inputs {
        materialize::revalidate(input)?;
    }
    Ok(())
}

fn validate_tree(
    declarations: &[&RetainedRuntimeInputDeclaration],
    root: &Path,
    cancellation: Option<&rewrite_types::CancellationToken>,
) -> Result<(), HelperFailure> {
    let directories = expected_directories(declarations.iter().copied())?;
    let files = declarations
        .iter()
        .map(|declaration| declaration.relative_alias.as_str())
        .collect::<BTreeSet<_>>();
    let mut observed = BTreeSet::new();
    let mut pending = vec![(root.to_owned(), String::new())];
    while let Some((directory, prefix)) = pending.pop() {
        for entry in
            fs::read_dir(&directory).map_err(|_| HelperFailure::RuntimeInputBoundaryBehavior)?
        {
            let entry = entry.map_err(|_| HelperFailure::RuntimeInputBoundaryBehavior)?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| HelperFailure::RuntimeInputBoundaryBehavior)?;
            let relative = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };
            if !observed.insert(relative.clone()) {
                return Err(HelperFailure::RuntimeInputBoundaryBehavior);
            }
            let path = directory.join(entry.file_name());
            let metadata = fs::symlink_metadata(&path)
                .map_err(|_| HelperFailure::RuntimeInputBoundaryBehavior)?;
            if directories.contains(&relative) && metadata.is_dir() {
                pending.push((path, relative));
            } else if files.contains(relative.as_str()) && metadata.is_file() {
                let expected = declarations
                    .iter()
                    .find(|declaration| declaration.relative_alias == relative)
                    .ok_or(HelperFailure::RuntimeInputBoundaryBehavior)?;
                require_metadata(expected, &metadata)?;
                let file =
                    File::open(path).map_err(|_| HelperFailure::RuntimeInputBoundaryBehavior)?;
                let digest =
                    runtime_input_file_digest(&file, expected.expected_bytes, cancellation)
                        .map_err(|_| HelperFailure::RuntimeInputBoundaryBehavior)?;
                if digest != expected.expected_digest {
                    return Err(HelperFailure::RuntimeInputBoundaryBehavior);
                }
            } else {
                return Err(HelperFailure::RuntimeInputBoundaryBehavior);
            }
        }
    }
    if observed.len() == files.len().saturating_add(directories.len()) {
        Ok(())
    } else {
        Err(HelperFailure::RuntimeInputBoundaryBehavior)
    }
}

fn expected_directories<'a>(
    declarations: impl IntoIterator<Item = &'a RetainedRuntimeInputDeclaration>,
) -> Result<BTreeSet<String>, HelperFailure> {
    let mut files = BTreeSet::new();
    let mut directories = BTreeSet::new();
    for declaration in declarations {
        if !files.insert(declaration.relative_alias.clone()) {
            return Err(HelperFailure::RuntimeInputObjectMismatch);
        }
        let components = declaration.relative_alias.split('/').collect::<Vec<_>>();
        let mut prefix = String::new();
        for component in &components[..components.len().saturating_sub(1)] {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(component);
            if files.contains(&prefix) {
                return Err(HelperFailure::RuntimeInputObjectMismatch);
            }
            directories.insert(prefix.clone());
        }
        if directories.contains(&declaration.relative_alias) {
            return Err(HelperFailure::RuntimeInputObjectMismatch);
        }
    }
    Ok(directories)
}

fn mount_flags_are_exact(mount_info: &str, _: &[&RetainedRuntimeInputDeclaration]) -> bool {
    exact_options(mount_info, "/tmp", &["rw", "nosuid", "nodev", "noexec"])
        && exact_options(
            mount_info,
            MANAGED_RUNTIME_INPUT_ROOT_V1,
            &["ro", "nosuid", "nodev", "noexec"],
        )
}

fn exact_options(text: &str, path: &str, required: &[&str]) -> bool {
    mount_options(text, path).is_some_and(|options| {
        required.iter().all(|flag| options.contains(flag))
            && !options.contains(&if required.contains(&"ro") { "rw" } else { "ro" })
    })
}

fn seal_directories(directories: &BTreeSet<String>) -> Result<(), HelperFailure> {
    for directory in directories.iter().rev() {
        fs::set_permissions(input_path(directory), fs::Permissions::from_mode(0o555))
            .map_err(|_| HelperFailure::RuntimeInputBoundarySetup)?;
    }
    fs::set_permissions(
        MANAGED_RUNTIME_INPUT_ROOT_V1,
        fs::Permissions::from_mode(0o555),
    )
    .map_err(|_| HelperFailure::RuntimeInputBoundarySetup)
}

fn require_scratch_mount() -> Result<(), HelperFailure> {
    let scratch = File::open("/tmp").map_err(|_| HelperFailure::RuntimeInputBoundarySetup)?;
    let status = fstatvfs(&scratch).map_err(|_| HelperFailure::RuntimeInputBoundarySetup)?;
    let bytes = status.f_blocks.saturating_mul(status.f_frsize);
    if status.f_flag.contains(StatVfsMountFlags::NODEV)
        && status.f_flag.contains(StatVfsMountFlags::NOSUID)
        && status.f_flag.contains(StatVfsMountFlags::NOEXEC)
        && bytes <= SCRATCH_MOUNT_BYTES
        && status.f_files <= SCRATCH_MOUNT_INODES
    {
        Ok(())
    } else {
        Err(HelperFailure::RuntimeInputBoundaryBehavior)
    }
}

fn require_metadata(
    declaration: &RetainedRuntimeInputDeclaration,
    metadata: &fs::Metadata,
) -> Result<(), HelperFailure> {
    if materialized_metadata_is_exact(declaration, metadata) {
        Ok(())
    } else {
        Err(HelperFailure::RuntimeInputBoundaryBehavior)
    }
}

fn materialized_metadata_is_exact(
    declaration: &RetainedRuntimeInputDeclaration,
    metadata: &fs::Metadata,
) -> bool {
    metadata.is_file()
        && metadata.nlink() == 1
        && metadata.len() == declaration.expected_bytes
        && metadata.mode() & 0o777 == 0o400
        && (metadata.dev(), metadata.ino())
            != (declaration.identity.device, declaration.identity.inode)
}

fn total_bytes(declarations: &[&RetainedRuntimeInputDeclaration]) -> Result<u64, HelperFailure> {
    declarations.iter().try_fold(0_u64, |total, declaration| {
        total
            .checked_add(declaration.expected_bytes)
            .filter(|bytes| *bytes <= MAXIMUM_MANAGED_RUNTIME_INPUT_BYTES)
            .ok_or(HelperFailure::RuntimeInputObjectMismatch)
    })
}

const fn input_mount_bytes(input_count: usize) -> u64 {
    if input_count == 0 {
        EMPTY_INPUT_MOUNT_BYTES
    } else {
        INPUT_MOUNT_BYTES
    }
}

fn identity(path: &Path) -> Result<NamespaceIdentity, HelperFailure> {
    let metadata = fs::metadata(path).map_err(|_| HelperFailure::RuntimeInputBoundaryBehavior)?;
    Ok(NamespaceIdentity::new(metadata.dev(), metadata.ino()))
}

fn input_path(relative: &str) -> PathBuf {
    Path::new(MANAGED_RUNTIME_INPUT_ROOT_V1).join(relative)
}

#[cfg(test)]
#[path = "linux_managed_input_mount/tests.rs"]
mod tests;
