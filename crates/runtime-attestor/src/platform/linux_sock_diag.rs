mod protocol;
#[cfg(test)]
mod tests;

use std::{fs::File, net::SocketAddr, num::NonZeroU32, time::Instant};

use rewrite_types::CancellationToken;
use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    fd::OwnedFd,
    fs::{OFlags, fcntl_getfl},
    io::{FdFlags, fcntl_getfd},
    net::{
        AddressFamily, Protocol, RecvFlags, SendFlags, SocketFlags, SocketType, bind, getpeername,
        getsockname,
        netlink::{SOCK_DIAG, SocketAddrNetlink},
        recvfrom, sendto, socket_with,
        sockopt::{socket_domain, socket_protocol, socket_type},
    },
};

use protocol::{
    DiagError, ExchangeBudget, ExchangeState, REQUEST_BYTES, RequestKind, encode_listener_request,
    encode_point_request,
};
pub(super) use protocol::{InetDiagRecord, NO_COOKIE};

use crate::{
    AttachedProcessWitnessError, AttachedProcessWitnessLimits, RetainedTcpConnection, ensure_active,
};

const RECEIVE_BUFFER_BYTES: usize = 32 * 1024;
const POINT_RECEIVE_ATTEMPTS: usize = 200;
const DUMP_RECEIVE_ATTEMPTS: usize = 256;
const DUMP_ATTEMPTS: usize = 2;
const POLL_SLICE_NANOSECONDS: i64 = 5_000_000;

impl DiagError {
    fn connection(self) -> AttachedProcessWitnessError {
        match self {
            Self::AccessDenied => AttachedProcessWitnessError::ProcessAccessDenied,
            Self::Cancelled => AttachedProcessWitnessError::Cancelled,
            Self::DeadlineExceeded => AttachedProcessWitnessError::DeadlineExceeded,
            Self::Incomplete | Self::Interrupted => {
                AttachedProcessWitnessError::ConnectionSnapshotIncomplete
            }
            Self::NotFound => AttachedProcessWitnessError::ConnectionNotFound,
            Self::ResourceLimit => AttachedProcessWitnessError::ResourceLimit,
            Self::Platform => AttachedProcessWitnessError::PlatformObservationFailed,
        }
    }

    fn listener(self) -> AttachedProcessWitnessError {
        match self {
            Self::AccessDenied => AttachedProcessWitnessError::ProcessAccessDenied,
            Self::Cancelled => AttachedProcessWitnessError::Cancelled,
            Self::DeadlineExceeded => AttachedProcessWitnessError::DeadlineExceeded,
            Self::Incomplete | Self::Interrupted | Self::NotFound => {
                AttachedProcessWitnessError::ListenerSnapshotIncomplete
            }
            Self::ResourceLimit => AttachedProcessWitnessError::ResourceLimit,
            Self::Platform => AttachedProcessWitnessError::PlatformObservationFailed,
        }
    }
}

pub(super) struct SockDiagSession {
    socket: OwnedFd,
    port_id: u32,
    next_sequence: u32,
}

impl SockDiagSession {
    pub(super) fn new() -> Result<Self, AttachedProcessWitnessError> {
        let socket = socket_with(
            AddressFamily::NETLINK,
            SocketType::RAW,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            Some(SOCK_DIAG),
        )
        .map_err(map_open_error)?;
        bind(&socket, &SocketAddrNetlink::new(0, 0)).map_err(map_open_error)?;
        let address = getsockname(&socket).map_err(map_open_error)?;
        let address = SocketAddrNetlink::try_from(address)
            .map_err(|_error| AttachedProcessWitnessError::PlatformObservationFailed)?;
        if address.pid() == 0 || address.groups() != 0 {
            return Err(AttachedProcessWitnessError::PlatformObservationFailed);
        }
        Ok(Self {
            socket,
            port_id: address.pid(),
            next_sequence: 1,
        })
    }

    pub(super) fn from_file(file: File) -> Result<Self, AttachedProcessWitnessError> {
        validate_supplied_socket(&file)?;
        let peer = getpeername(&file)
            .map_err(map_open_error)?
            .ok_or(AttachedProcessWitnessError::PlatformObservationFailed)?;
        let peer = SocketAddrNetlink::try_from(peer)
            .map_err(|_error| AttachedProcessWitnessError::PlatformObservationFailed)?;
        if peer.pid() != 0
            || peer.groups() != 0
            || poll_readable_file(&file).map_err(DiagError::listener)?
        {
            return Err(AttachedProcessWitnessError::PlatformObservationFailed);
        }
        let address = bound_address(&file)?;
        Ok(Self {
            socket: file.into(),
            port_id: address.pid(),
            next_sequence: 1,
        })
    }

    pub(super) const fn port_id(&self) -> u32 {
        self.port_id
    }

    pub(super) fn exact_connection(
        &mut self,
        connection: RetainedTcpConnection,
        interface: u32,
        cookie: [u32; 2],
        limits: AttachedProcessWitnessLimits,
        cancellation: &CancellationToken,
        started: Instant,
    ) -> Result<InetDiagRecord, AttachedProcessWitnessError> {
        let sequence = self.sequence().map_err(DiagError::connection)?;
        let request = encode_point_request(sequence, connection, interface, cookie);
        let mut budget = ExchangeBudget::new();
        let records = self
            .exchange(
                &request,
                sequence,
                RequestKind::Point,
                POINT_RECEIVE_ATTEMPTS,
                limits,
                cancellation,
                started,
                &mut budget,
            )
            .map_err(DiagError::connection)?;
        match records.as_slice() {
            [record] => Ok(*record),
            [] => Err(AttachedProcessWitnessError::ConnectionNotFound),
            _ => Err(AttachedProcessWitnessError::ConnectionAmbiguous),
        }
    }

    pub(super) fn listener_dump(
        &mut self,
        endpoint: SocketAddr,
        limits: AttachedProcessWitnessLimits,
        cancellation: &CancellationToken,
        started: Instant,
    ) -> Result<Vec<InetDiagRecord>, AttachedProcessWitnessError> {
        let mut budget = ExchangeBudget::new();
        for attempt in 0..DUMP_ATTEMPTS {
            let sequence = self.sequence().map_err(DiagError::listener)?;
            let request = encode_listener_request(sequence, endpoint);
            match self.exchange(
                &request,
                sequence,
                RequestKind::Dump,
                DUMP_RECEIVE_ATTEMPTS,
                limits,
                cancellation,
                started,
                &mut budget,
            ) {
                Ok(records) => return Ok(records),
                Err(DiagError::Interrupted) if attempt + 1 < DUMP_ATTEMPTS => {}
                Err(error) => return Err(error.listener()),
            }
        }
        Err(AttachedProcessWitnessError::ListenerSnapshotIncomplete)
    }

    fn sequence(&mut self) -> Result<u32, DiagError> {
        let sequence = self.next_sequence;
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .filter(|next| *next != 0)
            .ok_or(DiagError::ResourceLimit)?;
        Ok(sequence)
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "exchange policy is explicit at the syscall boundary"
    )]
    fn exchange(
        &self,
        request: &[u8; REQUEST_BYTES],
        sequence: u32,
        kind: RequestKind,
        maximum_attempts: usize,
        limits: AttachedProcessWitnessLimits,
        cancellation: &CancellationToken,
        started: Instant,
        budget: &mut ExchangeBudget,
    ) -> Result<Vec<InetDiagRecord>, DiagError> {
        ensure_active(cancellation, started, limits).map_err(map_active_error)?;
        let sent = sendto(
            &self.socket,
            request,
            SendFlags::DONTWAIT,
            &SocketAddrNetlink::new(0, 0),
        )
        .map_err(map_io_error)?;
        if sent != request.len() {
            return Err(DiagError::Incomplete);
        }
        let mut state = ExchangeState::new(kind, sequence, self.port_id, request);
        let buffer_bytes = RECEIVE_BUFFER_BYTES.min(limits.maximum_socket_table_bytes);
        let mut buffer = vec![0_u8; buffer_bytes];
        for _attempt in 0..maximum_attempts {
            ensure_active(cancellation, started, limits).map_err(map_active_error)?;
            if !poll_readable(&self.socket)? {
                continue;
            }
            let received = recvfrom(
                &self.socket,
                buffer.as_mut_slice(),
                RecvFlags::TRUNC | RecvFlags::DONTWAIT,
            );
            let (initialized, reported, sender) = match received {
                Ok(received) => received,
                Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
                    continue;
                }
                Err(error) => return Err(map_io_error(error)),
            };
            if reported > buffer.len() {
                return Err(DiagError::ResourceLimit);
            }
            budget.admit_datagram(reported, limits)?;
            let sender = sender.ok_or(DiagError::Incomplete)?;
            let sender =
                SocketAddrNetlink::try_from(sender).map_err(|_error| DiagError::Incomplete)?;
            if sender.pid() != 0 || sender.groups() != 0 {
                return Err(DiagError::Incomplete);
            }
            state.consume_datagram(&buffer[..initialized], limits, budget)?;
            if state.complete() {
                return state.finish();
            }
        }
        Err(DiagError::Incomplete)
    }
}

fn validate_supplied_socket(file: &File) -> Result<(), AttachedProcessWitnessError> {
    let address = bound_address(file)?;
    let status = fcntl_getfl(file).map_err(map_open_error)?;
    if socket_domain(file).map_err(map_open_error)? != AddressFamily::NETLINK
        || socket_type(file).map_err(map_open_error)? != SocketType::RAW
        || socket_protocol(file).map_err(map_open_error)? != Some(sock_diag_protocol())
        || !status.contains(OFlags::NONBLOCK)
        || status & OFlags::ACCMODE != OFlags::RDWR
        || !fcntl_getfd(file)
            .map_err(map_open_error)?
            .contains(FdFlags::CLOEXEC)
        || address.pid() == 0
        || address.groups() != 0
    {
        return Err(AttachedProcessWitnessError::PlatformObservationFailed);
    }
    Ok(())
}

fn bound_address(file: &File) -> Result<SocketAddrNetlink, AttachedProcessWitnessError> {
    let address = getsockname(file).map_err(map_open_error)?;
    SocketAddrNetlink::try_from(address)
        .map_err(|_error| AttachedProcessWitnessError::PlatformObservationFailed)
}

fn sock_diag_protocol() -> Protocol {
    Protocol::from_raw(NonZeroU32::new(4).expect("SOCK_DIAG protocol is nonzero"))
}

fn poll_readable(socket: &OwnedFd) -> Result<bool, DiagError> {
    poll_readable_fd(socket)
}

fn poll_readable_file(socket: &File) -> Result<bool, DiagError> {
    poll_readable_fd(socket)
}

fn poll_readable_fd<F: std::os::fd::AsFd>(socket: &F) -> Result<bool, DiagError> {
    let mut descriptors = [PollFd::new(socket, PollFlags::IN)];
    let timeout = Timespec {
        tv_sec: 0,
        tv_nsec: POLL_SLICE_NANOSECONDS,
    };
    let ready = poll(&mut descriptors, Some(&timeout)).map_err(map_io_error)?;
    if ready == 0 {
        return Ok(false);
    }
    let events = descriptors[0].revents();
    if events.intersects(PollFlags::ERR | PollFlags::HUP | PollFlags::NVAL) {
        return Err(DiagError::Incomplete);
    }
    Ok(events.contains(PollFlags::IN))
}

fn map_open_error(error: rustix::io::Errno) -> AttachedProcessWitnessError {
    if matches!(error, rustix::io::Errno::PERM | rustix::io::Errno::ACCESS) {
        AttachedProcessWitnessError::ProcessAccessDenied
    } else {
        AttachedProcessWitnessError::PlatformObservationFailed
    }
}

fn map_io_error(error: rustix::io::Errno) -> DiagError {
    match error {
        rustix::io::Errno::PERM | rustix::io::Errno::ACCESS => DiagError::AccessDenied,
        rustix::io::Errno::NOBUFS => DiagError::Interrupted,
        rustix::io::Errno::AGAIN | rustix::io::Errno::INTR => DiagError::Incomplete,
        rustix::io::Errno::NOMEM | rustix::io::Errno::MSGSIZE => DiagError::ResourceLimit,
        _ => DiagError::Platform,
    }
}

fn map_active_error(error: AttachedProcessWitnessError) -> DiagError {
    match error {
        AttachedProcessWitnessError::Cancelled => DiagError::Cancelled,
        AttachedProcessWitnessError::DeadlineExceeded => DiagError::DeadlineExceeded,
        AttachedProcessWitnessError::ResourceLimit => DiagError::ResourceLimit,
        _ => DiagError::Platform,
    }
}
