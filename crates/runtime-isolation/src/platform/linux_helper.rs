use std::{
    env,
    ffi::{OsStr, OsString},
    fs::File,
    io::Write as _,
    os::fd::{AsFd as _, AsRawFd as _, BorrowedFd},
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use rustix::process::{Signal, getpid, set_parent_process_death_signal};

use super::{
    linux_control::{MessageKind, pair, receive, send},
    linux_fd_exec::retained_fd_command,
    linux_helper_channel::{serve_parent_control, serve_stage_control},
    linux_helper_setup::{
        HelperFailure, drop_managed_privileges, establish_isolation, privileges_are_fully_reduced,
        validate_descriptor_set,
    },
    linux_helper_support::{
        arm_parent_death, control_failure, legacy_spawn_handshake, open_executable,
        operation_timeout, read_go_message, read_internal_u32, read_limits,
        read_target_environment, validate_executable, write_protocol, write_ready,
    },
    linux_managed_input_protocol::{
        MANAGED_INPUT_HEADER_BYTES, MANAGED_INPUT_VERIFICATION_TIMEOUT, decode_declaration,
        decode_header, encode_declaration, encode_header, header, validate_complete,
        validate_descriptor,
    },
    linux_managed_mount::{mount_private_proc, observe_current},
    linux_managed_protocol::{MANAGED_EVIDENCE_BYTES, ManagedReadyEvidence, decode, encode},
    linux_socket_policy::install_managed_target_policy,
    linux_startup::StartupDrains,
};
use crate::contract::{RetainedRuntimeInputDeclaration, RetainedRuntimeInputMember};

const INTERNAL_PREFIX: &str = "REWRITE_ISOLATION_INTERNAL_";

pub(super) use super::linux_helper_support::{
    decode_namespace_evidence, encode_namespace_evidence, validate_mode_arguments,
};

pub(crate) fn run() -> i32 {
    match run_inner() {
        Ok(code) => code,
        Err(failure) => {
            write_protocol(&format!("ERROR 1 {}\n", failure.code()));
            70
        }
    }
}

fn run_inner() -> Result<i32, HelperFailure> {
    let mut arguments = env::args_os();
    let _program = arguments.next();
    let mode = arguments.next().ok_or(HelperFailure::InvalidLaunch)?;
    let remaining = arguments.collect::<Vec<_>>();
    match mode.to_str() {
        Some("--stage1-probe") => stage_one(Mode::Probe, &remaining),
        Some("--stage1-launch") => stage_one(Mode::Launch, &remaining),
        Some("--stage1-build") => super::linux_helper_build::stage_one(&remaining),
        Some("--stage1-bootstrap") => super::linux_helper_bootstrap::stage_one(&remaining),
        Some("--stage2-probe") => stage_two_probe(&remaining),
        Some("--stage2-launch") => stage_two_launch(&remaining),
        Some("--stage2-build") => super::linux_helper_build::stage_two(&remaining),
        Some("--stage2-bootstrap") => super::linux_helper_bootstrap::stage_two(&remaining),
        _ => Err(HelperFailure::InvalidLaunch),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Mode {
    Probe,
    Launch,
    Build,
    Bootstrap,
}

fn stage_one(mode: Mode, arguments: &[OsString]) -> Result<i32, HelperFailure> {
    validate_mode_arguments(mode, arguments)?;
    validate_descriptor_set()?;
    arm_parent_death()?;
    match mode {
        Mode::Probe => stage_one_probe(),
        Mode::Launch => stage_one_launch(arguments),
        Mode::Build | Mode::Bootstrap => Err(HelperFailure::InvalidLaunch),
    }
}

fn stage_one_probe() -> Result<i32, HelperFailure> {
    let established = establish_isolation(read_limits()?, &[])?;
    let helper = open_executable(Path::new("/proc/self/exe"))?;
    let mut command = Command::new(format!("/proc/self/fd/{}", helper.as_raw_fd()));
    command
        .arg("--stage2-probe")
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::piped())
        .env(
            format!("{INTERNAL_PREFIX}GUARDIAN_PID"),
            getpid().as_raw_nonzero().get().to_string(),
        )
        .env(
            format!("{INTERNAL_PREFIX}LOOPBACK_INDEX"),
            established.loopback_index.to_string(),
        );
    let mut child = command.spawn().map_err(|_| HelperFailure::NamespaceSetup)?;
    let namespace_init_pid = child.id();
    drop_managed_privileges()?;
    legacy_spawn_handshake(&mut child, namespace_init_pid)?;
    let status = child.wait().map_err(|_| HelperFailure::NamespaceSetup)?;
    Ok(status.code().unwrap_or(1))
}

fn stage_one_launch(arguments: &[OsString]) -> Result<i32, HelperFailure> {
    let startup_timeout = operation_timeout()?;
    let parent_control = std::io::stdin();
    let launch = receive_managed_launch(parent_control.as_fd(), Instant::now() + startup_timeout)?;
    let timeout = if launch.inputs.is_empty() {
        startup_timeout
    } else {
        MANAGED_INPUT_VERIFICATION_TIMEOUT
    };
    let established = establish_isolation(read_limits()?, &launch.inputs)?;
    let guardian_pid = u32::try_from(getpid().as_raw_nonzero().get())
        .map_err(|_| HelperFailure::NamespaceSetup)?;
    let helper = open_executable(Path::new("/proc/self/exe"))?;
    let (stage_control, child_control) = pair().map_err(control_failure)?;
    let mut command = Command::new(format!("/proc/self/fd/{}", helper.as_raw_fd()));
    command
        .arg("--stage2-launch")
        .args(arguments)
        .stdin(Stdio::from(child_control))
        .stdout(Stdio::inherit())
        .stderr(Stdio::null())
        .env(
            format!("{INTERNAL_PREFIX}GUARDIAN_PID"),
            guardian_pid.to_string(),
        )
        .env(
            format!("{INTERNAL_PREFIX}LOOPBACK_INDEX"),
            established.loopback_index.to_string(),
        );
    let mut child = super::linux_helper_support::spawn_control_child(command)?;
    let namespace_init_pid = child.id();
    drop_managed_privileges()?;
    managed_spawn_handshake(stage_control.as_fd(), &launch, namespace_init_pid, timeout)?;
    drop(launch);
    let started =
        receive(stage_control.as_fd(), Instant::now() + timeout, None).map_err(control_failure)?;
    if started.kind != MessageKind::TargetStarted
        || !started.descriptors.is_empty()
        || started.payload.len() != MANAGED_EVIDENCE_BYTES
    {
        return Err(HelperFailure::InvalidLaunch);
    }
    let evidence = decode(&started.payload)?;
    write_ready(
        guardian_pid,
        namespace_init_pid,
        &evidence,
        established.loopback_index,
    );
    serve_parent_control(
        &mut child,
        parent_control.as_fd(),
        stage_control.as_fd(),
        timeout,
    )
}

struct ManagedLaunch {
    target: File,
    inputs: Vec<RetainedRuntimeInputMember>,
}

fn receive_managed_launch(
    control: BorrowedFd<'_>,
    deadline: Instant,
) -> Result<ManagedLaunch, HelperFailure> {
    let message = receive(control, deadline, None).map_err(control_failure)?;
    if message.kind != MessageKind::LaunchDescriptor || message.descriptors.len() != 1 {
        return Err(HelperFailure::InvalidLaunch);
    }
    let input_header = decode_header(&message.payload)?;
    let deadline = if input_header.count == 0 {
        deadline
    } else {
        Instant::now() + MANAGED_INPUT_VERIFICATION_TIMEOUT
    };
    let descriptor = message
        .descriptors
        .into_iter()
        .next()
        .ok_or(HelperFailure::InvalidLaunch)?;
    let file = File::from(descriptor);
    validate_executable(&file)?;
    let mut inputs = Vec::with_capacity(input_header.count);
    for index in 0..input_header.count {
        let message = receive(control, deadline, None).map_err(control_failure)?;
        if message.kind != MessageKind::ManagedInputDescriptor || message.descriptors.len() != 1 {
            return Err(HelperFailure::InvalidLaunch);
        }
        let declaration = decode_declaration(&message.payload, index)?;
        if inputs
            .last()
            .is_some_and(|input: &RetainedRuntimeInputMember| {
                input.declaration.relative_alias >= declaration.relative_alias
            })
        {
            return Err(HelperFailure::InvalidLaunch);
        }
        let input_file = File::from(
            message
                .descriptors
                .into_iter()
                .next()
                .ok_or(HelperFailure::InvalidLaunch)?,
        );
        validate_descriptor(&declaration, &input_file, None)?;
        inputs.push(RetainedRuntimeInputMember {
            declaration,
            file: input_file,
        });
    }
    let declarations = inputs
        .iter()
        .map(|input| input.declaration.clone())
        .collect::<Vec<_>>();
    validate_complete(&input_header, &declarations)?;
    Ok(ManagedLaunch {
        target: file,
        inputs,
    })
}

fn stage_two_probe(arguments: &[OsString]) -> Result<i32, HelperFailure> {
    validate_mode_arguments(Mode::Probe, arguments)?;
    validate_stage_two()?;
    let mut stderr = std::io::stderr().lock();
    stderr
        .write_all(b"ARMED 1\n")
        .map_err(|_| HelperFailure::NamespaceSetup)?;
    stderr.flush().map_err(|_| HelperFailure::NamespaceSetup)?;
    let namespace_init_pid = read_go_message()?;
    let evidence = complete_managed_setup(&[])?;
    let guardian_pid = read_internal_u32("GUARDIAN_PID")?;
    let loopback_index = read_internal_u32("LOOPBACK_INDEX")?;
    write_ready(guardian_pid, namespace_init_pid, &evidence, loopback_index);
    Ok(0)
}

fn stage_two_launch(arguments: &[OsString]) -> Result<i32, HelperFailure> {
    validate_mode_arguments(Mode::Launch, arguments)?;
    validate_stage_two()?;
    let timeout = operation_timeout()?;
    let control = std::io::stdin();
    let deadline = Instant::now() + timeout;
    send(
        control.as_fd(),
        MessageKind::Armed,
        &[],
        &[],
        deadline,
        None,
    )
    .map_err(control_failure)?;
    let go = receive(control.as_fd(), deadline, None).map_err(control_failure)?;
    if go.kind != MessageKind::Go
        || go.payload.len() != 4 + MANAGED_INPUT_HEADER_BYTES
        || go.descriptors.len() != 1
    {
        return Err(HelperFailure::InvalidLaunch);
    }
    let namespace_init_pid = u32::from_be_bytes(
        go.payload[..4]
            .try_into()
            .map_err(|_| HelperFailure::InvalidLaunch)?,
    );
    if namespace_init_pid == 0 {
        return Err(HelperFailure::InvalidLaunch);
    }
    let target = File::from(
        go.descriptors
            .into_iter()
            .next()
            .ok_or(HelperFailure::InvalidLaunch)?,
    );
    validate_executable(&target)?;
    let input_header = decode_header(&go.payload[4..])?;
    let input_deadline = if input_header.count == 0 {
        deadline
    } else {
        Instant::now() + MANAGED_INPUT_VERIFICATION_TIMEOUT
    };
    let inputs = receive_stage_inputs(control.as_fd(), input_deadline, &input_header)?;
    let declarations = inputs
        .iter()
        .map(|input| input.declaration.clone())
        .collect::<Vec<_>>();
    drop(inputs);
    let evidence = complete_managed_setup(&declarations)?;
    launch_target(target, arguments, control.as_fd(), &evidence, timeout)
}

fn complete_managed_setup(
    inputs: &[RetainedRuntimeInputDeclaration],
) -> Result<ManagedReadyEvidence, HelperFailure> {
    mount_private_proc()?;
    validate_descriptor_set()?;
    drop_managed_privileges()?;
    validate_reduced_privileges()?;
    install_managed_target_policy()?;
    ManagedReadyEvidence::current(
        observe_current()?,
        super::linux_managed_input_mount::observe(inputs)?,
    )
}

fn managed_spawn_handshake(
    control: BorrowedFd<'_>,
    launch: &ManagedLaunch,
    namespace_init_pid: u32,
    timeout: Duration,
) -> Result<(), HelperFailure> {
    let deadline = Instant::now() + timeout;
    let armed = receive(control, deadline, None).map_err(control_failure)?;
    if armed.kind != MessageKind::Armed
        || !armed.payload.is_empty()
        || !armed.descriptors.is_empty()
    {
        return Err(HelperFailure::NamespaceSetup);
    }
    let declarations = launch
        .inputs
        .iter()
        .map(|input| input.declaration.clone())
        .collect::<Vec<_>>();
    let input_header = header(&declarations)?;
    let mut payload = Vec::with_capacity(4 + MANAGED_INPUT_HEADER_BYTES);
    payload.extend_from_slice(&namespace_init_pid.to_be_bytes());
    payload.extend_from_slice(&encode_header(&input_header));
    send(
        control,
        MessageKind::Go,
        &payload,
        &[launch.target.as_fd()],
        deadline,
        None,
    )
    .map_err(control_failure)?;
    for (index, input) in launch.inputs.iter().enumerate() {
        send(
            control,
            MessageKind::ManagedInputDescriptor,
            &encode_declaration(index, &input.declaration)?,
            &[input.file.as_fd()],
            deadline,
            None,
        )
        .map_err(control_failure)?;
    }
    Ok(())
}

fn receive_stage_inputs(
    control: BorrowedFd<'_>,
    deadline: Instant,
    expected: &super::linux_managed_input_protocol::ManagedInputHeader,
) -> Result<Vec<RetainedRuntimeInputMember>, HelperFailure> {
    let mut inputs = Vec::with_capacity(expected.count);
    for index in 0..expected.count {
        let message = receive(control, deadline, None).map_err(control_failure)?;
        if message.kind != MessageKind::ManagedInputDescriptor || message.descriptors.len() != 1 {
            return Err(HelperFailure::InvalidLaunch);
        }
        let declaration = decode_declaration(&message.payload, index)?;
        let file = File::from(
            message
                .descriptors
                .into_iter()
                .next()
                .ok_or(HelperFailure::InvalidLaunch)?,
        );
        validate_descriptor(&declaration, &file, None)?;
        inputs.push(RetainedRuntimeInputMember { declaration, file });
    }
    let declarations = inputs
        .iter()
        .map(|input| input.declaration.clone())
        .collect::<Vec<_>>();
    validate_complete(expected, &declarations)?;
    Ok(inputs)
}

pub(super) fn validate_stage_two() -> Result<(), HelperFailure> {
    if getpid().as_raw_nonzero().get() != 1 {
        return Err(HelperFailure::NamespaceSetup);
    }
    validate_descriptor_set()?;
    set_parent_process_death_signal(Some(Signal::KILL)).map_err(|_| HelperFailure::NamespaceSetup)
}

pub(super) fn validate_reduced_privileges() -> Result<(), HelperFailure> {
    if privileges_are_fully_reduced()? {
        Ok(())
    } else {
        Err(HelperFailure::PrivilegeDrop)
    }
}

fn launch_target(
    target: File,
    arguments: &[OsString],
    control: BorrowedFd<'_>,
    evidence: &ManagedReadyEvidence,
    timeout: Duration,
) -> Result<i32, HelperFailure> {
    let environment = read_target_environment()?;
    let mut command = retained_fd_command(
        &target,
        OsStr::new("retonr-managed-target"),
        arguments,
        &environment,
    )?;
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(directory) = env::var_os(format!("{INTERNAL_PREFIX}CURRENT_DIRECTORY")) {
        if !Path::new(&directory).is_absolute() {
            return Err(HelperFailure::InvalidLaunch);
        }
        command.current_dir(directory);
    }
    let mut child = command.spawn().map_err(|_| HelperFailure::InvalidLaunch)?;
    drop(target);
    drop(command);
    let output = child.stdout.take().ok_or(HelperFailure::InvalidLaunch)?;
    let error = child.stderr.take().ok_or(HelperFailure::InvalidLaunch)?;
    let drains = StartupDrains::start(output, error);
    send(
        control,
        MessageKind::TargetStarted,
        &encode(evidence),
        &[],
        Instant::now() + timeout,
        None,
    )
    .map_err(control_failure)?;
    serve_stage_control(&mut child, &drains, control, timeout)
}
