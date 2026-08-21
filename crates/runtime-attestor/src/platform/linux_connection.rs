use std::{
    collections::BTreeSet,
    fs,
    net::IpAddr,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    time::Instant,
};

use rewrite_types::{CancellationToken, Digest};
use rustix::{fd::OwnedFd, process::geteuid};

use super::linux::{encoded_local_address, ensure_pidfd_alive, read_bounded};
use crate::{
    AttachedProcessWitnessError, AttachedProcessWitnessLimits, RetainedTcpConnection,
    RetainedTcpConnectionEvidence, RetainedTcpConnectionEvidenceInput,
    TcpConnectionAttributionKind, TcpConnectionSharingLimitation,
    connection::connection_digest_material, ensure_active,
};

const ESTABLISHED_STATE: &str = "01";

pub(super) fn observe_connection(
    connection: RetainedTcpConnection,
    limits: AttachedProcessWitnessLimits,
    cancellation: &CancellationToken,
    started: Instant,
    retained_pid: u32,
    pidfd: &OwnedFd,
    process_evidence_digest: &Digest,
) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError> {
    ensure_active(cancellation, started, limits)?;
    ensure_pidfd_alive(pidfd)?;
    let expected_uid = geteuid().as_raw();
    let inode = connection_inode(connection, expected_uid, limits, cancellation, started)?;
    let holders = visible_same_uid_holders(inode, expected_uid, limits, cancellation, started)
        .map_err(|error| {
            map_connection_snapshot_error(
                error,
                AttachedProcessWitnessError::ConnectionSnapshotIncomplete,
            )
        })?;
    require_retained_holder(&holders, retained_pid)?;
    ensure_pidfd_alive(pidfd)?;
    connection_evidence(
        connection,
        inode,
        expected_uid,
        retained_pid,
        process_evidence_digest,
    )
}

fn connection_evidence(
    connection: RetainedTcpConnection,
    inode: u64,
    expected_uid: u32,
    retained_pid: u32,
    process_evidence_digest: &Digest,
) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError> {
    let mut material =
        connection_digest_material(b"retonr:linux-proc-connection-inode:v1\0", connection);
    material.extend_from_slice(&inode.to_be_bytes());
    material.extend_from_slice(&expected_uid.to_be_bytes());
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

fn connection_inode(
    connection: RetainedTcpConnection,
    expected_uid: u32,
    limits: AttachedProcessWitnessLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<u64, AttachedProcessWitnessError> {
    let path = match connection.server().ip() {
        IpAddr::V4(_) => Path::new("/proc/net/tcp"),
        IpAddr::V6(_) => Path::new("/proc/net/tcp6"),
    };
    let bytes = read_bounded(path, limits.maximum_socket_table_bytes).map_err(|error| {
        map_connection_snapshot_error(
            error,
            AttachedProcessWitnessError::ConnectionSnapshotIncomplete,
        )
    })?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_error| AttachedProcessWitnessError::ConnectionSnapshotIncomplete)?;
    connection_inode_from_text(
        text,
        connection,
        expected_uid,
        limits,
        cancellation,
        started,
    )
}

fn connection_inode_from_text(
    text: &str,
    connection: RetainedTcpConnection,
    expected_uid: u32,
    limits: AttachedProcessWitnessLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<u64, AttachedProcessWitnessError> {
    let expected_local = encoded_local_address(connection.server());
    let expected_remote = encoded_local_address(connection.client());
    let mut matches = Vec::new();
    let mut rows = 0_usize;
    for line in text.lines().skip(1) {
        ensure_active(cancellation, started, limits)?;
        rows = rows
            .checked_add(1)
            .ok_or(AttachedProcessWitnessError::ResourceLimit)?;
        if rows > limits.maximum_socket_table_entries {
            return Err(AttachedProcessWitnessError::ResourceLimit);
        }
        let fields = line.split_ascii_whitespace().collect::<Vec<_>>();
        if fields.len() < 10 {
            return Err(AttachedProcessWitnessError::ConnectionSnapshotIncomplete);
        }
        if fields[1] != expected_local || fields[2] != expected_remote {
            continue;
        }
        let uid = fields[7]
            .parse::<u32>()
            .map_err(|_error| AttachedProcessWitnessError::ConnectionSnapshotIncomplete)?;
        let inode = fields[9]
            .parse::<u64>()
            .map_err(|_error| AttachedProcessWitnessError::ConnectionSnapshotIncomplete)?;
        matches.push((fields[3], uid, inode));
    }
    let (state, uid, inode) = match matches.as_slice() {
        [] => return Err(AttachedProcessWitnessError::ConnectionNotFound),
        [row] => *row,
        _ => return Err(AttachedProcessWitnessError::ConnectionAmbiguous),
    };
    if state != ESTABLISHED_STATE {
        return Err(AttachedProcessWitnessError::ConnectionNotEstablished);
    }
    if uid != expected_uid {
        return Err(AttachedProcessWitnessError::ConnectionProcessMismatch);
    }
    if inode == 0 {
        return Err(AttachedProcessWitnessError::ConnectionSnapshotIncomplete);
    }
    Ok(inode)
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
    use std::{
        net::{Ipv4Addr, Ipv6Addr, SocketAddr},
        time::Instant,
    };

    use rewrite_types::{CancellationToken, Digest};

    use super::{connection_evidence, connection_inode_from_text, require_retained_holder};
    use crate::{AttachedProcessWitnessError, AttachedProcessWitnessLimits, RetainedTcpConnection};

    const UID: u32 = 1000;
    const INODE: u64 = 998_877;

    #[test]
    fn proc_table_requires_exact_reverse_tuple() {
        assert_eq!(parse(&table(&[row("01", UID, INODE, true)])), Ok(INODE));
        assert_eq!(
            parse(&table(&[row("01", UID, INODE, false)])),
            Err(AttachedProcessWitnessError::ConnectionNotFound)
        );
    }

    #[test]
    fn proc_ipv6_table_requires_exact_reverse_tuple() {
        let connection = RetainedTcpConnection::new(
            SocketAddr::from((Ipv6Addr::LOCALHOST, 41_000)),
            SocketAddr::from((Ipv6Addr::LOCALHOST, 11_434)),
        )
        .expect("valid IPv6 connection");
        let exact = format!(
            "0: {} {} 01 00000000:00000000 00:00000000 00000000 {UID} 0 {INODE}",
            "00000000000000000000000001000000:2CAA", "00000000000000000000000001000000:A028"
        );
        assert_eq!(parse_connection(&table(&[exact]), connection), Ok(INODE));
    }

    #[test]
    fn proc_table_rejects_state_uid_multiple_rows_and_zero_inode() {
        assert_eq!(
            parse(&table(&[row("08", UID, INODE, true)])),
            Err(AttachedProcessWitnessError::ConnectionNotEstablished)
        );
        assert_eq!(
            parse(&table(&[row("01", UID + 1, INODE, true)])),
            Err(AttachedProcessWitnessError::ConnectionProcessMismatch)
        );
        let exact = row("01", UID, INODE, true);
        assert_eq!(
            parse(&table(&[exact.clone(), exact])),
            Err(AttachedProcessWitnessError::ConnectionAmbiguous)
        );
        assert_eq!(
            parse(&table(&[row("01", UID, 0, true)])),
            Err(AttachedProcessWitnessError::ConnectionSnapshotIncomplete)
        );
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
    fn inode_change_changes_redacted_evidence() {
        let process = Digest::sha256(b"process evidence");
        let first = connection_evidence(connection(), INODE, UID, 91, &process)
            .expect("first connection evidence");
        let second = connection_evidence(connection(), INODE + 1, UID, 91, &process)
            .expect("second connection evidence");
        assert_ne!(first.evidence_digest(), second.evidence_digest());
        let encoded = serde_json::to_string(&first).expect("serialize evidence");
        assert!(!encoded.contains(&INODE.to_string()));
        assert!(!encoded.contains("41000"));
        assert!(!encoded.contains("11434"));
    }

    fn parse(text: &str) -> Result<u64, AttachedProcessWitnessError> {
        parse_connection(text, connection())
    }

    fn parse_connection(
        text: &str,
        connection: RetainedTcpConnection,
    ) -> Result<u64, AttachedProcessWitnessError> {
        connection_inode_from_text(
            text,
            connection,
            UID,
            AttachedProcessWitnessLimits::default(),
            &CancellationToken::new(),
            Instant::now(),
        )
    }

    fn connection() -> RetainedTcpConnection {
        RetainedTcpConnection::new(
            SocketAddr::from((Ipv4Addr::LOCALHOST, 41_000)),
            SocketAddr::from((Ipv4Addr::LOCALHOST, 11_434)),
        )
        .expect("valid connection")
    }

    fn table(rows: &[String]) -> String {
        format!("header\n{}\n", rows.join("\n"))
    }

    fn row(state: &str, uid: u32, inode: u64, reverse: bool) -> String {
        let (local, remote) = if reverse {
            ("0100007F:2CAA", "0100007F:A028")
        } else {
            ("0100007F:A028", "0100007F:2CAA")
        };
        format!(
            "0: {local} {remote} {state} 00000000:00000000 00:00000000 00000000 {uid} 0 {inode}"
        )
    }
}
