use std::{os::fd::AsFd as _, process::Stdio, time::Instant};

use crate::{
    ControlledBuildExecution, ControlledBuildIsolationEvidence, ControlledBuildProcessStatus,
    IsolationError, IsolationResult,
    contract::{ControlledBuildIsolationObservation, RetainedProgramBootstrapRootObservation},
};

use super::{
    linux_build::{self, BuildObjectMetadata, BuildRequest},
    linux_build_response,
    linux_command::{apply_policy_environment, helper_command_with_input, spawn_helper},
    linux_control::{self, MessageKind},
    linux_process::{HelperProcess, read_protocol_line},
    linux_protocol::{BootstrapReadyMessage, BuildReadyMessage, parse_bootstrap_ready},
};

pub(super) fn execute(
    request: &mut BuildRequest<'_>,
    metadata: &BuildObjectMetadata,
    bootstrap_payload: &[u8],
) -> IsolationResult<(
    RetainedProgramBootstrapRootObservation,
    ControlledBuildExecution,
)> {
    let (ready, output, observed) = execute_helper(request, bootstrap_payload)?;
    revalidate(request, metadata)?;
    let root = ready.root;
    let observation = ControlledBuildIsolationObservation {
        guardian_pid: ready.guardian_pid,
        namespace_init_pid: ready.namespace_init_pid,
        network_namespace: ready.network_namespace,
        user_namespace: ready.user_namespace,
        process_namespace: ready.process_namespace,
        mount_namespace: ready.mount_namespace,
        landlock_abi: ready.landlock_abi,
        program_bytes: request.specification.program_bytes(),
        program_device: std::os::unix::fs::MetadataExt::dev(&metadata.program),
        program_inode: std::os::unix::fs::MetadataExt::ino(&metadata.program),
        input_device: std::os::unix::fs::MetadataExt::dev(&metadata.input),
        input_inode: std::os::unix::fs::MetadataExt::ino(&metadata.input),
        output_device: std::os::unix::fs::MetadataExt::dev(&metadata.output),
        output_inode: std::os::unix::fs::MetadataExt::ino(&metadata.output),
    };
    let isolation = ControlledBuildIsolationEvidence::from_observation(
        observation,
        request.preparation.clone(),
        request.policy.redacted_digest(),
        request.specification.program_digest().clone(),
        request
            .specification
            .redacted_digest_with_inputs(request.input_files),
    );
    drop(observed);
    Ok((
        root,
        ControlledBuildExecution::new(isolation, output, ControlledBuildProcessStatus::Success),
    ))
}

fn execute_helper(
    request: &BuildRequest<'_>,
    bootstrap_payload: &[u8],
) -> IsolationResult<(
    BootstrapReadyMessage,
    crate::ControlledBuildOutput,
    linux_build::ObservedBuildNamespaces,
)> {
    let (control, child_control) =
        linux_control::pair().map_err(|_error| IsolationError::HelperProtocol)?;
    let mut command = helper_command_with_input(request.helper, Stdio::from(child_control));
    command
        .arg("--stage1-bootstrap")
        .args(request.specification.arguments())
        .env(
            "REWRITE_ISOLATION_INTERNAL_BUILD_PROGRAM_RELATIVE_PATH",
            request.specification.program_relative_path(),
        )
        .env(
            "REWRITE_ISOLATION_INTERNAL_BUILD_TIMEOUT_MILLIS",
            request
                .specification
                .execution_timeout()
                .as_millis()
                .to_string(),
        );
    apply_policy_environment(&mut command, request.policy);
    linux_build::apply_target_environment_build(&mut command, request.specification);
    let mut child = spawn_helper(&mut command)?;
    drop(command);
    let startup_deadline = Instant::now() + request.policy.startup_timeout();
    send_capabilities(
        control.as_fd(),
        request,
        bootstrap_payload,
        startup_deadline,
    )
    .map_err(|error| {
        linux_build::abort_helper(&mut child, request.policy.shutdown_timeout(), error)
    })?;
    let ready = receive_ready(&mut child, request).map_err(|error| {
        linux_build::abort_helper(&mut child, request.policy.shutdown_timeout(), error)
    })?;
    let build_ready = ready_as_build(&ready);
    let observed = linux_build::validate_ready(&child, build_ready).map_err(|error| {
        linux_build::abort_helper(&mut child, request.policy.shutdown_timeout(), error)
    })?;
    linux_control::send(
        control.as_fd(),
        MessageKind::BootstrapGo,
        &[],
        &[],
        Instant::now() + request.policy.startup_timeout(),
        Some(request.cancellation),
    )
    .map_err(|error| {
        linux_build::abort_helper(
            &mut child,
            request.policy.shutdown_timeout(),
            linux_build::map_control_error(error, false),
        )
    })?;
    let finished = linux_control::receive(
        control.as_fd(),
        Instant::now() + request.specification.execution_timeout(),
        Some(request.cancellation),
    )
    .map_err(|error| {
        linux_build::abort_helper(
            &mut child,
            request.policy.shutdown_timeout(),
            linux_build::map_control_error(error, true),
        )
    })?;
    let response = linux_build_response::decode_bootstrap(&finished).map_err(|error| {
        linux_build::abort_helper(&mut child, request.policy.shutdown_timeout(), error)
    })?;
    let deadline = Instant::now() + request.policy.shutdown_timeout();
    let status = child
        .wait_for_exit(deadline, None, "wait-bootstrap-helper")
        .map_err(|error| linux_build::abort_helper_until(&mut child, deadline, error))?;
    Ok((ready, response.complete(status)?, observed))
}

fn send_capabilities(
    control: std::os::fd::BorrowedFd<'_>,
    request: &BuildRequest<'_>,
    bootstrap_payload: &[u8],
    deadline: Instant,
) -> IsolationResult<()> {
    linux_control::send(
        control,
        MessageKind::BootstrapDescriptors,
        bootstrap_payload,
        &[request.program.as_fd(), request.output_root.as_fd()],
        deadline,
        Some(request.cancellation),
    )
    .map_err(|error| linux_build::map_control_error(error, false))?;
    linux_build::send_input_files(control, request.input_files, deadline, request.cancellation)
}

fn receive_ready(
    child: &mut HelperProcess,
    request: &BuildRequest<'_>,
) -> IsolationResult<BootstrapReadyMessage> {
    let stdout = child.take_stdout()?;
    let timeout = crate::contract::CONTROLLED_BUILD_INPUT_SNAPSHOT_TIMEOUT
        .saturating_add(request.policy.startup_timeout());
    let line = read_protocol_line(
        stdout.as_fd(),
        Instant::now() + timeout,
        request.cancellation,
        &IsolationError::ControlledBuildSnapshotTimeout,
        "read-bootstrap-helper-protocol",
    )?;
    parse_bootstrap_ready(&line)
}

const fn ready_as_build(ready: &BootstrapReadyMessage) -> BuildReadyMessage {
    BuildReadyMessage {
        guardian_pid: ready.guardian_pid,
        namespace_init_pid: ready.namespace_init_pid,
        network_namespace: ready.network_namespace,
        user_namespace: ready.user_namespace,
        process_namespace: ready.process_namespace,
        mount_namespace: ready.mount_namespace,
        loopback_index: ready.loopback_index,
        landlock_abi: ready.landlock_abi,
    }
}

fn revalidate(
    request: &mut BuildRequest<'_>,
    metadata: &BuildObjectMetadata,
) -> IsolationResult<()> {
    linux_build::revalidate_object(request.program, &metadata.program)?;
    linux_build::revalidate_object(request.input_root, &metadata.input)?;
    for (input, expected) in request.input_files.iter().zip(&metadata.input_files) {
        super::linux_build_input::revalidate(input.file(), expected)?;
    }
    linux_build::revalidate_identity(request.output_root, &metadata.output)?;
    linux_build::verify_program(
        request.program,
        request.specification,
        request.policy.startup_timeout(),
        request.cancellation,
    )
}
