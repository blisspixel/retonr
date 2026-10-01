use std::{
    collections::BTreeSet,
    env,
    ffi::OsString,
    fs::File,
    io::{BufRead as _, BufReader, Read as _, Write as _},
    os::unix::fs::PermissionsExt as _,
    path::Path,
    process::{Child, Command},
    time::Duration,
};

use rustix::process::{Signal, getppid, set_parent_process_death_signal};

use super::{
    linux_control::ControlError,
    linux_executable::has_elf_magic,
    linux_helper::Mode,
    linux_helper_setup::{HelperFailure, NamespaceEvidence, RawNamespaceIdentity},
    linux_managed_protocol::ManagedReadyEvidence,
};
use crate::contract::RetainedProgramBootstrapRootObservation;

const INTERNAL_PREFIX: &str = "REWRITE_ISOLATION_INTERNAL_";
const HANDSHAKE_LIMIT: u64 = 32;

// Command owns the configured child endpoint even after spawn. Consume it here
// so a namespace child's exit remains observable as control-channel EOF.
pub(super) fn spawn_control_child(mut command: Command) -> Result<Child, HelperFailure> {
    let child = command.spawn().map_err(|_| HelperFailure::NamespaceSetup)?;
    drop(command);
    Ok(child)
}

pub(super) fn legacy_spawn_handshake(
    child: &mut Child,
    namespace_init_pid: u32,
) -> Result<(), HelperFailure> {
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

pub(super) fn validate_mode_arguments(
    mode: Mode,
    arguments: &[OsString],
) -> Result<(), HelperFailure> {
    match mode {
        Mode::Probe if arguments.is_empty() => Ok(()),
        Mode::Launch | Mode::Build | Mode::Bootstrap => Ok(()),
        Mode::Probe => Err(HelperFailure::InvalidLaunch),
    }
}

pub(super) fn read_target_environment() -> Result<Vec<(OsString, OsString)>, HelperFailure> {
    let count = read_internal_usize("ENV_COUNT")?;
    if count > 1_024 {
        return Err(HelperFailure::InvalidLaunch);
    }
    let mut environment = Vec::with_capacity(count);
    let mut keys = BTreeSet::new();
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
            || !keys.insert(key.clone())
        {
            return Err(HelperFailure::InvalidLaunch);
        }
        environment.push((key, value));
    }
    Ok(environment)
}

pub(super) fn arm_parent_death() -> Result<(), HelperFailure> {
    let parent = getppid().ok_or(HelperFailure::NamespaceSetup)?;
    set_parent_process_death_signal(Some(Signal::KILL))
        .map_err(|_| HelperFailure::NamespaceSetup)?;
    if getppid() != Some(parent) {
        return Err(HelperFailure::NamespaceSetup);
    }
    Ok(())
}

pub(super) fn read_go_message() -> Result<u32, HelperFailure> {
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

pub(super) fn read_limits() -> Result<(u64, u64), HelperFailure> {
    Ok((
        read_internal_u64("MAX_OPEN_FILES")?,
        read_internal_u64("MAX_PROCESSES")?,
    ))
}

pub(super) fn operation_timeout() -> Result<Duration, HelperFailure> {
    let milliseconds = read_internal_u64("STARTUP_TIMEOUT_MILLIS")?;
    if !(1..=30_000).contains(&milliseconds) {
        return Err(HelperFailure::InvalidLaunch);
    }
    Ok(Duration::from_millis(milliseconds))
}

pub(super) fn build_timeout() -> Result<Duration, HelperFailure> {
    let milliseconds = read_internal_u64("BUILD_TIMEOUT_MILLIS")?;
    if !(1..=14_400_000).contains(&milliseconds) {
        return Err(HelperFailure::InvalidLaunch);
    }
    Ok(Duration::from_millis(milliseconds))
}

pub(super) fn read_internal_u32(name: &str) -> Result<u32, HelperFailure> {
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

pub(super) fn open_executable(path: &Path) -> Result<File, HelperFailure> {
    let file = File::open(path).map_err(|_| HelperFailure::InvalidLaunch)?;
    validate_executable(&file)?;
    Ok(file)
}

pub(super) fn validate_executable(file: &File) -> Result<(), HelperFailure> {
    let metadata = file.metadata().map_err(|_| HelperFailure::InvalidLaunch)?;
    if !metadata.is_file()
        || metadata.permissions().mode() & 0o111 == 0
        || !has_elf_magic(file).map_err(|_| HelperFailure::InvalidLaunch)?
    {
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
        network: RawNamespaceIdentity {
            device: values[0],
            inode: values[1],
        },
        user: RawNamespaceIdentity {
            device: values[2],
            inode: values[3],
        },
        process: RawNamespaceIdentity {
            device: values[4],
            inode: values[5],
        },
    })
}

pub(super) fn control_failure(error: ControlError) -> HelperFailure {
    match error {
        ControlError::Cancelled
        | ControlError::Deadline
        | ControlError::Closed
        | ControlError::Invalid
        | ControlError::Native => HelperFailure::InvalidLaunch,
    }
}

pub(super) fn write_ready(
    guardian_pid: u32,
    namespace_init_pid: u32,
    evidence: &ManagedReadyEvidence,
    loopback_index: u32,
) {
    let namespace = evidence.namespace;
    let boundary = &evidence.device_boundary;
    let inputs = &evidence.runtime_inputs;
    let (major, minor) = boundary.null_device_numbers();
    let entries = boundary.visible_device_entries();
    let scratch_device = inputs.scratch_mount().device();
    let scratch_inode = inputs.scratch_mount().inode();
    let input_device = inputs.input_mount().device();
    let input_inode = inputs.input_mount().inode();
    let input_count = inputs.member_count();
    let input_bytes = inputs.total_bytes();
    let input_postconditions = inputs.postconditions();
    let device_digest = boundary.redacted_digest();
    let layout_digest = inputs.layout_digest();
    let input_digest = inputs.redacted_digest();
    write_protocol(&format!(
        "READY 3 {guardian_pid} {namespace_init_pid} {} {} {} {} {} {} {} {} {loopback_index} {} {} {} {} {} {} {} {} {major} {minor} {entries} {scratch_device} {scratch_inode} {input_device} {input_inode} {input_count} {input_bytes} {input_postconditions} {device_digest} {layout_digest} {input_digest}\n",
        namespace.network.device,
        namespace.network.inode,
        namespace.user.device,
        namespace.user.inode,
        namespace.process.device,
        namespace.process.inode,
        evidence.mount.device,
        evidence.mount.inode,
        boundary.policy().code(),
        boundary.postconditions(),
        boundary.device_mount().device(),
        boundary.device_mount().inode(),
        boundary.proc_mount().device(),
        boundary.proc_mount().inode(),
        boundary.null_device().device(),
        boundary.null_device().inode(),
    ));
}

pub(super) fn write_build_ready(
    guardian_pid: u32,
    namespace_init_pid: u32,
    evidence: NamespaceEvidence,
    mount: RawNamespaceIdentity,
    loopback_index: u32,
    landlock_abi: u32,
) {
    write_protocol(&format!(
        "BUILD_READY 2 {guardian_pid} {namespace_init_pid} {} {} {} {} {} {} {} {} {loopback_index} {landlock_abi}\n",
        evidence.network.device,
        evidence.network.inode,
        evidence.user.device,
        evidence.user.inode,
        evidence.process.device,
        evidence.process.inode,
        mount.device,
        mount.inode,
    ));
}

pub(super) fn write_bootstrap_ready(
    guardian_pid: u32,
    namespace_init_pid: u32,
    evidence: NamespaceEvidence,
    mount: RawNamespaceIdentity,
    loopback_index: u32,
    landlock_abi: u32,
    root: &RetainedProgramBootstrapRootObservation,
) {
    write_protocol(&format!(
        "BOOTSTRAP_READY 3 {guardian_pid} {namespace_init_pid} {} {} {} {} {} {} {} {} {loopback_index} {landlock_abi} {} {} {} {} {} {} {} {} {} {}\n",
        evidence.network.device,
        evidence.network.inode,
        evidence.user.device,
        evidence.user.inode,
        evidence.process.device,
        evidence.process.inode,
        mount.device,
        mount.inode,
        root.root_mount.device(),
        root.root_mount.inode(),
        root.alpine_installed_database_digest,
        root.alpine_loader_digest,
        root.alpine_libc_linker_name_digest,
        root.alpine_libgcc_digest,
        root.alpine_libgcc_linker_name_digest,
        root.alpine_busybox_digest,
        root.rust_toolchain_layout_digest,
        root.normalized_alpine_link_plan_digest,
    ));
}

pub(super) fn write_protocol(message: &str) {
    let mut output = std::io::stdout().lock();
    let _ = output.write_all(message.as_bytes());
    let _ = output.flush();
}

#[cfg(test)]
#[path = "linux_helper_support/control_child_tests.rs"]
mod control_child_tests;
