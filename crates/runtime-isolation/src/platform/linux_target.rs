use std::{fs, fs::File, os::unix::fs::MetadataExt as _};

use rustix::{
    fd::OwnedFd,
    process::{Pid, PidfdFlags, pidfd_open},
};

use super::linux_socket_policy::socket_policy_is_active;
use super::linux_validation::{
    ensure_pidfd_alive, namespace_identity, native_errno, open_namespace, privileges_are_reduced,
};
use crate::{
    IsolationError, IsolationResult, NamespaceIdentity, TargetProcessEvidence, error::native,
};

#[derive(Debug)]
pub(super) struct TargetObservation {
    pub(super) pidfd: OwnedFd,
    pub(super) executable: File,
    pub(super) evidence: TargetProcessEvidence,
}

pub(super) fn observe_target(
    namespace_init_pid: u32,
    expected_executable: &File,
    expected_network: NamespaceIdentity,
    expected_user: NamespaceIdentity,
    expected_process: NamespaceIdentity,
) -> IsolationResult<TargetObservation> {
    let target_pid = direct_child(namespace_init_pid)?;
    let raw_pid = i32::try_from(target_pid)
        .ok()
        .and_then(Pid::from_raw)
        .ok_or(IsolationError::HelperProtocol)?;
    let pidfd = pidfd_open(raw_pid, PidfdFlags::empty())
        .map_err(|error| native_errno("open-target-pidfd", error))?;
    ensure_pidfd_alive(&pidfd)?;
    let executable = File::open(format!("/proc/{target_pid}/exe"))
        .map_err(|error| native("open-target-executable", &error))?;
    let expected_metadata = expected_executable
        .metadata()
        .map_err(|error| native("read-expected-executable", &error))?;
    let actual_metadata = executable
        .metadata()
        .map_err(|error| native("read-target-executable", &error))?;
    if (
        actual_metadata.dev(),
        actual_metadata.ino(),
        actual_metadata.len(),
    ) != (
        expected_metadata.dev(),
        expected_metadata.ino(),
        expected_metadata.len(),
    ) {
        return Err(IsolationError::EvidenceChanged);
    }
    ensure_target_namespaces(
        target_pid,
        expected_network,
        expected_user,
        expected_process,
    )?;
    let namespace_pid = target_relationship(target_pid, namespace_init_pid)?;
    let process_start_token = process_start_token(target_pid)?;
    let namespace_user_id = namespace_user_id(target_pid)?;
    if direct_child(namespace_init_pid)? != target_pid
        || !privileges_are_reduced(target_pid)?
        || !socket_policy_is_active(target_pid).map_err(|_error| IsolationError::EvidenceChanged)?
    {
        return Err(IsolationError::EvidenceChanged);
    }
    ensure_pidfd_alive(&pidfd)?;
    Ok(TargetObservation {
        pidfd,
        executable,
        evidence: TargetProcessEvidence::new(
            target_pid,
            namespace_pid,
            process_start_token,
            namespace_user_id,
            actual_metadata.dev(),
            actual_metadata.ino(),
            actual_metadata.len(),
        ),
    })
}

pub(super) fn reobserve_target(
    namespace_init_pid: u32,
    expected: TargetProcessEvidence,
    retained_executable: &File,
    expected_network: NamespaceIdentity,
    expected_user: NamespaceIdentity,
    expected_process: NamespaceIdentity,
) -> IsolationResult<()> {
    let target_pid = expected.outer_pid();
    if direct_child(namespace_init_pid)? != target_pid
        || target_relationship(target_pid, namespace_init_pid)? != expected.namespace_pid()
        || process_start_token(target_pid)? != expected.process_start_token()
        || namespace_user_id(target_pid)? != expected.namespace_user_id()
        || !privileges_are_reduced(target_pid)?
        || !socket_policy_is_active(target_pid).map_err(|_error| IsolationError::EvidenceChanged)?
    {
        return Err(IsolationError::EvidenceChanged);
    }
    ensure_target_namespaces(
        target_pid,
        expected_network,
        expected_user,
        expected_process,
    )?;
    let retained = retained_executable
        .metadata()
        .map_err(|error| native("read-retained-target-executable", &error))?;
    let current = File::open(format!("/proc/{target_pid}/exe"))
        .and_then(|file| file.metadata())
        .map_err(|error| native("reopen-target-executable", &error))?;
    let identity = (
        expected.executable_device(),
        expected.executable_inode(),
        expected.executable_bytes(),
    );
    if (retained.dev(), retained.ino(), retained.len()) != identity
        || (current.dev(), current.ino(), current.len()) != identity
    {
        return Err(IsolationError::EvidenceChanged);
    }
    Ok(())
}

fn direct_child(namespace_init_pid: u32) -> IsolationResult<u32> {
    let path = format!("/proc/{namespace_init_pid}/task/{namespace_init_pid}/children");
    let text = fs::read_to_string(path).map_err(|error| native("read-pid1-children", &error))?;
    if text.len() > 4_096 {
        return Err(IsolationError::HelperProtocol);
    }
    let children = text
        .split_ascii_whitespace()
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_error| IsolationError::HelperProtocol)?;
    match children.as_slice() {
        [target] if *target > 0 => Ok(*target),
        _ => Err(IsolationError::HelperProtocol),
    }
}

fn target_relationship(target_pid: u32, namespace_init_pid: u32) -> IsolationResult<u32> {
    let status = fs::read_to_string(format!("/proc/{target_pid}/status"))
        .map_err(|error| native("read-target-status", &error))?;
    let parent = status_value(&status, "PPid:")?;
    let namespace_pids = status
        .lines()
        .find_map(|line| line.strip_prefix("NSpid:"))
        .ok_or(IsolationError::EvidenceChanged)?
        .split_ascii_whitespace()
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_error| IsolationError::EvidenceChanged)?;
    let namespace_pid = namespace_pids
        .last()
        .copied()
        .filter(|pid| *pid > 1)
        .ok_or(IsolationError::EvidenceChanged)?;
    if parent != namespace_init_pid || namespace_pids.first().copied() != Some(target_pid) {
        return Err(IsolationError::EvidenceChanged);
    }
    Ok(namespace_pid)
}

fn status_value(status: &str, label: &str) -> IsolationResult<u32> {
    status
        .lines()
        .find_map(|line| line.strip_prefix(label))
        .and_then(|value| value.trim().parse::<u32>().ok())
        .ok_or(IsolationError::EvidenceChanged)
}

fn process_start_token(pid: u32) -> IsolationResult<u64> {
    let text = fs::read_to_string(format!("/proc/{pid}/stat"))
        .map_err(|error| native("read-target-start-token", &error))?;
    if text.len() > 16 * 1024 {
        return Err(IsolationError::EvidenceChanged);
    }
    let close = text.rfind(')').ok_or(IsolationError::EvidenceChanged)?;
    text.get(close.saturating_add(1)..)
        .ok_or(IsolationError::EvidenceChanged)?
        .split_ascii_whitespace()
        .nth(19)
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .ok_or(IsolationError::EvidenceChanged)
}

fn namespace_user_id(pid: u32) -> IsolationResult<u32> {
    let status = fs::read_to_string(format!("/proc/{pid}/status"))
        .map_err(|error| native("read-target-credentials", &error))?;
    let mapping = fs::read_to_string(format!("/proc/{pid}/uid_map"))
        .map_err(|error| native("read-target-user-map", &error))?;
    if status.len() > 128 * 1024 || mapping.len() > 4_096 {
        return Err(IsolationError::EvidenceChanged);
    }
    parse_namespace_user_id(&status, &mapping)
}

fn parse_namespace_user_id(status: &str, mapping: &str) -> IsolationResult<u32> {
    let credentials = status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))
        .ok_or(IsolationError::EvidenceChanged)?
        .split_ascii_whitespace()
        .map(str::parse::<u64>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_error| IsolationError::EvidenceChanged)?;
    let host_user_id = match credentials.as_slice() {
        [real, effective, saved, filesystem]
            if real == effective && real == saved && real == filesystem =>
        {
            *real
        }
        _ => return Err(IsolationError::EvidenceChanged),
    };
    let lines = mapping.lines().collect::<Vec<_>>();
    let fields = match lines.as_slice() {
        [line] => line
            .split_ascii_whitespace()
            .map(str::parse::<u64>)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_error| IsolationError::EvidenceChanged)?,
        _ => return Err(IsolationError::EvidenceChanged),
    };
    let (namespace_start, host_start, count) = match fields.as_slice() {
        [namespace_start, host_start, count] if *count > 0 => {
            (*namespace_start, *host_start, *count)
        }
        _ => return Err(IsolationError::EvidenceChanged),
    };
    let offset = host_user_id
        .checked_sub(host_start)
        .filter(|offset| *offset < count)
        .ok_or(IsolationError::EvidenceChanged)?;
    u32::try_from(
        namespace_start
            .checked_add(offset)
            .ok_or(IsolationError::EvidenceChanged)?,
    )
    .map_err(|_error| IsolationError::EvidenceChanged)
}

fn ensure_target_namespaces(
    target_pid: u32,
    network: NamespaceIdentity,
    user: NamespaceIdentity,
    process: NamespaceIdentity,
) -> IsolationResult<()> {
    if namespace_identity(&open_namespace(target_pid, "net")?)? != network
        || namespace_identity(&open_namespace(target_pid, "user")?)? != user
        || namespace_identity(&open_namespace(target_pid, "pid")?)? != process
    {
        return Err(IsolationError::EvidenceChanged);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_namespace_user_id;
    use crate::IsolationError;

    #[test]
    fn namespace_user_id_requires_exact_credentials_and_mapping() {
        assert_eq!(
            parse_namespace_user_id("Uid:\t1000\t1000\t1000\t1000\n", "0 1000 1\n"),
            Ok(0)
        );
        assert_eq!(
            parse_namespace_user_id("Uid:\t1000\t1001\t1000\t1000\n", "0 1000 1\n"),
            Err(IsolationError::EvidenceChanged)
        );
        assert_eq!(
            parse_namespace_user_id("Uid:\t1000\t1000\t1000\t1000\n", "0 2000 1\n"),
            Err(IsolationError::EvidenceChanged)
        );
    }
}
