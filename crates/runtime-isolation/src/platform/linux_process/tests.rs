use std::{
    io::{Read as _, Write as _},
    os::unix::net::UnixStream,
    os::{fd::AsFd as _, unix::process::ExitStatusExt as _},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use rewrite_types::CancellationToken;
use rustix::process::{Pid, WaitOptions, waitpid};

use super::*;

#[test]
fn readiness_read_obeys_cancellation_without_waiting_for_eof() {
    let (reader, writer) = UnixStream::pair().expect("protocol pair");
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert_eq!(
        read_protocol_line(
            reader.as_fd(),
            Instant::now() + Duration::from_mins(1),
            &cancellation,
            &IsolationError::StartupTimeout,
            "test-read-protocol",
        ),
        Err(IsolationError::Cancelled)
    );
    drop(writer);
}

#[test]
fn readiness_read_preserves_the_exact_deadline_error() {
    for expected in [
        IsolationError::StartupTimeout,
        IsolationError::ControlledBuildSnapshotTimeout,
    ] {
        let (reader, writer) = UnixStream::pair().expect("protocol pair");
        assert_eq!(
            read_protocol_line(
                reader.as_fd(),
                Instant::now(),
                &CancellationToken::new(),
                &expected,
                "test-read-protocol",
            ),
            Err(expected)
        );
        drop(writer);
    }
}

#[test]
fn readiness_read_returns_a_line_without_waiting_for_eof() {
    let (reader, mut writer) = UnixStream::pair().expect("protocol pair");
    writer.write_all(b"READY 1\n").expect("write ready line");
    assert_eq!(
        read_protocol_line(
            reader.as_fd(),
            Instant::now() + Duration::from_secs(1),
            &CancellationToken::new(),
            &IsolationError::StartupTimeout,
            "test-read-protocol",
        ),
        Ok(b"READY 1\n".to_vec())
    );
}

#[test]
fn readiness_read_rejects_an_oversized_line() {
    let (reader, mut writer) = UnixStream::pair().expect("protocol pair");
    writer
        .write_all(&vec![b'x'; PROTOCOL_LIMIT + 1])
        .expect("write oversized line");
    assert_eq!(
        read_protocol_line(
            reader.as_fd(),
            Instant::now() + Duration::from_secs(1),
            &CancellationToken::new(),
            &IsolationError::StartupTimeout,
            "test-read-protocol",
        ),
        Err(IsolationError::HelperProtocol)
    );
}

#[test]
fn terminate_reaps_the_exact_direct_child() {
    let (child, pid) = blocked_child();
    let mut process = HelperProcess::new(child);
    let status = process
        .terminate_and_reap(Instant::now() + Duration::from_secs(5))
        .expect("terminate and reap child");
    assert_eq!(status.signal(), Some(libc::SIGKILL));
    assert!(matches!(
        waitpid(Some(pid), WaitOptions::NOHANG),
        Err(rustix::io::Errno::CHILD)
    ));
}

#[test]
fn detached_reaper_retains_child_ownership_until_wait_completes() {
    let (mut child, pid) = blocked_child();
    let _ = child.kill();
    let receiver = match start_reaper(child) {
        ReaperStart::Spawned(receiver) => receiver,
        ReaperStart::Completed(status) => {
            status.expect("synchronous fallback reap");
            assert!(matches!(
                waitpid(Some(pid), WaitOptions::NOHANG),
                Err(rustix::io::Errno::CHILD)
            ));
            return;
        }
    };
    receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("reaper completion")
        .expect("wait for child");
    assert!(matches!(
        waitpid(Some(pid), WaitOptions::NOHANG),
        Err(rustix::io::Errno::CHILD)
    ));
}

#[test]
fn detached_reaper_wait_obeys_an_expired_deadline() {
    let (_sender, receiver) = mpsc::sync_channel(1);
    let mut process = HelperProcess {
        child: None,
        exit_status: None,
        pid: 1,
        reaper: Some(receiver),
    };

    assert_eq!(
        process.wait_for_exit(Instant::now(), None, "test-detached-reaper-timeout"),
        Err(IsolationError::ShutdownTimeout)
    );
}

#[test]
fn successful_exit_status_is_exact() {
    assert_eq!(
        require_successful_exit(std::process::ExitStatus::from_raw(0)),
        Ok(())
    );
    assert_eq!(
        require_successful_exit(std::process::ExitStatus::from_raw(7 << 8)),
        Err(IsolationError::HelperProtocol)
    );
    assert_eq!(
        require_successful_exit(std::process::ExitStatus::from_raw(libc::SIGKILL)),
        Err(IsolationError::HelperProtocol)
    );
}

fn blocked_child() -> (Child, Pid) {
    let mut child = Command::new("/bin/sh")
        .arg("-c")
        .arg("printf R; IFS= read -r line")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn blocked child");
    let mut ready = [0_u8; 1];
    child
        .stdout
        .as_mut()
        .expect("child stdout")
        .read_exact(&mut ready)
        .expect("child readiness barrier");
    assert_eq!(ready, [b'R']);
    let pid = i32::try_from(child.id())
        .ok()
        .and_then(Pid::from_raw)
        .expect("child pid");
    (child, pid)
}
