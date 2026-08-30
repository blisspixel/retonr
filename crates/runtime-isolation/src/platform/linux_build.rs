use std::{
    fs::{File, Metadata},
    io::{Read as _, Seek as _, SeekFrom},
    os::{fd::AsFd as _, unix::fs::MetadataExt as _},
    process::Stdio,
    time::{Duration, Instant},
};

use rewrite_types::{CancellationToken, Digest as DomainDigest};
use rustix::process::{Pid, PidfdFlags, pidfd_open};
use sha2::{Digest as _, Sha256};

use crate::{
    ControlledBuildExecution, ControlledBuildInputFile, ControlledBuildIsolationEvidence,
    ControlledBuildLaunchSpec, IsolationError, IsolationPolicy, IsolationPreparationEvidence,
    IsolationResult,
    contract::{CONTROLLED_BUILD_INPUT_SNAPSHOT_TIMEOUT, ControlledBuildIsolationObservation},
    error::native,
};

use super::{
    linux_build_protocol::{
        BUILD_ROOT_DESCRIPTOR_COUNT, encode_input_count, encode_input_declarations,
    },
    linux_build_response,
    linux_command::{apply_policy_environment, helper_command_with_input, spawn_helper},
    linux_control::{self, ControlError, MessageKind},
    linux_helper_identity::validate_executable,
    linux_process::{HelperProcess, read_protocol_line},
    linux_protocol::{BuildReadyMessage, parse_build_ready},
    linux_validation::{
        ensure_pidfd_alive, namespace_identity, open_namespace, privileges_are_reduced,
    },
};

const HASH_BUFFER_BYTES: usize = 64 * 1024;

pub(super) struct BuildRequest<'a> {
    pub(super) helper: &'a File,
    pub(super) preparation: &'a IsolationPreparationEvidence,
    pub(super) specification: &'a ControlledBuildLaunchSpec,
    pub(super) program: &'a mut File,
    pub(super) input_root: &'a File,
    pub(super) input_files: &'a [ControlledBuildInputFile],
    pub(super) output_root: &'a File,
    pub(super) policy: IsolationPolicy,
    pub(super) cancellation: &'a CancellationToken,
}

pub(super) struct BuildObjectMetadata {
    pub(super) program: Metadata,
    pub(super) input: Metadata,
    pub(super) input_files: Vec<Metadata>,
    pub(super) output: Metadata,
}

pub(super) fn run(mut request: BuildRequest<'_>) -> IsolationResult<ControlledBuildExecution> {
    let metadata = validate_objects(&mut request)?;
    let (ready, output, observed) = execute_helper(&request)?;
    revalidate_object(request.program, &metadata.program)?;
    revalidate_object(request.input_root, &metadata.input)?;
    for (input_file, expected) in request.input_files.iter().zip(&metadata.input_files) {
        super::linux_build_input::revalidate(input_file.file(), expected)?;
    }
    revalidate_identity(request.output_root, &metadata.output)?;
    verify_program(
        request.program,
        request.specification,
        request.policy.startup_timeout(),
        request.cancellation,
    )?;
    let observation = ControlledBuildIsolationObservation {
        guardian_pid: ready.guardian_pid,
        namespace_init_pid: ready.namespace_init_pid,
        network_namespace: ready.network_namespace,
        user_namespace: ready.user_namespace,
        process_namespace: ready.process_namespace,
        mount_namespace: ready.mount_namespace,
        landlock_abi: ready.landlock_abi,
        program_bytes: request.specification.program_bytes(),
        program_device: metadata.program.dev(),
        program_inode: metadata.program.ino(),
        input_device: metadata.input.dev(),
        input_inode: metadata.input.ino(),
        output_device: metadata.output.dev(),
        output_inode: metadata.output.ino(),
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
    Ok(ControlledBuildExecution::new(
        isolation,
        output,
        crate::ControlledBuildProcessStatus::Success,
    ))
}

fn validate_objects(request: &mut BuildRequest<'_>) -> IsolationResult<BuildObjectMetadata> {
    ensure_active(request.cancellation)?;
    validate_executable(request.program, false)?;
    let program = request
        .program
        .metadata()
        .map_err(|error| native("read-build-program-metadata", &error))?;
    let input = validate_directory(request.input_root)?;
    let output = validate_directory(request.output_root)?;
    if same_object(&input, &output) {
        return Err(IsolationError::ControlledBuildObjectMismatch);
    }
    let input_files = super::linux_build_input::validate(request, &program)?;
    verify_program(
        request.program,
        request.specification,
        request.policy.startup_timeout(),
        request.cancellation,
    )?;
    Ok(BuildObjectMetadata {
        program,
        input,
        input_files,
        output,
    })
}

pub(super) fn validate_retained_request(
    request: &mut BuildRequest<'_>,
) -> IsolationResult<BuildObjectMetadata> {
    validate_objects(request)
}

fn execute_helper(
    request: &BuildRequest<'_>,
) -> IsolationResult<(
    BuildReadyMessage,
    crate::ControlledBuildOutput,
    ObservedBuildNamespaces,
)> {
    let (control, child_control) =
        linux_control::pair().map_err(|_error| IsolationError::HelperProtocol)?;
    let mut command = helper_command_with_input(request.helper, Stdio::from(child_control));
    command
        .arg("--stage1-build")
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
    apply_target_environment_build(&mut command, request.specification);
    let mut child = spawn_helper(&mut command)?;
    drop(command);
    let startup_deadline = Instant::now() + request.policy.startup_timeout();
    send_build_capabilities(control.as_fd(), request, startup_deadline)
        .map_err(|error| abort_helper(&mut child, request.policy.shutdown_timeout(), error))?;
    let ready = receive_build_ready(
        &mut child,
        CONTROLLED_BUILD_INPUT_SNAPSHOT_TIMEOUT.saturating_add(request.policy.startup_timeout()),
        request.cancellation,
    )
    .map_err(|error| abort_helper(&mut child, request.policy.shutdown_timeout(), error))?;
    let observed = validate_ready(&child, ready)
        .map_err(|error| abort_helper(&mut child, request.policy.shutdown_timeout(), error))?;
    if let Err(error) = linux_control::send(
        control.as_fd(),
        MessageKind::BuildGo,
        &[],
        &[],
        Instant::now() + request.policy.startup_timeout(),
        Some(request.cancellation),
    ) {
        return Err(abort_helper(
            &mut child,
            request.policy.shutdown_timeout(),
            map_control_error(error, false),
        ));
    }
    let finished = linux_control::receive(
        control.as_fd(),
        Instant::now() + request.specification.execution_timeout(),
        Some(request.cancellation),
    )
    .map_err(|error| {
        abort_helper(
            &mut child,
            request.policy.shutdown_timeout(),
            map_control_error(error, true),
        )
    })?;
    let response = linux_build_response::decode(&finished)
        .map_err(|error| abort_helper(&mut child, request.policy.shutdown_timeout(), error))?;
    let shutdown_deadline = Instant::now() + request.policy.shutdown_timeout();
    let helper_status = child
        .wait_for_exit(shutdown_deadline, None, "wait-build-helper")
        .map_err(|error| abort_helper_until(&mut child, shutdown_deadline, error))?;
    let output = response.complete(helper_status)?;
    Ok((ready, output, observed))
}

fn send_build_capabilities(
    control: std::os::fd::BorrowedFd<'_>,
    request: &BuildRequest<'_>,
    deadline: Instant,
) -> IsolationResult<()> {
    let input_count = encode_input_count(request.input_files.len())
        .ok_or(IsolationError::ControlledBuildObjectMismatch)?;
    let descriptors: [std::os::fd::BorrowedFd<'_>; BUILD_ROOT_DESCRIPTOR_COUNT] =
        [request.program.as_fd(), request.output_root.as_fd()];
    linux_control::send(
        control,
        MessageKind::BuildDescriptors,
        &input_count,
        &descriptors,
        deadline,
        Some(request.cancellation),
    )
    .map_err(|error| map_control_error(error, false))?;
    send_input_files(control, request.input_files, deadline, request.cancellation)
}

pub(super) fn send_input_files(
    control: std::os::fd::BorrowedFd<'_>,
    input_files: &[ControlledBuildInputFile],
    deadline: Instant,
    cancellation: &CancellationToken,
) -> IsolationResult<()> {
    for batch in input_files.chunks(linux_control::MAX_RECEIVED_DESCRIPTORS) {
        let declarations = batch
            .iter()
            .map(|input| {
                (
                    input.relative_path(),
                    input.expected_digest(),
                    input.expected_bytes(),
                )
            })
            .collect::<Vec<_>>();
        let payload =
            encode_input_declarations(&declarations).ok_or(IsolationError::HelperProtocol)?;
        let descriptors = batch
            .iter()
            .map(|input| input.file().as_fd())
            .collect::<Vec<_>>();
        linux_control::send(
            control,
            MessageKind::BuildInputFiles,
            &payload,
            &descriptors,
            deadline,
            Some(cancellation),
        )
        .map_err(|error| map_control_error(error, false))?;
    }
    Ok(())
}

pub(super) fn apply_target_environment_build(
    command: &mut std::process::Command,
    specification: &ControlledBuildLaunchSpec,
) {
    command.env(
        "REWRITE_ISOLATION_INTERNAL_ENV_COUNT",
        specification.environment().len().to_string(),
    );
    for (index, (key, value)) in specification.environment().iter().enumerate() {
        command
            .env(format!("REWRITE_ISOLATION_INTERNAL_ENV_{index}_KEY"), key)
            .env(
                format!("REWRITE_ISOLATION_INTERNAL_ENV_{index}_VALUE"),
                value,
            );
    }
}

pub(super) fn verify_program(
    program: &mut File,
    specification: &ControlledBuildLaunchSpec,
    timeout: Duration,
    cancellation: &CancellationToken,
) -> IsolationResult<()> {
    let metadata = program
        .metadata()
        .map_err(|error| native("read-build-program-metadata", &error))?;
    if metadata.len() != specification.program_bytes() {
        return Err(IsolationError::ControlledBuildObjectMismatch);
    }
    program
        .seek(SeekFrom::Start(0))
        .map_err(|error| native("seek-build-program", &error))?;
    let started = Instant::now();
    let mut hasher = Sha256::new();
    let mut observed = 0_u64;
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
    loop {
        ensure_active(cancellation)?;
        if started.elapsed() >= timeout {
            return Err(IsolationError::StartupTimeout);
        }
        let read = program
            .read(&mut buffer)
            .map_err(|error| native("hash-build-program", &error))?;
        if read == 0 {
            break;
        }
        observed = observed
            .checked_add(u64::try_from(read).map_err(|_error| IsolationError::HelperProtocol)?)
            .ok_or(IsolationError::ControlledBuildObjectMismatch)?;
        if observed > specification.program_bytes() {
            return Err(IsolationError::ControlledBuildObjectMismatch);
        }
        hasher.update(&buffer[..read]);
    }
    let digest = DomainDigest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_error| IsolationError::ControlledBuildObjectMismatch)?;
    if observed == specification.program_bytes() && &digest == specification.program_digest() {
        Ok(())
    } else {
        Err(IsolationError::ControlledBuildObjectMismatch)
    }
}

pub(super) struct ObservedBuildNamespaces {
    _guardian: rustix::fd::OwnedFd,
    _namespace_init: rustix::fd::OwnedFd,
    _network: File,
    _user: File,
    _process: File,
    _mount: File,
}

pub(super) fn validate_ready(
    child: &HelperProcess,
    ready: BuildReadyMessage,
) -> IsolationResult<ObservedBuildNamespaces> {
    if ready.guardian_pid != child.id() || ready.loopback_index == 0 {
        return Err(IsolationError::HelperProtocol);
    }
    (|| {
        let guardian_pid = pid(ready.guardian_pid)?;
        let namespace_init_pid = pid(ready.namespace_init_pid)?;
        let guardian = pidfd_open(guardian_pid, PidfdFlags::empty())
            .map_err(|error| super::linux_validation::native_errno("open-build-guardian", error))?;
        let namespace_init =
            pidfd_open(namespace_init_pid, PidfdFlags::empty()).map_err(|error| {
                super::linux_validation::native_errno("open-build-namespace-init", error)
            })?;
        ensure_pidfd_alive(&guardian)?;
        ensure_pidfd_alive(&namespace_init)?;
        let network = open_namespace(ready.guardian_pid, "net")?;
        let user = open_namespace(ready.guardian_pid, "user")?;
        let process = open_namespace(ready.namespace_init_pid, "pid")?;
        let mount = open_namespace(ready.guardian_pid, "mnt")?;
        if namespace_identity(&network)? != ready.network_namespace
            || namespace_identity(&user)? != ready.user_namespace
            || namespace_identity(&process)? != ready.process_namespace
            || namespace_identity(&mount)? != ready.mount_namespace
            || !privileges_are_reduced(ready.guardian_pid)?
            || !privileges_are_reduced(ready.namespace_init_pid)?
        {
            return Err(IsolationError::EvidenceChanged);
        }
        Ok(ObservedBuildNamespaces {
            _guardian: guardian,
            _namespace_init: namespace_init,
            _network: network,
            _user: user,
            _process: process,
            _mount: mount,
        })
    })()
}

fn receive_build_ready(
    child: &mut HelperProcess,
    timeout: Duration,
    cancellation: &CancellationToken,
) -> IsolationResult<BuildReadyMessage> {
    let stdout = child.take_stdout()?;
    let line = read_protocol_line(
        stdout.as_fd(),
        Instant::now() + timeout,
        cancellation,
        &IsolationError::ControlledBuildSnapshotTimeout,
        "read-build-helper-protocol",
    )?;
    parse_build_ready(&line)
}

fn validate_directory(directory: &File) -> IsolationResult<Metadata> {
    let metadata = directory
        .metadata()
        .map_err(|error| native("read-build-directory", &error))?;
    if metadata.is_dir() {
        Ok(metadata)
    } else {
        Err(IsolationError::ControlledBuildObjectMismatch)
    }
}

pub(super) fn revalidate_object(file: &File, expected: &Metadata) -> IsolationResult<()> {
    let current = file
        .metadata()
        .map_err(|error| native("revalidate-build-object", &error))?;
    if same_object(&current, expected) && current.len() == expected.len() {
        Ok(())
    } else {
        Err(IsolationError::ControlledBuildObjectMismatch)
    }
}

pub(super) fn revalidate_identity(file: &File, expected: &Metadata) -> IsolationResult<()> {
    let current = file
        .metadata()
        .map_err(|error| native("revalidate-build-output", &error))?;
    if same_object(&current, expected) {
        Ok(())
    } else {
        Err(IsolationError::ControlledBuildObjectMismatch)
    }
}

fn same_object(left: &Metadata, right: &Metadata) -> bool {
    left.dev() == right.dev() && left.ino() == right.ino()
}

fn pid(value: u32) -> IsolationResult<Pid> {
    i32::try_from(value)
        .ok()
        .and_then(Pid::from_raw)
        .ok_or(IsolationError::HelperProtocol)
}

pub(super) fn map_control_error(error: ControlError, build: bool) -> IsolationError {
    match error {
        ControlError::Cancelled => IsolationError::Cancelled,
        ControlError::Deadline if build => IsolationError::ControlledBuildTimeout,
        ControlError::Deadline => IsolationError::StartupTimeout,
        ControlError::Closed | ControlError::Invalid | ControlError::Native => {
            IsolationError::HelperProtocol
        }
    }
}

fn ensure_active(cancellation: &CancellationToken) -> IsolationResult<()> {
    if cancellation.is_cancelled() {
        Err(IsolationError::Cancelled)
    } else {
        Ok(())
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
