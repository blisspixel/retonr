use std::{
    env,
    ffi::OsString,
    fs::File,
    os::{
        fd::{AsFd as _, BorrowedFd},
        unix::fs::MetadataExt as _,
    },
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use rustix::{
    fs::{Mode as FileMode, OFlags, ResolveFlags, openat2},
    process::getpid,
};

use crate::ControlledBuildInputFile;

use super::{
    linux_bootstrap_protocol::{
        BOOTSTRAP_ROOT_DESCRIPTOR_COUNT, BootstrapRequest, decode as decode_request,
    },
    linux_bootstrap_root,
    linux_bootstrap_target::PreparedBootstrapTarget,
    linux_build_mount::require_read_only_mount,
    linux_build_output,
    linux_build_protocol::{InputDeclaration, decode_input_declarations},
    linux_build_protocol::{
        decode_process_finished, encode_bootstrap_armed, encode_finished, encode_helper_failure,
        encode_process_finished, require_bootstrap_armed,
    },
    linux_build_sandbox,
    linux_control::{MessageKind, receive},
    linux_control::{pair, send},
    linux_helper::{Mode, validate_reduced_privileges, validate_stage_two},
    linux_helper_setup::{
        HelperFailure, begin_bootstrap_isolation, bootstrap_capability_limit,
        drop_bootstrap_guardian_privileges, drop_bootstrap_namespace_init_privileges,
        prepare_bootstrap_child_namespace, validate_descriptor_set,
    },
    linux_helper_support::{
        arm_parent_death, build_timeout, control_failure, open_executable, operation_timeout,
        read_limits, read_target_environment, validate_executable, validate_mode_arguments,
        write_bootstrap_ready,
    },
    linux_socket_policy::install_build_network_policy,
};

const INTERNAL_PREFIX: &str = "REWRITE_ISOLATION_INTERNAL_";

struct BootstrapCapabilities {
    program: File,
    host_output: File,
    input_files: Vec<ControlledBuildInputFile>,
    declarations: Vec<InputDeclaration>,
}

pub(super) fn stage_one(arguments: &[OsString]) -> Result<i32, HelperFailure> {
    validate_mode_arguments(Mode::Bootstrap, arguments)?;
    validate_descriptor_set()?;
    arm_parent_death()?;
    let deadline = Instant::now() + operation_timeout()?;
    let control = std::io::stdin();
    let (request, capabilities) = receive_capabilities(control.as_fd(), deadline)?;
    let helper = open_executable(Path::new("/proc/self/exe"))?;
    let capability_limit = bootstrap_capability_limit()?;
    let relative_path = env::var(format!("{INTERNAL_PREFIX}BUILD_PROGRAM_RELATIVE_PATH"))
        .map_err(|_| HelperFailure::InvalidLaunch)?;
    require_mapped_program(&capabilities, &relative_path)?;
    begin_bootstrap_isolation(
        &capabilities.host_output,
        &capabilities.input_files,
        deadline,
    )?;
    let mut root = linux_bootstrap_root::execute(&request, &capabilities.declarations, &helper)?;
    let private_input =
        File::open("/inputs").map_err(|_| HelperFailure::BootstrapRootVerification)?;
    let snapshot_program = open_snapshot_program(&private_input, &relative_path)?;
    drop(capabilities.input_files);
    drop(capabilities.program);
    let established = prepare_bootstrap_child_namespace(read_limits()?)?;
    let guardian_pid = u32::try_from(getpid().as_raw_nonzero().get())
        .map_err(|_| HelperFailure::NamespaceSetup)?;
    let (stage_control, child_control) = pair().map_err(control_failure)?;
    let mut command = Command::new("/bin/retonr-isolation-helper");
    command
        .arg("--stage2-bootstrap")
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
    let armed = receive(stage_control.as_fd(), deadline, None).map_err(control_failure)?;
    if armed.kind != MessageKind::Armed
        || !armed.payload.is_empty()
        || !armed.descriptors.is_empty()
    {
        return Err(HelperFailure::InvalidLaunch);
    }
    drop_bootstrap_guardian_privileges(capability_limit)?;
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
    if armed.kind != MessageKind::BootstrapArmed || !armed.descriptors.is_empty() {
        return Err(HelperFailure::InvalidLaunch);
    }
    let (evidence, mount, landlock_abi) = require_bootstrap_armed(&armed.payload)?;
    root.record_proc_mtab_validation();
    write_bootstrap_ready(
        guardian_pid,
        namespace_init_pid,
        evidence,
        mount,
        established.loopback_index,
        landlock_abi,
        &root,
    );
    let result = serve_parent(
        &mut child,
        control.as_fd(),
        stage_control.as_fd(),
        &capabilities.host_output,
        operation_timeout()?,
        build_timeout()?,
    );
    if let Err(failure) = result {
        report_parent_failure(control.as_fd(), failure, operation_timeout()?);
    }
    result
}

pub(super) fn stage_two(arguments: &[OsString]) -> Result<i32, HelperFailure> {
    validate_mode_arguments(Mode::Bootstrap, arguments)?;
    super::linux_bootstrap_root_native::remount_proc_for_namespace_init()?;
    super::linux_bootstrap_root_native::validate_proc_mtab_after_mount()?;
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
    let private_output =
        File::open("/output").map_err(|_| HelperFailure::BootstrapRootVerification)?;
    let relative_path = env::var(format!("{INTERNAL_PREFIX}BUILD_PROGRAM_RELATIVE_PATH"))
        .map_err(|_| HelperFailure::InvalidLaunch)?;
    linux_build_sandbox::validate_objects(&program, &input_root, &private_output, &relative_path)?;
    require_read_only_mount(&input_root)?;
    drop_bootstrap_namespace_init_privileges()?;
    validate_reduced_privileges()?;
    install_build_network_policy()?;
    let environment = read_target_environment()?;
    let target = PreparedBootstrapTarget::prepare(&program, arguments, &environment)?;
    let evidence = super::linux_helper_setup::NamespaceEvidence::current()?;
    let mount = super::linux_helper_setup::mount_namespace_identity()?;
    let landlock_abi = super::linux_bootstrap_sandbox::install_and_probe()?;
    send(
        control.as_fd(),
        MessageKind::BootstrapArmed,
        &encode_bootstrap_armed(evidence, mount, landlock_abi),
        &[],
        Instant::now() + startup_timeout,
        None,
    )
    .map_err(control_failure)?;
    let go = receive(control.as_fd(), Instant::now() + startup_timeout, None)
        .map_err(control_failure)?;
    if go.kind != MessageKind::BootstrapGo || !go.payload.is_empty() || !go.descriptors.is_empty() {
        return Err(HelperFailure::InvalidLaunch);
    }
    let output = target.run()?;
    if output.status() == crate::ControlledBuildProcessStatus::Success {
        super::linux_bootstrap_archive_regeneration::execute(&input_root, &private_output)?;
    }
    send(
        control.as_fd(),
        MessageKind::BootstrapFinished,
        &encode_process_finished(&output).ok_or(HelperFailure::InvalidLaunch)?,
        &[],
        Instant::now() + build_timeout,
        None,
    )
    .map_err(control_failure)?;
    Ok(0)
}

fn receive_capabilities(
    control: std::os::fd::BorrowedFd<'_>,
    deadline: Instant,
) -> Result<(BootstrapRequest, BootstrapCapabilities), HelperFailure> {
    let message = receive(control, deadline, None).map_err(control_failure)?;
    if message.kind != MessageKind::BootstrapDescriptors
        || message.descriptors.len() != BOOTSTRAP_ROOT_DESCRIPTOR_COUNT
    {
        return Err(HelperFailure::InvalidLaunch);
    }
    let request = decode_request(&message.payload).ok_or(HelperFailure::InvalidLaunch)?;
    let mut roots = message.descriptors.into_iter().map(File::from);
    let program = roots.next().ok_or(HelperFailure::InvalidLaunch)?;
    let host_output = roots.next().ok_or(HelperFailure::InvalidLaunch)?;
    if roots.next().is_some() {
        return Err(HelperFailure::InvalidLaunch);
    }
    validate_executable(&program)?;
    if !host_output
        .metadata()
        .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?
        .is_dir()
    {
        return Err(HelperFailure::ControlledBuildObjectMismatch);
    }
    let (input_files, declarations) = receive_input_files(control, request.input_count, deadline)?;
    if !request.joins(&declarations) {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    Ok((
        request,
        BootstrapCapabilities {
            program,
            host_output,
            input_files,
            declarations,
        },
    ))
}

fn receive_input_files(
    control: std::os::fd::BorrowedFd<'_>,
    input_count: usize,
    deadline: Instant,
) -> Result<(Vec<ControlledBuildInputFile>, Vec<InputDeclaration>), HelperFailure> {
    let mut input_files = Vec::with_capacity(input_count);
    let mut retained_declarations = Vec::with_capacity(input_count);
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
            let input = ControlledBuildInputFile::new(
                declaration.relative_path.clone(),
                declaration.expected_digest.clone(),
                declaration.expected_bytes,
                File::from(descriptor),
            )
            .map_err(|_| HelperFailure::InvalidLaunch)?;
            retained_declarations.push(declaration);
            input_files.push(input);
        }
    }
    Ok((input_files, retained_declarations))
}

fn require_mapped_program(
    capabilities: &BootstrapCapabilities,
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

fn open_snapshot_program(input_root: &File, relative_path: &str) -> Result<File, HelperFailure> {
    openat2(
        input_root,
        relative_path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        FileMode::empty(),
        ResolveFlags::BENEATH
            | ResolveFlags::NO_MAGICLINKS
            | ResolveFlags::NO_SYMLINKS
            | ResolveFlags::NO_XDEV,
    )
    .map(File::from)
    .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)
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
    if go.kind != MessageKind::BootstrapGo || !go.payload.is_empty() || !go.descriptors.is_empty() {
        return Err(HelperFailure::InvalidLaunch);
    }
    send(
        stage_control,
        MessageKind::BootstrapGo,
        &[],
        &[],
        Instant::now() + startup_timeout,
        None,
    )
    .map_err(control_failure)?;
    let finished =
        receive(stage_control, Instant::now() + build_timeout, None).map_err(control_failure)?;
    if finished.kind != MessageKind::BootstrapFinished || !finished.descriptors.is_empty() {
        return Err(HelperFailure::InvalidLaunch);
    }
    let mut output =
        decode_process_finished(&finished.payload).ok_or(HelperFailure::InvalidLaunch)?;
    let status = child.wait().map_err(|_| HelperFailure::InvalidLaunch)?;
    if !status.success() {
        return Err(HelperFailure::InvalidLaunch);
    }
    if output.status() == crate::ControlledBuildProcessStatus::Success {
        let private_output =
            File::open("/output").map_err(|_| HelperFailure::BootstrapRootVerification)?;
        let tree = linux_build_output::inspect_retained_output(&private_output)?;
        linux_build_output::publish_retained_output(host_output, &private_output, &tree)?;
        output = output.with_tree(tree);
    }
    let encoded = encode_finished(&output).ok_or(HelperFailure::InvalidLaunch)?;
    send(
        parent_control,
        MessageKind::BootstrapFinished,
        &encoded,
        &[],
        Instant::now() + startup_timeout,
        None,
    )
    .map_err(control_failure)?;
    Ok(0)
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

#[cfg(test)]
mod tests {
    use std::{os::fd::AsFd as _, time::Duration};

    use super::*;
    use crate::{
        RetainedProgramBootstrapAttempt, RetainedProgramBootstrapInputKind,
        platform::{
            linux_bootstrap_protocol::{
                BootstrapExecutable, BootstrapRequest, BootstrapSignedInput,
            },
            linux_build_protocol::{encode_input_count, encode_input_declarations},
            linux_control::{pair, send},
        },
    };
    use rewrite_types::Digest;

    #[test]
    fn input_receiver_rejects_wrong_message_kind() {
        let (sender, receiver) = pair().expect("pair");
        send(
            sender.as_fd(),
            MessageKind::BuildGo,
            &encode_input_count(1).expect("count"),
            &[],
            Instant::now() + Duration::from_secs(1),
            None,
        )
        .expect("send");
        assert!(matches!(
            receive_input_files(receiver.as_fd(), 1, Instant::now() + Duration::from_secs(1)),
            Err(HelperFailure::InvalidLaunch)
        ));
    }

    #[test]
    fn authenticated_declarations_survive_descriptor_receipt() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let path = temporary.path().join("input");
        std::fs::write(&path, b"x").expect("input");
        let file = File::open(path).expect("open");
        let digest = Digest::sha256(b"x");
        let (sender, receiver) = pair().expect("pair");
        send(
            sender.as_fd(),
            MessageKind::BuildInputFiles,
            &encode_input_declarations(&[("input", &digest, 1)]).expect("declaration"),
            &[file.as_fd()],
            Instant::now() + Duration::from_secs(1),
            None,
        )
        .expect("send");
        let (inputs, declarations) =
            receive_input_files(receiver.as_fd(), 1, Instant::now() + Duration::from_secs(1))
                .expect("receive");
        assert_eq!(inputs[0].relative_path(), "input");
        assert_eq!(declarations[0].expected_digest, digest);
    }

    #[test]
    fn busybox_role_is_not_replaceable_by_an_untyped_binary() {
        let signed_inputs = RetainedProgramBootstrapInputKind::ALL
            .into_iter()
            .map(|kind| BootstrapSignedInput {
                kind,
                digest: Digest::sha256(kind.relative_path().as_bytes()),
                byte_size: 1,
            })
            .collect::<Vec<_>>();
        let request = BootstrapRequest {
            attempt: RetainedProgramBootstrapAttempt::Primary,
            input_count: 15,
            launch_digest: Digest::sha256(b"launch"),
            recipe_digest: Digest::sha256(b"recipe"),
            recipe_bytes: 1,
            busybox_executable: BootstrapExecutable {
                digest: Digest::sha256(b"busybox executable"),
                byte_size: 18,
            },
            signed_inputs,
        };
        assert_eq!(
            request.busybox_package().expect("busybox package").kind,
            RetainedProgramBootstrapInputKind::AlpineBusyboxStaticPackage
        );
        assert!(
            request
                .signed_inputs
                .iter()
                .all(|input| input.kind.relative_path() != "toolchains/busybox")
        );
        assert_eq!(request.busybox_executable().byte_size, 18);
    }
}
