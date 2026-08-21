use std::{
    net::{SocketAddr, TcpStream},
    num::NonZeroU32,
    os::fd::{AsFd as _, BorrowedFd, OwnedFd},
    process::Child,
    thread,
    time::{Duration, Instant},
};

use rustix::net::{
    AddressFamily, Protocol, SocketFlags, SocketType, bind, getsockname,
    netlink::SocketAddrNetlink, socket_with,
};

use super::{
    linux_control::{ControlError, MessageKind, decode_endpoint, receive, send},
    linux_helper_setup::HelperFailure,
    linux_startup::{self, StartupDrains},
};

const CONTROL_POLL: Duration = Duration::from_millis(25);

pub(super) fn serve_parent_control(
    child: &mut Child,
    parent_control: BorrowedFd<'_>,
    stage_control: BorrowedFd<'_>,
    timeout: Duration,
) -> Result<i32, HelperFailure> {
    let mut channel_transferred = false;
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|_| HelperFailure::NamespaceSetup)?
        {
            return Ok(status.code().unwrap_or(1));
        }
        match receive(parent_control, Instant::now() + CONTROL_POLL, None) {
            Err(ControlError::Deadline) => {}
            Err(error) => return Err(control_failure(error)),
            Ok(request) => {
                if channel_transferred
                    || request.kind != MessageKind::Connect
                    || !request.descriptors.is_empty()
                {
                    send_error(parent_control, timeout);
                    return Err(HelperFailure::InvalidLaunch);
                }
                let transferred = transfer_channel(
                    child,
                    parent_control,
                    stage_control,
                    &request.payload,
                    timeout,
                );
                if let Err(failure) = transferred {
                    send_error(parent_control, timeout);
                    return Err(failure);
                }
                channel_transferred = true;
            }
        }
    }
}

fn transfer_channel(
    child: &mut Child,
    parent_control: BorrowedFd<'_>,
    stage_control: BorrowedFd<'_>,
    payload: &[u8],
    timeout: Duration,
) -> Result<(), HelperFailure> {
    let endpoint = decode_endpoint(payload).map_err(control_failure)?;
    let deadline = Instant::now() + timeout;
    let stream = connect_exact_loopback(child, endpoint, deadline)?;
    let diagnostics = create_socket_diagnostics()?;
    let capture = request_startup_capture(stage_control, deadline)?;
    send(
        parent_control,
        MessageKind::Connected,
        &capture,
        &[stream.as_fd(), diagnostics.as_fd()],
        deadline,
        None,
    )
    .map_err(control_failure)
}

fn connect_exact_loopback(
    child: &mut Child,
    endpoint: SocketAddr,
    deadline: Instant,
) -> Result<TcpStream, HelperFailure> {
    let stream = loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(HelperFailure::InvalidLaunch);
        }
        match TcpStream::connect_timeout(&endpoint, remaining.min(Duration::from_millis(100))) {
            Ok(stream) => break stream,
            Err(error) if error.kind() == std::io::ErrorKind::ConnectionRefused => {
                if child
                    .try_wait()
                    .map_err(|_| HelperFailure::InvalidLaunch)?
                    .is_some()
                {
                    return Err(HelperFailure::InvalidLaunch);
                }
                let wait = deadline
                    .saturating_duration_since(Instant::now())
                    .min(CONTROL_POLL);
                if wait.is_zero() {
                    return Err(HelperFailure::InvalidLaunch);
                }
                thread::sleep(wait);
            }
            Err(_error) => return Err(HelperFailure::InvalidLaunch),
        }
    };
    let peer = stream
        .peer_addr()
        .map_err(|_| HelperFailure::InvalidLaunch)?;
    let local = stream
        .local_addr()
        .map_err(|_| HelperFailure::InvalidLaunch)?;
    if peer != endpoint
        || !local.ip().is_loopback()
        || local.port() == 0
        || local.is_ipv4() != endpoint.is_ipv4()
        || stream
            .take_error()
            .map_err(|_| HelperFailure::InvalidLaunch)?
            .is_some()
    {
        return Err(HelperFailure::InvalidLaunch);
    }
    Ok(stream)
}

fn create_socket_diagnostics() -> Result<OwnedFd, HelperFailure> {
    let descriptor = socket_with(
        AddressFamily::NETLINK,
        SocketType::RAW,
        SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
        Some(socket_diagnostics_protocol()),
    )
    .map_err(|_| HelperFailure::InvalidLaunch)?;
    bind(&descriptor, &SocketAddrNetlink::new(0, 0)).map_err(|_| HelperFailure::InvalidLaunch)?;
    let address = SocketAddrNetlink::try_from(
        getsockname(&descriptor).map_err(|_| HelperFailure::InvalidLaunch)?,
    )
    .map_err(|_| HelperFailure::InvalidLaunch)?;
    if address.pid() == 0 || address.groups() != 0 {
        return Err(HelperFailure::InvalidLaunch);
    }
    Ok(descriptor)
}

fn socket_diagnostics_protocol() -> Protocol {
    Protocol::from_raw(NonZeroU32::new(4).expect("SOCK_DIAG protocol is nonzero"))
}

fn request_startup_capture(
    control: BorrowedFd<'_>,
    deadline: Instant,
) -> Result<Vec<u8>, HelperFailure> {
    send(control, MessageKind::Capture, &[], &[], deadline, None).map_err(control_failure)?;
    let response = receive(control, deadline, None).map_err(control_failure)?;
    if response.kind != MessageKind::Captured
        || !response.descriptors.is_empty()
        || linux_startup::decode(&response.payload).is_none()
    {
        return Err(HelperFailure::InvalidLaunch);
    }
    Ok(response.payload)
}

fn send_error(control: BorrowedFd<'_>, timeout: Duration) {
    let _ = send(
        control,
        MessageKind::Error,
        &[],
        &[],
        Instant::now() + timeout,
        None,
    );
}

pub(super) fn serve_stage_control(
    child: &mut Child,
    drains: &StartupDrains,
    control: BorrowedFd<'_>,
    timeout: Duration,
) -> Result<i32, HelperFailure> {
    let mut capture_sent = false;
    loop {
        if let Some(status) = child.try_wait().map_err(|_| HelperFailure::InvalidLaunch)? {
            return Ok(status.code().unwrap_or(1));
        }
        match receive(control, Instant::now() + CONTROL_POLL, None) {
            Err(ControlError::Deadline) => {}
            Err(error) => return Err(control_failure(error)),
            Ok(request) => {
                if capture_sent
                    || request.kind != MessageKind::Capture
                    || !request.payload.is_empty()
                    || !request.descriptors.is_empty()
                {
                    send_error(control, timeout);
                    return Err(HelperFailure::InvalidLaunch);
                }
                let capture = linux_startup::encode(&drains.snapshot())
                    .ok_or(HelperFailure::InvalidLaunch)?;
                send(
                    control,
                    MessageKind::Captured,
                    &capture,
                    &[],
                    Instant::now() + timeout,
                    None,
                )
                .map_err(control_failure)?;
                capture_sent = true;
            }
        }
    }
}

fn control_failure(error: ControlError) -> HelperFailure {
    match error {
        ControlError::Cancelled
        | ControlError::Deadline
        | ControlError::Closed
        | ControlError::Invalid
        | ControlError::Native => HelperFailure::InvalidLaunch,
    }
}
