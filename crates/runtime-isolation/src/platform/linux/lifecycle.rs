use std::{
    os::fd::AsFd as _,
    thread,
    time::{Duration, Instant},
};

use rewrite_types::CancellationToken;
use rustix::fd::OwnedFd;

use crate::{IsolationError, IsolationResult};

use super::super::{
    linux_process::{HelperProcess, read_protocol_line},
    linux_protocol::{ReadyMessage, parse_ready},
    linux_validation::ensure_pidfd_alive,
};

const POLL_INTERVAL: Duration = Duration::from_millis(10);

pub(super) fn receive_ready(
    child: &mut HelperProcess,
    timeout: Duration,
    cancellation: &CancellationToken,
) -> IsolationResult<ReadyMessage> {
    receive_ready_until(
        child,
        Instant::now() + timeout,
        &IsolationError::StartupTimeout,
        cancellation,
    )
}

pub(super) fn receive_ready_until(
    child: &mut HelperProcess,
    deadline: Instant,
    timeout_error: &IsolationError,
    cancellation: &CancellationToken,
) -> IsolationResult<ReadyMessage> {
    let stdout = child.take_stdout()?;
    let line = read_protocol_line(
        stdout.as_fd(),
        deadline,
        cancellation,
        timeout_error,
        "read-helper-protocol",
    )?;
    parse_ready(&line)
}

pub(super) fn wait_for_pidfd_exit(pidfd: &OwnedFd, deadline: Instant) -> IsolationResult<()> {
    loop {
        match ensure_pidfd_alive(pidfd) {
            Err(IsolationError::ProcessExited) => return Ok(()),
            Err(error) => return Err(error),
            Ok(()) => {}
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(IsolationError::ShutdownTimeout);
        }
        thread::sleep(remaining.min(POLL_INTERVAL));
    }
}

pub(super) fn ensure_active(cancellation: &CancellationToken) -> IsolationResult<()> {
    if cancellation.is_cancelled() {
        Err(IsolationError::Cancelled)
    } else {
        Ok(())
    }
}

pub(super) fn ensure_active_until(
    cancellation: &CancellationToken,
    operation_deadline: Option<Instant>,
) -> IsolationResult<()> {
    if operation_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
        Err(IsolationError::OperationDeadlineExceeded)
    } else {
        ensure_active(cancellation)
    }
}

pub(super) fn abort_helper(
    child: &mut HelperProcess,
    timeout: Duration,
    error: IsolationError,
) -> IsolationError {
    abort_helper_until(child, Instant::now() + timeout, error)
}

pub(super) fn abort_helper_until(
    child: &mut HelperProcess,
    deadline: Instant,
    error: IsolationError,
) -> IsolationError {
    match child.terminate_and_reap(deadline) {
        Ok(_status) => error,
        Err(cleanup) => cleanup,
    }
}
