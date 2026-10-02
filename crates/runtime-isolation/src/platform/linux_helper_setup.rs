use std::{
    fs::{self, File},
    io,
    os::unix::fs::MetadataExt as _,
    time::Instant,
};

use rustix::{
    process::{getgid, getuid},
    thread::{
        CapabilitySet, UnshareFlags, capabilities, capability_is_in_bounding_set, no_new_privs,
    },
};

pub(super) use super::linux_helper_guards::privileges_are_fully_reduced;
use super::{
    linux_build_mount,
    linux_helper_guards::{
        apply_resource_limits, drop_capabilities, drop_privileges, read_capability_limit,
        run_network_canaries,
    },
    linux_managed_mount,
};
use crate::contract::RetainedRuntimeInputMember;
use crate::{ControlledBuildInputFile, IsolationError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HelperFailure {
    HostPolicyDenied,
    NamespaceSetup,
    LoopbackSetup,
    NetworkCanary,
    DescriptorLeak,
    PrivilegeDrop,
    SocketPolicyCompile,
    SocketPolicyInstall,
    SocketPolicyInactive,
    SocketPolicyBehavior,
    ManagedDeviceBoundaryUnavailable,
    ManagedDeviceBoundarySetup,
    ManagedPrivatePropagationSetup,
    ManagedNullStageMount,
    ManagedDeviceTmpfsSetup,
    ManagedNullStagingSetup,
    ManagedNullBind,
    ManagedNullMountRemount,
    ManagedProcSetup,
    ManagedDeviceBoundaryBehavior,
    ManagedContainmentPolicyCompile,
    ManagedContainmentPolicyInstall,
    ManagedContainmentPolicyInactive,
    ManagedContainmentPolicyBehavior,
    RuntimeInputObjectMismatch,
    RuntimeInputBoundaryUnavailable,
    RuntimeInputBoundarySetup,
    RuntimeInputBoundaryBehavior,
    FilesystemIsolationUnavailable,
    FilesystemIsolationSetup,
    FilesystemAliasPropagation,
    FilesystemAliasWorkspace,
    FilesystemAliasInput,
    FilesystemAliasInputPermission,
    FilesystemAliasInputNotFound,
    FilesystemAliasOutput,
    FilesystemIsolationBehavior,
    ControlledBuildObjectMismatch,
    ControlledBuildOutputNotEmpty,
    ControlledBuildSnapshotTimeout,
    StartupTimeout,
    BootstrapRootPreparation,
    BootstrapRootVerification,
    InvalidLaunch,
}

impl HelperFailure {
    pub(super) const fn code(self) -> &'static str {
        match self {
            Self::HostPolicyDenied => "host-policy-denied",
            Self::NamespaceSetup => "namespace-setup",
            Self::LoopbackSetup => "loopback-setup",
            Self::NetworkCanary => "network-canary",
            Self::DescriptorLeak => "descriptor-leak",
            Self::PrivilegeDrop => "privilege-drop",
            Self::SocketPolicyCompile => "socket-policy-compile",
            Self::SocketPolicyInstall => "socket-policy-install",
            Self::SocketPolicyInactive => "socket-policy-inactive",
            Self::SocketPolicyBehavior => "socket-policy-behavior",
            Self::ManagedDeviceBoundaryUnavailable => "managed-device-boundary-unavailable",
            Self::ManagedDeviceBoundarySetup => "managed-device-boundary-setup",
            Self::ManagedPrivatePropagationSetup => "managed-private-propagation-setup",
            Self::ManagedNullStageMount => "managed-null-stage-mount",
            Self::ManagedDeviceTmpfsSetup => "managed-device-tmpfs-setup",
            Self::ManagedNullStagingSetup => "managed-null-staging-setup",
            Self::ManagedNullBind => "managed-null-bind",
            Self::ManagedNullMountRemount => "managed-null-mount-remount",
            Self::ManagedProcSetup => "managed-proc-setup",
            Self::ManagedDeviceBoundaryBehavior => "managed-device-boundary-behavior",
            Self::ManagedContainmentPolicyCompile => "managed-containment-policy-compile",
            Self::ManagedContainmentPolicyInstall => "managed-containment-policy-install",
            Self::ManagedContainmentPolicyInactive => "managed-containment-policy-inactive",
            Self::ManagedContainmentPolicyBehavior => "managed-containment-policy-behavior",
            Self::RuntimeInputObjectMismatch => "runtime-input-object-mismatch",
            Self::RuntimeInputBoundaryUnavailable => "runtime-input-boundary-unavailable",
            Self::RuntimeInputBoundarySetup => "runtime-input-boundary-setup",
            Self::RuntimeInputBoundaryBehavior => "runtime-input-boundary-behavior",
            Self::FilesystemIsolationUnavailable => "filesystem-isolation-unavailable",
            Self::FilesystemIsolationSetup => "filesystem-isolation-setup",
            Self::FilesystemAliasPropagation => "filesystem-alias-propagation",
            Self::FilesystemAliasWorkspace => "filesystem-alias-workspace",
            Self::FilesystemAliasInput => "filesystem-alias-input",
            Self::FilesystemAliasInputPermission => "filesystem-alias-input-permission",
            Self::FilesystemAliasInputNotFound => "filesystem-alias-input-not-found",
            Self::FilesystemAliasOutput => "filesystem-alias-output",
            Self::FilesystemIsolationBehavior => "filesystem-isolation-behavior",
            Self::ControlledBuildObjectMismatch => "controlled-build-object-mismatch",
            Self::ControlledBuildOutputNotEmpty => "controlled-build-output-not-empty",
            Self::ControlledBuildSnapshotTimeout => "controlled-build-snapshot-timeout",
            Self::StartupTimeout => "startup-timeout",
            Self::BootstrapRootPreparation => "bootstrap-root-preparation",
            Self::BootstrapRootVerification => "bootstrap-root-verification",
            Self::InvalidLaunch => "invalid-launch",
        }
    }

    pub(super) const fn into_isolation_error(self) -> IsolationError {
        match self {
            Self::HostPolicyDenied => IsolationError::HostPolicyDenied,
            Self::NamespaceSetup => IsolationError::NamespaceSetup,
            Self::LoopbackSetup => IsolationError::LoopbackSetup,
            Self::NetworkCanary => IsolationError::NetworkCanary,
            Self::DescriptorLeak => IsolationError::DescriptorLeak,
            Self::PrivilegeDrop => IsolationError::PrivilegeDrop,
            Self::SocketPolicyCompile => IsolationError::SocketPolicyCompile,
            Self::SocketPolicyInstall => IsolationError::SocketPolicyInstall,
            Self::SocketPolicyInactive => IsolationError::SocketPolicyInactive,
            Self::SocketPolicyBehavior => IsolationError::SocketPolicyBehavior,
            Self::ManagedDeviceBoundaryUnavailable => {
                IsolationError::ManagedDeviceBoundaryUnavailable
            }
            Self::ManagedDeviceBoundarySetup
            | Self::ManagedPrivatePropagationSetup
            | Self::ManagedNullStageMount
            | Self::ManagedDeviceTmpfsSetup
            | Self::ManagedNullStagingSetup
            | Self::ManagedNullBind
            | Self::ManagedNullMountRemount
            | Self::ManagedProcSetup => IsolationError::ManagedDeviceBoundarySetup,
            Self::ManagedDeviceBoundaryBehavior => IsolationError::ManagedDeviceBoundaryBehavior,
            Self::ManagedContainmentPolicyCompile => {
                IsolationError::ManagedContainmentPolicyCompile
            }
            Self::ManagedContainmentPolicyInstall => {
                IsolationError::ManagedContainmentPolicyInstall
            }
            Self::ManagedContainmentPolicyInactive => {
                IsolationError::ManagedContainmentPolicyInactive
            }
            Self::ManagedContainmentPolicyBehavior => {
                IsolationError::ManagedContainmentPolicyBehavior
            }
            Self::RuntimeInputObjectMismatch => IsolationError::RuntimeInputObjectMismatch,
            Self::RuntimeInputBoundaryUnavailable => {
                IsolationError::RuntimeInputBoundaryUnavailable
            }
            Self::RuntimeInputBoundarySetup => IsolationError::RuntimeInputBoundarySetup,
            Self::RuntimeInputBoundaryBehavior => IsolationError::RuntimeInputBoundaryBehavior,
            Self::FilesystemIsolationUnavailable => IsolationError::FilesystemIsolationUnavailable,
            Self::FilesystemIsolationSetup => IsolationError::FilesystemIsolationSetup,
            Self::FilesystemAliasPropagation => IsolationError::FilesystemAliasSetup("propagation"),
            Self::FilesystemAliasWorkspace => IsolationError::FilesystemAliasSetup("workspace"),
            Self::FilesystemAliasInput => IsolationError::FilesystemAliasSetup("input"),
            Self::FilesystemAliasInputPermission => {
                IsolationError::FilesystemAliasSetup("input-permission")
            }
            Self::FilesystemAliasInputNotFound => {
                IsolationError::FilesystemAliasSetup("input-not-found")
            }
            Self::FilesystemAliasOutput => IsolationError::FilesystemAliasSetup("output"),
            Self::FilesystemIsolationBehavior => IsolationError::FilesystemIsolationBehavior,
            Self::ControlledBuildObjectMismatch => IsolationError::ControlledBuildObjectMismatch,
            Self::ControlledBuildOutputNotEmpty => IsolationError::ControlledBuildOutputNotEmpty,
            Self::ControlledBuildSnapshotTimeout => IsolationError::ControlledBuildSnapshotTimeout,
            Self::StartupTimeout => IsolationError::StartupTimeout,
            Self::BootstrapRootPreparation => IsolationError::BootstrapRootPreparation,
            Self::BootstrapRootVerification => IsolationError::BootstrapRootVerification,
            Self::InvalidLaunch => IsolationError::InvalidLaunch("helper validation"),
        }
    }

    #[cfg(test)]
    pub(super) const ALL: [Self; 44] = [
        Self::HostPolicyDenied,
        Self::NamespaceSetup,
        Self::LoopbackSetup,
        Self::NetworkCanary,
        Self::DescriptorLeak,
        Self::PrivilegeDrop,
        Self::SocketPolicyCompile,
        Self::SocketPolicyInstall,
        Self::SocketPolicyInactive,
        Self::SocketPolicyBehavior,
        Self::ManagedDeviceBoundaryUnavailable,
        Self::ManagedDeviceBoundarySetup,
        Self::ManagedPrivatePropagationSetup,
        Self::ManagedNullStageMount,
        Self::ManagedDeviceTmpfsSetup,
        Self::ManagedNullStagingSetup,
        Self::ManagedNullBind,
        Self::ManagedNullMountRemount,
        Self::ManagedProcSetup,
        Self::ManagedDeviceBoundaryBehavior,
        Self::ManagedContainmentPolicyCompile,
        Self::ManagedContainmentPolicyInstall,
        Self::ManagedContainmentPolicyInactive,
        Self::ManagedContainmentPolicyBehavior,
        Self::RuntimeInputObjectMismatch,
        Self::RuntimeInputBoundaryUnavailable,
        Self::RuntimeInputBoundarySetup,
        Self::RuntimeInputBoundaryBehavior,
        Self::FilesystemIsolationUnavailable,
        Self::FilesystemIsolationSetup,
        Self::FilesystemAliasPropagation,
        Self::FilesystemAliasWorkspace,
        Self::FilesystemAliasInput,
        Self::FilesystemAliasInputPermission,
        Self::FilesystemAliasInputNotFound,
        Self::FilesystemAliasOutput,
        Self::FilesystemIsolationBehavior,
        Self::ControlledBuildObjectMismatch,
        Self::ControlledBuildOutputNotEmpty,
        Self::ControlledBuildSnapshotTimeout,
        Self::StartupTimeout,
        Self::BootstrapRootPreparation,
        Self::BootstrapRootVerification,
        Self::InvalidLaunch,
    ];
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct EstablishedIsolation {
    pub(super) loopback_index: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RawNamespaceIdentity {
    pub(super) device: u64,
    pub(super) inode: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct NamespaceEvidence {
    pub(super) network: RawNamespaceIdentity,
    pub(super) user: RawNamespaceIdentity,
    pub(super) process: RawNamespaceIdentity,
}

impl NamespaceEvidence {
    pub(super) fn current() -> Result<Self, HelperFailure> {
        Ok(Self {
            network: namespace_identity("/proc/self/ns/net")?,
            user: namespace_identity("/proc/self/ns/user")?,
            process: namespace_identity("/proc/self/ns/pid")?,
        })
    }
}

pub(super) fn mount_namespace_identity() -> Result<RawNamespaceIdentity, HelperFailure> {
    namespace_identity("/proc/self/ns/mnt")
}

pub(super) fn establish_isolation(
    limits: (u64, u64),
    inputs: &[RetainedRuntimeInputMember],
) -> Result<EstablishedIsolation, HelperFailure> {
    let null = linux_managed_mount::open_retained_null()?;
    establish_managed_namespaces()?;
    linux_managed_mount::establish_private_device(&null, inputs)?;
    apply_resource_limits(limits)?;
    let loopback_index = super::linux_link::enable_and_validate_loopback()?;
    run_network_canaries()?;
    Ok(EstablishedIsolation { loopback_index })
}

pub(super) fn drop_managed_privileges() -> Result<(), HelperFailure> {
    drop_privileges()
}

pub(super) fn establish_build_isolation(
    limits: (u64, u64),
    output: &File,
    input_files: &[ControlledBuildInputFile],
    snapshot_deadline: Instant,
) -> Result<EstablishedIsolation, HelperFailure> {
    establish_isolation_inner(limits, Some((output, input_files, snapshot_deadline)))
}

pub(super) fn begin_bootstrap_isolation(
    output: &File,
    input_files: &[ControlledBuildInputFile],
    snapshot_deadline: Instant,
) -> Result<(), HelperFailure> {
    establish_bootstrap_namespaces(output, input_files, snapshot_deadline)
}

pub(super) fn prepare_bootstrap_child_namespace(
    limits: (u64, u64),
) -> Result<EstablishedIsolation, HelperFailure> {
    enter_bootstrap_process_namespace()?;
    apply_resource_limits(limits)?;
    let loopback_index = super::linux_link::enable_and_validate_loopback()?;
    run_network_canaries()?;
    Ok(EstablishedIsolation { loopback_index })
}

pub(super) fn bootstrap_capability_limit() -> Result<u32, HelperFailure> {
    read_capability_limit()
}

pub(super) fn drop_bootstrap_guardian_privileges(
    last_capability: u32,
) -> Result<(), HelperFailure> {
    drop_capabilities(last_capability)?;
    let current = capabilities(None).map_err(|_| HelperFailure::PrivilegeDrop)?;
    let bounding_empty = (0..=last_capability).try_fold(true, |empty, bit| {
        let capability = CapabilitySet::from_bits_retain(1_u64 << bit);
        capability_is_in_bounding_set(capability)
            .map(|present| empty && !present)
            .map_err(|_| HelperFailure::PrivilegeDrop)
    })?;
    if no_new_privs().map_err(|_| HelperFailure::PrivilegeDrop)?
        && current.effective.is_empty()
        && current.permitted.is_empty()
        && current.inheritable.is_empty()
        && bounding_empty
    {
        Ok(())
    } else {
        Err(HelperFailure::PrivilegeDrop)
    }
}

pub(super) fn drop_bootstrap_namespace_init_privileges() -> Result<(), HelperFailure> {
    drop_privileges()
}

fn establish_bootstrap_namespaces(
    output: &File,
    input_files: &[ControlledBuildInputFile],
    snapshot_deadline: Instant,
) -> Result<(), HelperFailure> {
    if visible_thread_count()? != 1 {
        return Err(HelperFailure::NamespaceSetup);
    }
    let host_user_id = getuid().as_raw();
    let host_group_id = getgid().as_raw();
    let namespaces = UnshareFlags::NEWUSER
        | UnshareFlags::NEWNET
        | UnshareFlags::NEWNS
        | UnshareFlags::NEWIPC
        | UnshareFlags::NEWUTS;
    #[expect(
        deprecated,
        reason = "the dedicated helper is verified single-threaded"
    )]
    rustix::thread::unshare(namespaces).map_err(classify_unshare_error)?;
    write_identity_maps(host_user_id, host_group_id)?;
    linux_build_mount::establish(output, input_files, snapshot_deadline)
}

fn enter_bootstrap_process_namespace() -> Result<(), HelperFailure> {
    #[expect(
        deprecated,
        reason = "the dedicated helper is verified single-threaded"
    )]
    rustix::thread::unshare(UnshareFlags::NEWPID).map_err(classify_unshare_error)
}

fn establish_isolation_inner(
    limits: (u64, u64),
    build_roots: Option<(&File, &[ControlledBuildInputFile], Instant)>,
) -> Result<EstablishedIsolation, HelperFailure> {
    establish_namespaces(build_roots)?;
    finish_isolation(limits)
}

fn establish_managed_namespaces() -> Result<(), HelperFailure> {
    if visible_thread_count()? != 1 {
        return Err(HelperFailure::NamespaceSetup);
    }
    let host_user_id = getuid().as_raw();
    let host_group_id = getgid().as_raw();
    let namespaces =
        UnshareFlags::NEWUSER | UnshareFlags::NEWNET | UnshareFlags::NEWPID | UnshareFlags::NEWNS;
    #[expect(
        deprecated,
        reason = "the dedicated helper is verified single-threaded"
    )]
    rustix::thread::unshare(namespaces).map_err(classify_unshare_error)?;
    write_identity_maps(host_user_id, host_group_id)
}

fn establish_namespaces(
    build_roots: Option<(&File, &[ControlledBuildInputFile], Instant)>,
) -> Result<(), HelperFailure> {
    if visible_thread_count()? != 1 {
        return Err(HelperFailure::NamespaceSetup);
    }
    let host_user_id = getuid().as_raw();
    let host_group_id = getgid().as_raw();
    let mut namespaces = UnshareFlags::NEWUSER | UnshareFlags::NEWNET | UnshareFlags::NEWPID;
    if build_roots.is_some() {
        namespaces |= UnshareFlags::NEWNS | UnshareFlags::NEWIPC | UnshareFlags::NEWUTS;
    }
    #[expect(
        deprecated,
        reason = "the dedicated helper is verified single-threaded"
    )]
    let result = rustix::thread::unshare(namespaces);
    result.map_err(classify_unshare_error)?;
    write_identity_maps(host_user_id, host_group_id)?;
    if let Some((output, input_files, snapshot_deadline)) = build_roots {
        linux_build_mount::establish(output, input_files, snapshot_deadline)?;
    }
    Ok(())
}

fn finish_isolation(limits: (u64, u64)) -> Result<EstablishedIsolation, HelperFailure> {
    apply_resource_limits(limits)?;
    let loopback_index = super::linux_link::enable_and_validate_loopback()?;
    run_network_canaries()?;
    drop_privileges()?;
    Ok(EstablishedIsolation { loopback_index })
}

pub(super) fn validate_descriptor_set() -> Result<(), HelperFailure> {
    // Seal ambient caller descriptors before stage two exec, then verify the postcondition.
    close_fds::set_fds_cloexec(3, &[]);
    let entries = fs::read_dir("/proc/self/fd").map_err(|_| HelperFailure::DescriptorLeak)?;
    let descriptors = entries
        .map(|entry| {
            entry
                .map_err(|_| HelperFailure::DescriptorLeak)?
                .file_name()
                .to_string_lossy()
                .parse::<u32>()
                .map_err(|_| HelperFailure::DescriptorLeak)
        })
        .collect::<Result<Vec<_>, _>>()?;
    for descriptor in descriptors.into_iter().filter(|descriptor| *descriptor > 2) {
        let path = format!("/proc/self/fdinfo/{descriptor}");
        let info = match fs::read_to_string(path) {
            Ok(info) => info,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(_) => return Err(HelperFailure::DescriptorLeak),
        };
        if !descriptor_has_close_on_exec(&info) {
            return Err(HelperFailure::DescriptorLeak);
        }
    }
    Ok(())
}

fn descriptor_has_close_on_exec(info: &str) -> bool {
    info.lines()
        .find_map(|line| line.strip_prefix("flags:\t"))
        .is_some_and(|flags| {
            u64::from_str_radix(flags, 8)
                .is_ok_and(|flags| flags & u64::try_from(libc::O_CLOEXEC).unwrap_or(0) != 0)
        })
}

fn visible_thread_count() -> Result<usize, HelperFailure> {
    fs::read_dir("/proc/self/task")
        .map_err(|_| HelperFailure::NamespaceSetup)?
        .try_fold(0_usize, |count, entry| {
            entry
                .map(|_entry| count.saturating_add(1))
                .map_err(|_| HelperFailure::NamespaceSetup)
        })
}

fn write_identity_maps(uid: u32, gid: u32) -> Result<(), HelperFailure> {
    fs::write("/proc/self/setgroups", "deny\n").map_err(|error| classify_mapping_error(&error))?;
    fs::write("/proc/self/uid_map", format!("0 {uid} 1\n"))
        .map_err(|error| classify_mapping_error(&error))?;
    fs::write("/proc/self/gid_map", format!("0 {gid} 1\n"))
        .map_err(|error| classify_mapping_error(&error))?;
    Ok(())
}

fn classify_unshare_error(error: rustix::io::Errno) -> HelperFailure {
    if matches!(error, rustix::io::Errno::PERM | rustix::io::Errno::ACCESS) {
        HelperFailure::HostPolicyDenied
    } else {
        HelperFailure::NamespaceSetup
    }
}

fn classify_mapping_error(error: &io::Error) -> HelperFailure {
    if error.kind() == io::ErrorKind::PermissionDenied {
        HelperFailure::HostPolicyDenied
    } else {
        HelperFailure::NamespaceSetup
    }
}

fn namespace_identity(path: &str) -> Result<RawNamespaceIdentity, HelperFailure> {
    let metadata = File::open(path)
        .and_then(|file| file.metadata())
        .map_err(|_| HelperFailure::NamespaceSetup)?;
    Ok(RawNamespaceIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

#[cfg(test)]
mod descriptor_tests {
    use super::descriptor_has_close_on_exec;

    #[test]
    fn fdinfo_requires_a_valid_close_on_exec_flag() {
        assert!(descriptor_has_close_on_exec("pos:\t0\nflags:\t02000000\n"));
        assert!(!descriptor_has_close_on_exec("pos:\t0\nflags:\t00\n"));
        assert!(!descriptor_has_close_on_exec("flags:\tnot-octal\n"));
        assert!(!descriptor_has_close_on_exec("pos:\t0\n"));
    }
}
