use std::{
    env,
    ffi::OsString,
    fs::File,
    io::{BufRead as _, BufReader, Read as _, Write as _},
    os::{
        fd::{AsFd as _, AsRawFd as _, BorrowedFd},
        unix::fs::PermissionsExt as _,
    },
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use rustix::process::{Signal, getpid, getppid, set_parent_process_death_signal};

use super::{
    linux_control::{ControlError, MessageKind, pair, receive, send},
    linux_helper_channel::{serve_parent_control, serve_stage_control},
    linux_helper_setup::{
        HelperFailure, NamespaceEvidence, establish_isolation, privileges_are_fully_reduced,
        validate_descriptor_set,
    },
    linux_socket_policy::install_target_socket_policy,
    linux_startup::StartupDrains,
};

const INTERNAL_PREFIX: &str = "REWRITE_ISOLATION_INTERNAL_";
const HANDSHAKE_LIMIT: u64 = 32;

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
        Some("--stage2-probe") => stage_two_probe(&remaining),
        Some("--stage2-launch") => stage_two_launch(&remaining),
        _ => Err(HelperFailure::InvalidLaunch),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Mode {
    Probe,
    Launch,
}

fn stage_one(mode: Mode, arguments: &[OsString]) -> Result<i32, HelperFailure> {
    validate_mode_arguments(mode, arguments)?;
    validate_descriptor_set()?;
    arm_parent_death()?;
    match mode {
        Mode::Probe => stage_one_probe(),
        Mode::Launch => stage_one_launch(arguments),
    }
}

fn stage_one_probe() -> Result<i32, HelperFailure> {
    let established = establish_isolation(read_limits()?)?;
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
    legacy_spawn_handshake(&mut child, namespace_init_pid)?;
    let status = child.wait().map_err(|_| HelperFailure::NamespaceSetup)?;
    Ok(status.code().unwrap_or(1))
}

fn stage_one_launch(arguments: &[OsString]) -> Result<i32, HelperFailure> {
    let timeout = operation_timeout()?;
    let parent_control = std::io::stdin();
    let target = receive_launch_descriptor(parent_control.as_fd(), Instant::now() + timeout)?;
    let established = establish_isolation(read_limits()?)?;
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
    let mut child = command.spawn().map_err(|_| HelperFailure::NamespaceSetup)?;
    let namespace_init_pid = child.id();
    controlled_spawn_handshake(
        stage_control.as_fd(),
        target.as_fd(),
        namespace_init_pid,
        timeout,
    )?;
    drop(target);
    let started =
        receive(stage_control.as_fd(), Instant::now() + timeout, None).map_err(control_failure)?;
    if started.kind != MessageKind::TargetStarted
        || !started.descriptors.is_empty()
        || started.payload.len() != 48
    {
        return Err(HelperFailure::InvalidLaunch);
    }
    let evidence = decode_namespace_evidence(&started.payload)?;
    write_ready(
        guardian_pid,
        namespace_init_pid,
        evidence,
        established.loopback_index,
    );
    serve_parent_control(
        &mut child,
        parent_control.as_fd(),
        stage_control.as_fd(),
        timeout,
    )
}

fn receive_launch_descriptor(
    control: BorrowedFd<'_>,
    deadline: Instant,
) -> Result<File, HelperFailure> {
    let message = receive(control, deadline, None).map_err(control_failure)?;
    if message.kind != MessageKind::LaunchDescriptor
        || !message.payload.is_empty()
        || message.descriptors.len() != 1
    {
        return Err(HelperFailure::InvalidLaunch);
    }
    let descriptor = message
        .descriptors
        .into_iter()
        .next()
        .ok_or(HelperFailure::InvalidLaunch)?;
    let file = File::from(descriptor);
    validate_executable(&file)?;
    Ok(file)
}

fn controlled_spawn_handshake(
    control: BorrowedFd<'_>,
    target: BorrowedFd<'_>,
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
    send(
        control,
        MessageKind::Go,
        &namespace_init_pid.to_be_bytes(),
        &[target],
        deadline,
        None,
    )
    .map_err(control_failure)
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
    validate_reduced_privileges()?;
    install_target_socket_policy()?;
    let evidence = NamespaceEvidence::current()?;
    let guardian_pid = read_internal_u32("GUARDIAN_PID")?;
    let loopback_index = read_internal_u32("LOOPBACK_INDEX")?;
    write_ready(guardian_pid, namespace_init_pid, evidence, loopback_index);
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
    if go.kind != MessageKind::Go || go.payload.len() != 4 || go.descriptors.len() != 1 {
        return Err(HelperFailure::InvalidLaunch);
    }
    let namespace_init_pid = u32::from_be_bytes(
        go.payload
            .as_slice()
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
    validate_reduced_privileges()?;
    install_target_socket_policy()?;
    let evidence = NamespaceEvidence::current()?;
    launch_target(target, arguments, control.as_fd(), evidence, timeout)
}

fn validate_stage_two() -> Result<(), HelperFailure> {
    if getpid().as_raw_nonzero().get() != 1 {
        return Err(HelperFailure::NamespaceSetup);
    }
    validate_descriptor_set()?;
    set_parent_process_death_signal(Some(Signal::KILL)).map_err(|_| HelperFailure::NamespaceSetup)
}

fn validate_reduced_privileges() -> Result<(), HelperFailure> {
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
    evidence: NamespaceEvidence,
    timeout: Duration,
) -> Result<i32, HelperFailure> {
    let mut command = Command::new(format!("/proc/self/fd/{}", target.as_raw_fd()));
    command
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_clear();
    apply_target_environment(&mut command)?;
    if let Some(directory) = env::var_os(format!("{INTERNAL_PREFIX}CURRENT_DIRECTORY")) {
        if !Path::new(&directory).is_absolute() {
            return Err(HelperFailure::InvalidLaunch);
        }
        command.current_dir(directory);
    }
    let mut child = command.spawn().map_err(|_| HelperFailure::InvalidLaunch)?;
    drop(target);
    let output = child.stdout.take().ok_or(HelperFailure::InvalidLaunch)?;
    let error = child.stderr.take().ok_or(HelperFailure::InvalidLaunch)?;
    let drains = StartupDrains::start(output, error);
    send(
        control,
        MessageKind::TargetStarted,
        &encode_namespace_evidence(evidence),
        &[],
        Instant::now() + timeout,
        None,
    )
    .map_err(control_failure)?;
    serve_stage_control(&mut child, &drains, control, timeout)
}

fn legacy_spawn_handshake(child: &mut Child, namespace_init_pid: u32) -> Result<(), HelperFailure> {
    let stderr = child.stderr.take().ok_or(HelperFailure::NamespaceSetup)?;
    let mut armed = Vec::new();
    BufReader::new(stderr)
        .take(HANDSHAKE_LIMIT)
        .read_until(b'\n', &mut armed)
        .map_err(|_| HelperFailure::NamespaceSetup)?;
    if armed != b"ARMED 1\n" {
        return Err(HelperFailure::NamespaceSetup);
    }
    let mut stdin = child.stdin.take().ok_or(HelperFailure::NamespaceSetup)?;
    stdin
        .write_all(format!("GO 1 {namespace_init_pid}\n").as_bytes())
        .map_err(|_| HelperFailure::NamespaceSetup)?;
    drop(stdin);
    Ok(())
}

fn apply_target_environment(command: &mut Command) -> Result<(), HelperFailure> {
    let count = read_internal_usize("ENV_COUNT")?;
    if count > 1_024 {
        return Err(HelperFailure::InvalidLaunch);
    }
    for index in 0..count {
        let key = env::var_os(format!("{INTERNAL_PREFIX}ENV_{index}_KEY"))
            .ok_or(HelperFailure::InvalidLaunch)?;
        let value = env::var_os(format!("{INTERNAL_PREFIX}ENV_{index}_VALUE"))
            .ok_or(HelperFailure::InvalidLaunch)?;
        if key.is_empty()
            || key.as_encoded_bytes().contains(&0)
            || key.to_string_lossy().contains('=')
            || key.to_string_lossy().starts_with(INTERNAL_PREFIX)
            || value.as_encoded_bytes().contains(&0)
        {
            return Err(HelperFailure::InvalidLaunch);
        }
        command.env(key, value);
    }
    Ok(())
}

pub(super) fn validate_mode_arguments(
    mode: Mode,
    arguments: &[OsString],
) -> Result<(), HelperFailure> {
    match mode {
        Mode::Probe if arguments.is_empty() => Ok(()),
        Mode::Launch => Ok(()),
        Mode::Probe => Err(HelperFailure::InvalidLaunch),
    }
}

fn arm_parent_death() -> Result<(), HelperFailure> {
    let parent = getppid().ok_or(HelperFailure::NamespaceSetup)?;
    set_parent_process_death_signal(Some(Signal::KILL))
        .map_err(|_| HelperFailure::NamespaceSetup)?;
    if getppid() != Some(parent) {
        return Err(HelperFailure::NamespaceSetup);
    }
    Ok(())
}

fn read_go_message() -> Result<u32, HelperFailure> {
    let mut input = Vec::new();
    std::io::stdin()
        .lock()
        .take(HANDSHAKE_LIMIT)
        .read_until(b'\n', &mut input)
        .map_err(|_| HelperFailure::NamespaceSetup)?;
    let text = std::str::from_utf8(&input).map_err(|_| HelperFailure::NamespaceSetup)?;
    let fields = text.split_ascii_whitespace().collect::<Vec<_>>();
    if fields.len() != 3 || fields[0] != "GO" || fields[1] != "1" {
        return Err(HelperFailure::NamespaceSetup);
    }
    fields[2]
        .parse::<u32>()
        .ok()
        .filter(|pid| *pid > 0)
        .ok_or(HelperFailure::NamespaceSetup)
}

fn read_limits() -> Result<(u64, u64), HelperFailure> {
    Ok((
        read_internal_u64("MAX_OPEN_FILES")?,
        read_internal_u64("MAX_PROCESSES")?,
    ))
}

fn operation_timeout() -> Result<Duration, HelperFailure> {
    let milliseconds = read_internal_u64("STARTUP_TIMEOUT_MILLIS")?;
    if !(1..=30_000).contains(&milliseconds) {
        return Err(HelperFailure::InvalidLaunch);
    }
    Ok(Duration::from_millis(milliseconds))
}

fn read_internal_u32(name: &str) -> Result<u32, HelperFailure> {
    u32::try_from(read_internal_u64(name)?).map_err(|_| HelperFailure::InvalidLaunch)
}

fn read_internal_usize(name: &str) -> Result<usize, HelperFailure> {
    usize::try_from(read_internal_u64(name)?).map_err(|_| HelperFailure::InvalidLaunch)
}

fn read_internal_u64(name: &str) -> Result<u64, HelperFailure> {
    env::var(format!("{INTERNAL_PREFIX}{name}"))
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(HelperFailure::InvalidLaunch)
}

fn open_executable(path: &Path) -> Result<File, HelperFailure> {
    let file = File::open(path).map_err(|_| HelperFailure::InvalidLaunch)?;
    validate_executable(&file)?;
    Ok(file)
}

fn validate_executable(file: &File) -> Result<(), HelperFailure> {
    let metadata = file.metadata().map_err(|_| HelperFailure::InvalidLaunch)?;
    if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
        return Err(HelperFailure::InvalidLaunch);
    }
    Ok(())
}

pub(super) fn encode_namespace_evidence(evidence: NamespaceEvidence) -> [u8; 48] {
    let values = [
        evidence.network.device,
        evidence.network.inode,
        evidence.user.device,
        evidence.user.inode,
        evidence.process.device,
        evidence.process.inode,
    ];
    let mut encoded = [0_u8; 48];
    for (index, value) in values.into_iter().enumerate() {
        let start = index * 8;
        encoded[start..start + 8].copy_from_slice(&value.to_be_bytes());
    }
    encoded
}

pub(super) fn decode_namespace_evidence(
    payload: &[u8],
) -> Result<NamespaceEvidence, HelperFailure> {
    if payload.len() != 48 {
        return Err(HelperFailure::InvalidLaunch);
    }
    let mut values = [0_u64; 6];
    for (index, value) in values.iter_mut().enumerate() {
        let start = index * 8;
        *value = u64::from_be_bytes(
            payload[start..start + 8]
                .try_into()
                .map_err(|_| HelperFailure::InvalidLaunch)?,
        );
    }
    Ok(NamespaceEvidence {
        network: super::linux_helper_setup::RawNamespaceIdentity {
            device: values[0],
            inode: values[1],
        },
        user: super::linux_helper_setup::RawNamespaceIdentity {
            device: values[2],
            inode: values[3],
        },
        process: super::linux_helper_setup::RawNamespaceIdentity {
            device: values[4],
            inode: values[5],
        },
    })
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

fn write_ready(
    guardian_pid: u32,
    namespace_init_pid: u32,
    evidence: NamespaceEvidence,
    loopback_index: u32,
) {
    write_protocol(&format!(
        "READY 1 {guardian_pid} {namespace_init_pid} {} {} {} {} {} {} {loopback_index}\n",
        evidence.network.device,
        evidence.network.inode,
        evidence.user.device,
        evidence.user.inode,
        evidence.process.device,
        evidence.process.inode,
    ));
}

fn write_protocol(message: &str) {
    let mut output = std::io::stdout().lock();
    let _ = output.write_all(message.as_bytes());
    let _ = output.flush();
}
