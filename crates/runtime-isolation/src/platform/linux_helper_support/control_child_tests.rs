use super::spawn_control_child;
use crate::platform::linux_control::{ControlError, pair, receive};
use std::{
    os::fd::AsFd as _,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[test]
fn exiting_unarmed_child_closes_control_without_waiting_for_deadline() {
    let (parent, child) = pair().expect("control pair");
    let mut command = Command::new("/bin/true");
    command
        .stdin(Stdio::from(child))
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut process = spawn_control_child(command).expect("spawn");
    assert!(process.wait().expect("exit").success());
    let started = Instant::now();
    assert!(matches!(
        receive(parent.as_fd(), started + Duration::from_secs(5), None),
        Err(ControlError::Closed)
    ));
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[test]
fn spawn_failure_releases_the_configured_child_endpoint() {
    let (parent, child) = pair().expect("control pair");
    let mut command = Command::new("/retonr-nonexistent-control-child");
    command.stdin(Stdio::from(child));
    assert!(spawn_control_child(command).is_err());
    assert!(matches!(
        receive(
            parent.as_fd(),
            Instant::now() + Duration::from_secs(1),
            None
        ),
        Err(ControlError::Closed)
    ));
}
