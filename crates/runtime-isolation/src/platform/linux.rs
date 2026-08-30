use std::{
    fs::File,
    net::{SocketAddr, TcpStream},
    os::fd::AsFd as _,
    process::Stdio,
    time::{Duration, Instant},
};

use super::linux_command::{
    apply_policy_environment, apply_target_environment, helper_command_with_input, spawn_helper,
};
use super::linux_control::{self, MessageKind, map_error as map_control_error};
use super::linux_helper_identity::{open_executable, validate_executable};
use super::linux_process::HelperProcess;
use super::linux_protocol::ReadyMessage;
use super::linux_startup;
use super::linux_target::reobserve_target;
use super::linux_validation::{
    ensure_pidfd_alive, namespace_identity, open_namespace, privileges_are_reduced,
    validate_connected_stream, validate_socket_diagnostics,
};
use super::{LeasePlatform, PreparedPlatform};
use crate::contract::RetainedRuntimeInputMember;
use crate::{
    ControlledBuildExecution, ControlledBuildInputFile, ControlledBuildLaunchSpec, IsolationError,
    IsolationEvidence, IsolationPolicy, IsolationPreparationEvidence, IsolationResult, LaunchSpec,
    LinuxSocketDiagnosticsCapability, ManagedLoopbackChannel, RetainedProgramBootstrapCapabilities,
    RetainedProgramBootstrapExecution, RetainedProgramBootstrapLaunchSpec,
    RetainedRuntimeInputTree,
};
use rewrite_types::CancellationToken;
use rustix::fd::OwnedFd;

mod deadline;
mod finish;
mod lifecycle;
mod prepare;
mod verify;

use deadline::{StageDeadline, operation_error_precedence, operation_result_precedence};
use finish::{FinishLaunchRequest, finish_launch};
use lifecycle::{
    abort_helper, abort_helper_until, ensure_active, ensure_active_until, receive_ready,
    receive_ready_until, wait_for_pidfd_exit,
};

pub(crate) use prepare::{prepare, prepare_retained};

#[derive(Debug)]
pub(crate) struct Prepared {
    helper: File,
    preparation: IsolationPreparationEvidence,
}

#[derive(Debug)]
pub(crate) struct Lease {
    child: HelperProcess,
    pidfd: OwnedFd,
    namespace_init_pidfd: OwnedFd,
    target_pidfd: OwnedFd,
    namespace_init_pid: u32,
    target_executable: File,
    network_namespace: File,
    user_namespace: File,
    process_namespace: File,
    mount_namespace: File,
    runtime_inputs: Vec<RetainedRuntimeInputMember>,
    initial: IsolationEvidence,
    control: OwnedFd,
    channel_timeout: Duration,
    channel_requested: bool,
    shutdown_timeout: Duration,
    closed: bool,
}

#[cfg(feature = "test-support")]
impl Prepared {
    pub(crate) fn test_support(preparation: IsolationPreparationEvidence) -> Self {
        let executable = std::env::current_exe().expect("test-support executable path");
        let helper = File::open(executable).expect("test-support executable handle");
        Self {
            helper,
            preparation,
        }
    }
}

impl PreparedPlatform for Prepared {
    fn launch(
        &self,
        specification: &LaunchSpec,
        policy: IsolationPolicy,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> IsolationResult<Lease> {
        ensure_active_until(cancellation, operation_deadline)?;
        let target = open_executable(specification.executable(), false)?;
        launch_retained(
            self,
            specification,
            &target,
            Vec::new(),
            policy,
            cancellation,
            operation_deadline,
        )
    }

    fn launch_retained(
        &self,
        specification: &LaunchSpec,
        executable: File,
        policy: IsolationPolicy,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> IsolationResult<Lease> {
        ensure_active_until(cancellation, operation_deadline)?;
        validate_executable(&executable, false)?;
        launch_retained(
            self,
            specification,
            &executable,
            Vec::new(),
            policy,
            cancellation,
            operation_deadline,
        )
    }

    fn launch_retained_with_inputs(
        &self,
        specification: &LaunchSpec,
        executable: File,
        inputs: RetainedRuntimeInputTree,
        policy: IsolationPolicy,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> IsolationResult<Lease> {
        ensure_active_until(cancellation, operation_deadline)?;
        validate_executable(&executable, false)?;
        launch_retained(
            self,
            specification,
            &executable,
            inputs.into_members(),
            policy,
            cancellation,
            operation_deadline,
        )
    }

    fn run_controlled_build_retained(
        &self,
        specification: &ControlledBuildLaunchSpec,
        program: File,
        input_root: File,
        input_files: Vec<ControlledBuildInputFile>,
        output_root: File,
        policy: IsolationPolicy,
        cancellation: &CancellationToken,
    ) -> IsolationResult<ControlledBuildExecution> {
        let mut program = program;
        super::linux_build::run(super::linux_build::BuildRequest {
            helper: &self.helper,
            preparation: &self.preparation,
            specification,
            program: &mut program,
            input_root: &input_root,
            input_files: &input_files,
            output_root: &output_root,
            policy,
            cancellation,
        })
    }

    fn run_retained_program_bootstrap(
        &self,
        specification: &RetainedProgramBootstrapLaunchSpec,
        capabilities: RetainedProgramBootstrapCapabilities,
        policy: IsolationPolicy,
        cancellation: &CancellationToken,
    ) -> IsolationResult<RetainedProgramBootstrapExecution> {
        super::linux_bootstrap::run(
            &self.helper,
            &self.preparation,
            specification,
            capabilities,
            policy,
            cancellation,
        )
    }
}

fn launch_retained(
    prepared: &Prepared,
    specification: &LaunchSpec,
    target: &File,
    runtime_inputs: Vec<RetainedRuntimeInputMember>,
    policy: IsolationPolicy,
    cancellation: &CancellationToken,
    operation_deadline: Option<Instant>,
) -> IsolationResult<Lease> {
    ensure_active_until(cancellation, operation_deadline)?;
    let declarations = runtime_inputs
        .iter()
        .map(|input| input.declaration.clone())
        .collect::<Vec<_>>();
    let input_header = super::linux_managed_input_protocol::header(&declarations)
        .map_err(super::linux_helper_setup::HelperFailure::into_isolation_error)?;
    let operation_timeout = if runtime_inputs.is_empty() {
        policy.startup_timeout()
    } else {
        super::linux_managed_input_protocol::MANAGED_INPUT_VERIFICATION_TIMEOUT
    };
    let deadline = StageDeadline::new(operation_deadline, operation_timeout)?;
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
        &super::linux_managed_input_protocol::encode_header(&input_header),
        &[target.as_fd()],
        deadline.at(),
        Some(cancellation),
    ) {
        let error = deadline.map_control_error(error);
        let error = operation_error_precedence(error, cancellation, operation_deadline);
        return Err(abort_helper(&mut child, policy.shutdown_timeout(), error));
    }
    for (index, input) in runtime_inputs.iter().enumerate() {
        let payload =
            super::linux_managed_input_protocol::encode_declaration(index, &input.declaration)
                .map_err(super::linux_helper_setup::HelperFailure::into_isolation_error)?;
        if let Err(error) = linux_control::send(
            control.as_fd(),
            MessageKind::ManagedInputDescriptor,
            &payload,
            &[input.file.as_fd()],
            deadline.at(),
            Some(cancellation),
        ) {
            let error = deadline.map_control_error(error);
            let error = operation_error_precedence(error, cancellation, operation_deadline);
            return Err(abort_helper(&mut child, policy.shutdown_timeout(), error));
        }
    }
    let ready = receive_ready_until(
        &mut child,
        deadline.at(),
        &deadline.timeout_error,
        cancellation,
    )
    .map_err(|error| {
        let error = deadline.map_isolation_error(error);
        let error = operation_error_precedence(error, cancellation, operation_deadline);
        abort_helper(&mut child, policy.shutdown_timeout(), error)
    })?;
    finish_launch(
        child,
        FinishLaunchRequest {
            target,
            runtime_inputs,
            control,
            ready,
            policy,
            preparation: &prepared.preparation,
            cancellation,
            operation_deadline,
        },
    )
}

impl LeasePlatform for Lease {
    fn initial_evidence(&self) -> IsolationEvidence {
        self.initial.clone()
    }

    fn reobserve(
        &mut self,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> IsolationResult<IsolationEvidence> {
        let result = (|| {
            ensure_active_until(cancellation, operation_deadline)?;
            if self.closed || self.child.try_wait("poll-guardian")?.is_some() {
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
                self.initial.mount_namespace(),
            )?;
            super::linux_managed_mount::reobserve(
                self.initial.target().outer_pid(),
                self.initial.device_boundary(),
            )?;
            let runtime_input_declarations = self
                .runtime_inputs
                .iter()
                .map(|input| input.declaration.clone())
                .collect::<Vec<_>>();
            for input in &self.runtime_inputs {
                ensure_active_until(cancellation, operation_deadline)?;
                if let Err(failure) = super::linux_managed_input_protocol::validate_descriptor(
                    &input.declaration,
                    &input.file,
                    Some(cancellation),
                ) {
                    return Err(if cancellation.is_cancelled() {
                        IsolationError::Cancelled
                    } else {
                        failure.into_isolation_error()
                    });
                }
            }
            super::linux_managed_input_mount::reobserve(
                self.initial.target().outer_pid(),
                self.initial.runtime_inputs(),
                &runtime_input_declarations,
                cancellation,
            )?;
            if namespace_identity(&self.network_namespace)?
                != namespace_identity(&open_namespace(guardian_pid, "net")?)?
                || namespace_identity(&self.user_namespace)?
                    != namespace_identity(&open_namespace(guardian_pid, "user")?)?
                || namespace_identity(&self.process_namespace)?
                    != namespace_identity(&open_namespace(self.namespace_init_pid, "pid")?)?
                || namespace_identity(&self.mount_namespace)?
                    != namespace_identity(&open_namespace(guardian_pid, "mnt")?)?
                || namespace_identity(&self.mount_namespace)?
                    != namespace_identity(&open_namespace(self.namespace_init_pid, "mnt")?)?
                || namespace_identity(&self.mount_namespace)?
                    != namespace_identity(&open_namespace(
                        self.initial.target().outer_pid(),
                        "mnt",
                    )?)?
                || !privileges_are_reduced(guardian_pid)?
                || !privileges_are_reduced(self.namespace_init_pid)?
            {
                return Err(IsolationError::EvidenceChanged);
            }
            ensure_active_until(cancellation, operation_deadline)?;
            Ok(self.initial.clone())
        })();
        operation_result_precedence(result, cancellation, operation_deadline)
    }

    fn connect_loopback(
        &mut self,
        endpoint: SocketAddr,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> IsolationResult<ManagedLoopbackChannel> {
        let result = (|| {
            if self.channel_requested {
                return Err(IsolationError::ChannelAlreadyRequested);
            }
            self.channel_requested = true;
            ensure_active_until(cancellation, operation_deadline)?;
            let payload = linux_control::encode_endpoint(endpoint)
                .map_err(|_error| IsolationError::InvalidChannelEndpoint)?;
            if self.closed || self.child.try_wait("poll-guardian")?.is_some() {
                return Err(IsolationError::ProcessExited);
            }
            ensure_pidfd_alive(&self.pidfd)?;
            ensure_pidfd_alive(&self.namespace_init_pidfd)?;
            ensure_pidfd_alive(&self.target_pidfd)?;
            let deadline = StageDeadline::new(operation_deadline, self.channel_timeout)?;
            linux_control::send(
                self.control.as_fd(),
                MessageKind::Connect,
                &payload,
                &[],
                deadline.at(),
                Some(cancellation),
            )
            .map_err(|error| deadline.map_control_error(error))?;
            let response =
                linux_control::receive(self.control.as_fd(), deadline.at(), Some(cancellation))
                    .map_err(|error| deadline.map_control_error(error))?;
            deadline.ensure_not_expired()?;
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
            ensure_active_until(cancellation, operation_deadline)?;
            Ok(ManagedLoopbackChannel::new(
                stream,
                LinuxSocketDiagnosticsCapability::new(diagnostics),
                startup_output,
            ))
        })();
        operation_result_precedence(result, cancellation, operation_deadline)
    }

    fn close(&mut self, cancellation: &CancellationToken) -> IsolationResult<()> {
        if self.closed {
            return Ok(());
        }
        let cancelled = cancellation.is_cancelled();
        let deadline = Instant::now() + self.shutdown_timeout;
        self.child.terminate_and_reap(deadline)?;
        wait_for_pidfd_exit(&self.target_pidfd, deadline)?;
        wait_for_pidfd_exit(&self.namespace_init_pidfd, deadline)?;
        self.closed = true;
        if cancelled {
            Err(IsolationError::Cancelled)
        } else {
            Ok(())
        }
    }
}
