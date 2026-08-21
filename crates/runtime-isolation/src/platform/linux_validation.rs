use std::{
    fs::{self, File},
    net::{SocketAddr, TcpStream},
    num::NonZeroU32,
    os::unix::fs::MetadataExt as _,
};

use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    fd::OwnedFd,
    fs::{OFlags, fcntl_getfl},
    io::{FdFlags, fcntl_getfd},
    net::{
        AddressFamily, Protocol, SocketType, getsockname,
        netlink::SocketAddrNetlink,
        sockopt::{socket_domain, socket_protocol, socket_type},
    },
};

use crate::{IsolationError, IsolationResult, NamespaceIdentity, error::native};

pub(super) fn open_namespace(pid: u32, name: &'static str) -> IsolationResult<File> {
    File::open(format!("/proc/{pid}/ns/{name}"))
        .map_err(|error| native("open-retained-namespace", &error))
}

pub(super) fn namespace_identity(file: &File) -> IsolationResult<NamespaceIdentity> {
    let metadata = file
        .metadata()
        .map_err(|error| native("read-namespace-identity", &error))?;
    Ok(NamespaceIdentity::new(metadata.dev(), metadata.ino()))
}

pub(super) fn privileges_are_reduced(pid: u32) -> IsolationResult<bool> {
    let status = fs::read_to_string(format!("/proc/{pid}/status"))
        .map_err(|error| native("read-process-privileges", &error))?;
    let required_zero = ["CapInh:", "CapPrm:", "CapEff:", "CapBnd:", "CapAmb:"];
    Ok(required_zero.iter().all(|label| {
        status
            .lines()
            .find_map(|line| line.strip_prefix(label))
            .is_some_and(|value| value.trim() == "0000000000000000")
    }) && status.lines().any(|line| line == "NoNewPrivs:\t1"))
}

pub(super) fn ensure_pidfd_alive(pidfd: &OwnedFd) -> IsolationResult<()> {
    let mut descriptors = [PollFd::new(pidfd, PollFlags::IN)];
    let ready = poll(&mut descriptors, Some(&Timespec::default()))
        .map_err(|error| native_errno("poll-guardian-pidfd", error))?;
    if ready == 0 {
        Ok(())
    } else {
        Err(IsolationError::ProcessExited)
    }
}

pub(super) fn validate_connected_stream(
    stream: &TcpStream,
    endpoint: SocketAddr,
) -> IsolationResult<()> {
    let peer = stream
        .peer_addr()
        .map_err(|error| native("read-managed-peer", &error))?;
    let local = stream
        .local_addr()
        .map_err(|error| native("read-managed-local", &error))?;
    let domain =
        socket_domain(stream).map_err(|error| native_errno("read-managed-socket-domain", error))?;
    let expected_domain = if endpoint.is_ipv4() {
        AddressFamily::INET
    } else {
        AddressFamily::INET6
    };
    if peer != endpoint
        || local.port() == 0
        || !local.ip().is_loopback()
        || local.is_ipv4() != endpoint.is_ipv4()
        || domain != expected_domain
        || socket_type(stream).map_err(|error| native_errno("read-managed-socket-type", error))?
            != SocketType::STREAM
        || !fcntl_getfd(stream)
            .map_err(|error| native_errno("read-managed-descriptor-flags", error))?
            .contains(FdFlags::CLOEXEC)
        || stream
            .take_error()
            .map_err(|error| native("read-managed-socket-error", &error))?
            .is_some()
    {
        return Err(IsolationError::HelperProtocol);
    }
    Ok(())
}

pub(super) fn validate_socket_diagnostics(diagnostics: &File) -> IsolationResult<()> {
    let address = SocketAddrNetlink::try_from(
        getsockname(diagnostics)
            .map_err(|error| native_errno("read-socket-diagnostics-address", error))?,
    )
    .map_err(|_error| IsolationError::HelperProtocol)?;
    if socket_domain(diagnostics)
        .map_err(|error| native_errno("read-socket-diagnostics-domain", error))?
        != AddressFamily::NETLINK
        || socket_type(diagnostics)
            .map_err(|error| native_errno("read-socket-diagnostics-type", error))?
            != SocketType::RAW
        || socket_protocol(diagnostics)
            .map_err(|error| native_errno("read-socket-diagnostics-protocol", error))?
            != Some(socket_diagnostics_protocol())
        || !fcntl_getfl(diagnostics)
            .map_err(|error| native_errno("read-socket-diagnostics-status", error))?
            .contains(OFlags::NONBLOCK)
        || !fcntl_getfd(diagnostics)
            .map_err(|error| native_errno("read-socket-diagnostics-flags", error))?
            .contains(FdFlags::CLOEXEC)
        || address.pid() == 0
        || address.groups() != 0
    {
        return Err(IsolationError::HelperProtocol);
    }
    Ok(())
}

fn socket_diagnostics_protocol() -> Protocol {
    Protocol::from_raw(NonZeroU32::new(4).expect("SOCK_DIAG protocol is nonzero"))
}

pub(super) fn native_errno(operation: &'static str, error: rustix::io::Errno) -> IsolationError {
    native(
        operation,
        &std::io::Error::from_raw_os_error(error.raw_os_error()),
    )
}

#[cfg(test)]
mod tests {
    use super::privileges_are_reduced;

    #[test]
    fn current_unreduced_process_is_not_accepted() {
        assert!(!privileges_are_reduced(std::process::id()).expect("read process status"));
    }
}
