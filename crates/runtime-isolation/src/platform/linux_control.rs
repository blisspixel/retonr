use std::{
    io::{IoSlice, IoSliceMut},
    mem::MaybeUninit,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
    os::fd::{BorrowedFd, OwnedFd},
    thread,
    time::{Duration, Instant},
};

use crate::IsolationError;
use rewrite_types::CancellationToken;
use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    net::{
        AddressFamily, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags,
        SendAncillaryBuffer, SendAncillaryMessage, SendFlags, SocketFlags, SocketType, recvmsg,
        sendmsg, socketpair,
    },
};

const MAGIC: &[u8; 8] = b"RTNRISO1";
const HEADER_BYTES: usize = 16;
const MAX_PAYLOAD_BYTES: usize = 64 * 1024 + 16;
const MAX_FRAME_BYTES: usize = HEADER_BYTES + MAX_PAYLOAD_BYTES;
const POLL_INTERVAL: Duration = Duration::from_millis(5);
const MAX_RECEIVED_DESCRIPTORS: usize = 2;
const ENDPOINT_BYTES: usize = 19;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(super) enum MessageKind {
    LaunchDescriptor = 1,
    Armed = 2,
    Go = 3,
    TargetStarted = 4,
    Connect = 5,
    Connected = 6,
    Capture = 7,
    Captured = 8,
    Error = 9,
}

impl MessageKind {
    fn parse(value: u8) -> Result<Self, ControlError> {
        match value {
            1 => Ok(Self::LaunchDescriptor),
            2 => Ok(Self::Armed),
            3 => Ok(Self::Go),
            4 => Ok(Self::TargetStarted),
            5 => Ok(Self::Connect),
            6 => Ok(Self::Connected),
            7 => Ok(Self::Capture),
            8 => Ok(Self::Captured),
            9 => Ok(Self::Error),
            _ => Err(ControlError::Invalid),
        }
    }
}

#[derive(Debug)]
pub(super) struct ControlMessage {
    pub(super) kind: MessageKind,
    pub(super) payload: Vec<u8>,
    pub(super) descriptors: Vec<OwnedFd>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ControlError {
    Cancelled,
    Deadline,
    Closed,
    Invalid,
    Native,
}

pub(super) const fn map_error(error: ControlError) -> IsolationError {
    match error {
        ControlError::Cancelled => IsolationError::Cancelled,
        ControlError::Deadline => IsolationError::StartupTimeout,
        ControlError::Closed | ControlError::Invalid | ControlError::Native => {
            IsolationError::HelperProtocol
        }
    }
}

pub(super) fn pair() -> Result<(OwnedFd, OwnedFd), ControlError> {
    socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
        None,
    )
    .map_err(|_error| ControlError::Native)
}

pub(super) fn send(
    socket: BorrowedFd<'_>,
    kind: MessageKind,
    payload: &[u8],
    descriptors: &[BorrowedFd<'_>],
    deadline: Instant,
    cancellation: Option<&CancellationToken>,
) -> Result<(), ControlError> {
    if payload.len() > MAX_PAYLOAD_BYTES || descriptors.len() > MAX_RECEIVED_DESCRIPTORS {
        return Err(ControlError::Invalid);
    }
    let payload_len = u32::try_from(payload.len()).map_err(|_error| ControlError::Invalid)?;
    let descriptor_count =
        u8::try_from(descriptors.len()).map_err(|_error| ControlError::Invalid)?;
    let mut header = [0_u8; HEADER_BYTES];
    header[..MAGIC.len()].copy_from_slice(MAGIC);
    header[8] = 1;
    header[9] = kind as u8;
    header[10] = descriptor_count;
    header[11] = 0;
    header[12..].copy_from_slice(&payload_len.to_be_bytes());
    let slices = [IoSlice::new(&header), IoSlice::new(payload)];
    loop {
        ensure_active(deadline, cancellation)?;
        let mut ancillary_space =
            [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(MAX_RECEIVED_DESCRIPTORS))];
        let mut ancillary = SendAncillaryBuffer::new(&mut ancillary_space);
        if !descriptors.is_empty() && !ancillary.push(SendAncillaryMessage::ScmRights(descriptors))
        {
            return Err(ControlError::Invalid);
        }
        match sendmsg(
            socket,
            &slices,
            &mut ancillary,
            SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
        ) {
            Ok(sent) if sent == HEADER_BYTES + payload.len() => return Ok(()),
            Ok(_) => return Err(ControlError::Invalid),
            Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
                wait(socket, PollFlags::OUT, deadline, cancellation)?;
            }
            Err(_error) => return Err(ControlError::Native),
        }
    }
}

pub(super) fn receive(
    socket: BorrowedFd<'_>,
    deadline: Instant,
    cancellation: Option<&CancellationToken>,
) -> Result<ControlMessage, ControlError> {
    loop {
        ensure_active(deadline, cancellation)?;
        let mut frame = vec![0_u8; MAX_FRAME_BYTES];
        let mut ancillary_space =
            [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(MAX_RECEIVED_DESCRIPTORS))];
        let mut ancillary = RecvAncillaryBuffer::new(&mut ancillary_space);
        let received = {
            let mut slices = [IoSliceMut::new(&mut frame)];
            recvmsg(
                socket,
                &mut slices,
                &mut ancillary,
                RecvFlags::DONTWAIT | RecvFlags::TRUNC | RecvFlags::CMSG_CLOEXEC,
            )
        };
        match received {
            Ok(message) => {
                let bytes = message.bytes;
                let flags = message.flags;
                return parse_received(&frame, bytes, flags, &mut ancillary);
            }
            Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
                wait(socket, PollFlags::IN, deadline, cancellation)?;
            }
            Err(_error) => return Err(ControlError::Native),
        }
    }
}

fn parse_received(
    frame: &[u8],
    bytes: usize,
    flags: ReturnFlags,
    ancillary: &mut RecvAncillaryBuffer<'_>,
) -> Result<ControlMessage, ControlError> {
    if bytes == 0 {
        return Err(ControlError::Closed);
    }
    if bytes > frame.len()
        || flags.intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
        || bytes < HEADER_BYTES
        || &frame[..MAGIC.len()] != MAGIC
        || frame[8] != 1
        || frame[11] != 0
    {
        return Err(ControlError::Invalid);
    }
    let kind = MessageKind::parse(frame[9])?;
    let descriptor_count = usize::from(frame[10]);
    let payload_len = usize::try_from(u32::from_be_bytes(
        frame[12..HEADER_BYTES]
            .try_into()
            .map_err(|_error| ControlError::Invalid)?,
    ))
    .map_err(|_error| ControlError::Invalid)?;
    if payload_len > MAX_PAYLOAD_BYTES || bytes != HEADER_BYTES + payload_len {
        return Err(ControlError::Invalid);
    }
    let mut descriptors = Vec::new();
    for message in ancillary.drain() {
        match message {
            RecvAncillaryMessage::ScmRights(rights) => descriptors.extend(rights),
            _ => return Err(ControlError::Invalid),
        }
    }
    if descriptor_count != descriptors.len() || descriptor_count > MAX_RECEIVED_DESCRIPTORS {
        return Err(ControlError::Invalid);
    }
    Ok(ControlMessage {
        kind,
        payload: frame[HEADER_BYTES..bytes].to_vec(),
        descriptors,
    })
}

pub(super) fn encode_endpoint(endpoint: SocketAddr) -> Result<[u8; ENDPOINT_BYTES], ControlError> {
    if endpoint.port() == 0 || !endpoint.ip().is_loopback() {
        return Err(ControlError::Invalid);
    }
    let mut bytes = [0_u8; ENDPOINT_BYTES];
    match endpoint {
        SocketAddr::V4(address) => {
            bytes[0] = 4;
            bytes[1..5].copy_from_slice(&address.ip().octets());
        }
        SocketAddr::V6(address) if address.flowinfo() == 0 && address.scope_id() == 0 => {
            bytes[0] = 6;
            bytes[1..17].copy_from_slice(&address.ip().octets());
        }
        SocketAddr::V6(_) => return Err(ControlError::Invalid),
    }
    bytes[17..].copy_from_slice(&endpoint.port().to_be_bytes());
    Ok(bytes)
}

pub(super) fn decode_endpoint(bytes: &[u8]) -> Result<SocketAddr, ControlError> {
    if bytes.len() != ENDPOINT_BYTES {
        return Err(ControlError::Invalid);
    }
    let port = u16::from_be_bytes(
        bytes[17..]
            .try_into()
            .map_err(|_error| ControlError::Invalid)?,
    );
    let endpoint = match bytes[0] {
        4 if bytes[5..17].iter().all(|byte| *byte == 0) => {
            let address = Ipv4Addr::from(
                <[u8; 4]>::try_from(&bytes[1..5]).map_err(|_error| ControlError::Invalid)?,
            );
            SocketAddr::V4(SocketAddrV4::new(address, port))
        }
        6 => {
            let address = Ipv6Addr::from(
                <[u8; 16]>::try_from(&bytes[1..17]).map_err(|_error| ControlError::Invalid)?,
            );
            SocketAddr::V6(SocketAddrV6::new(address, port, 0, 0))
        }
        _ => return Err(ControlError::Invalid),
    };
    if port == 0 || !endpoint.ip().is_loopback() {
        return Err(ControlError::Invalid);
    }
    Ok(endpoint)
}

fn wait(
    socket: BorrowedFd<'_>,
    events: PollFlags,
    deadline: Instant,
    cancellation: Option<&CancellationToken>,
) -> Result<(), ControlError> {
    ensure_active(deadline, cancellation)?;
    let mut descriptors = [PollFd::new(&socket, events)];
    let remaining = deadline.saturating_duration_since(Instant::now());
    let wait = remaining.min(POLL_INTERVAL);
    let timeout = Timespec {
        tv_sec: i64::try_from(wait.as_secs()).unwrap_or(i64::MAX),
        tv_nsec: i64::from(wait.subsec_nanos()),
    };
    match poll(&mut descriptors, Some(&timeout)) {
        Ok(0) => {
            thread::yield_now();
            Ok(())
        }
        Ok(_) => {
            let returned = descriptors[0].revents();
            if returned.intersects(PollFlags::ERR | PollFlags::HUP | PollFlags::NVAL) {
                Err(ControlError::Closed)
            } else if returned.contains(events) {
                Ok(())
            } else {
                Err(ControlError::Invalid)
            }
        }
        Err(rustix::io::Errno::INTR) => Ok(()),
        Err(_error) => Err(ControlError::Native),
    }
}

fn ensure_active(
    deadline: Instant,
    cancellation: Option<&CancellationToken>,
) -> Result<(), ControlError> {
    if cancellation.is_some_and(CancellationToken::is_cancelled) {
        return Err(ControlError::Cancelled);
    }
    if Instant::now() >= deadline {
        return Err(ControlError::Deadline);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        net::{Ipv6Addr, SocketAddr, SocketAddrV6},
        os::fd::AsFd as _,
        time::Duration,
    };

    use super::{ControlError, MessageKind, decode_endpoint, encode_endpoint, pair, receive, send};

    fn deadline() -> std::time::Instant {
        std::time::Instant::now() + Duration::from_secs(1)
    }

    fn exact_header(kind: u8, descriptors: u8, payload: u32) -> [u8; 16] {
        let mut frame = [0_u8; 16];
        frame[..8].copy_from_slice(b"RTNRISO1");
        frame[8] = 1;
        frame[9] = kind;
        frame[10] = descriptors;
        frame[12..].copy_from_slice(&payload.to_be_bytes());
        frame
    }

    #[test]
    fn exact_frame_round_trip_preserves_kind_payload_and_descriptors() {
        let (left, right) = pair().expect("control pair");
        let (sent, _peer) = pair().expect("descriptor pair");
        send(
            left.as_fd(),
            MessageKind::Go,
            &41_u32.to_be_bytes(),
            &[sent.as_fd()],
            deadline(),
            None,
        )
        .expect("send frame");
        let message = receive(right.as_fd(), deadline(), None).expect("receive frame");
        assert_eq!(message.kind, MessageKind::Go);
        assert_eq!(message.payload, 41_u32.to_be_bytes());
        assert_eq!(message.descriptors.len(), 1);
    }

    #[test]
    fn unknown_kind_and_descriptor_count_mismatch_fail_closed() {
        let (left, right) = pair().expect("control pair");
        let frame = exact_header(255, 0, 0);
        rustix::net::send(left.as_fd(), &frame, rustix::net::SendFlags::empty())
            .expect("send malformed frame");
        assert!(matches!(
            receive(right.as_fd(), deadline(), None),
            Err(ControlError::Invalid)
        ));

        send(left.as_fd(), MessageKind::Armed, &[], &[], deadline(), None)
            .expect("send valid frame");
        let message = receive(right.as_fd(), deadline(), None).expect("receive valid frame");
        assert!(message.descriptors.is_empty());
    }

    #[test]
    fn malformed_lengths_reserved_fields_and_missing_rights_fail_closed() {
        for frame in [
            {
                let mut frame = exact_header(MessageKind::Armed as u8, 0, 0);
                frame[11] = 1;
                frame
            },
            exact_header(MessageKind::Armed as u8, 0, 1),
            exact_header(MessageKind::Armed as u8, 1, 0),
        ] {
            let (left, right) = pair().expect("control pair");
            rustix::net::send(left.as_fd(), &frame, rustix::net::SendFlags::empty())
                .expect("send malformed frame");
            assert!(matches!(
                receive(right.as_fd(), deadline(), None),
                Err(ControlError::Invalid)
            ));
        }
    }

    #[test]
    fn cancellation_precedes_socket_io() {
        let (left, _right) = pair().expect("control pair");
        let cancellation = rewrite_types::CancellationToken::new();
        cancellation.cancel();
        assert!(matches!(
            receive(left.as_fd(), deadline(), Some(&cancellation)),
            Err(ControlError::Cancelled)
        ));
    }

    #[test]
    fn oversized_and_expired_operations_are_rejected_before_io() {
        let (left, _right) = pair().expect("control pair");
        assert_eq!(
            send(
                left.as_fd(),
                MessageKind::Captured,
                &vec![0_u8; super::MAX_PAYLOAD_BYTES + 1],
                &[],
                deadline(),
                None,
            ),
            Err(ControlError::Invalid)
        );
        assert!(matches!(
            receive(left.as_fd(), std::time::Instant::now(), None),
            Err(ControlError::Deadline)
        ));
    }

    #[test]
    fn endpoint_codec_accepts_only_exact_unscoped_loopback_literals() {
        for endpoint in ["127.0.0.1:11434", "[::1]:11434"] {
            let endpoint = endpoint.parse::<SocketAddr>().expect("literal endpoint");
            assert_eq!(
                decode_endpoint(&encode_endpoint(endpoint).expect("encode endpoint")),
                Ok(endpoint)
            );
        }
        assert_eq!(
            encode_endpoint("192.0.2.1:80".parse().expect("literal endpoint")),
            Err(ControlError::Invalid)
        );
        assert_eq!(
            encode_endpoint(SocketAddr::V6(SocketAddrV6::new(
                Ipv6Addr::LOCALHOST,
                80,
                0,
                1,
            ))),
            Err(ControlError::Invalid)
        );
        let mut noncanonical = encode_endpoint("127.0.0.1:80".parse().expect("literal endpoint"))
            .expect("encode endpoint");
        noncanonical[16] = 1;
        assert_eq!(decode_endpoint(&noncanonical), Err(ControlError::Invalid));
        assert_eq!(decode_endpoint(&[0; 18]), Err(ControlError::Invalid));
    }
}
