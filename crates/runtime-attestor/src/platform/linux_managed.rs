use std::{
    fs::{self, File},
    net::SocketAddr,
    os::unix::fs::MetadataExt,
    time::Instant,
};

use rewrite_types::{CancellationToken, Digest};
use rustix::{
    fd::OwnedFd,
    process::{Pid, PidfdFlags, geteuid, pidfd_open},
};

use super::{
    file::hash_opened_file,
    linux::{
        ListenerOwner, ListenerSocketIdentity, ensure_pidfd_alive, entrypoint_metadata,
        listener_identity, process_start_token,
    },
    linux_connection::{RetainedConnectionIdentity, visible_same_uid_holders},
    linux_sock_diag::SockDiagSession,
};
use crate::{
    AttachedProcessEvidence, AttachedProcessEvidenceClass, AttachedProcessEvidenceInput,
    AttachedProcessWitnessError, AttachedProcessWitnessLimits, ListenerEndpoint,
    ManagedLinuxProcessExpectation, NativeLoadObservationLimits, NativeLoadObservationRequest,
    NativeLoadObserverError, RetainedTcpConnection, RetainedTcpConnectionEvidence, ensure_active,
};

mod retry;
#[cfg(test)]
mod test_diagnostics;

use retry::retry_incomplete_snapshot;
#[cfg(test)]
pub(super) use test_diagnostics::{ManagedSnapshotTestReason, record_snapshot_test_reason};

pub(crate) struct Lease {
    endpoint: ListenerEndpoint,
    owner: ListenerOwner,
    pidfd: OwnedFd,
    entrypoint: File,
    network_namespace: File,
    expected: ManagedLinuxProcessExpectation,
    outer_uid: u32,
    initial: AttachedProcessEvidence,
    sock_diag: SockDiagSession,
    connection_identity: Option<RetainedConnectionIdentity>,
}

impl Lease {
    pub(crate) fn attach(
        endpoint: ListenerEndpoint,
        diagnostics: File,
        expected: ManagedLinuxProcessExpectation,
        limits: AttachedProcessWitnessLimits,
        cancellation: &CancellationToken,
        started: Instant,
    ) -> Result<Self, AttachedProcessWitnessError> {
        ensure_active(cancellation, started, limits)?;
        let outer_uid = geteuid().as_raw();
        let pidfd = open_pidfd(expected.outer_pid())?;
        let mut entrypoint = open_process_file(expected.outer_pid(), "exe")?;
        let network_namespace = open_process_file(expected.outer_pid(), "ns/net")?;
        validate_target(expected, outer_uid, &pidfd, &entrypoint, &network_namespace)?;
        let mut sock_diag = SockDiagSession::from_file(diagnostics)?;
        let owner = managed_listener_owner(
            &mut sock_diag,
            endpoint.socket(),
            expected,
            outer_uid,
            limits,
            cancellation,
            started,
        )?;
        let initial = observe_target(
            endpoint,
            owner,
            &mut entrypoint,
            &network_namespace,
            expected,
            outer_uid,
            sock_diag.port_id(),
            limits,
            cancellation,
            started,
        )?;
        let confirmed = managed_listener_owner(
            &mut sock_diag,
            endpoint.socket(),
            expected,
            outer_uid,
            limits,
            cancellation,
            started,
        )?;
        if confirmed != owner {
            return Err(AttachedProcessWitnessError::ListenerRebound);
        }
        confirm_entrypoint_digest(
            &mut entrypoint,
            initial.entrypoint_digest(),
            expected,
            limits,
            cancellation,
            started,
        )?;
        validate_target(expected, outer_uid, &pidfd, &entrypoint, &network_namespace)?;
        Ok(Self {
            endpoint,
            owner,
            pidfd,
            entrypoint,
            network_namespace,
            expected,
            outer_uid,
            initial,
            sock_diag,
            connection_identity: None,
        })
    }

    pub(crate) fn initial_evidence(&self) -> &AttachedProcessEvidence {
        &self.initial
    }

    pub(crate) fn reobserve(
        &mut self,
        limits: AttachedProcessWitnessLimits,
        cancellation: &CancellationToken,
        started: Instant,
    ) -> Result<AttachedProcessEvidence, AttachedProcessWitnessError> {
        validate_target(
            self.expected,
            self.outer_uid,
            &self.pidfd,
            &self.entrypoint,
            &self.network_namespace,
        )?;
        let owner = managed_listener_owner(
            &mut self.sock_diag,
            self.endpoint.socket(),
            self.expected,
            self.outer_uid,
            limits,
            cancellation,
            started,
        )?;
        if owner != self.owner {
            return Err(AttachedProcessWitnessError::ListenerRebound);
        }
        let mut current_entrypoint = open_process_file(self.expected.outer_pid(), "exe")?;
        let current_namespace = open_process_file(self.expected.outer_pid(), "ns/net")?;
        let observed = observe_target(
            self.endpoint,
            owner,
            &mut current_entrypoint,
            &current_namespace,
            self.expected,
            self.outer_uid,
            self.sock_diag.port_id(),
            limits,
            cancellation,
            started,
        )?;
        let confirmed = managed_listener_owner(
            &mut self.sock_diag,
            self.endpoint.socket(),
            self.expected,
            self.outer_uid,
            limits,
            cancellation,
            started,
        )?;
        if confirmed != owner {
            return Err(AttachedProcessWitnessError::ListenerRebound);
        }
        confirm_entrypoint_digest(
            &mut current_entrypoint,
            observed.entrypoint_digest(),
            self.expected,
            limits,
            cancellation,
            started,
        )?;
        ensure_pidfd_alive(&self.pidfd)?;
        Ok(observed)
    }

    pub(crate) fn observe_connection(
        &mut self,
        connection: RetainedTcpConnection,
        limits: AttachedProcessWitnessLimits,
        cancellation: &CancellationToken,
        started: Instant,
    ) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError> {
        super::linux_connection::observe_connection(
            connection,
            &mut self.sock_diag,
            &mut self.connection_identity,
            limits,
            cancellation,
            started,
            self.expected.outer_pid(),
            self.expected.diagnostics_uid(),
            self.outer_uid,
            &self.pidfd,
            self.initial.evidence_digest(),
        )
    }

    pub(crate) fn observe_native_load(
        &mut self,
        request: &NativeLoadObservationRequest<'_>,
        limits: NativeLoadObservationLimits,
        cancellation: &CancellationToken,
        started: Instant,
        process_evidence_digest: &Digest,
    ) -> Result<rewrite_model::NativeLoadObservation, NativeLoadObserverError> {
        super::linux_native_load::observe(
            self.expected.outer_pid(),
            &self.pidfd,
            &self.entrypoint,
            request,
            limits,
            cancellation,
            started,
            process_evidence_digest,
        )
    }
}

fn open_pidfd(pid: u32) -> Result<OwnedFd, AttachedProcessWitnessError> {
    let pid = i32::try_from(pid)
        .ok()
        .and_then(Pid::from_raw)
        .ok_or(AttachedProcessWitnessError::ProcessInstanceUnavailable)?;
    let pidfd = pidfd_open(pid, PidfdFlags::empty()).map_err(|error| {
        if matches!(error, rustix::io::Errno::PERM | rustix::io::Errno::ACCESS) {
            AttachedProcessWitnessError::ProcessAccessDenied
        } else {
            AttachedProcessWitnessError::ProcessInstanceUnavailable
        }
    })?;
    ensure_pidfd_alive(&pidfd)?;
    Ok(pidfd)
}

fn open_process_file(pid: u32, member: &str) -> Result<File, AttachedProcessWitnessError> {
    File::open(format!("/proc/{pid}/{member}")).map_err(|error| {
        if error.kind() == std::io::ErrorKind::PermissionDenied {
            AttachedProcessWitnessError::ProcessAccessDenied
        } else if member == "exe" {
            AttachedProcessWitnessError::EntrypointUnavailable
        } else {
            AttachedProcessWitnessError::ProcessInstanceUnavailable
        }
    })
}

fn validate_target(
    expected: ManagedLinuxProcessExpectation,
    outer_uid: u32,
    pidfd: &OwnedFd,
    entrypoint: &File,
    network_namespace: &File,
) -> Result<(), AttachedProcessWitnessError> {
    ensure_pidfd_alive(pidfd)?;
    let process_metadata =
        fs::metadata(format!("/proc/{}", expected.outer_pid())).map_err(|error| {
            if error.kind() == std::io::ErrorKind::PermissionDenied {
                AttachedProcessWitnessError::ProcessAccessDenied
            } else {
                AttachedProcessWitnessError::ProcessInstanceUnavailable
            }
        })?;
    if process_metadata.uid() != outer_uid {
        return Err(AttachedProcessWitnessError::ProcessAccessDenied);
    }
    if process_start_token(expected.outer_pid())? != expected.process_start_token() {
        return Err(AttachedProcessWitnessError::ProcessInstanceChanged);
    }
    validate_entrypoint(entrypoint, expected)?;
    validate_namespace(network_namespace, expected)?;
    let current_namespace = open_process_file(expected.outer_pid(), "ns/net")?;
    validate_namespace(&current_namespace, expected)?;
    ensure_pidfd_alive(pidfd)?;
    Ok(())
}

fn confirm_entrypoint_digest(
    entrypoint: &mut File,
    expected_digest: &Digest,
    expected: ManagedLinuxProcessExpectation,
    limits: AttachedProcessWitnessLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<(), AttachedProcessWitnessError> {
    let digest = hash_opened_file(
        entrypoint,
        expected.executable_bytes(),
        limits,
        cancellation,
        started,
    )?;
    if &digest != expected_digest {
        return Err(AttachedProcessWitnessError::EntrypointChanged);
    }
    Ok(())
}

fn validate_entrypoint(
    entrypoint: &File,
    expected: ManagedLinuxProcessExpectation,
) -> Result<(), AttachedProcessWitnessError> {
    let metadata = entrypoint_metadata(entrypoint)?;
    if metadata.dev() != expected.executable_device()
        || metadata.ino() != expected.executable_inode()
        || metadata.len() != expected.executable_bytes()
    {
        return Err(AttachedProcessWitnessError::EntrypointChanged);
    }
    Ok(())
}

fn validate_namespace(
    namespace: &File,
    expected: ManagedLinuxProcessExpectation,
) -> Result<(), AttachedProcessWitnessError> {
    let metadata = namespace
        .metadata()
        .map_err(|_error| AttachedProcessWitnessError::ProcessInstanceUnavailable)?;
    if metadata.dev() != expected.network_namespace_device()
        || metadata.ino() != expected.network_namespace_inode()
    {
        return Err(AttachedProcessWitnessError::ListenerSnapshotIncomplete);
    }
    Ok(())
}

fn managed_listener_owner(
    sock_diag: &mut SockDiagSession,
    endpoint: SocketAddr,
    expected: ManagedLinuxProcessExpectation,
    outer_uid: u32,
    limits: AttachedProcessWitnessLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<ListenerOwner, AttachedProcessWitnessError> {
    retry_incomplete_snapshot(limits, cancellation, started, || {
        managed_listener_owner_once(
            sock_diag,
            endpoint,
            expected,
            outer_uid,
            limits,
            cancellation,
            started,
        )
    })
}

fn managed_listener_owner_once(
    sock_diag: &mut SockDiagSession,
    endpoint: SocketAddr,
    expected: ManagedLinuxProcessExpectation,
    outer_uid: u32,
    limits: AttachedProcessWitnessLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<ListenerOwner, AttachedProcessWitnessError> {
    let before = listener_identity(
        sock_diag,
        endpoint,
        expected.diagnostics_uid(),
        limits,
        cancellation,
        started,
    )
    .inspect_err(|_error| {
        #[cfg(test)]
        record_snapshot_test_reason(ManagedSnapshotTestReason::ListenerBefore);
    })?;
    require_target_holder(
        before,
        expected.outer_pid(),
        outer_uid,
        limits,
        cancellation,
        started,
    )?;
    let after = listener_identity(
        sock_diag,
        endpoint,
        expected.diagnostics_uid(),
        limits,
        cancellation,
        started,
    )
    .inspect_err(|_error| {
        #[cfg(test)]
        record_snapshot_test_reason(ManagedSnapshotTestReason::ListenerAfter);
    })?;
    if after != before {
        return Err(AttachedProcessWitnessError::ListenerRebound);
    }
    Ok(ListenerOwner {
        pid: expected.outer_pid(),
        socket_inode: u64::from(before.inode),
        socket_cookie: before.cookie,
    })
}

fn require_target_holder(
    identity: ListenerSocketIdentity,
    expected_pid: u32,
    outer_uid: u32,
    limits: AttachedProcessWitnessLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<(), AttachedProcessWitnessError> {
    let holders = visible_same_uid_holders(
        u64::from(identity.inode),
        outer_uid,
        limits,
        cancellation,
        started,
    )?;
    match holders.as_slice() {
        [pid] if *pid == expected_pid => Ok(()),
        [] => {
            #[cfg(test)]
            record_snapshot_test_reason(ManagedSnapshotTestReason::HolderEmpty);
            Err(AttachedProcessWitnessError::ListenerSnapshotIncomplete)
        }
        [_] => {
            #[cfg(test)]
            record_snapshot_test_reason(ManagedSnapshotTestReason::HolderWrong);
            Err(AttachedProcessWitnessError::ListenerRebound)
        }
        _ => {
            #[cfg(test)]
            record_snapshot_test_reason(ManagedSnapshotTestReason::HolderAmbiguous);
            Err(AttachedProcessWitnessError::ListenerOwnershipAmbiguous)
        }
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "the evidence record binds every retained managed-process fact"
)]
fn observe_target(
    endpoint: ListenerEndpoint,
    owner: ListenerOwner,
    entrypoint: &mut File,
    network_namespace: &File,
    expected: ManagedLinuxProcessExpectation,
    outer_uid: u32,
    diagnostics_port: u32,
    limits: AttachedProcessWitnessLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<AttachedProcessEvidence, AttachedProcessWitnessError> {
    ensure_active(cancellation, started, limits)?;
    validate_entrypoint(entrypoint, expected)?;
    validate_namespace(network_namespace, expected)?;
    if process_start_token(expected.outer_pid())? != expected.process_start_token() {
        return Err(AttachedProcessWitnessError::ProcessInstanceChanged);
    }
    let digest = hash_opened_file(
        entrypoint,
        expected.executable_bytes(),
        limits,
        cancellation,
        started,
    )?;
    let process = format!(
        "linux-managed-pid-start-v2\0{}\0{}",
        expected.outer_pid(),
        expected.process_start_token()
    );
    let object = format!(
        "linux-managed-file-object-v2\0{}\0{}\0{}",
        expected.executable_device(),
        expected.executable_inode(),
        expected.executable_bytes()
    );
    let ownership = format!(
        "linux-managed-listener-owner-v2\0{}\0{}\0{}\0{}\0{}\0{}\0{}",
        endpoint.socket(),
        expected.outer_pid(),
        owner.socket_inode,
        owner.socket_cookie[0],
        owner.socket_cookie[1],
        expected.diagnostics_uid(),
        outer_uid
    );
    let platform = format!(
        "linux-managed-platform-v2\0{}\0{}\0{}\0{}\0{}",
        expected.network_namespace_device(),
        expected.network_namespace_inode(),
        expected.diagnostics_uid(),
        outer_uid,
        diagnostics_port
    );
    AttachedProcessEvidence::new_managed_linux(AttachedProcessEvidenceInput {
        evidence_class: AttachedProcessEvidenceClass::LinuxManagedNamespaceSockDiag,
        owner_pid: expected.outer_pid(),
        process_instance_digest: Digest::sha256(process.as_bytes()),
        ownership_snapshot_digest: Digest::sha256(ownership.as_bytes()),
        entrypoint_object_digest: Digest::sha256(object.as_bytes()),
        entrypoint_digest: digest,
        entrypoint_bytes: expected.executable_bytes(),
        platform_evidence_digest: Digest::sha256(platform.as_bytes()),
    })
}

#[cfg(test)]
mod tests;
