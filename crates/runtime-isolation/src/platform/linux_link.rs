use std::time::Duration;

use rustix::net::{
    AddressFamily, RecvFlags, SendFlags, SocketFlags, SocketType, bind, getsockname,
    netlink::SocketAddrNetlink,
    recvfrom, sendto, socket_with,
    sockopt::{Timeout, set_socket_timeout},
};

use super::linux_helper_setup::HelperFailure;

const NETLINK_TIMEOUT: Duration = Duration::from_secs(1);
const MAXIMUM_DUMP_DATAGRAMS: usize = 8;
const MAXIMUM_DATAGRAM_BYTES: usize = 4 * 1024;
const RTM_NEWLINK: u16 = 16;
const RTM_GETLINK: u16 = 18;
const NLMSG_ERROR: u16 = 2;
const NLMSG_DONE: u16 = 3;
const NLM_F_REQUEST: u16 = 1;
const NLM_F_MULTI: u16 = 2;
const NLM_F_ACK: u16 = 4;
const NLM_F_DUMP_INTR: u16 = 16;
const NLM_F_DUMP: u16 = 0x300;
const IFF_UP: u32 = 1;
const IFF_LOOPBACK: u32 = 8;
const IFLA_IFNAME: u16 = 3;
const ARPHRD_LOOPBACK: u16 = 772;

pub(super) fn enable_and_validate_loopback() -> Result<u32, HelperFailure> {
    let socket = socket_with(
        AddressFamily::NETLINK,
        SocketType::RAW,
        SocketFlags::CLOEXEC,
        None,
    )
    .map_err(|_| HelperFailure::LoopbackSetup)?;
    bind(&socket, &SocketAddrNetlink::new(0, 0)).map_err(|_| HelperFailure::LoopbackSetup)?;
    let local_port = SocketAddrNetlink::try_from(
        getsockname(&socket).map_err(|_| HelperFailure::LoopbackSetup)?,
    )
    .map_err(|_| HelperFailure::LoopbackSetup)?
    .pid();
    if local_port == 0 {
        return Err(HelperFailure::LoopbackSetup);
    }
    set_socket_timeout(&socket, Timeout::Recv, Some(NETLINK_TIMEOUT))
        .map_err(|_| HelperFailure::LoopbackSetup)?;
    let index = dump_only_loopback(&socket, 1, local_port, false)?;
    set_link_up(&socket, index, 2, local_port)?;
    if dump_only_loopback(&socket, 3, local_port, true)? != index {
        return Err(HelperFailure::LoopbackSetup);
    }
    Ok(index)
}

fn dump_only_loopback(
    socket: &impl std::os::fd::AsFd,
    sequence: u32,
    local_port: u32,
    require_up: bool,
) -> Result<u32, HelperFailure> {
    send_exact(socket, &get_link_request(sequence))?;
    let mut loopback_index = None;
    for _ in 0..MAXIMUM_DUMP_DATAGRAMS {
        let mut response = [0_u8; MAXIMUM_DATAGRAM_BYTES];
        let (received, reported, sender) = recvfrom(socket, &mut response, RecvFlags::empty())
            .map_err(|_| HelperFailure::LoopbackSetup)?;
        let sender = sender
            .and_then(|address| SocketAddrNetlink::try_from(address).ok())
            .ok_or(HelperFailure::LoopbackSetup)?;
        if received == 0 || received != reported || sender.pid() != 0 || sender.groups() != 0 {
            return Err(HelperFailure::LoopbackSetup);
        }
        if parse_dump_datagram(
            &response[..received],
            sequence,
            local_port,
            require_up,
            &mut loopback_index,
        )? {
            return loopback_index.ok_or(HelperFailure::LoopbackSetup);
        }
    }
    Err(HelperFailure::LoopbackSetup)
}

fn parse_dump_datagram(
    response: &[u8],
    sequence: u32,
    local_port: u32,
    require_up: bool,
    loopback_index: &mut Option<u32>,
) -> Result<bool, HelperFailure> {
    let mut offset = 0;
    while offset < response.len() {
        let length = usize::try_from(read_u32(response, offset)?)
            .map_err(|_| HelperFailure::LoopbackSetup)?;
        if length < 16 || length > response.len() - offset {
            return Err(HelperFailure::LoopbackSetup);
        }
        let aligned = align(length)?;
        if aligned > response.len() - offset {
            return Err(HelperFailure::LoopbackSetup);
        }
        let message = &response[offset..offset + length];
        let message_type = read_u16(message, 4)?;
        let flags = read_u16(message, 6)?;
        if read_u32(message, 8)? != sequence
            || read_u32(message, 12)? != local_port
            || flags & NLM_F_MULTI == 0
            || flags & NLM_F_DUMP_INTR != 0
        {
            return Err(HelperFailure::LoopbackSetup);
        }
        match message_type {
            RTM_NEWLINK => {
                if loopback_index.is_some() {
                    return Err(HelperFailure::LoopbackSetup);
                }
                *loopback_index = Some(parse_loopback_link(message, require_up)?);
            }
            NLMSG_DONE => {
                if length != 20 || read_i32(message, 16)? != 0 || offset + aligned != response.len()
                {
                    return Err(HelperFailure::LoopbackSetup);
                }
                return Ok(true);
            }
            _ => return Err(HelperFailure::LoopbackSetup),
        }
        offset += aligned;
    }
    Ok(false)
}

fn parse_loopback_link(message: &[u8], require_up: bool) -> Result<u32, HelperFailure> {
    if message.len() < 32 || message[16] != 0 || read_u16(message, 18)? != ARPHRD_LOOPBACK {
        return Err(HelperFailure::LoopbackSetup);
    }
    let index = read_i32(message, 20)?;
    let flags = read_u32(message, 24)?;
    if index <= 0
        || flags & IFF_LOOPBACK == 0
        || (require_up && flags & IFF_UP == 0)
        || interface_name(message)? != b"lo"
    {
        return Err(HelperFailure::LoopbackSetup);
    }
    u32::try_from(index).map_err(|_| HelperFailure::LoopbackSetup)
}

fn interface_name(message: &[u8]) -> Result<&[u8], HelperFailure> {
    let mut offset = 32;
    let mut name = None;
    while offset < message.len() {
        let length = usize::from(read_u16(message, offset)?);
        if length < 4 || length > message.len() - offset {
            return Err(HelperFailure::LoopbackSetup);
        }
        let aligned = align(length)?;
        if aligned > message.len() - offset {
            return Err(HelperFailure::LoopbackSetup);
        }
        if read_u16(message, offset + 2)? & 0x3fff == IFLA_IFNAME {
            if name.is_some() {
                return Err(HelperFailure::LoopbackSetup);
            }
            let value = &message[offset + 4..offset + length];
            let terminated = value
                .strip_suffix(&[0])
                .ok_or(HelperFailure::LoopbackSetup)?;
            if terminated.is_empty() || terminated.contains(&0) {
                return Err(HelperFailure::LoopbackSetup);
            }
            name = Some(terminated);
        }
        offset += aligned;
    }
    name.ok_or(HelperFailure::LoopbackSetup)
}

fn set_link_up(
    socket: &impl std::os::fd::AsFd,
    index: u32,
    sequence: u32,
    local_port: u32,
) -> Result<(), HelperFailure> {
    let request = link_up_request(index, sequence)?;
    send_exact(socket, &request)?;
    let mut response = [0_u8; 256];
    let (received, reported, sender) = recvfrom(socket, &mut response, RecvFlags::empty())
        .map_err(|_| HelperFailure::LoopbackSetup)?;
    let sender = sender
        .and_then(|address| SocketAddrNetlink::try_from(address).ok())
        .ok_or(HelperFailure::LoopbackSetup)?;
    if received != reported || sender.pid() != 0 || sender.groups() != 0 {
        return Err(HelperFailure::LoopbackSetup);
    }
    validate_netlink_ack(&response[..received], sequence, local_port)
}

fn send_exact(socket: &impl std::os::fd::AsFd, request: &[u8]) -> Result<(), HelperFailure> {
    let sent = sendto(
        socket,
        request,
        SendFlags::empty(),
        &SocketAddrNetlink::new(0, 0),
    )
    .map_err(|_| HelperFailure::LoopbackSetup)?;
    if sent == request.len() {
        Ok(())
    } else {
        Err(HelperFailure::LoopbackSetup)
    }
}

fn get_link_request(sequence: u32) -> [u8; 32] {
    link_request(RTM_GETLINK, NLM_F_REQUEST | NLM_F_DUMP, sequence, 0, 0, 0)
}

fn link_up_request(index: u32, sequence: u32) -> Result<[u8; 32], HelperFailure> {
    let index = i32::try_from(index).map_err(|_| HelperFailure::LoopbackSetup)?;
    Ok(link_request(
        RTM_NEWLINK,
        NLM_F_REQUEST | NLM_F_ACK,
        sequence,
        index,
        IFF_UP,
        IFF_UP,
    ))
}

fn link_request(
    message_type: u16,
    flags: u16,
    sequence: u32,
    index: i32,
    link_flags: u32,
    change: u32,
) -> [u8; 32] {
    let mut message = [0_u8; 32];
    message[..4].copy_from_slice(&32_u32.to_ne_bytes());
    message[4..6].copy_from_slice(&message_type.to_ne_bytes());
    message[6..8].copy_from_slice(&flags.to_ne_bytes());
    message[8..12].copy_from_slice(&sequence.to_ne_bytes());
    message[20..24].copy_from_slice(&index.to_ne_bytes());
    message[24..28].copy_from_slice(&link_flags.to_ne_bytes());
    message[28..32].copy_from_slice(&change.to_ne_bytes());
    message
}

fn validate_netlink_ack(
    response: &[u8],
    sequence: u32,
    local_port: u32,
) -> Result<(), HelperFailure> {
    if response.len() < 20
        || usize::try_from(read_u32(response, 0)?).ok() != Some(response.len())
        || read_u16(response, 4)? != NLMSG_ERROR
        || read_u32(response, 8)? != sequence
        || read_u32(response, 12)? != local_port
        || read_i32(response, 16)? != 0
    {
        return Err(HelperFailure::LoopbackSetup);
    }
    Ok(())
}

fn align(length: usize) -> Result<usize, HelperFailure> {
    length
        .checked_add(3)
        .map(|value| value & !3)
        .ok_or(HelperFailure::LoopbackSetup)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, HelperFailure> {
    bytes
        .get(offset..offset + 2)
        .and_then(|value| value.try_into().ok())
        .map(u16::from_ne_bytes)
        .ok_or(HelperFailure::LoopbackSetup)
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, HelperFailure> {
    bytes
        .get(offset..offset + 4)
        .and_then(|value| value.try_into().ok())
        .map(u32::from_ne_bytes)
        .ok_or(HelperFailure::LoopbackSetup)
}

fn read_i32(bytes: &[u8], offset: usize) -> Result<i32, HelperFailure> {
    bytes
        .get(offset..offset + 4)
        .and_then(|value| value.try_into().ok())
        .map(i32::from_ne_bytes)
        .ok_or(HelperFailure::LoopbackSetup)
}

#[cfg(test)]
mod tests {
    use super::{HelperFailure, get_link_request, link_up_request, validate_netlink_ack};

    #[test]
    fn link_requests_and_ack_are_strictly_framed() {
        assert_eq!(get_link_request(1).len(), 32);
        let request = link_up_request(1, 7).expect("link request");
        assert_eq!(request.len(), 32);
        let mut ack = Vec::new();
        ack.extend_from_slice(&20_u32.to_ne_bytes());
        ack.extend_from_slice(&2_u16.to_ne_bytes());
        ack.extend_from_slice(&0_u16.to_ne_bytes());
        ack.extend_from_slice(&7_u32.to_ne_bytes());
        ack.extend_from_slice(&1_u32.to_ne_bytes());
        ack.extend_from_slice(&0_i32.to_ne_bytes());
        assert!(validate_netlink_ack(&ack, 7, 1).is_ok());
        ack[8] = 8;
        assert_eq!(
            validate_netlink_ack(&ack, 7, 1),
            Err(HelperFailure::LoopbackSetup)
        );
    }
}
