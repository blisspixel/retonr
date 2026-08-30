use std::{fs::File, time::Instant};

use rewrite_types::CancellationToken;
use rustix::{
    fd::OwnedFd,
    process::{Pid, PidfdFlags, pidfd_open},
};

use crate::{
    IsolationError, IsolationPolicy, IsolationPreparationEvidence, IsolationResult,
    contract::{ManagedNamespaceEvidence, RetainedRuntimeInputDeclaration},
};

use super::super::{
    linux_helper_setup::HelperFailure,
    linux_managed_input_mount, linux_managed_input_protocol, linux_managed_mount,
    linux_protocol::ReadyMessage,
    linux_target::{TargetObservation, observe_target},
    linux_validation::{namespace_identity, native_errno, open_namespace, privileges_are_reduced},
};
use super::lifecycle::ensure_active_until;

pub(super) struct VerifiedLaunch {
    pub(super) pidfd: OwnedFd,
    pub(super) namespace_init_pidfd: OwnedFd,
    pub(super) network_namespace: File,
    pub(super) user_namespace: File,
    pub(super) process_namespace: File,
    pub(super) mount_namespace: File,
    pub(super) namespaces: ManagedNamespaceEvidence,
    pub(super) target: TargetObservation,
}

#[derive(Clone, Copy)]
pub(super) struct LaunchRequest<'a> {
    pub(super) guardian_pid: u32,
    pub(super) target: &'a File,
    pub(super) runtime_inputs: &'a [RetainedRuntimeInputDeclaration],
    pub(super) ready: &'a ReadyMessage,
    pub(super) policy: IsolationPolicy,
    pub(super) preparation: &'a IsolationPreparationEvidence,
    pub(super) cancellation: &'a CancellationToken,
    pub(super) operation_deadline: Option<Instant>,
}

pub(super) fn launch(request: LaunchRequest<'_>) -> IsolationResult<VerifiedLaunch> {
    let LaunchRequest {
        guardian_pid,
        target,
        runtime_inputs,
        ready,
        policy,
        preparation,
        cancellation,
        operation_deadline,
    } = request;
    ensure_active_until(cancellation, operation_deadline)?;
    let pidfd = open_pidfd(guardian_pid, "open-guardian-pidfd")?;
    let network_namespace = open_namespace(guardian_pid, "net")?;
    let user_namespace = open_namespace(guardian_pid, "user")?;
    let process_namespace = open_namespace(ready.namespace_init_pid, "pid")?;
    let mount_namespace = open_namespace(guardian_pid, "mnt")?;
    let namespace_init_pidfd = open_pidfd(ready.namespace_init_pid, "open-namespace-init-pidfd")?;
    let namespaces = ManagedNamespaceEvidence {
        network: namespace_identity(&network_namespace)?,
        user: namespace_identity(&user_namespace)?,
        process: namespace_identity(&process_namespace)?,
        mount: namespace_identity(&mount_namespace)?,
    };
    let target = observe_target(
        ready.namespace_init_pid,
        target,
        namespaces.network,
        namespaces.user,
        namespaces.process,
        namespaces.mount,
    )?;
    linux_managed_mount::reobserve(target.evidence.outer_pid(), &ready.device_boundary)?;
    linux_managed_input_mount::reobserve(
        target.evidence.outer_pid(),
        &ready.runtime_inputs,
        runtime_inputs,
        cancellation,
    )?;
    let expected_inputs = linux_managed_input_protocol::header(runtime_inputs)
        .map_err(HelperFailure::into_isolation_error)?;
    if namespaces.network != ready.network_namespace
        || namespaces.user != ready.user_namespace
        || namespaces.process != ready.process_namespace
        || namespaces.mount != ready.mount_namespace
        || namespace_identity(&open_namespace(ready.namespace_init_pid, "mnt")?)?
            != namespaces.mount
        || ready.device_boundary.policy() != policy.managed_device_visibility()
        || ready.device_boundary.policy() != preparation.managed_device_visibility()
        || !ready.device_boundary.all_visibility_canaries_passed()
        || ready.runtime_inputs.member_count()
            != u32::try_from(expected_inputs.count).unwrap_or(u32::MAX)
        || ready.runtime_inputs.total_bytes() != expected_inputs.total_bytes
        || ready.runtime_inputs.layout_digest() != &expected_inputs.layout_digest
        || !privileges_are_reduced(guardian_pid)?
        || !privileges_are_reduced(ready.namespace_init_pid)?
    {
        return Err(IsolationError::EvidenceChanged);
    }
    ensure_active_until(cancellation, operation_deadline)?;
    Ok(VerifiedLaunch {
        pidfd,
        namespace_init_pidfd,
        network_namespace,
        user_namespace,
        process_namespace,
        mount_namespace,
        namespaces,
        target,
    })
}

fn open_pidfd(pid: u32, operation: &'static str) -> IsolationResult<OwnedFd> {
    let raw_pid = i32::try_from(pid)
        .ok()
        .and_then(Pid::from_raw)
        .ok_or(IsolationError::HelperProtocol)?;
    pidfd_open(raw_pid, PidfdFlags::empty()).map_err(|error| native_errno(operation, error))
}
