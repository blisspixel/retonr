use std::{
    fs::{self, File},
    net::SocketAddr,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    time::Instant,
};

use rewrite_types::{CancellationToken, Digest};
use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    fd::OwnedFd,
    process::{Pid, PidfdFlags, geteuid, pidfd_open},
};

use super::file::hash_opened_file;
use super::linux_connection::RetainedConnectionIdentity;
use super::linux_sock_diag::{InetDiagRecord, SockDiagSession};
use crate::{
    AttachedProcessEvidence, AttachedProcessEvidenceClass, AttachedProcessEvidenceInput,
    AttachedProcessWitnessError, AttachedProcessWitnessLimits, ListenerEndpoint,
    RetainedTcpConnection, RetainedTcpConnectionEvidence, ensure_active,
};
pub(crate) struct Lease {
    endpoint: ListenerEndpoint,
    pub(super) owner: ListenerOwner,
    pub(super) pidfd: OwnedFd,
    pub(super) entrypoint: File,
    initial: AttachedProcessEvidence,
    sock_diag: SockDiagSession,
    expected_uid: u32,
    network_namespace: PathBuf,
    connection_identity: Option<RetainedConnectionIdentity>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ListenerOwner {
    pub(super) pid: u32,
    pub(super) socket_inode: u64,
    pub(super) socket_cookie: [u32; 2],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ListenerSocketIdentity {
    pub(super) inode: u32,
    pub(super) cookie: [u32; 2],
}

impl Lease {
    pub(crate) fn attach(
        endpoint: ListenerEndpoint,
        limits: AttachedProcessWitnessLimits,
        cancellation: &CancellationToken,
        started: Instant,
    ) -> Result<Self, AttachedProcessWitnessError> {
        let expected_uid = geteuid().as_raw();
        let network_namespace = current_network_namespace()?;
        let mut sock_diag = SockDiagSession::new()?;
        if current_network_namespace()? != network_namespace {
            return Err(AttachedProcessWitnessError::ListenerSnapshotIncomplete);
        }
        let owner = listener_owner(
            &mut sock_diag,
            endpoint.socket(),
            expected_uid,
            limits,
            cancellation,
            started,
        )?;
        let pid = owner.pid;
        let raw_pid = i32::try_from(pid)
            .ok()
            .and_then(Pid::from_raw)
            .ok_or(AttachedProcessWitnessError::ProcessInstanceUnavailable)?;
        let pidfd = pidfd_open(raw_pid, PidfdFlags::empty()).map_err(|error| {
            if error == rustix::io::Errno::PERM || error == rustix::io::Errno::ACCESS {
                AttachedProcessWitnessError::ProcessAccessDenied
            } else {
                AttachedProcessWitnessError::ProcessInstanceUnavailable
            }
        })?;
        ensure_pidfd_alive(&pidfd)?;
        let mut entrypoint = open_entrypoint(pid)?;
        let initial = observe_process(
            endpoint,
            owner,
            &mut entrypoint,
            limits,
            cancellation,
            started,
            &network_namespace,
        )?;
        let confirmed = listener_owner(
            &mut sock_diag,
            endpoint.socket(),
            expected_uid,
            limits,
            cancellation,
            started,
        )?;
        if confirmed != owner {
            return Err(AttachedProcessWitnessError::ListenerRebound);
        }
        ensure_pidfd_alive(&pidfd)?;
        Ok(Self {
            endpoint,
            owner,
            pidfd,
            entrypoint,
            initial,
            sock_diag,
            expected_uid,
            network_namespace,
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
        ensure_pidfd_alive(&self.pidfd)?;
        ensure_expected_uid(self.expected_uid)?;
        let owner = listener_owner(
            &mut self.sock_diag,
            self.endpoint.socket(),
            self.expected_uid,
            limits,
            cancellation,
            started,
        )?;
        if owner != self.owner {
            return Err(AttachedProcessWitnessError::ListenerRebound);
        }
        let retained_metadata = entrypoint_metadata(&self.entrypoint)?;
        let retained_digest = hash_opened_file(
            &mut self.entrypoint,
            retained_metadata.len(),
            limits,
            cancellation,
            started,
        )?;
        if retained_digest != *self.initial.entrypoint_digest() {
            return Err(AttachedProcessWitnessError::EntrypointChanged);
        }
        let mut current = open_entrypoint(self.owner.pid)?;
        let observed = observe_process(
            self.endpoint,
            self.owner,
            &mut current,
            limits,
            cancellation,
            started,
            &self.network_namespace,
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
            self.owner.pid,
            self.expected_uid,
            self.expected_uid,
            &self.pidfd,
            self.initial.evidence_digest(),
        )
    }
}

fn listener_owner(
    sock_diag: &mut SockDiagSession,
    endpoint: SocketAddr,
    expected_uid: u32,
    limits: AttachedProcessWitnessLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<ListenerOwner, AttachedProcessWitnessError> {
    ensure_expected_uid(expected_uid)?;
    let identity = listener_identity(
        sock_diag,
        endpoint,
        expected_uid,
        limits,
        cancellation,
        started,
    )?;
    let pid = owners_for_inode(
        u64::from(identity.inode),
        expected_uid,
        limits,
        cancellation,
        started,
    )?;
    let confirmed = listener_identity(
        sock_diag,
        endpoint,
        expected_uid,
        limits,
        cancellation,
        started,
    )?;
    if confirmed != identity {
        return Err(AttachedProcessWitnessError::ListenerRebound);
    }
    Ok(ListenerOwner {
        pid,
        socket_inode: u64::from(identity.inode),
        socket_cookie: identity.cookie,
    })
}

pub(super) fn listener_identity(
    sock_diag: &mut SockDiagSession,
    endpoint: SocketAddr,
    expected_uid: u32,
    limits: AttachedProcessWitnessLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<ListenerSocketIdentity, AttachedProcessWitnessError> {
    let records = sock_diag.listener_dump(endpoint, limits, cancellation, started)?;
    let mut candidates = Vec::new();
    for record in records {
        validate_listener_record(record, endpoint)?;
        if record.local.ip() != endpoint.ip() {
            continue;
        }
        if record.uid != expected_uid {
            return Err(AttachedProcessWitnessError::ProcessAccessDenied);
        }
        if record.inode == 0 || !record.usable_cookie() {
            return Err(AttachedProcessWitnessError::ListenerSnapshotIncomplete);
        }
        candidates.push(ListenerSocketIdentity {
            inode: record.inode,
            cookie: record.cookie,
        });
    }
    match candidates.as_slice() {
        [identity] => Ok(*identity),
        [] => Err(AttachedProcessWitnessError::ListenerNotFound),
        _ => Err(AttachedProcessWitnessError::ListenerOwnershipAmbiguous),
    }
}

pub(super) fn validate_listener_record(
    record: InetDiagRecord,
    endpoint: SocketAddr,
) -> Result<(), AttachedProcessWitnessError> {
    if !record.is_listening()
        || record.local.is_ipv4() != endpoint.is_ipv4()
        || record.local.port() != endpoint.port()
        || !record.remote.ip().is_unspecified()
        || record.remote.port() != 0
    {
        return Err(AttachedProcessWitnessError::ListenerSnapshotIncomplete);
    }
    Ok(())
}

fn current_network_namespace() -> Result<PathBuf, AttachedProcessWitnessError> {
    fs::read_link("/proc/thread-self/ns/net")
        .map_err(|_error| AttachedProcessWitnessError::ListenerSnapshotIncomplete)
}

fn ensure_expected_uid(expected_uid: u32) -> Result<(), AttachedProcessWitnessError> {
    if geteuid().as_raw() != expected_uid {
        return Err(AttachedProcessWitnessError::ProcessAccessDenied);
    }
    Ok(())
}

fn owners_for_inode(
    inode: u64,
    expected_uid: u32,
    limits: AttachedProcessWitnessLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<u32, AttachedProcessWitnessError> {
    let owners = super::linux_connection::visible_same_uid_holders(
        inode,
        expected_uid,
        limits,
        cancellation,
        started,
    )?;
    match owners.as_slice() {
        [] => Err(AttachedProcessWitnessError::ListenerSnapshotIncomplete),
        [owner] => Ok(*owner),
        _ => Err(AttachedProcessWitnessError::ListenerOwnershipAmbiguous),
    }
}

fn observe_process(
    endpoint: ListenerEndpoint,
    owner: ListenerOwner,
    entrypoint: &mut File,
    limits: AttachedProcessWitnessLimits,
    cancellation: &CancellationToken,
    started: Instant,
    network_namespace: &Path,
) -> Result<AttachedProcessEvidence, AttachedProcessWitnessError> {
    ensure_active(cancellation, started, limits)?;
    let pid = owner.pid;
    let start_token = process_start_token(pid)?;
    let process_namespace = fs::read_link(format!("/proc/{pid}/ns/net")).map_err(|error| {
        if error.kind() == std::io::ErrorKind::PermissionDenied {
            AttachedProcessWitnessError::ProcessAccessDenied
        } else {
            AttachedProcessWitnessError::ProcessInstanceUnavailable
        }
    })?;
    if network_namespace != process_namespace {
        return Err(AttachedProcessWitnessError::ListenerSnapshotIncomplete);
    }
    let metadata = entrypoint_metadata(entrypoint)?;
    let entrypoint_digest =
        hash_opened_file(entrypoint, metadata.len(), limits, cancellation, started)?;
    let process_instance_digest =
        Digest::sha256(format!("linux-pid-start-v1\0{pid}\0{start_token}").as_bytes());
    let object_material = format!(
        "linux-file-object-v1\0{}\0{}\0{}",
        metadata.dev(),
        metadata.ino(),
        metadata.len()
    );
    let endpoint_material = format!(
        "linux-listener-owner-v2\0{}\0{pid}\0{start_token}\0{}\0{}\0{}",
        endpoint.socket(),
        owner.socket_inode,
        owner.socket_cookie[0],
        owner.socket_cookie[1]
    );
    let platform_material = format!(
        "linux-platform-evidence-v1\0{}",
        network_namespace.to_string_lossy()
    );
    AttachedProcessEvidence::new(AttachedProcessEvidenceInput {
        evidence_class: AttachedProcessEvidenceClass::LinuxProcPidfd,
        owner_pid: pid,
        process_instance_digest,
        ownership_snapshot_digest: Digest::sha256(endpoint_material.as_bytes()),
        entrypoint_object_digest: Digest::sha256(object_material.as_bytes()),
        entrypoint_digest,
        entrypoint_bytes: metadata.len(),
        platform_evidence_digest: Digest::sha256(platform_material.as_bytes()),
    })
}

fn open_entrypoint(pid: u32) -> Result<File, AttachedProcessWitnessError> {
    File::open(format!("/proc/{pid}/exe")).map_err(|error| {
        if error.kind() == std::io::ErrorKind::PermissionDenied {
            AttachedProcessWitnessError::ProcessAccessDenied
        } else {
            AttachedProcessWitnessError::EntrypointUnavailable
        }
    })
}

pub(super) fn entrypoint_metadata(
    file: &File,
) -> Result<fs::Metadata, AttachedProcessWitnessError> {
    let metadata = file
        .metadata()
        .map_err(|_error| AttachedProcessWitnessError::EntrypointUnavailable)?;
    if !metadata.is_file() {
        return Err(AttachedProcessWitnessError::EntrypointNotRegular);
    }
    if metadata.nlink() != 1 {
        return Err(AttachedProcessWitnessError::EntrypointAliased);
    }
    Ok(metadata)
}

pub(super) fn process_start_token(pid: u32) -> Result<u64, AttachedProcessWitnessError> {
    let bytes = read_bounded(Path::new(&format!("/proc/{pid}/stat")), 16 * 1024)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_error| AttachedProcessWitnessError::ProcessInstanceUnavailable)?;
    let close = text
        .rfind(')')
        .ok_or(AttachedProcessWitnessError::ProcessInstanceUnavailable)?;
    let fields = text
        .get(close.saturating_add(1)..)
        .ok_or(AttachedProcessWitnessError::ProcessInstanceUnavailable)?
        .split_ascii_whitespace()
        .collect::<Vec<_>>();
    let start = fields
        .get(19)
        .ok_or(AttachedProcessWitnessError::ProcessInstanceUnavailable)?
        .parse::<u64>()
        .map_err(|_error| AttachedProcessWitnessError::ProcessInstanceUnavailable)?;
    if start == 0 {
        return Err(AttachedProcessWitnessError::ProcessInstanceUnavailable);
    }
    Ok(start)
}

pub(super) fn ensure_pidfd_alive(pidfd: &OwnedFd) -> Result<(), AttachedProcessWitnessError> {
    let mut descriptors = [PollFd::new(pidfd, PollFlags::IN)];
    let ready = poll(&mut descriptors, Some(&Timespec::default()))
        .map_err(|_error| AttachedProcessWitnessError::ProcessInstanceUnavailable)?;
    if ready == 0 {
        Ok(())
    } else {
        Err(AttachedProcessWitnessError::ProcessExited)
    }
}

pub(super) fn read_bounded(
    path: &Path,
    maximum: usize,
) -> Result<Vec<u8>, AttachedProcessWitnessError> {
    use std::io::Read as _;

    let file = File::open(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::PermissionDenied {
            AttachedProcessWitnessError::ProcessAccessDenied
        } else {
            AttachedProcessWitnessError::ListenerSnapshotIncomplete
        }
    })?;
    let limit = u64::try_from(maximum)
        .map_err(|_error| AttachedProcessWitnessError::ResourceLimit)?
        .saturating_add(1);
    let mut bytes = Vec::with_capacity(maximum.min(64 * 1024));
    file.take(limit)
        .read_to_end(&mut bytes)
        .map_err(|_error| AttachedProcessWitnessError::ListenerSnapshotIncomplete)?;
    if bytes.len() > maximum {
        return Err(AttachedProcessWitnessError::ResourceLimit);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        net::{Ipv4Addr, TcpListener},
        os::fd::AsRawFd,
        time::Instant,
    };

    use rewrite_types::CancellationToken;

    use super::{ListenerOwner, current_network_namespace, observe_process, open_entrypoint};
    use crate::platform::linux_connection::process_has_inode;
    use crate::{AttachedProcessWitnessLimits, ListenerEndpoint};

    #[test]
    fn current_process_descriptor_and_executable_produce_native_evidence() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind listener");
        let endpoint = ListenerEndpoint::new(listener.local_addr().expect("listener address"))
            .expect("loopback endpoint");
        let target = fs::read_link(format!("/proc/self/fd/{}", listener.as_raw_fd()))
            .expect("listener descriptor link");
        let target = target.to_str().expect("socket descriptor text");
        let inode = target
            .strip_prefix("socket:[")
            .and_then(|value| value.strip_suffix(']'))
            .expect("socket inode wrapper")
            .parse::<u64>()
            .expect("socket inode");
        let limits = AttachedProcessWitnessLimits::default();
        let cancellation = CancellationToken::new();
        assert!(
            process_has_inode(
                std::process::id(),
                inode,
                limits.maximum_descriptors_per_process,
                &cancellation,
                Instant::now(),
                limits,
            )
            .expect("inspect current descriptors")
        );
        let mut entrypoint = open_entrypoint(std::process::id()).expect("open current executable");
        let evidence = observe_process(
            endpoint,
            ListenerOwner {
                pid: std::process::id(),
                socket_inode: inode,
                socket_cookie: [1, 2],
            },
            &mut entrypoint,
            limits,
            &cancellation,
            Instant::now(),
            &current_network_namespace().expect("current network namespace"),
        )
        .expect("observe current process");
        assert_eq!(evidence.owner_pid(), std::process::id());
        assert!(evidence.entrypoint_bytes() > 0);
    }
}
