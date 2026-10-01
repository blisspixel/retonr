use std::{
    env,
    ffi::OsString,
    fs::File,
    os::fd::{AsFd as _, AsRawFd as _, BorrowedFd},
    os::unix::fs::MetadataExt as _,
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use rustix::process::getpid;

use crate::{CONTROLLED_BUILD_INPUT_SNAPSHOT_TIMEOUT, ControlledBuildInputFile};

use super::{
    linux_build_mount, linux_build_output,
    linux_build_protocol::{
        BUILD_ROOT_DESCRIPTOR_COUNT, decode_input_count, decode_input_declarations,
        decode_process_finished, encode_armed, encode_finished, encode_helper_failure,
        encode_process_finished, require_armed,
    },
    linux_build_sandbox,
    linux_build_target::PreparedBuildTarget,
    linux_control::{MessageKind, pair, receive, send},
    linux_helper::{Mode, validate_reduced_privileges, validate_stage_two},
    linux_helper_setup::{
        HelperFailure, NamespaceEvidence, establish_build_isolation, mount_namespace_identity,
        validate_descriptor_set,
    },
    linux_helper_support::{
        arm_parent_death, build_timeout, control_failure, open_executable, operation_timeout,
        read_limits, read_target_environment, validate_mode_arguments, write_build_ready,
    },
    linux_socket_policy::install_build_network_policy,
};

const INTERNAL_PREFIX: &str = "REWRITE_ISOLATION_INTERNAL_";

struct BuildCapabilities {
    program: File,
    host_output: File,
    input_files: Vec<ControlledBuildInputFile>,
}

pub(super) fn stage_one(arguments: &[OsString]) -> Result<i32, HelperFailure> {
    validate_mode_arguments(Mode::Build, arguments)?;
    validate_descriptor_set()?;
    arm_parent_death()?;
    let startup_timeout = operation_timeout()?;
    let build_timeout = build_timeout()?;
    let parent_control = std::io::stdin();
    let receive_deadline = Instant::now() + startup_timeout;
    let capabilities = receive_build_capabilities(parent_control.as_fd(), receive_deadline)?;
    let relative_path = env::var(format!("{INTERNAL_PREFIX}BUILD_PROGRAM_RELATIVE_PATH"))
        .map_err(|_| HelperFailure::InvalidLaunch)?;
    require_mapped_program(&capabilities, &relative_path)?;
    let snapshot_deadline = Instant::now() + CONTROLLED_BUILD_INPUT_SNAPSHOT_TIMEOUT;
    let established = establish_build_isolation(
        read_limits()?,
        &capabilities.host_output,
        &capabilities.input_files,
        snapshot_deadline,
    )?;
    let private_input = linux_build_mount::open_private_input()?;
    let snapshot_program = linux_build_mount::open_private_regular(&relative_path)?;
    drop(capabilities.input_files);
    drop(capabilities.program);
    let guardian_pid = u32::try_from(getpid().as_raw_nonzero().get())
        .map_err(|_| HelperFailure::NamespaceSetup)?;
    let helper = open_executable(Path::new("/proc/self/exe"))?;
    let (stage_control, child_control) = pair().map_err(control_failure)?;
    let mut command = Command::new(format!("/proc/self/fd/{}", helper.as_raw_fd()));
    command
        .arg("--stage2-build")
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
    let deadline = Instant::now() + startup_timeout;
    let armed = receive(stage_control.as_fd(), deadline, None).map_err(control_failure)?;
    if armed.kind != MessageKind::Armed
        || !armed.payload.is_empty()
        || !armed.descriptors.is_empty()
    {
        return Err(HelperFailure::InvalidLaunch);
    }
    send(
        stage_control.as_fd(),
        MessageKind::Go,
        &namespace_init_pid.to_be_bytes(),
        &[snapshot_program.as_fd(), private_input.as_fd()],
        deadline,
        None,
    )
    .map_err(control_failure)?;
    let armed = receive(stage_control.as_fd(), deadline, None).map_err(control_failure)?;
    if armed.kind != MessageKind::BuildArmed || !armed.descriptors.is_empty() {
        return Err(HelperFailure::InvalidLaunch);
    }
    let (evidence, mount, landlock_abi) = require_armed(&armed.payload)?;
    write_build_ready(
        guardian_pid,
        namespace_init_pid,
        evidence,
        mount,
        established.loopback_index,
        landlock_abi,
    );
    let result = serve_parent(
        &mut child,
        parent_control.as_fd(),
        stage_control.as_fd(),
        &capabilities.host_output,
        startup_timeout,
        build_timeout,
    );
    if let Err(failure) = result {
        report_parent_failure(parent_control.as_fd(), failure, startup_timeout);
    }
    result
}

fn report_parent_failure(
    parent_control: BorrowedFd<'_>,
    failure: HelperFailure,
    timeout: Duration,
) {
    let _ = send(
        parent_control,
        MessageKind::Error,
        &encode_helper_failure(failure),
        &[],
        Instant::now() + timeout,
        None,
    );
}

fn receive_build_capabilities(
    control: BorrowedFd<'_>,
    deadline: Instant,
) -> Result<BuildCapabilities, HelperFailure> {
    let message = receive(control, deadline, None).map_err(control_failure)?;
    if message.kind != MessageKind::BuildDescriptors
        || message.descriptors.len() != BUILD_ROOT_DESCRIPTOR_COUNT
    {
        return Err(HelperFailure::InvalidLaunch);
    }
    let input_count = decode_input_count(&message.payload).ok_or(HelperFailure::InvalidLaunch)?;
    let mut roots = message.descriptors.into_iter().map(File::from);
    let program = roots.next().ok_or(HelperFailure::InvalidLaunch)?;
    let host_output = roots.next().ok_or(HelperFailure::InvalidLaunch)?;
    if roots.next().is_some() {
        return Err(HelperFailure::InvalidLaunch);
    }
    let mut input_files = Vec::with_capacity(input_count);
    while input_files.len() < input_count {
        let message = receive(control, deadline, None).map_err(control_failure)?;
        if message.kind != MessageKind::BuildInputFiles {
            return Err(HelperFailure::InvalidLaunch);
        }
        let declarations =
            decode_input_declarations(&message.payload).ok_or(HelperFailure::InvalidLaunch)?;
        if declarations.len() != message.descriptors.len()
            || declarations.len() > input_count.saturating_sub(input_files.len())
        {
            return Err(HelperFailure::InvalidLaunch);
        }
        for (declaration, descriptor) in declarations.into_iter().zip(message.descriptors) {
            input_files.push(
                ControlledBuildInputFile::new(
                    declaration.relative_path,
                    declaration.expected_digest,
                    declaration.expected_bytes,
                    File::from(descriptor),
                )
                .map_err(|_| HelperFailure::InvalidLaunch)?,
            );
        }
    }
    Ok(BuildCapabilities {
        program,
        host_output,
        input_files,
    })
}

fn require_mapped_program(
    capabilities: &BuildCapabilities,
    relative_path: &str,
) -> Result<(), HelperFailure> {
    let program = capabilities
        .program
        .metadata()
        .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?;
    let mapped = capabilities
        .input_files
        .iter()
        .find(|input| input.relative_path() == relative_path)
        .ok_or(HelperFailure::ControlledBuildObjectMismatch)?;
    let retained = mapped
        .file()
        .metadata()
        .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?;
    if program.is_file()
        && retained.is_file()
        && program.nlink() == 1
        && retained.nlink() == 1
        && program.dev() == retained.dev()
        && program.ino() == retained.ino()
        && program.len() == retained.len()
    {
        Ok(())
    } else {
        Err(HelperFailure::ControlledBuildObjectMismatch)
    }
}

fn serve_parent(
    child: &mut Child,
    parent_control: BorrowedFd<'_>,
    stage_control: BorrowedFd<'_>,
    host_output: &File,
    startup_timeout: Duration,
    build_timeout: Duration,
) -> Result<i32, HelperFailure> {
    let go =
        receive(parent_control, Instant::now() + startup_timeout, None).map_err(control_failure)?;
    if go.kind != MessageKind::BuildGo || !go.payload.is_empty() || !go.descriptors.is_empty() {
        return Err(HelperFailure::InvalidLaunch);
    }
    send(
        stage_control,
        MessageKind::BuildGo,
        &[],
        &[],
        Instant::now() + startup_timeout,
        None,
    )
    .map_err(control_failure)?;
    let finished =
        receive(stage_control, Instant::now() + build_timeout, None).map_err(control_failure)?;
    if finished.kind != MessageKind::BuildFinished || !finished.descriptors.is_empty() {
        return Err(HelperFailure::InvalidLaunch);
    }
    let mut output =
        decode_process_finished(&finished.payload).ok_or(HelperFailure::InvalidLaunch)?;
    let status = child.wait().map_err(|_| HelperFailure::InvalidLaunch)?;
    if !status.success() {
        return Err(HelperFailure::InvalidLaunch);
    }
    if output.status() == crate::ControlledBuildProcessStatus::Success {
        let tree = linux_build_output::inspect_private_output()?;
        linux_build_output::publish_private_output(host_output, &tree)?;
        output = output.with_tree(tree);
    }
    let encoded = encode_finished(&output).ok_or(HelperFailure::InvalidLaunch)?;
    send(
        parent_control,
        MessageKind::BuildFinished,
        &encoded,
        &[],
        Instant::now() + startup_timeout,
        None,
    )
    .map_err(control_failure)?;
    Ok(0)
}

pub(super) fn stage_two(arguments: &[OsString]) -> Result<i32, HelperFailure> {
    validate_mode_arguments(Mode::Build, arguments)?;
    validate_stage_two()?;
    let startup_timeout = operation_timeout()?;
    let build_timeout = build_timeout()?;
    let control = std::io::stdin();
    let deadline = Instant::now() + startup_timeout;
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
    if go.kind != MessageKind::Go || go.payload.len() != 4 || go.descriptors.len() != 2 {
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
    let mut descriptors = go.descriptors.into_iter().map(File::from);
    let program = descriptors.next().ok_or(HelperFailure::InvalidLaunch)?;
    let input_root = descriptors.next().ok_or(HelperFailure::InvalidLaunch)?;
    if descriptors.next().is_some() {
        return Err(HelperFailure::InvalidLaunch);
    }
    let relative_path = env::var(format!("{INTERNAL_PREFIX}BUILD_PROGRAM_RELATIVE_PATH"))
        .map_err(|_| HelperFailure::InvalidLaunch)?;
    let private_output = linux_build_mount::open_private_output()?;
    linux_build_sandbox::validate_objects(&program, &input_root, &private_output, &relative_path)?;
    linux_build_mount::validate_private(&input_root)?;
    validate_reduced_privileges()?;
    install_build_network_policy()?;
    let environment = read_target_environment()?;
    let target = PreparedBuildTarget::prepare(
        &program,
        &input_root,
        &private_output,
        arguments,
        &environment,
    )?;
    let evidence = NamespaceEvidence::current()?;
    let mount = mount_namespace_identity()?;
    let landlock_abi =
        linux_build_sandbox::install_and_probe(&input_root, &private_output, &relative_path)?;
    send(
        control.as_fd(),
        MessageKind::BuildArmed,
        &encode_armed(evidence, mount, landlock_abi),
        &[],
        Instant::now() + startup_timeout,
        None,
    )
    .map_err(control_failure)?;
    let go = receive(control.as_fd(), Instant::now() + startup_timeout, None)
        .map_err(control_failure)?;
    if go.kind != MessageKind::BuildGo || !go.payload.is_empty() || !go.descriptors.is_empty() {
        return Err(HelperFailure::InvalidLaunch);
    }
    let output = target.run()?;
    let encoded = encode_process_finished(&output).ok_or(HelperFailure::InvalidLaunch)?;
    send(
        control.as_fd(),
        MessageKind::BuildFinished,
        &encoded,
        &[],
        Instant::now() + build_timeout,
        None,
    )
    .map_err(control_failure)?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use std::{os::fd::AsFd as _, time::Duration};

    use super::{HelperFailure, MessageKind, pair, receive, report_parent_failure};

    #[test]
    fn late_failure_is_sent_to_the_parent_control_socket() {
        let (helper, parent) = pair().expect("control pair");
        report_parent_failure(
            helper.as_fd(),
            HelperFailure::FilesystemAliasOutput,
            Duration::from_secs(1),
        );
        let received = receive(
            parent.as_fd(),
            std::time::Instant::now() + Duration::from_secs(1),
            None,
        )
        .expect("late failure");
        assert_eq!(received.kind, MessageKind::Error);
        assert_eq!(
            received.payload,
            super::encode_helper_failure(HelperFailure::FilesystemAliasOutput)
        );
        assert!(received.descriptors.is_empty());
    }
}
