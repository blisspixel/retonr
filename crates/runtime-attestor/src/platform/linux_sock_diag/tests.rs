use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use super::protocol::{
    DiagError, ExchangeBudget, ExchangeState, InetDiagRecord, NLMSG_HEADER_BYTES, NO_COOKIE,
    REQUEST_BYTES, RequestKind, encode_listener_request, encode_point_request, put_u16_ne,
    put_u32_ne,
};
use crate::{AttachedProcessWitnessLimits, RetainedTcpConnection};

const SEQUENCE: u32 = 71;
const PORT_ID: u32 = 812;
const SOCK_DIAG_BY_FAMILY: u16 = 20;
const NLMSG_ERROR: u16 = 2;
const NLMSG_DONE: u16 = 3;
const NLMSG_OVERRUN: u16 = 4;
const NLM_F_MULTI: u16 = 0x02;
const NLM_F_DUMP_INTR: u16 = 0x10;
const NLM_F_CAPPED: u16 = 0x100;
const NLM_F_ACK_TLVS: u16 = 0x200;

#[test]
fn requests_encode_exact_ipv4_and_ipv6_filters() {
    let ipv4 = connection(
        SocketAddr::from((Ipv4Addr::LOCALHOST, 41_000)),
        SocketAddr::from((Ipv4Addr::LOCALHOST, 11_434)),
    );
    let point = encode_point_request(SEQUENCE, ipv4, 9, [17, 23]);
    assert_eq!(
        read_u32(&point, 0),
        u32::try_from(REQUEST_BYTES).expect("request length fits u32")
    );
    assert_eq!(read_u16(&point, 4), SOCK_DIAG_BY_FAMILY);
    assert_eq!(read_u16(&point, 6), 0x05);
    assert_eq!(read_u32(&point, 8), SEQUENCE);
    assert_eq!(&point[12..16], &[0; 4]);
    assert_eq!(&point[16..20], &[2, 6, 0, 0]);
    assert_eq!(read_u32(&point, 20), 1 << 1);
    assert_eq!(&point[24..28], &[0x2c, 0xaa, 0xa0, 0x28]);
    assert_eq!(&point[28..32], &[127, 0, 0, 1]);
    assert_eq!(&point[44..48], &[127, 0, 0, 1]);
    assert_eq!(read_u32(&point, 60), 9);
    assert_eq!([read_u32(&point, 64), read_u32(&point, 68)], [17, 23]);

    let listener = encode_listener_request(SEQUENCE, ipv4.server());
    assert_eq!(read_u16(&listener, 6), 0x305);
    assert_eq!(read_u32(&listener, 20), 1 << 10);
    assert_eq!(&listener[24..28], &[0x2c, 0xaa, 0, 0]);
    assert!(listener[28..60].iter().all(|byte| *byte == 0));
    assert_eq!(
        [read_u32(&listener, 64), read_u32(&listener, 68)],
        NO_COOKIE
    );

    let ipv6 = connection(
        SocketAddr::from((Ipv6Addr::LOCALHOST, 41_000)),
        SocketAddr::from((Ipv6Addr::LOCALHOST, 11_434)),
    );
    let point = encode_point_request(SEQUENCE, ipv6, 0, NO_COOKIE);
    assert_eq!(point[16], 10);
    assert_eq!(&point[28..44], &Ipv6Addr::LOCALHOST.octets());
    assert_eq!(&point[44..60], &Ipv6Addr::LOCALHOST.octets());
}

#[test]
fn point_requires_one_record_and_matching_success_ack() {
    let request = request(RequestKind::Point);
    let mut exchange = state(RequestKind::Point, &request);
    let mut budget = ExchangeBudget::new();
    exchange
        .consume_datagram(
            &record_message(record(), 0, SEQUENCE, PORT_ID, &[]),
            limits(),
            &mut budget,
        )
        .expect("point record");
    assert!(!exchange.complete());
    exchange
        .consume_datagram(&ack_message(&request, NLM_F_CAPPED), limits(), &mut budget)
        .expect("point acknowledgement");
    assert_eq!(exchange.finish(), Ok(vec![record()]));

    let mut ack_first = state(RequestKind::Point, &request);
    let mut budget = ExchangeBudget::new();
    ack_first
        .consume_datagram(&ack_message(&request, 0), limits(), &mut budget)
        .expect("ack before data");
    ack_first
        .consume_datagram(
            &record_message(record(), 0, SEQUENCE, PORT_ID, &[]),
            limits(),
            &mut budget,
        )
        .expect("record after ack");
    assert!(ack_first.complete());
}

#[test]
fn dump_requires_multipart_rows_and_terminal_done() {
    let request = request(RequestKind::Dump);
    let mut exchange = state(RequestKind::Dump, &request);
    let mut budget = ExchangeBudget::new();
    exchange
        .consume_datagram(
            &record_message(record(), NLM_F_MULTI, SEQUENCE, PORT_ID, &[]),
            limits(),
            &mut budget,
        )
        .expect("multipart record");
    assert_eq!(
        exchange.finish(),
        Err(DiagError::Incomplete),
        "a row cannot terminate a dump"
    );

    let mut empty = state(RequestKind::Dump, &request);
    let mut budget = ExchangeBudget::new();
    empty
        .consume_datagram(&done_message(0), limits(), &mut budget)
        .expect("empty dump completion");
    assert_eq!(empty.finish(), Ok(Vec::new()));
}

#[test]
fn parser_rejects_wrong_envelopes_and_truncation() {
    let request = request(RequestKind::Point);
    let valid = record_message(record(), 0, SEQUENCE, PORT_ID, &[]);
    for malformed in [
        Vec::new(),
        valid[..15].to_vec(),
        with_u32(valid.clone(), 0, 15),
        with_u32(
            valid.clone(),
            0,
            u32::try_from(valid.len() + 4).expect("fixture length fits u32"),
        ),
        with_u32(valid.clone(), 8, SEQUENCE + 1),
        with_u32(valid.clone(), 12, PORT_ID + 1),
        with_u16(valid.clone(), 6, NLM_F_ACK_TLVS),
    ] {
        assert_eq!(
            consume_point(&request, &malformed),
            Err(DiagError::Incomplete)
        );
    }
    let mut trailing = valid;
    trailing.push(0);
    assert_eq!(
        consume_point(&request, &trailing),
        Err(DiagError::Incomplete)
    );
}

#[test]
fn parser_rejects_malformed_records_and_attributes() {
    let request = request(RequestKind::Point);
    let mut wrong_family = record_message(record(), 0, SEQUENCE, PORT_ID, &[]);
    wrong_family[NLMSG_HEADER_BYTES] = 99;
    assert_eq!(
        consume_point(&request, &wrong_family),
        Err(DiagError::Incomplete)
    );

    let mut tainted_ipv4 = record_message(record(), 0, SEQUENCE, PORT_ID, &[]);
    tainted_ipv4[NLMSG_HEADER_BYTES + 8 + 4] = 1;
    assert_eq!(
        consume_point(&request, &tainted_ipv4),
        Err(DiagError::Incomplete)
    );

    for attributes in [&[3, 0, 1, 0][..], &[8, 0, 1, 0][..], &[4, 0, 77][..]] {
        let message = record_message(record(), 0, SEQUENCE, PORT_ID, attributes);
        assert_eq!(
            consume_point(&request, &message),
            Err(DiagError::Incomplete)
        );
    }
    let unknown = record_message(record(), 0, SEQUENCE, PORT_ID, &[4, 0, 77, 0]);
    assert!(consume_point(&request, &unknown).is_ok());
}

#[test]
fn errors_validate_echoes_and_map_fail_closed() {
    let request = request(RequestKind::Point);
    assert_eq!(
        consume_point(&request, &error_message(&request, -2, NLM_F_CAPPED)),
        Err(DiagError::NotFound)
    );
    assert_eq!(
        consume_point(&request, &error_message(&request, -13, NLM_F_CAPPED)),
        Err(DiagError::AccessDenied)
    );
    assert_eq!(
        consume_point(&request, &error_message(&request, -12, NLM_F_CAPPED)),
        Err(DiagError::ResourceLimit)
    );
    assert_eq!(
        consume_point(&request, &error_message(&request, 1, NLM_F_CAPPED)),
        Err(DiagError::Incomplete)
    );

    let mut wrong_echo = error_message(&request, -2, NLM_F_CAPPED);
    wrong_echo[NLMSG_HEADER_BYTES + 4 + 8] ^= 1;
    assert_eq!(
        consume_point(&request, &wrong_echo),
        Err(DiagError::Incomplete)
    );
    let mut capped_trailing = error_message(&request, -2, NLM_F_CAPPED);
    capped_trailing.extend_from_slice(&[0; 4]);
    let capped_length = u32::try_from(capped_trailing.len()).expect("fixture length fits u32");
    put_u32_ne(&mut capped_trailing, 0, capped_length);
    assert_eq!(
        consume_point(&request, &capped_trailing),
        Err(DiagError::Incomplete)
    );

    let uncapped = error_message(&request, -2, 0);
    assert_eq!(consume_point(&request, &uncapped), Err(DiagError::NotFound));
    let mut wrong_body = uncapped;
    *wrong_body.last_mut().expect("error body") ^= 1;
    assert_eq!(
        consume_point(&request, &wrong_body),
        Err(DiagError::Incomplete)
    );
}

#[test]
fn dump_interrupt_and_overrun_are_retryable_only_for_dumps() {
    let request = request(RequestKind::Dump);
    let interrupted = record_message(
        record(),
        NLM_F_MULTI | NLM_F_DUMP_INTR,
        SEQUENCE,
        PORT_ID,
        &[],
    );
    assert_eq!(
        consume(RequestKind::Dump, &request, &interrupted),
        Err(DiagError::Interrupted)
    );
    assert_eq!(
        consume(
            RequestKind::Dump,
            &request,
            &message(NLMSG_OVERRUN, 0, SEQUENCE, PORT_ID, &[])
        ),
        Err(DiagError::Interrupted)
    );
    assert_eq!(
        consume(RequestKind::Point, &request, &interrupted),
        Err(DiagError::Incomplete)
    );
    assert_eq!(
        consume(RequestKind::Dump, &request, &done_message(-4)),
        Err(DiagError::Interrupted)
    );
}

#[test]
fn record_and_byte_budgets_are_cumulative() {
    let mut constrained = limits();
    constrained.maximum_socket_table_entries = 1;
    let request = request(RequestKind::Dump);
    let first = record_message(record(), NLM_F_MULTI, SEQUENCE, PORT_ID, &[]);
    let mut datagram = first.clone();
    datagram.extend_from_slice(&first);
    let mut state = state(RequestKind::Dump, &request);
    assert_eq!(
        state.consume_datagram(&datagram, constrained, &mut ExchangeBudget::new()),
        Err(DiagError::ResourceLimit)
    );

    let mut budget = ExchangeBudget::new();
    let mut constrained = limits();
    constrained.maximum_socket_table_bytes = first.len();
    assert_eq!(budget.admit_datagram(first.len(), constrained), Ok(()));
    assert_eq!(
        budget.admit_datagram(1, constrained),
        Err(DiagError::ResourceLimit)
    );
}

fn consume_point(request: &[u8; REQUEST_BYTES], datagram: &[u8]) -> Result<(), DiagError> {
    consume(RequestKind::Point, request, datagram)
}

fn consume(
    kind: RequestKind,
    request: &[u8; REQUEST_BYTES],
    datagram: &[u8],
) -> Result<(), DiagError> {
    state(kind, request).consume_datagram(datagram, limits(), &mut ExchangeBudget::new())
}

fn state(kind: RequestKind, request: &[u8; REQUEST_BYTES]) -> ExchangeState<'_> {
    ExchangeState::new(kind, SEQUENCE, PORT_ID, request)
}

fn request(kind: RequestKind) -> [u8; REQUEST_BYTES] {
    let connection = connection(
        SocketAddr::from((Ipv4Addr::LOCALHOST, 41_000)),
        SocketAddr::from((Ipv4Addr::LOCALHOST, 11_434)),
    );
    match kind {
        RequestKind::Point => encode_point_request(SEQUENCE, connection, 0, NO_COOKIE),
        RequestKind::Dump => encode_listener_request(SEQUENCE, connection.server()),
    }
}

fn connection(client: SocketAddr, server: SocketAddr) -> RetainedTcpConnection {
    RetainedTcpConnection::new(client, server).expect("valid retained connection")
}

fn record() -> InetDiagRecord {
    InetDiagRecord {
        state: 1,
        local: SocketAddr::from((Ipv4Addr::LOCALHOST, 11_434)),
        remote: SocketAddr::from((Ipv4Addr::LOCALHOST, 41_000)),
        interface: 3,
        cookie: [17, 23],
        uid: 1000,
        inode: 91_001,
    }
}

fn record_message(
    record: InetDiagRecord,
    flags: u16,
    sequence: u32,
    port_id: u32,
    attributes: &[u8],
) -> Vec<u8> {
    let mut payload = vec![0_u8; 72];
    payload[0] = match record.local.ip() {
        IpAddr::V4(_) => 2,
        IpAddr::V6(_) => 10,
    };
    payload[1] = record.state;
    payload[4..6].copy_from_slice(&record.local.port().to_be_bytes());
    payload[6..8].copy_from_slice(&record.remote.port().to_be_bytes());
    put_address(&mut payload[8..24], record.local.ip());
    put_address(&mut payload[24..40], record.remote.ip());
    put_u32_ne(&mut payload, 40, record.interface);
    put_u32_ne(&mut payload, 44, record.cookie[0]);
    put_u32_ne(&mut payload, 48, record.cookie[1]);
    put_u32_ne(&mut payload, 64, record.uid);
    put_u32_ne(&mut payload, 68, record.inode);
    payload.extend_from_slice(attributes);
    message(SOCK_DIAG_BY_FAMILY, flags, sequence, port_id, &payload)
}

fn ack_message(request: &[u8; REQUEST_BYTES], flags: u16) -> Vec<u8> {
    let mut payload = 0_i32.to_ne_bytes().to_vec();
    payload.extend_from_slice(&request[..NLMSG_HEADER_BYTES]);
    message(NLMSG_ERROR, flags, SEQUENCE, PORT_ID, &payload)
}

fn error_message(request: &[u8; REQUEST_BYTES], error: i32, flags: u16) -> Vec<u8> {
    let mut payload = error.to_ne_bytes().to_vec();
    if flags & NLM_F_CAPPED == 0 {
        payload.extend_from_slice(request);
    } else {
        payload.extend_from_slice(&request[..NLMSG_HEADER_BYTES]);
    }
    message(NLMSG_ERROR, flags, SEQUENCE, PORT_ID, &payload)
}

fn done_message(status: i32) -> Vec<u8> {
    message(
        NLMSG_DONE,
        NLM_F_MULTI,
        SEQUENCE,
        PORT_ID,
        &status.to_ne_bytes(),
    )
}

fn message(message_type: u16, flags: u16, sequence: u32, port_id: u32, payload: &[u8]) -> Vec<u8> {
    let length = NLMSG_HEADER_BYTES + payload.len();
    let aligned = (length + 3) & !3;
    let mut bytes = vec![0_u8; aligned];
    put_u32_ne(
        &mut bytes,
        0,
        u32::try_from(length).expect("fixture length fits u32"),
    );
    put_u16_ne(&mut bytes, 4, message_type);
    put_u16_ne(&mut bytes, 6, flags);
    put_u32_ne(&mut bytes, 8, sequence);
    put_u32_ne(&mut bytes, 12, port_id);
    bytes[NLMSG_HEADER_BYTES..length].copy_from_slice(payload);
    bytes
}

fn put_address(target: &mut [u8], address: IpAddr) {
    match address {
        IpAddr::V4(address) => target[..4].copy_from_slice(&address.octets()),
        IpAddr::V6(address) => target.copy_from_slice(&address.octets()),
    }
}

fn with_u16(mut bytes: Vec<u8>, offset: usize, value: u16) -> Vec<u8> {
    put_u16_ne(&mut bytes, offset, value);
    bytes
}

fn with_u32(mut bytes: Vec<u8>, offset: usize, value: u32) -> Vec<u8> {
    put_u32_ne(&mut bytes, offset, value);
    bytes
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_ne_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_ne_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn limits() -> AttachedProcessWitnessLimits {
    AttachedProcessWitnessLimits::default()
}
