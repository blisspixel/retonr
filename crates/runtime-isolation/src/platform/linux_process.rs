use std::{
    io,
    os::fd::BorrowedFd,
    process::{Child, ChildStdout, ExitStatus},
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

use rewrite_types::CancellationToken;
use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    fs::{OFlags, fcntl_getfl, fcntl_setfl},
};

use crate::{IsolationError, IsolationResult, error::native};

use super::linux_validation::native_errno;

const POLL_INTERVAL: Duration = Duration::from_millis(10);
const PROTOCOL_LIMIT: usize = 1_024;

#[derive(Debug)]
pub(super) struct HelperProcess {
    child: Option<Child>,
    exit_status: Option<ExitStatus>,
    pid: u32,
    reaper: Option<mpsc::Receiver<io::Result<ExitStatus>>>,
}

impl HelperProcess {
    pub(super) fn new(child: Child) -> Self {
        Self {
            pid: child.id(),
            child: Some(child),
            exit_status: None,
            reaper: None,
        }
    }

    pub(super) fn id(&self) -> u32 {
        self.pid
    }

    pub(super) fn take_stdout(&mut self) -> IsolationResult<ChildStdout> {
        self.child
            .as_mut()
            .and_then(|child| child.stdout.take())
            .ok_or(IsolationError::HelperProtocol)
    }

    pub(super) fn try_wait(
        &mut self,
        operation: &'static str,
    ) -> IsolationResult<Option<ExitStatus>> {
        if let Some(status) = self.exit_status {
            return Ok(Some(status));
        }
        if let Some(receiver) = &self.reaper {
            return match receiver.try_recv() {
                Ok(status) => self.cache_reaped(status).map(Some),
                Err(mpsc::TryRecvError::Empty) => Ok(None),
                Err(mpsc::TryRecvError::Disconnected) => Err(IsolationError::HelperProtocol),
            };
        }
        let status = self
            .child
            .as_mut()
            .ok_or(IsolationError::HelperProtocol)?
            .try_wait()
            .map_err(|error| native(operation, &error))?;
        if let Some(status) = status {
            self.exit_status = Some(status);
        }
        Ok(status)
    }

    pub(super) fn wait_for_exit(
        &mut self,
        deadline: Instant,
        cancellation: Option<&CancellationToken>,
        operation: &'static str,
    ) -> IsolationResult<ExitStatus> {
        loop {
            if cancellation.is_some_and(CancellationToken::is_cancelled) {
                return Err(IsolationError::Cancelled);
            }
            if let Some(status) = self.try_wait(operation)? {
                return Ok(status);
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(IsolationError::ShutdownTimeout);
            }
            thread::sleep(remaining.min(POLL_INTERVAL));
        }
    }

    pub(super) fn terminate_and_reap(&mut self, deadline: Instant) -> IsolationResult<ExitStatus> {
        if let Some(status) = self.try_wait("poll-isolation-helper-before-termination")? {
            return Ok(status);
        }
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            match start_reaper(child) {
                ReaperStart::Spawned(receiver) => self.reaper = Some(receiver),
                ReaperStart::Completed(status) => return self.cache_reaped(status),
            }
        }
        self.wait_for_exit(deadline, None, "reap-isolation-helper")
    }

    fn cache_reaped(&mut self, status: io::Result<ExitStatus>) -> IsolationResult<ExitStatus> {
        let status = status.map_err(|error| native("reap-isolation-helper", &error))?;
        self.reaper = None;
        self.exit_status = Some(status);
        Ok(status)
    }
}

impl Drop for HelperProcess {
    fn drop(&mut self) {
        if self.exit_status.is_some() || self.reaper.is_some() {
            return;
        }
        let Some(mut child) = self.child.take() else {
            return;
        };
        if child.try_wait().ok().flatten().is_some() {
            return;
        }
        let _ = child.kill();
        let _ = start_reaper(child);
    }
}

enum ReaperStart {
    Spawned(mpsc::Receiver<io::Result<ExitStatus>>),
    Completed(io::Result<ExitStatus>),
}

fn start_reaper(child: Child) -> ReaperStart {
    let shared = Arc::new(Mutex::new(Some(child)));
    let worker_child = Arc::clone(&shared);
    let (sender, receiver) = mpsc::sync_channel(1);
    let worker = thread::Builder::new()
        .name("retonr-isolation-child-reaper".to_owned())
        .spawn(move || {
            let mut child = match worker_child.lock() {
                Ok(mut child) => child.take(),
                Err(poisoned) => poisoned.into_inner().take(),
            };
            let result = child.as_mut().map_or_else(
                || Err(io::Error::other("missing isolation helper child")),
                |child| {
                    let _ = child.kill();
                    child.wait()
                },
            );
            let _ = sender.send(result);
        });
    if worker.is_ok() {
        ReaperStart::Spawned(receiver)
    } else {
        let mut child = match shared.lock() {
            Ok(mut child) => child.take(),
            Err(poisoned) => poisoned.into_inner().take(),
        };
        ReaperStart::Completed(child.as_mut().map_or_else(
            || Err(io::Error::other("missing isolation helper child")),
            |child| {
                let _ = child.kill();
                child.wait()
            },
        ))
    }
}

pub(super) fn read_protocol_line(
    source: BorrowedFd<'_>,
    deadline: Instant,
    cancellation: &CancellationToken,
    timeout_error: &IsolationError,
    operation: &'static str,
) -> IsolationResult<Vec<u8>> {
    let flags = fcntl_getfl(source).map_err(|error| native_errno(operation, error))?;
    fcntl_setfl(source, flags | OFlags::NONBLOCK)
        .map_err(|error| native_errno(operation, error))?;
    let mut line = Vec::new();
    let mut buffer = [0_u8; 256];
    loop {
        if cancellation.is_cancelled() {
            return Err(IsolationError::Cancelled);
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err((*timeout_error).clone());
        }
        let wait = remaining.min(POLL_INTERVAL);
        let timeout = Timespec {
            tv_sec: i64::try_from(wait.as_secs()).unwrap_or(i64::MAX),
            tv_nsec: i64::from(wait.subsec_nanos()),
        };
        let mut descriptors = [PollFd::new(&source, PollFlags::IN)];
        match poll(&mut descriptors, Some(&timeout)) {
            Ok(0) | Err(rustix::io::Errno::INTR) => continue,
            Err(error) => return Err(native_errno(operation, error)),
            Ok(_) => {}
        }
        let returned = descriptors[0].revents();
        if returned.intersects(PollFlags::ERR | PollFlags::NVAL) {
            return Err(IsolationError::HelperProtocol);
        }
        if !returned.intersects(PollFlags::IN | PollFlags::HUP) {
            continue;
        }
        let limit = (PROTOCOL_LIMIT + 1 - line.len()).min(buffer.len());
        let read = match rustix::io::read(source, &mut buffer[..limit]) {
            Ok(read) => read,
            Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => continue,
            Err(error) => return Err(native_errno(operation, error)),
        };
        if read == 0 {
            return Ok(line);
        }
        if let Some(newline) = buffer[..read].iter().position(|byte| *byte == b'\n') {
            let end = newline + 1;
            if line.len().saturating_add(end) > PROTOCOL_LIMIT {
                return Err(IsolationError::HelperProtocol);
            }
            line.extend_from_slice(&buffer[..end]);
            return Ok(line);
        }
        line.extend_from_slice(&buffer[..read]);
        if line.len() > PROTOCOL_LIMIT {
            return Err(IsolationError::HelperProtocol);
        }
    }
}

pub(super) fn require_successful_exit(status: ExitStatus) -> IsolationResult<()> {
    if status.success() {
        Ok(())
    } else {
        Err(IsolationError::HelperProtocol)
    }
}

#[cfg(test)]
mod tests;
