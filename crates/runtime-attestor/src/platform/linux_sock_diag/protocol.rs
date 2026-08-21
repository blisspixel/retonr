use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use crate::{AttachedProcessWitnessLimits, RetainedTcpConnection};

const AF_INET: u8 = 2;
const AF_INET6: u8 = 10;
const IPPROTO_TCP: u8 = 6;
const TCP_ESTABLISHED: u8 = 1;
const TCP_LISTEN: u8 = 10;
const SOCK_DIAG_BY_FAMILY: u16 = 20;
pub(super) const NLMSG_HEADER_BYTES: usize = 16;
const INET_DIAG_REQUEST_BYTES: usize = 56;
const INET_DIAG_MESSAGE_BYTES: usize = 72;
pub(super) const REQUEST_BYTES: usize = NLMSG_HEADER_BYTES + INET_DIAG_REQUEST_BYTES;
const REQUEST_LENGTH: u32 = 72;
const NLM_F_REQUEST: u16 = 0x01;
const NLM_F_MULTI: u16 = 0x02;
const NLM_F_ACK: u16 = 0x04;
const NLM_F_DUMP_INTR: u16 = 0x10;
const NLM_F_DUMP_FILTERED: u16 = 0x20;
const NLM_F_ROOT: u16 = 0x100;
const NLM_F_MATCH: u16 = 0x200;
const NLM_F_DUMP: u16 = NLM_F_ROOT | NLM_F_MATCH;
const NLM_F_CAPPED: u16 = 0x100;
const NLM_F_ACK_TLVS: u16 = 0x200;
const NLMSG_ERROR: u16 = 2;
const NLMSG_DONE: u16 = 3;
const NLMSG_OVERRUN: u16 = 4;
pub(crate) const NO_COOKIE: [u32; 2] = [u32::MAX; 2];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct InetDiagRecord {
    pub(crate) state: u8,
    pub(crate) local: SocketAddr,
    pub(crate) remote: SocketAddr,
    pub(crate) interface: u32,
    pub(crate) cookie: [u32; 2],
    pub(crate) uid: u32,
    pub(crate) inode: u32,
}

impl InetDiagRecord {
    pub(crate) fn usable_cookie(self) -> bool {
        self.cookie != [0, 0] && self.cookie != NO_COOKIE
    }

    pub(crate) const fn is_established(self) -> bool {
        self.state == TCP_ESTABLISHED
    }

    pub(crate) const fn is_listening(self) -> bool {
        self.state == TCP_LISTEN
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RequestKind {
    Point,
    Dump,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DiagError {
    AccessDenied,
    Cancelled,
    DeadlineExceeded,
    Incomplete,
    Interrupted,
    NotFound,
    ResourceLimit,
    Platform,
}

#[derive(Debug)]
pub(super) struct ExchangeBudget {
    bytes: usize,
    records: usize,
}

impl ExchangeBudget {
    pub(super) const fn new() -> Self {
        Self {
            bytes: 0,
            records: 0,
        }
    }

    pub(super) fn admit_datagram(
        &mut self,
        bytes: usize,
        limits: AttachedProcessWitnessLimits,
    ) -> Result<(), DiagError> {
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or(DiagError::ResourceLimit)?;
        if self.bytes > limits.maximum_socket_table_bytes {
            return Err(DiagError::ResourceLimit);
        }
        Ok(())
    }

    fn admit_record(&mut self, limits: AttachedProcessWitnessLimits) -> Result<(), DiagError> {
        self.records = self
            .records
            .checked_add(1)
            .ok_or(DiagError::ResourceLimit)?;
        if self.records > limits.maximum_socket_table_entries {
            return Err(DiagError::ResourceLimit);
        }
        Ok(())
    }
}

pub(super) fn encode_point_request(
    sequence: u32,
    connection: RetainedTcpConnection,
    interface: u32,
    cookie: [u32; 2],
) -> [u8; REQUEST_BYTES] {
    encode_request(
        sequence,
        NLM_F_REQUEST | NLM_F_ACK,
        connection.server().ip(),
        1_u32 << TCP_ESTABLISHED,
        connection.server(),
        connection.client(),
        interface,
        cookie,
    )
}

pub(super) fn encode_listener_request(sequence: u32, endpoint: SocketAddr) -> [u8; REQUEST_BYTES] {
    let unspecified = match endpoint.ip() {
        IpAddr::V4(_) => SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)),
        IpAddr::V6(_) => SocketAddr::from((Ipv6Addr::UNSPECIFIED, 0)),
    };
    encode_request(
        sequence,
        NLM_F_REQUEST | NLM_F_ACK | NLM_F_DUMP,
        endpoint.ip(),
        1_u32 << TCP_LISTEN,
        SocketAddr::new(unspecified.ip(), endpoint.port()),
        unspecified,
        0,
        NO_COOKIE,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "the fixed UAPI request fields are explicit"
)]
fn encode_request(
    sequence: u32,
    flags: u16,
    family: IpAddr,
    states: u32,
    local: SocketAddr,
    remote: SocketAddr,
    interface: u32,
    cookie: [u32; 2],
) -> [u8; REQUEST_BYTES] {
    let mut bytes = [0_u8; REQUEST_BYTES];
    put_u32_ne(&mut bytes, 0, REQUEST_LENGTH);
    put_u16_ne(&mut bytes, 4, SOCK_DIAG_BY_FAMILY);
    put_u16_ne(&mut bytes, 6, flags);
    put_u32_ne(&mut bytes, 8, sequence);
    bytes[16] = family_code(family);
    bytes[17] = IPPROTO_TCP;
    put_u32_ne(&mut bytes, 20, states);
    bytes[24..26].copy_from_slice(&local.port().to_be_bytes());
    bytes[26..28].copy_from_slice(&remote.port().to_be_bytes());
    put_address(&mut bytes[28..44], local.ip());
    put_address(&mut bytes[44..60], remote.ip());
    put_u32_ne(&mut bytes, 60, interface);
    put_u32_ne(&mut bytes, 64, cookie[0]);
    put_u32_ne(&mut bytes, 68, cookie[1]);
    bytes
}

const fn family_code(address: IpAddr) -> u8 {
    match address {
        IpAddr::V4(_) => AF_INET,
        IpAddr::V6(_) => AF_INET6,
    }
}

fn put_address(target: &mut [u8], address: IpAddr) {
    match address {
        IpAddr::V4(address) => target[..4].copy_from_slice(&address.octets()),
        IpAddr::V6(address) => target.copy_from_slice(&address.octets()),
    }
}

pub(super) fn put_u16_ne(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_ne_bytes());
}

pub(super) fn put_u32_ne(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
}

fn read_u16_ne(bytes: &[u8], offset: usize) -> Result<u16, DiagError> {
    let value = bytes.get(offset..offset + 2).ok_or(DiagError::Incomplete)?;
    Ok(u16::from_ne_bytes([value[0], value[1]]))
}

fn read_u32_ne(bytes: &[u8], offset: usize) -> Result<u32, DiagError> {
    let value = bytes.get(offset..offset + 4).ok_or(DiagError::Incomplete)?;
    Ok(u32::from_ne_bytes([value[0], value[1], value[2], value[3]]))
}

fn read_i32_ne(bytes: &[u8], offset: usize) -> Result<i32, DiagError> {
    let value = bytes.get(offset..offset + 4).ok_or(DiagError::Incomplete)?;
    Ok(i32::from_ne_bytes([value[0], value[1], value[2], value[3]]))
}

fn align4(value: usize) -> Result<usize, DiagError> {
    value
        .checked_add(3)
        .map(|value| value & !3)
        .ok_or(DiagError::Incomplete)
}

pub(super) struct ExchangeState<'request> {
    kind: RequestKind,
    sequence: u32,
    port_id: u32,
    request: &'request [u8; REQUEST_BYTES],
    records: Vec<InetDiagRecord>,
    ack: bool,
    done: bool,
}

impl<'request> ExchangeState<'request> {
    pub(super) fn new(
        kind: RequestKind,
        sequence: u32,
        port_id: u32,
        request: &'request [u8; REQUEST_BYTES],
    ) -> Self {
        Self {
            kind,
            sequence,
            port_id,
            request,
            records: Vec::new(),
            ack: false,
            done: false,
        }
    }

    pub(super) fn complete(&self) -> bool {
        match self.kind {
            RequestKind::Point => self.ack && !self.records.is_empty(),
            RequestKind::Dump => self.done,
        }
    }

    pub(super) fn finish(self) -> Result<Vec<InetDiagRecord>, DiagError> {
        if self.complete() {
            Ok(self.records)
        } else {
            Err(DiagError::Incomplete)
        }
    }

    pub(super) fn consume_datagram(
        &mut self,
        datagram: &[u8],
        limits: AttachedProcessWitnessLimits,
        budget: &mut ExchangeBudget,
    ) -> Result<(), DiagError> {
        if datagram.is_empty() || self.complete() {
            return Err(DiagError::Incomplete);
        }
        let mut offset = 0_usize;
        while offset < datagram.len() {
            let remaining = datagram.len() - offset;
            if remaining < NLMSG_HEADER_BYTES {
                return Err(DiagError::Incomplete);
            }
            let length = usize::try_from(read_u32_ne(datagram, offset)?)
                .map_err(|_error| DiagError::Incomplete)?;
            if length < NLMSG_HEADER_BYTES || length > remaining {
                return Err(DiagError::Incomplete);
            }
            let aligned = align4(length)?;
            if aligned > remaining {
                return Err(DiagError::Incomplete);
            }
            let message = &datagram[offset..offset + length];
            self.consume_message(message, limits, budget)?;
            offset = offset.checked_add(aligned).ok_or(DiagError::Incomplete)?;
            if self.complete() && offset != datagram.len() {
                return Err(DiagError::Incomplete);
            }
        }
        Ok(())
    }

    fn consume_message(
        &mut self,
        message: &[u8],
        limits: AttachedProcessWitnessLimits,
        budget: &mut ExchangeBudget,
    ) -> Result<(), DiagError> {
        let message_type = read_u16_ne(message, 4)?;
        let flags = read_u16_ne(message, 6)?;
        if read_u32_ne(message, 8)? != self.sequence || read_u32_ne(message, 12)? != self.port_id {
            return Err(DiagError::Incomplete);
        }
        if flags & NLM_F_ACK_TLVS != 0 {
            return Err(DiagError::Incomplete);
        }
        if flags & NLM_F_DUMP_INTR != 0 {
            return Err(if self.kind == RequestKind::Dump {
                DiagError::Interrupted
            } else {
                DiagError::Incomplete
            });
        }
        match message_type {
            SOCK_DIAG_BY_FAMILY => self.consume_record(message, flags, limits, budget),
            NLMSG_ERROR => self.consume_error(message, flags),
            NLMSG_DONE => self.consume_done(message, flags),
            NLMSG_OVERRUN if self.kind == RequestKind::Dump => Err(DiagError::Interrupted),
            _ => Err(DiagError::Incomplete),
        }
    }

    fn consume_record(
        &mut self,
        message: &[u8],
        flags: u16,
        limits: AttachedProcessWitnessLimits,
        budget: &mut ExchangeBudget,
    ) -> Result<(), DiagError> {
        let allowed = match self.kind {
            RequestKind::Point => 0,
            RequestKind::Dump => NLM_F_MULTI | NLM_F_DUMP_FILTERED,
        };
        if flags & !allowed != 0
            || matches!(self.kind, RequestKind::Point) && flags != 0
            || matches!(self.kind, RequestKind::Dump) && flags & NLM_F_MULTI == 0
        {
            return Err(DiagError::Incomplete);
        }
        let payload = message
            .get(NLMSG_HEADER_BYTES..)
            .ok_or(DiagError::Incomplete)?;
        let record = decode_record(payload)?;
        validate_attributes(
            payload
                .get(INET_DIAG_MESSAGE_BYTES..)
                .ok_or(DiagError::Incomplete)?,
        )?;
        budget.admit_record(limits)?;
        self.records.push(record);
        Ok(())
    }

    fn consume_error(&mut self, message: &[u8], flags: u16) -> Result<(), DiagError> {
        if self.done || self.ack || flags & !NLM_F_CAPPED != 0 {
            return Err(DiagError::Incomplete);
        }
        let payload = message
            .get(NLMSG_HEADER_BYTES..)
            .ok_or(DiagError::Incomplete)?;
        if payload.len() < 20 {
            return Err(DiagError::Incomplete);
        }
        validate_echoed_header(&payload[4..20], self.request)?;
        let error = read_i32_ne(payload, 0)?;
        if error == 0 {
            if self.kind != RequestKind::Point || payload.len() != 20 {
                return Err(DiagError::Incomplete);
            }
            self.ack = true;
            return Ok(());
        }
        if error > 0 {
            return Err(DiagError::Incomplete);
        }
        if flags & NLM_F_CAPPED != 0 {
            if payload.len() != 20 {
                return Err(DiagError::Incomplete);
            }
        } else {
            let echoed = payload.get(4..).ok_or(DiagError::Incomplete)?;
            if echoed != self.request {
                return Err(DiagError::Incomplete);
            }
        }
        let mapped = map_kernel_errno(error.saturating_neg());
        if self.kind == RequestKind::Dump && mapped == DiagError::Incomplete {
            Err(DiagError::Interrupted)
        } else {
            Err(mapped)
        }
    }

    fn consume_done(&mut self, message: &[u8], flags: u16) -> Result<(), DiagError> {
        if self.kind != RequestKind::Dump
            || self.done
            || flags & NLM_F_MULTI == 0
            || flags & !(NLM_F_MULTI | NLM_F_DUMP_FILTERED) != 0
        {
            return Err(DiagError::Incomplete);
        }
        let payload = message
            .get(NLMSG_HEADER_BYTES..)
            .ok_or(DiagError::Incomplete)?;
        if payload.len() != 4 {
            return Err(DiagError::Incomplete);
        }
        let status = read_i32_ne(payload, 0)?;
        if status < 0 {
            return Err(map_kernel_errno(status.saturating_neg()));
        }
        if status != 0 {
            return Err(DiagError::Incomplete);
        }
        self.done = true;
        Ok(())
    }
}

fn validate_echoed_header(echoed: &[u8], request: &[u8; REQUEST_BYTES]) -> Result<(), DiagError> {
    if echoed != &request[..NLMSG_HEADER_BYTES] {
        return Err(DiagError::Incomplete);
    }
    Ok(())
}

fn map_kernel_errno(errno: i32) -> DiagError {
    match rustix::io::Errno::from_raw_os_error(errno) {
        rustix::io::Errno::NOENT | rustix::io::Errno::SRCH => DiagError::NotFound,
        rustix::io::Errno::PERM | rustix::io::Errno::ACCESS => DiagError::AccessDenied,
        rustix::io::Errno::NOBUFS | rustix::io::Errno::INTR => DiagError::Interrupted,
        rustix::io::Errno::NOMEM | rustix::io::Errno::MSGSIZE => DiagError::ResourceLimit,
        _ => DiagError::Platform,
    }
}

fn decode_record(payload: &[u8]) -> Result<InetDiagRecord, DiagError> {
    if payload.len() < INET_DIAG_MESSAGE_BYTES {
        return Err(DiagError::Incomplete);
    }
    let family = payload[0];
    let local_ip = decode_address(family, &payload[8..24])?;
    let remote_ip = decode_address(family, &payload[24..40])?;
    let local_port = u16::from_be_bytes([payload[4], payload[5]]);
    let remote_port = u16::from_be_bytes([payload[6], payload[7]]);
    Ok(InetDiagRecord {
        state: payload[1],
        local: SocketAddr::new(local_ip, local_port),
        remote: SocketAddr::new(remote_ip, remote_port),
        interface: read_u32_ne(payload, 40)?,
        cookie: [read_u32_ne(payload, 44)?, read_u32_ne(payload, 48)?],
        uid: read_u32_ne(payload, 64)?,
        inode: read_u32_ne(payload, 68)?,
    })
}

fn decode_address(family: u8, bytes: &[u8]) -> Result<IpAddr, DiagError> {
    match family {
        AF_INET => {
            if bytes.len() != 16 || bytes[4..].iter().any(|byte| *byte != 0) {
                return Err(DiagError::Incomplete);
            }
            Ok(IpAddr::V4(Ipv4Addr::new(
                bytes[0], bytes[1], bytes[2], bytes[3],
            )))
        }
        AF_INET6 => {
            let octets = <[u8; 16]>::try_from(bytes).map_err(|_error| DiagError::Incomplete)?;
            Ok(IpAddr::V6(Ipv6Addr::from(octets)))
        }
        _ => Err(DiagError::Incomplete),
    }
}

fn validate_attributes(mut attributes: &[u8]) -> Result<(), DiagError> {
    while !attributes.is_empty() {
        if attributes.len() < 4 {
            return Err(DiagError::Incomplete);
        }
        let length = usize::from(read_u16_ne(attributes, 0)?);
        if length < 4 || length > attributes.len() {
            return Err(DiagError::Incomplete);
        }
        let aligned = align4(length)?;
        if aligned > attributes.len() {
            return Err(DiagError::Incomplete);
        }
        attributes = &attributes[aligned..];
    }
    Ok(())
}
