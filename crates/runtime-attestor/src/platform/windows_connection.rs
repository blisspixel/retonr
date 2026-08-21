use std::{
    mem::size_of,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr},
    os::windows::io::OwnedHandle,
    ptr,
    time::Instant,
};

use rewrite_types::{CancellationToken, Digest};
use windows_sys::Win32::NetworkManagement::IpHelper::{
    MIB_TCP_STATE_ESTAB, MIB_TCP6ROW_OWNER_PID, MIB_TCPROW_OWNER_PID,
    TCP_TABLE_OWNER_PID_CONNECTIONS,
};

use super::windows::{
    ensure_process_alive, process_creation_time, tcp_table, validate_table_extent,
};
use crate::{
    AttachedProcessWitnessError, AttachedProcessWitnessLimits, RetainedTcpConnection,
    RetainedTcpConnectionEvidence, RetainedTcpConnectionEvidenceInput,
    TcpConnectionAttributionKind, TcpConnectionSharingLimitation,
    connection::connection_digest_material, ensure_active,
};

#[derive(Clone, Copy)]
pub(super) struct RetainedProcess<'a> {
    pub(super) pid: u32,
    pub(super) handle: &'a OwnedHandle,
    pub(super) creation_time: u64,
    pub(super) evidence_digest: &'a Digest,
}

pub(super) fn observe_connection(
    connection: RetainedTcpConnection,
    limits: AttachedProcessWitnessLimits,
    cancellation: &CancellationToken,
    started: Instant,
    retained: RetainedProcess<'_>,
) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError> {
    ensure_active(cancellation, started, limits)?;
    ensure_process_alive(retained.handle)?;
    if process_creation_time(retained.handle)? != retained.creation_time {
        return Err(AttachedProcessWitnessError::ProcessInstanceChanged);
    }
    let table = tcp_table(
        connection.server().ip(),
        TCP_TABLE_OWNER_PID_CONNECTIONS,
        limits,
    )?;
    let row = matching_connection(&table, connection, limits)?;
    validate_connection_row(row, retained.pid)?;
    ensure_process_alive(retained.handle)?;
    if process_creation_time(retained.handle)? != retained.creation_time {
        return Err(AttachedProcessWitnessError::ProcessInstanceChanged);
    }
    let mut material = connection_digest_material(
        b"retonr:windows-context-binding-connection:v1\0",
        connection,
    );
    material.extend_from_slice(&retained.pid.to_be_bytes());
    material.extend_from_slice(&retained.creation_time.to_be_bytes());
    RetainedTcpConnectionEvidence::new(&RetainedTcpConnectionEvidenceInput {
        attribution_kind: TcpConnectionAttributionKind::WindowsContextBindingPid,
        sharing_limitation: TcpConnectionSharingLimitation::WindowsDuplicatedHandlesNotObservable,
        process_evidence_digest: retained.evidence_digest.clone(),
        platform_connection_digest: Digest::sha256(&material),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ConnectionRow {
    state: u32,
    pid: u32,
}

fn validate_connection_row(
    row: ConnectionRow,
    retained_pid: u32,
) -> Result<(), AttachedProcessWitnessError> {
    if row.state != MIB_TCP_STATE_ESTAB.cast_unsigned() {
        return Err(AttachedProcessWitnessError::ConnectionNotEstablished);
    }
    if row.pid == 0 || row.pid != retained_pid {
        return Err(AttachedProcessWitnessError::ConnectionProcessMismatch);
    }
    Ok(())
}

fn matching_connection(
    table: &[u32],
    connection: RetainedTcpConnection,
    limits: AttachedProcessWitnessLimits,
) -> Result<ConnectionRow, AttachedProcessWitnessError> {
    let bytes = table.len().saturating_mul(size_of::<u32>());
    if bytes < size_of::<u32>() {
        return Err(AttachedProcessWitnessError::ConnectionSnapshotIncomplete);
    }
    let rows =
        usize::try_from(table[0]).map_err(|_error| AttachedProcessWitnessError::ResourceLimit)?;
    if rows > limits.maximum_socket_table_entries {
        return Err(AttachedProcessWitnessError::ResourceLimit);
    }
    let matches = match (connection.server(), connection.client()) {
        (SocketAddr::V4(server), SocketAddr::V4(client)) => matching_ipv4_connections(
            table,
            rows,
            bytes,
            server.ip(),
            server.port(),
            client.ip(),
            client.port(),
        )?,
        (SocketAddr::V6(server), SocketAddr::V6(client)) => {
            matching_ipv6_connections(table, rows, bytes, &server, &client)?
        }
        _ => return Err(AttachedProcessWitnessError::InvalidConnectionEndpoints),
    };
    match matches.as_slice() {
        [] => Err(AttachedProcessWitnessError::ConnectionNotFound),
        [row] => Ok(*row),
        _ => Err(AttachedProcessWitnessError::ConnectionAmbiguous),
    }
}

fn matching_ipv4_connections(
    table: &[u32],
    rows: usize,
    bytes: usize,
    server_address: &Ipv4Addr,
    server_port: u16,
    client_address: &Ipv4Addr,
    client_port: u16,
) -> Result<Vec<ConnectionRow>, AttachedProcessWitnessError> {
    let row_size = size_of::<MIB_TCPROW_OWNER_PID>();
    validate_connection_table_extent(rows, row_size, bytes)?;
    let base = table.as_ptr().cast::<u8>();
    let mut matches = Vec::new();
    for index in 0..rows {
        let offset = size_of::<u32>() + index * row_size;
        // SAFETY: The table extent check proves the complete row is present,
        // and unaligned reads do not impose an additional alignment contract.
        let row = unsafe { ptr::read_unaligned(base.add(offset).cast::<MIB_TCPROW_OWNER_PID>()) };
        let local_address = Ipv4Addr::from(u32::from_be(row.dwLocalAddr));
        let remote_address = Ipv4Addr::from(u32::from_be(row.dwRemoteAddr));
        let local_port = windows_port(row.dwLocalPort)?;
        let remote_port = windows_port(row.dwRemotePort)?;
        if local_address == *server_address
            && local_port == server_port
            && remote_address == *client_address
            && remote_port == client_port
        {
            matches.push(ConnectionRow {
                state: row.dwState,
                pid: row.dwOwningPid,
            });
        }
    }
    Ok(matches)
}

fn matching_ipv6_connections(
    table: &[u32],
    rows: usize,
    bytes: usize,
    server: &std::net::SocketAddrV6,
    client: &std::net::SocketAddrV6,
) -> Result<Vec<ConnectionRow>, AttachedProcessWitnessError> {
    let row_size = size_of::<MIB_TCP6ROW_OWNER_PID>();
    validate_connection_table_extent(rows, row_size, bytes)?;
    let base = table.as_ptr().cast::<u8>();
    let mut matches = Vec::new();
    for index in 0..rows {
        let offset = size_of::<u32>() + index * row_size;
        // SAFETY: The table extent check proves the complete row is present,
        // and unaligned reads do not impose an additional alignment contract.
        let row = unsafe { ptr::read_unaligned(base.add(offset).cast::<MIB_TCP6ROW_OWNER_PID>()) };
        let local_port = windows_port(row.dwLocalPort)?;
        let remote_port = windows_port(row.dwRemotePort)?;
        if Ipv6Addr::from(row.ucLocalAddr) == *server.ip()
            && u32::from_be(row.dwLocalScopeId) == server.scope_id()
            && local_port == server.port()
            && Ipv6Addr::from(row.ucRemoteAddr) == *client.ip()
            && u32::from_be(row.dwRemoteScopeId) == client.scope_id()
            && remote_port == client.port()
        {
            matches.push(ConnectionRow {
                state: row.dwState,
                pid: row.dwOwningPid,
            });
        }
    }
    Ok(matches)
}

fn windows_port(value: u32) -> Result<u16, AttachedProcessWitnessError> {
    u16::try_from(value)
        .map(u16::from_be)
        .map_err(|_error| AttachedProcessWitnessError::ConnectionSnapshotIncomplete)
}

fn validate_connection_table_extent(
    rows: usize,
    row_size: usize,
    bytes: usize,
) -> Result<(), AttachedProcessWitnessError> {
    validate_table_extent(rows, row_size, bytes).map_err(|error| match error {
        AttachedProcessWitnessError::ListenerSnapshotIncomplete => {
            AttachedProcessWitnessError::ConnectionSnapshotIncomplete
        }
        other => other,
    })
}

#[cfg(test)]
mod tests {
    use std::{
        mem::size_of,
        net::{Ipv4Addr, Ipv6Addr, SocketAddr},
        ptr,
    };

    use windows_sys::Win32::NetworkManagement::IpHelper::{
        MIB_TCP_STATE_CLOSE_WAIT, MIB_TCP_STATE_ESTAB, MIB_TCP6ROW_OWNER_PID, MIB_TCPROW_OWNER_PID,
    };

    use super::{ConnectionRow, matching_connection, validate_connection_row};
    use crate::{AttachedProcessWitnessError, AttachedProcessWitnessLimits, RetainedTcpConnection};

    #[test]
    fn connection_table_requires_exact_reverse_tuple() {
        let connection = connection();
        let exact = ipv4_row(
            connection.server(),
            connection.client(),
            MIB_TCP_STATE_ESTAB,
            91,
        );
        assert_eq!(
            matching_connection(
                &table(&[exact]),
                connection,
                AttachedProcessWitnessLimits::default(),
            ),
            Ok(ConnectionRow {
                state: MIB_TCP_STATE_ESTAB.cast_unsigned(),
                pid: 91,
            })
        );
        let forward = ipv4_row(
            connection.client(),
            connection.server(),
            MIB_TCP_STATE_ESTAB,
            91,
        );
        assert_eq!(
            matching_connection(
                &table(&[forward]),
                connection,
                AttachedProcessWitnessLimits::default(),
            ),
            Err(AttachedProcessWitnessError::ConnectionNotFound)
        );
    }

    #[test]
    fn ipv6_connection_table_requires_exact_reverse_tuple() {
        let connection = RetainedTcpConnection::new(
            SocketAddr::from((Ipv6Addr::LOCALHOST, 41_000)),
            SocketAddr::from((Ipv6Addr::LOCALHOST, 11_434)),
        )
        .expect("valid IPv6 connection");
        let exact = ipv6_row(
            connection.server(),
            connection.client(),
            MIB_TCP_STATE_ESTAB,
            91,
        );
        assert_eq!(
            matching_connection(
                &table(&[exact]),
                connection,
                AttachedProcessWitnessLimits::default(),
            ),
            Ok(ConnectionRow {
                state: MIB_TCP_STATE_ESTAB.cast_unsigned(),
                pid: 91,
            })
        );
        let forward = ipv6_row(
            connection.client(),
            connection.server(),
            MIB_TCP_STATE_ESTAB,
            91,
        );
        assert_eq!(
            matching_connection(
                &table(&[forward]),
                connection,
                AttachedProcessWitnessLimits::default(),
            ),
            Err(AttachedProcessWitnessError::ConnectionNotFound)
        );
    }

    #[test]
    fn connection_table_rejects_state_pid_ambiguity_and_truncation() {
        assert_eq!(
            validate_connection_row(
                ConnectionRow {
                    state: MIB_TCP_STATE_CLOSE_WAIT.cast_unsigned(),
                    pid: 91,
                },
                91,
            ),
            Err(AttachedProcessWitnessError::ConnectionNotEstablished)
        );
        assert_eq!(
            validate_connection_row(
                ConnectionRow {
                    state: MIB_TCP_STATE_ESTAB.cast_unsigned(),
                    pid: 92,
                },
                91,
            ),
            Err(AttachedProcessWitnessError::ConnectionProcessMismatch)
        );
        let connection = connection();
        let row = ipv4_row(
            connection.server(),
            connection.client(),
            MIB_TCP_STATE_ESTAB,
            91,
        );
        assert_eq!(
            matching_connection(
                &table(&[row, row]),
                connection,
                AttachedProcessWitnessLimits::default(),
            ),
            Err(AttachedProcessWitnessError::ConnectionAmbiguous)
        );
        assert_eq!(
            matching_connection(&[1], connection, AttachedProcessWitnessLimits::default(),),
            Err(AttachedProcessWitnessError::ConnectionSnapshotIncomplete)
        );
    }

    fn connection() -> RetainedTcpConnection {
        RetainedTcpConnection::new(
            SocketAddr::from((Ipv4Addr::LOCALHOST, 41_000)),
            SocketAddr::from((Ipv4Addr::LOCALHOST, 11_434)),
        )
        .expect("valid connection")
    }

    fn ipv4_row(
        local: SocketAddr,
        remote: SocketAddr,
        state: i32,
        pid: u32,
    ) -> MIB_TCPROW_OWNER_PID {
        let (SocketAddr::V4(local), SocketAddr::V4(remote)) = (local, remote) else {
            panic!("test endpoints are IPv4")
        };
        MIB_TCPROW_OWNER_PID {
            dwState: state.cast_unsigned(),
            dwLocalAddr: u32::from(*local.ip()).to_be(),
            dwLocalPort: u32::from(local.port().to_be()),
            dwRemoteAddr: u32::from(*remote.ip()).to_be(),
            dwRemotePort: u32::from(remote.port().to_be()),
            dwOwningPid: pid,
        }
    }

    fn ipv6_row(
        local: SocketAddr,
        remote: SocketAddr,
        state: i32,
        pid: u32,
    ) -> MIB_TCP6ROW_OWNER_PID {
        let (SocketAddr::V6(local), SocketAddr::V6(remote)) = (local, remote) else {
            panic!("test endpoints are IPv6")
        };
        MIB_TCP6ROW_OWNER_PID {
            ucLocalAddr: local.ip().octets(),
            dwLocalScopeId: local.scope_id().to_be(),
            dwLocalPort: u32::from(local.port().to_be()),
            ucRemoteAddr: remote.ip().octets(),
            dwRemoteScopeId: remote.scope_id().to_be(),
            dwRemotePort: u32::from(remote.port().to_be()),
            dwState: state.cast_unsigned(),
            dwOwningPid: pid,
        }
    }

    fn table<T>(rows: &[T]) -> Vec<u32> {
        let bytes = size_of::<u32>() + size_of_val(rows);
        let words = bytes.div_ceil(size_of::<u32>());
        let mut table = vec![0_u32; words];
        table[0] = u32::try_from(rows.len()).expect("test row count");
        // SAFETY: The destination starts after the count word and has exactly
        // enough allocated bytes for every source row. The regions do not overlap.
        unsafe {
            ptr::copy_nonoverlapping(
                rows.as_ptr().cast::<u8>(),
                table.as_mut_ptr().cast::<u8>().add(size_of::<u32>()),
                size_of_val(rows),
            );
        }
        table
    }
}
