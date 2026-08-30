use std::{fs::File, io::Read as _, os::unix::fs::MetadataExt as _, time::Instant};

use rewrite_types::CancellationToken;
use rustix::{
    fd::OwnedFd,
    io::Errno,
    process::{Pid, PidfdFlags, pidfd_open},
};

use super::super::super::linux::ensure_pidfd_alive;
use crate::managed_worker::{ensure_worker_active, parse_high_water_resident_bytes};
use crate::{
    AttachedProcessWitnessError, ManagedGenerationWorkerError, ManagedGenerationWorkerLimits,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(in crate::platform::linux_managed_worker) struct ObjectKey {
    pub(in crate::platform::linux_managed_worker) device: u64,
    pub(in crate::platform::linux_managed_worker) inode: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::platform::linux_managed_worker) struct ObjectIdentity {
    pub(in crate::platform::linux_managed_worker) key: ObjectKey,
    pub(in crate::platform::linux_managed_worker) bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::platform::linux_managed_worker) struct NamespaceIdentity {
    pub(in crate::platform::linux_managed_worker) device: u64,
    pub(in crate::platform::linux_managed_worker) inode: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::platform::linux_managed_worker) struct NamespaceSet {
    pub(in crate::platform::linux_managed_worker) pid: NamespaceIdentity,
    pub(in crate::platform::linux_managed_worker) user: NamespaceIdentity,
    pub(in crate::platform::linux_managed_worker) network: NamespaceIdentity,
    pub(in crate::platform::linux_managed_worker) mount: NamespaceIdentity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::platform::linux_managed_worker) struct ProcessStat {
    pub(in crate::platform::linux_managed_worker) parent_pid: u32,
    pub(in crate::platform::linux_managed_worker) start_token: u64,
}

pub(in crate::platform::linux_managed_worker) struct RetainedProcess {
    pub(in crate::platform::linux_managed_worker) pid: u32,
    pub(in crate::platform::linux_managed_worker) start_token: u64,
    pub(in crate::platform::linux_managed_worker) pidfd: OwnedFd,
    process_directory: File,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProcAccessClass {
    Exited,
    AccessDenied,
    ResourceLimit,
    Incomplete,
}

pub(in crate::platform::linux_managed_worker) fn retain_process(
    pid: u32,
) -> Result<RetainedProcess, ManagedGenerationWorkerError> {
    let pidfd = open_pidfd(pid)?;
    let process_directory = File::open(format!("/proc/{pid}")).map_err(map_visibility_error)?;
    let stat = process_stat(pid, usize::MAX, None, None)?;
    ensure_alive(&pidfd)?;
    Ok(RetainedProcess {
        pid,
        start_token: stat.start_token,
        pidfd,
        process_directory,
    })
}

pub(in crate::platform::linux_managed_worker) fn open_process_member(
    pid: u32,
    member: &str,
) -> Result<File, ManagedGenerationWorkerError> {
    File::open(format!("/proc/{pid}/{member}")).map_err(map_visibility_error)
}

pub(in crate::platform::linux_managed_worker) fn object_identity(
    file: &File,
) -> Result<ObjectIdentity, ManagedGenerationWorkerError> {
    let metadata = file
        .metadata()
        .map_err(|_error| ManagedGenerationWorkerError::PlatformObservationFailed)?;
    if !metadata.is_file() || metadata.dev() == 0 || metadata.ino() == 0 || metadata.len() == 0 {
        return Err(ManagedGenerationWorkerError::PlatformObservationFailed);
    }
    Ok(ObjectIdentity {
        key: ObjectKey {
            device: metadata.dev(),
            inode: metadata.ino(),
        },
        bytes: metadata.len(),
    })
}

pub(in crate::platform::linux_managed_worker) fn namespaces(
    pid: u32,
    limits: ManagedGenerationWorkerLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<NamespaceSet, ManagedGenerationWorkerError> {
    ensure_worker_active(cancellation, started, limits)?;
    Ok(NamespaceSet {
        pid: namespace(pid, "pid")?,
        user: namespace(pid, "user")?,
        network: namespace(pid, "net")?,
        mount: namespace(pid, "mnt")?,
    })
}

pub(in crate::platform::linux_managed_worker) fn status(
    pid: u32,
    limits: ManagedGenerationWorkerLimits,
    cancellation: &CancellationToken,
    started: Instant,
    require_worker_privileges: bool,
) -> Result<u32, ManagedGenerationWorkerError> {
    let bytes = read_bounded(
        &format!("/proc/{pid}/status"),
        limits.maximum_metadata_bytes,
        cancellation,
        started,
        limits,
    )?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_error| ManagedGenerationWorkerError::PlatformObservationFailed)?;
    parse_status(text, require_worker_privileges)
}

pub(in crate::platform::linux_managed_worker) fn worker_high_water_resident_bytes(
    process: &RetainedProcess,
    limits: ManagedGenerationWorkerLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<u64, ManagedGenerationWorkerError> {
    validate_retained_process(process, ManagedGenerationWorkerError::WorkerChanged)?;
    let bytes = read_bounded(
        &format!("/proc/{}/status", process.pid),
        limits.maximum_metadata_bytes,
        cancellation,
        started,
        limits,
    )?;
    validate_retained_process(process, ManagedGenerationWorkerError::WorkerChanged)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_error| ManagedGenerationWorkerError::PlatformObservationFailed)?;
    parse_high_water_resident_bytes(text)
}

pub(in crate::platform::linux_managed_worker) fn validate_retained_process(
    process: &RetainedProcess,
    error: ManagedGenerationWorkerError,
) -> Result<(), ManagedGenerationWorkerError> {
    ensure_alive(&process.pidfd).map_err(|_error| error)?;
    process
        .process_directory
        .metadata()
        .map_err(|_error| error)?;
    let stat = process_stat(process.pid, usize::MAX, None, None).map_err(|_error| error)?;
    if stat.start_token != process.start_token {
        return Err(error);
    }
    Ok(())
}

pub(in crate::platform::linux_managed_worker) fn process_stat(
    pid: u32,
    maximum_bytes: usize,
    cancellation: Option<&CancellationToken>,
    timing: Option<(Instant, ManagedGenerationWorkerLimits)>,
) -> Result<ProcessStat, ManagedGenerationWorkerError> {
    let bytes = if let (Some(cancellation), Some((started, limits))) = (cancellation, timing) {
        read_bounded(
            &format!("/proc/{pid}/stat"),
            maximum_bytes,
            cancellation,
            started,
            limits,
        )?
    } else {
        let mut bytes = Vec::new();
        File::open(format!("/proc/{pid}/stat"))
            .map_err(map_visibility_error)?
            .take(u64::try_from(maximum_bytes).unwrap_or(u64::MAX))
            .read_to_end(&mut bytes)
            .map_err(|error| map_io_error(&error))?;
        bytes
    };
    parse_stat(&bytes)
}

fn parse_stat(bytes: &[u8]) -> Result<ProcessStat, ManagedGenerationWorkerError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_error| ManagedGenerationWorkerError::PlatformObservationFailed)?;
    let close = text
        .rfind(')')
        .ok_or(ManagedGenerationWorkerError::PlatformObservationFailed)?;
    let fields = text
        .get(close + 1..)
        .ok_or(ManagedGenerationWorkerError::PlatformObservationFailed)?
        .split_ascii_whitespace()
        .collect::<Vec<_>>();
    if fields.len() < 20 || matches!(fields[0], "Z" | "X" | "x") {
        return Err(ManagedGenerationWorkerError::WorkerChanged);
    }
    Ok(ProcessStat {
        parent_pid: fields[1]
            .parse()
            .map_err(|_error| ManagedGenerationWorkerError::PlatformObservationFailed)?,
        start_token: fields[19]
            .parse()
            .map_err(|_error| ManagedGenerationWorkerError::PlatformObservationFailed)?,
    })
}

fn parse_status(text: &str, require_privileges: bool) -> Result<u32, ManagedGenerationWorkerError> {
    let value = |name: &str| text.lines().find_map(|line| line.strip_prefix(name));
    let uid = value("Uid:")
        .ok_or(ManagedGenerationWorkerError::PrivilegeMismatch)?
        .split_ascii_whitespace()
        .nth(1)
        .ok_or(ManagedGenerationWorkerError::PrivilegeMismatch)?
        .parse::<u32>()
        .map_err(|_error| ManagedGenerationWorkerError::PrivilegeMismatch)?;
    if !require_privileges {
        return Ok(uid);
    }
    let zero_cap = |name: &str| {
        value(name).is_some_and(|raw| {
            let raw = raw.trim();
            !raw.is_empty() && raw.len() <= 16 && raw.bytes().all(|byte| byte == b'0')
        })
    };
    if value("NoNewPrivs:").map(str::trim) != Some("1")
        || value("Seccomp:").map(str::trim) != Some("2")
        || !["CapInh:", "CapPrm:", "CapEff:", "CapBnd:", "CapAmb:"]
            .into_iter()
            .all(zero_cap)
    {
        return Err(ManagedGenerationWorkerError::PrivilegeMismatch);
    }
    Ok(uid)
}

fn namespace(pid: u32, name: &str) -> Result<NamespaceIdentity, ManagedGenerationWorkerError> {
    let metadata = open_process_member(pid, &format!("ns/{name}"))?
        .metadata()
        .map_err(|error| map_io_error(&error))?;
    if metadata.dev() == 0 || metadata.ino() == 0 {
        return Err(ManagedGenerationWorkerError::NamespaceMismatch);
    }
    Ok(NamespaceIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

fn open_pidfd(pid: u32) -> Result<OwnedFd, ManagedGenerationWorkerError> {
    let pid = i32::try_from(pid)
        .ok()
        .and_then(Pid::from_raw)
        .ok_or(ManagedGenerationWorkerError::PlatformObservationFailed)?;
    let pidfd = pidfd_open(pid, PidfdFlags::empty()).map_err(map_pidfd_error)?;
    ensure_alive(&pidfd)?;
    Ok(pidfd)
}

pub(in crate::platform::linux_managed_worker) fn ensure_alive(
    pidfd: &OwnedFd,
) -> Result<(), ManagedGenerationWorkerError> {
    ensure_pidfd_alive(pidfd).map_err(map_pidfd_liveness_error)
}

pub(in crate::platform::linux_managed_worker) fn read_bounded(
    path: &str,
    maximum: usize,
    cancellation: &CancellationToken,
    started: Instant,
    limits: ManagedGenerationWorkerLimits,
) -> Result<Vec<u8>, ManagedGenerationWorkerError> {
    let mut file = File::open(path).map_err(map_visibility_error)?;
    let mut bytes = Vec::with_capacity(maximum.min(16 * 1024));
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        ensure_worker_active(cancellation, started, limits)?;
        let read = file
            .read(&mut buffer)
            .map_err(|error| map_io_error(&error))?;
        if read == 0 {
            break;
        }
        if bytes.len().saturating_add(read) > maximum {
            return Err(ManagedGenerationWorkerError::ResourceLimit);
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    Ok(bytes)
}

fn classify_errno(error: Errno) -> ProcAccessClass {
    match error {
        Errno::NOENT | Errno::SRCH => ProcAccessClass::Exited,
        Errno::ACCESS | Errno::PERM => ProcAccessClass::AccessDenied,
        Errno::MFILE | Errno::NFILE | Errno::NOMEM => ProcAccessClass::ResourceLimit,
        _ => ProcAccessClass::Incomplete,
    }
}

fn map_pidfd_error(error: Errno) -> ManagedGenerationWorkerError {
    match classify_errno(error) {
        ProcAccessClass::Exited => ManagedGenerationWorkerError::WorkerChanged,
        ProcAccessClass::AccessDenied => {
            ManagedGenerationWorkerError::ProcessVisibilityInsufficient
        }
        ProcAccessClass::ResourceLimit => ManagedGenerationWorkerError::ResourceLimit,
        ProcAccessClass::Incomplete => ManagedGenerationWorkerError::PlatformObservationFailed,
    }
}

fn map_pidfd_liveness_error(error: AttachedProcessWitnessError) -> ManagedGenerationWorkerError {
    match error {
        AttachedProcessWitnessError::ProcessExited => ManagedGenerationWorkerError::WorkerChanged,
        AttachedProcessWitnessError::ProcessAccessDenied => {
            ManagedGenerationWorkerError::ProcessVisibilityInsufficient
        }
        AttachedProcessWitnessError::ResourceLimit => ManagedGenerationWorkerError::ResourceLimit,
        _ => ManagedGenerationWorkerError::PlatformObservationFailed,
    }
}

fn map_io_error(error: &std::io::Error) -> ManagedGenerationWorkerError {
    error.raw_os_error().map(Errno::from_raw_os_error).map_or(
        ManagedGenerationWorkerError::PlatformObservationFailed,
        |error| match classify_errno(error) {
            ProcAccessClass::AccessDenied => {
                ManagedGenerationWorkerError::ProcessVisibilityInsufficient
            }
            ProcAccessClass::ResourceLimit => ManagedGenerationWorkerError::ResourceLimit,
            ProcAccessClass::Exited | ProcAccessClass::Incomplete => {
                ManagedGenerationWorkerError::PlatformObservationFailed
            }
        },
    )
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Result::map_err supplies the owned I/O error"
)]
fn map_visibility_error(error: std::io::Error) -> ManagedGenerationWorkerError {
    map_io_error(&error)
}

#[cfg(test)]
mod tests;
