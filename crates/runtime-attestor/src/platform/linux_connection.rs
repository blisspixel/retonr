use std::{collections::BTreeSet, fs, os::unix::fs::MetadataExt, path::PathBuf, time::Instant};

use rewrite_types::{CancellationToken, Digest};
use rustix::fd::OwnedFd;

use super::{
    linux::ensure_pidfd_alive,
    linux_sock_diag::{InetDiagRecord, NO_COOKIE, SockDiagSession},
};
use crate::{
    AttachedProcessWitnessError, AttachedProcessWitnessLimits, RetainedTcpConnection,
    RetainedTcpConnectionEvidence, RetainedTcpConnectionEvidenceInput,
    TcpConnectionAttributionKind, TcpConnectionSharingLimitation,
    connection::connection_digest_material, ensure_active,
};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) struct RetainedConnectionIdentity {
    connection: RetainedTcpConnection,
    interface: u32,
    cookie: [u32; 2],
    uid: u32,
    inode: u32,
}

#[expect(
    clippy::too_many_arguments,
    reason = "the observation trust boundary is explicit"
)]
pub(super) fn observe_connection(
    connection: RetainedTcpConnection,
    sock_diag: &mut SockDiagSession,
    retained_identity: &mut Option<RetainedConnectionIdentity>,
    limits: AttachedProcessWitnessLimits,
    cancellation: &CancellationToken,
    started: Instant,
    retained_pid: u32,
    diagnostics_uid: u32,
    outer_uid: u32,
    pidfd: &OwnedFd,
    process_evidence_digest: &Digest,
) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError> {
    ensure_active(cancellation, started, limits)?;
    ensure_pidfd_alive(pidfd)?;
    let expected = retained_identity.as_ref().copied();
    if expected.is_some_and(|identity| identity.connection != connection) {
        return Err(AttachedProcessWitnessError::ConnectionSnapshotIncomplete);
    }
    let (interface, cookie) = expected.map_or((0, NO_COOKIE), |identity| {
        (identity.interface, identity.cookie)
    });
    let before_record =
        sock_diag.exact_connection(connection, interface, cookie, limits, cancellation, started)?;
    require_established(before_record)?;
    let before = validate_identity(before_record, connection, diagnostics_uid)?;
    if expected.is_some_and(|identity| identity != before) {
        return Err(AttachedProcessWitnessError::ConnectionSnapshotIncomplete);
    }
    *retained_identity = Some(before);
    let holders = visible_same_uid_holders(
        u64::from(before.inode),
        outer_uid,
        limits,
        cancellation,
        started,
    )
    .map_err(|error| {
        map_connection_snapshot_error(
            error,
            AttachedProcessWitnessError::ConnectionSnapshotIncomplete,
        )
    })?;
    require_retained_holder(&holders, retained_pid)?;
    let after_record = sock_diag.exact_connection(
        connection,
        before.interface,
        before.cookie,
        limits,
        cancellation,
        started,
    )?;
    require_established(after_record)?;
    let after = validate_identity(after_record, connection, diagnostics_uid)?;
    if after != before {
        return Err(AttachedProcessWitnessError::ConnectionSnapshotIncomplete);
    }
    ensure_pidfd_alive(pidfd)?;
    connection_evidence(before, retained_pid, process_evidence_digest)
}

fn validate_identity(
    record: InetDiagRecord,
    connection: RetainedTcpConnection,
    expected_uid: u32,
) -> Result<RetainedConnectionIdentity, AttachedProcessWitnessError> {
    if record.local != connection.server() || record.remote != connection.client() {
        return Err(AttachedProcessWitnessError::ConnectionSnapshotIncomplete);
    }
    if record.uid != expected_uid {
        return Err(AttachedProcessWitnessError::ConnectionProcessMismatch);
    }
    if record.inode == 0 || !record.usable_cookie() {
        return Err(AttachedProcessWitnessError::ConnectionSnapshotIncomplete);
    }
    Ok(RetainedConnectionIdentity {
        connection,
        interface: record.interface,
        cookie: record.cookie,
        uid: record.uid,
        inode: record.inode,
    })
}

fn require_established(record: InetDiagRecord) -> Result<(), AttachedProcessWitnessError> {
    if record.is_established() {
        Ok(())
    } else {
        Err(AttachedProcessWitnessError::ConnectionNotEstablished)
    }
}

fn connection_evidence(
    identity: RetainedConnectionIdentity,
    retained_pid: u32,
    process_evidence_digest: &Digest,
) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError> {
    let mut material = connection_digest_material(
        b"retonr:linux-sock-diag-connection:v1\0",
        identity.connection,
    );
    material.extend_from_slice(&identity.interface.to_be_bytes());
    material.extend_from_slice(&identity.cookie[0].to_be_bytes());
    material.extend_from_slice(&identity.cookie[1].to_be_bytes());
    material.extend_from_slice(&identity.uid.to_be_bytes());
    material.extend_from_slice(&identity.inode.to_be_bytes());
    material.extend_from_slice(&retained_pid.to_be_bytes());
    RetainedTcpConnectionEvidence::new(&RetainedTcpConnectionEvidenceInput {
        attribution_kind: TcpConnectionAttributionKind::LinuxSocketInodeVisibleSameUidHolder,
        sharing_limitation:
            TcpConnectionSharingLimitation::LinuxOnlyVisibleSameUidDescriptorHoldersChecked,
        process_evidence_digest: process_evidence_digest.clone(),
        platform_connection_digest: Digest::sha256(&material),
    })
}

fn require_retained_holder(
    holders: &[u32],
    retained_pid: u32,
) -> Result<(), AttachedProcessWitnessError> {
    match holders {
        [pid] if *pid == retained_pid => Ok(()),
        [] => Err(AttachedProcessWitnessError::ConnectionSnapshotIncomplete),
        [_] => Err(AttachedProcessWitnessError::ConnectionProcessMismatch),
        _ => Err(AttachedProcessWitnessError::ConnectionAmbiguous),
    }
}

fn map_connection_snapshot_error(
    error: AttachedProcessWitnessError,
    snapshot_error: AttachedProcessWitnessError,
) -> AttachedProcessWitnessError {
    match error {
        AttachedProcessWitnessError::ListenerSnapshotIncomplete => snapshot_error,
        other => other,
    }
}

pub(super) fn visible_same_uid_holders(
    inode: u64,
    expected_uid: u32,
    limits: AttachedProcessWitnessLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<Vec<u32>, AttachedProcessWitnessError> {
    let mut owners = BTreeSet::new();
    let mut process_count = 0_usize;
    let entries = fs::read_dir("/proc")
        .map_err(|_error| AttachedProcessWitnessError::ListenerSnapshotIncomplete)?;
    for entry in entries {
        ensure_active(cancellation, started, limits)?;
        let entry =
            entry.map_err(|_error| AttachedProcessWitnessError::ListenerSnapshotIncomplete)?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Ok(pid) = name.parse::<u32>() else {
            continue;
        };
        process_count = process_count
            .checked_add(1)
            .ok_or(AttachedProcessWitnessError::ResourceLimit)?;
        if process_count > limits.maximum_processes {
            return Err(AttachedProcessWitnessError::ResourceLimit);
        }
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                return Err(AttachedProcessWitnessError::ProcessAccessDenied);
            }
            Err(_) => return Err(AttachedProcessWitnessError::ListenerSnapshotIncomplete),
        };
        if metadata.uid() != expected_uid {
            continue;
        }
        if process_has_inode(
            pid,
            inode,
            limits.maximum_descriptors_per_process,
            cancellation,
            started,
            limits,
        )? {
            owners.insert(pid);
        }
    }
    Ok(owners.into_iter().collect())
}

pub(super) fn process_has_inode(
    pid: u32,
    inode: u64,
    maximum_descriptors: usize,
    cancellation: &CancellationToken,
    started: Instant,
    limits: AttachedProcessWitnessLimits,
) -> Result<bool, AttachedProcessWitnessError> {
    let directory = PathBuf::from(format!("/proc/{pid}/fd"));
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            return Err(AttachedProcessWitnessError::ProcessAccessDenied);
        }
        Err(_) => return Err(AttachedProcessWitnessError::ListenerSnapshotIncomplete),
    };
    let expected = format!("socket:[{inode}]");
    let mut descriptor_count = 0_usize;
    for entry in entries {
        ensure_active(cancellation, started, limits)?;
        descriptor_count = descriptor_count
            .checked_add(1)
            .ok_or(AttachedProcessWitnessError::ResourceLimit)?;
        if descriptor_count > maximum_descriptors {
            return Err(AttachedProcessWitnessError::ResourceLimit);
        }
        let entry =
            entry.map_err(|_error| AttachedProcessWitnessError::ListenerSnapshotIncomplete)?;
        match fs::read_link(entry.path()) {
            Ok(target) if target.as_os_str() == expected.as_str() => return Ok(true),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(AttachedProcessWitnessError::ListenerSnapshotIncomplete),
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, SocketAddr};

    use rewrite_types::Digest;

    use super::{
        RetainedConnectionIdentity, connection_evidence, require_established,
        require_retained_holder, validate_identity,
    };
    use crate::{
        AttachedProcessWitnessError, RetainedTcpConnection,
        platform::linux_sock_diag::InetDiagRecord,
    };

    const UID: u32 = 1000;
    const INODE: u32 = 998_877;

    #[test]
    fn record_requires_exact_established_tuple_uid_inode_and_cookie() {
        let connection = connection();
        assert!(validate_identity(record(connection), connection, UID).is_ok());

        let mut candidate = record(connection);
        candidate.state = 8;
        assert!(matches!(
            require_established(candidate),
            Err(AttachedProcessWitnessError::ConnectionNotEstablished)
        ));
        assert!(validate_identity(candidate, connection, UID).is_ok());
        let mut candidate = record(connection);
        candidate.remote.set_port(candidate.remote.port() + 1);
        assert!(matches!(
            validate_identity(candidate, connection, UID),
            Err(AttachedProcessWitnessError::ConnectionSnapshotIncomplete)
        ));
        let mut candidate = record(connection);
        candidate.uid += 1;
        assert!(matches!(
            validate_identity(candidate, connection, UID),
            Err(AttachedProcessWitnessError::ConnectionProcessMismatch)
        ));
        let mut candidate = record(connection);
        candidate.inode = 0;
        assert!(matches!(
            validate_identity(candidate, connection, UID),
            Err(AttachedProcessWitnessError::ConnectionSnapshotIncomplete)
        ));
        let mut candidate = record(connection);
        candidate.cookie = [u32::MAX; 2];
        assert!(matches!(
            validate_identity(candidate, connection, UID),
            Err(AttachedProcessWitnessError::ConnectionSnapshotIncomplete)
        ));
    }

    #[test]
    fn visible_holder_policy_rejects_missing_mismatched_and_shared_views() {
        assert_eq!(
            require_retained_holder(&[], 91),
            Err(AttachedProcessWitnessError::ConnectionSnapshotIncomplete)
        );
        assert_eq!(
            require_retained_holder(&[92], 91),
            Err(AttachedProcessWitnessError::ConnectionProcessMismatch)
        );
        assert_eq!(
            require_retained_holder(&[91, 92], 91),
            Err(AttachedProcessWitnessError::ConnectionAmbiguous)
        );
        assert_eq!(require_retained_holder(&[91], 91), Ok(()));
    }

    #[test]
    fn socket_identity_changes_redacted_evidence() {
        let process = Digest::sha256(b"process evidence");
        let identity = validate_identity(record(connection()), connection(), UID)
            .expect("valid connection identity");
        let first = connection_evidence(identity, 91, &process).expect("first evidence");
        let second = connection_evidence(
            RetainedConnectionIdentity {
                cookie: [identity.cookie[0] + 1, identity.cookie[1]],
                ..identity
            },
            91,
            &process,
        )
        .expect("second evidence");
        assert_ne!(first.evidence_digest(), second.evidence_digest());
        let encoded = serde_json::to_string(&first).expect("serialize evidence");
        assert!(!encoded.contains(&INODE.to_string()));
        assert!(!encoded.contains("41000"));
        assert!(!encoded.contains("11434"));
        assert!(!encoded.contains("123456"));
    }

    fn record(connection: RetainedTcpConnection) -> InetDiagRecord {
        InetDiagRecord {
            state: 1,
            local: connection.server(),
            remote: connection.client(),
            interface: 7,
            cookie: [123_456, 654_321],
            uid: UID,
            inode: INODE,
        }
    }

    fn connection() -> RetainedTcpConnection {
        RetainedTcpConnection::new(
            SocketAddr::from((Ipv4Addr::LOCALHOST, 41_000)),
            SocketAddr::from((Ipv4Addr::LOCALHOST, 11_434)),
        )
        .expect("valid connection")
    }
}
