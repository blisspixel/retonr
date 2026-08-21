use std::{
    fs::File,
    io::{BufRead as _, BufReader, Read as _},
    net::{SocketAddr, TcpStream},
    os::fd::AsFd as _,
    path::Path,
    process::{Child, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use super::linux_command::{
    apply_policy_environment, apply_target_environment, helper_command, helper_command_with_input,
    spawn_helper,
};
use super::linux_control::{self, MessageKind, map_error as map_control_error};
use super::linux_helper_identity::{hash_helper, open_executable, validate_executable};
use super::linux_protocol::{ReadyMessage, parse_ready};
use super::linux_startup;
use super::linux_target::{observe_target, reobserve_target};
use super::linux_validation::{
    ensure_pidfd_alive, namespace_identity, native_errno, open_namespace, privileges_are_reduced,
    validate_connected_stream, validate_socket_diagnostics,
};
use super::{LeasePlatform, PrepareOutput, PreparedPlatform};
use crate::{
    IsolationError, IsolationEvidence, IsolationPolicy, IsolationPreparationEvidence,
    IsolationResult, LaunchSpec, LinuxSocketDiagnosticsCapability, ManagedLoopbackChannel,
    error::native,
};
use rewrite_types::CancellationToken;
use rustix::{
    fd::OwnedFd,
    process::{Pid, PidfdFlags, pidfd_open},
};

const PROTOCOL_LIMIT: usize = 1024;
const POLL_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Debug)]
pub(crate) struct Prepared {
    helper: File,
    preparation: IsolationPreparationEvidence,
}

#[derive(Debug)]
pub(crate) struct Lease {
    child: Child,
    pidfd: OwnedFd,
    namespace_init_pidfd: OwnedFd,
    target_pidfd: OwnedFd,
    namespace_init_pid: u32,
    target_executable: File,
    network_namespace: File,
    user_namespace: File,
    process_namespace: File,
    initial: IsolationEvidence,
    control: OwnedFd,
    channel_timeout: Duration,
    channel_requested: bool,
    shutdown_timeout: Duration,
    closed: bool,
}

pub(crate) fn prepare(
    helper_executable: &Path,
    policy: IsolationPolicy,
    cancellation: &CancellationToken,
) -> IsolationResult<PrepareOutput> {
    ensure_active(cancellation)?;
    let mut helper = open_executable(helper_executable, true)?;
    let started = Instant::now();
    let (helper_digest, helper_bytes) =
        hash_helper(&mut helper, policy.startup_timeout(), cancellation, started)?;
    let mut command = helper_command(&helper);
    command.arg("--stage1-probe");
    apply_policy_environment(&mut command, policy);
    let mut child = spawn_helper(&mut command)?;
    let remaining = policy.startup_timeout().saturating_sub(started.elapsed());
    if remaining.is_zero() {
        terminate(&mut child);
        return Err(IsolationError::StartupTimeout);
    }
    let ready = receive_ready(&mut child, remaining, cancellation)?;
    wait_for_exit(&mut child, policy.shutdown_timeout(), cancellation, false)?;
    let preparation =
        IsolationPreparationEvidence::verified(ready.loopback_index, helper_digest, helper_bytes);
    Ok((
        Prepared {
            helper,
            preparation: preparation.clone(),
        },
        preparation,
    ))
}

impl PreparedPlatform for Prepared {
    fn launch(
        &self,
        specification: &LaunchSpec,
        policy: IsolationPolicy,
        cancellation: &CancellationToken,
    ) -> IsolationResult<Lease> {
        ensure_active(cancellation)?;
        let target = open_executable(specification.executable(), false)?;
        launch_retained(self, specification, &target, policy, cancellation)
    }

    fn launch_retained(
        &self,
        specification: &LaunchSpec,
        executable: File,
        policy: IsolationPolicy,
        cancellation: &CancellationToken,
    ) -> IsolationResult<Lease> {
        validate_executable(&executable, false)?;
        launch_retained(self, specification, &executable, policy, cancellation)
    }
}

fn launch_retained(
    prepared: &Prepared,
    specification: &LaunchSpec,
    target: &File,
    policy: IsolationPolicy,
    cancellation: &CancellationToken,
) -> IsolationResult<Lease> {
    ensure_active(cancellation)?;
    let (control, child_control) = linux_control::pair().map_err(map_control_error)?;
    let mut command = helper_command_with_input(&prepared.helper, Stdio::from(child_control));
    command.arg("--stage1-launch");
    command.args(specification.arguments());
    apply_policy_environment(&mut command, policy);
    apply_target_environment(&mut command, specification);
    let mut child = spawn_helper(&mut command)?;
    if let Err(error) = linux_control::send(
        control.as_fd(),
        MessageKind::LaunchDescriptor,
        &[],
        &[target.as_fd()],
        Instant::now() + policy.startup_timeout(),
        Some(cancellation),
    ) {
        terminate(&mut child);
        return Err(map_control_error(error));
    }
    let ready = receive_ready(&mut child, policy.startup_timeout(), cancellation)?;
    finish_launch(child, target, control, ready, policy, &prepared.preparation)
}

fn finish_launch(
    mut child: Child,
    target: &File,
    control: OwnedFd,
    ready: ReadyMessage,
    policy: IsolationPolicy,
    preparation: &IsolationPreparationEvidence,
) -> IsolationResult<Lease> {
    let guardian_pid = child.id();
    if ready.guardian_pid != guardian_pid {
        terminate(&mut child);
        return Err(IsolationError::HelperProtocol);
    }
    let verified = (|| {
        let raw_pid = i32::try_from(guardian_pid)
            .ok()
            .and_then(Pid::from_raw)
            .ok_or(IsolationError::HelperProtocol)?;
        let pidfd = pidfd_open(raw_pid, PidfdFlags::empty())
            .map_err(|error| native_errno("open-guardian-pidfd", error))?;
        let network_namespace = open_namespace(guardian_pid, "net")?;
        let user_namespace = open_namespace(guardian_pid, "user")?;
        let process_namespace = open_namespace(ready.namespace_init_pid, "pid")?;
        let init_raw_pid = i32::try_from(ready.namespace_init_pid)
            .ok()
            .and_then(Pid::from_raw)
            .ok_or(IsolationError::HelperProtocol)?;
        let namespace_init_pidfd = pidfd_open(init_raw_pid, PidfdFlags::empty())
            .map_err(|error| native_errno("open-namespace-init-pidfd", error))?;
        let actual_network = namespace_identity(&network_namespace)?;
        let actual_user = namespace_identity(&user_namespace)?;
        let actual_process = namespace_identity(&process_namespace)?;
        let target_observation = observe_target(
            ready.namespace_init_pid,
            target,
            actual_network,
            actual_user,
            actual_process,
        )?;
        if actual_network != ready.network_namespace
            || actual_user != ready.user_namespace
            || actual_process != ready.process_namespace
            || !privileges_are_reduced(guardian_pid)?
            || !privileges_are_reduced(ready.namespace_init_pid)?
        {
            return Err(IsolationError::EvidenceChanged);
        }
        Ok((
            pidfd,
            namespace_init_pidfd,
            network_namespace,
            user_namespace,
            process_namespace,
            actual_network,
            actual_user,
            actual_process,
            target_observation,
        ))
    })();
    let (
        pidfd,
        namespace_init_pidfd,
        network_namespace,
        user_namespace,
        process_namespace,
        actual_network,
        actual_user,
        actual_process,
        target_observation,
    ) = match verified {
        Ok(verified) => verified,
        Err(error) => {
            terminate(&mut child);
            return Err(error);
        }
    };
    let initial = IsolationEvidence::new(
        guardian_pid,
        actual_network,
        actual_user,
        actual_process,
        preparation.clone(),
        target_observation.evidence,
    );
    Ok(Lease {
        child,
        pidfd,
        namespace_init_pidfd,
        target_pidfd: target_observation.pidfd,
        namespace_init_pid: ready.namespace_init_pid,
        target_executable: target_observation.executable,
        network_namespace,
        user_namespace,
        process_namespace,
        initial,
        control,
        channel_timeout: policy.startup_timeout(),
        channel_requested: false,
        shutdown_timeout: policy.shutdown_timeout(),
        closed: false,
    })
}

impl LeasePlatform for Lease {
    fn initial_evidence(&self) -> IsolationEvidence {
        self.initial.clone()
    }

    fn reobserve(
        &mut self,
        cancellation: &CancellationToken,
    ) -> IsolationResult<IsolationEvidence> {
        ensure_active(cancellation)?;
        if self.closed
            || self
                .child
                .try_wait()
                .map_err(|error| native("poll-guardian", &error))?
                .is_some()
        {
            return Err(IsolationError::ProcessExited);
        }
        ensure_pidfd_alive(&self.pidfd)?;
        ensure_pidfd_alive(&self.namespace_init_pidfd)?;
        let guardian_pid = self.initial.guardian_pid();
        ensure_pidfd_alive(&self.target_pidfd)?;
        reobserve_target(
            self.namespace_init_pid,
            self.initial.target(),
            &self.target_executable,
            self.initial.network_namespace(),
            self.initial.user_namespace(),
            self.initial.process_namespace(),
        )?;
        if namespace_identity(&self.network_namespace)?
            != namespace_identity(&open_namespace(guardian_pid, "net")?)?
            || namespace_identity(&self.user_namespace)?
                != namespace_identity(&open_namespace(guardian_pid, "user")?)?
            || namespace_identity(&self.process_namespace)?
                != namespace_identity(&open_namespace(self.namespace_init_pid, "pid")?)?
            || !privileges_are_reduced(guardian_pid)?
            || !privileges_are_reduced(self.namespace_init_pid)?
        {
            return Err(IsolationError::EvidenceChanged);
        }
        Ok(self.initial.clone())
    }

    fn connect_loopback(
        &mut self,
        endpoint: SocketAddr,
        cancellation: &CancellationToken,
    ) -> IsolationResult<ManagedLoopbackChannel> {
        if self.channel_requested {
            return Err(IsolationError::ChannelAlreadyRequested);
        }
        self.channel_requested = true;
        ensure_active(cancellation)?;
        let payload = linux_control::encode_endpoint(endpoint)
            .map_err(|_error| IsolationError::InvalidChannelEndpoint)?;
        if self.closed
            || self
                .child
                .try_wait()
                .map_err(|error| native("poll-guardian", &error))?
                .is_some()
        {
            return Err(IsolationError::ProcessExited);
        }
        ensure_pidfd_alive(&self.pidfd)?;
        ensure_pidfd_alive(&self.namespace_init_pidfd)?;
        ensure_pidfd_alive(&self.target_pidfd)?;
        let deadline = Instant::now() + self.channel_timeout;
        linux_control::send(
            self.control.as_fd(),
            MessageKind::Connect,
            &payload,
            &[],
            deadline,
            Some(cancellation),
        )
        .map_err(map_control_error)?;
        let response = linux_control::receive(self.control.as_fd(), deadline, Some(cancellation))
            .map_err(map_control_error)?;
        if response.kind != MessageKind::Connected || response.descriptors.len() != 2 {
            return Err(IsolationError::HelperProtocol);
        }
        let startup_output =
            linux_startup::decode(&response.payload).ok_or(IsolationError::HelperProtocol)?;
        let mut descriptors = response.descriptors.into_iter();
        let stream = TcpStream::from(descriptors.next().ok_or(IsolationError::HelperProtocol)?);
        let diagnostics = File::from(descriptors.next().ok_or(IsolationError::HelperProtocol)?);
        if descriptors.next().is_some() {
            return Err(IsolationError::HelperProtocol);
        }
        validate_connected_stream(&stream, endpoint)?;
        validate_socket_diagnostics(&diagnostics)?;
        Ok(ManagedLoopbackChannel::new(
            stream,
            LinuxSocketDiagnosticsCapability::new(diagnostics),
            startup_output,
        ))
    }

    fn close(&mut self, cancellation: &CancellationToken) -> IsolationResult<()> {
        if self.closed {
            return Ok(());
        }
        let cancelled = cancellation.is_cancelled();
        terminate(&mut self.child);
        wait_for_exit(
            &mut self.child,
            self.shutdown_timeout,
            &CancellationToken::new(),
            true,
        )?;
        let started = Instant::now();
        wait_for_pidfd_exit(&self.target_pidfd, started, self.shutdown_timeout)?;
        wait_for_pidfd_exit(&self.namespace_init_pidfd, started, self.shutdown_timeout)?;
        self.closed = true;
        if cancelled {
            Err(IsolationError::Cancelled)
        } else {
            Ok(())
        }
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        if !self.closed {
            terminate(&mut self.child);
            let _ = self.child.try_wait();
        }
    }
}

fn receive_ready(
    child: &mut Child,
    timeout: Duration,
    cancellation: &CancellationToken,
) -> IsolationResult<ReadyMessage> {
    let stdout = child.stdout.take().ok_or(IsolationError::HelperProtocol)?;
    let (sender, receiver) = mpsc::sync_channel(1);
    let reader = thread::spawn(move || {
        let mut line = Vec::new();
        let result = BufReader::new(stdout)
            .take(u64::try_from(PROTOCOL_LIMIT).unwrap_or(u64::MAX) + 1)
            .read_until(b'\n', &mut line)
            .map(|_| line);
        let _ = sender.send(result);
    });
    let started = Instant::now();
    let result = loop {
        if cancellation.is_cancelled() {
            break Err(IsolationError::Cancelled);
        }
        let remaining = timeout.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            break Err(IsolationError::StartupTimeout);
        }
        match receiver.recv_timeout(remaining.min(POLL_INTERVAL)) {
            Ok(Ok(line)) => break parse_ready(&line),
            Ok(Err(error)) => break Err(native("read-helper-protocol", &error)),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break Err(IsolationError::HelperProtocol);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    };
    if result.is_err() {
        terminate(child);
    }
    let _ = reader.join();
    result
}

fn wait_for_exit(
    child: &mut Child,
    timeout: Duration,
    cancellation: &CancellationToken,
    already_terminated: bool,
) -> IsolationResult<()> {
    let started = Instant::now();
    loop {
        if cancellation.is_cancelled() {
            if !already_terminated {
                terminate(child);
            }
            return Err(IsolationError::Cancelled);
        }
        if child
            .try_wait()
            .map_err(|error| native("wait-isolation-helper", &error))?
            .is_some()
        {
            return Ok(());
        }
        if started.elapsed() >= timeout {
            terminate(child);
            return Err(IsolationError::ShutdownTimeout);
        }
        thread::sleep(POLL_INTERVAL);
    }
}

fn wait_for_pidfd_exit(
    pidfd: &OwnedFd,
    started: Instant,
    timeout: Duration,
) -> IsolationResult<()> {
    loop {
        match ensure_pidfd_alive(pidfd) {
            Err(IsolationError::ProcessExited) => return Ok(()),
            Err(error) => return Err(error),
            Ok(()) => {}
        }
        if started.elapsed() >= timeout {
            return Err(IsolationError::ShutdownTimeout);
        }
        thread::sleep(POLL_INTERVAL);
    }
}

fn ensure_active(cancellation: &CancellationToken) -> IsolationResult<()> {
    if cancellation.is_cancelled() {
        Err(IsolationError::Cancelled)
    } else {
        Ok(())
    }
}

fn terminate(child: &mut Child) {
    let _ = child.kill();
    let _ = child.try_wait();
}
