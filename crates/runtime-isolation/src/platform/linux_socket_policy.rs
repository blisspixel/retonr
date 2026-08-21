use std::{collections::BTreeMap, env::consts::ARCH, fs};

use rustix::{
    io::Errno,
    net::{AddressFamily, SocketType, socket},
};
use seccompiler::{
    BpfProgram, SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition, SeccompFilter,
    SeccompRule,
};

use super::linux_helper_setup::HelperFailure;

const SECCOMP_FILTER_MODE: &str = "2";

pub(super) fn install_target_socket_policy() -> Result<(), HelperFailure> {
    let program = target_socket_filter()?;
    seccompiler::apply_filter(&program).map_err(classify_filter_error)?;
    if !current_socket_policy_is_active()? {
        return Err(HelperFailure::SocketPolicyInactive);
    }
    if !socket_policy_behaves_as_required() {
        return Err(HelperFailure::SocketPolicyBehavior);
    }
    Ok(())
}

fn current_socket_policy_is_active() -> Result<bool, HelperFailure> {
    read_socket_policy_status("/proc/self/status")
}

fn classify_filter_error(error: seccompiler::Error) -> HelperFailure {
    match error {
        seccompiler::Error::Prctl(error) | seccompiler::Error::Seccomp(error)
            if matches!(error.raw_os_error(), Some(libc::EPERM | libc::EACCES)) =>
        {
            HelperFailure::HostPolicyDenied
        }
        _ => HelperFailure::SocketPolicyInstall,
    }
}

pub(super) fn socket_policy_is_active(pid: u32) -> Result<bool, HelperFailure> {
    read_socket_policy_status(&format!("/proc/{pid}/status"))
}

fn read_socket_policy_status(path: &str) -> Result<bool, HelperFailure> {
    let status = fs::read_to_string(path).map_err(|_error| HelperFailure::SocketPolicyInactive)?;
    Ok(status_reports_filter(&status))
}

fn status_reports_filter(status: &str) -> bool {
    status
        .lines()
        .find_map(|line| line.strip_prefix("Seccomp:"))
        .is_some_and(|value| value.trim() == SECCOMP_FILTER_MODE)
}

fn target_socket_filter() -> Result<BpfProgram, HelperFailure> {
    let socket_rule = SeccompRule::new(vec![
        SeccompCondition::new(
            0,
            SeccompCmpArgLen::Dword,
            SeccompCmpOp::Ne,
            libc::AF_INET as u64,
        )
        .map_err(|_error| HelperFailure::SocketPolicyCompile)?,
        SeccompCondition::new(
            0,
            SeccompCmpArgLen::Dword,
            SeccompCmpOp::Ne,
            libc::AF_INET6 as u64,
        )
        .map_err(|_error| HelperFailure::SocketPolicyCompile)?,
    ])
    .map_err(|_error| HelperFailure::SocketPolicyCompile)?;
    let rules = BTreeMap::from([
        (libc::SYS_io_uring_setup, Vec::new()),
        (libc::SYS_socket, vec![socket_rule]),
    ]);
    let filter = SeccompFilter::new(
        rules,
        SeccompAction::Allow,
        SeccompAction::Errno(libc::EPERM as u32),
        ARCH.try_into()
            .map_err(|_error| HelperFailure::SocketPolicyCompile)?,
    )
    .map_err(|_error| HelperFailure::SocketPolicyCompile)?;
    filter
        .try_into()
        .map_err(|_error| HelperFailure::SocketPolicyCompile)
}

fn socket_policy_behaves_as_required() -> bool {
    denied_socket(AddressFamily::UNIX)
        && denied_socket(AddressFamily::VSOCK)
        && allowed_socket(AddressFamily::INET)
        && allowed_socket(AddressFamily::INET6)
}

fn denied_socket(family: AddressFamily) -> bool {
    socket(family, SocketType::STREAM, None).is_err_and(|error| error == Errno::PERM)
}

fn allowed_socket(family: AddressFamily) -> bool {
    socket(family, SocketType::STREAM, None).is_ok()
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::{
        HelperFailure, classify_filter_error, status_reports_filter, target_socket_filter,
    };

    #[test]
    fn target_filter_compiles_for_the_current_architecture() {
        assert!(!target_socket_filter().expect("compile filter").is_empty());
    }

    #[test]
    fn status_parser_requires_filter_mode_two() {
        assert!(status_reports_filter("Name:\thelper\nSeccomp:\t2\n"));
        assert!(!status_reports_filter("Name:\thelper\nSeccomp:\t0\n"));
        assert!(!status_reports_filter("Name:\thelper\n"));
    }

    #[test]
    fn installation_permission_failures_are_host_policy_denials() {
        for error in [
            seccompiler::Error::Prctl(io::Error::from_raw_os_error(libc::EPERM)),
            seccompiler::Error::Seccomp(io::Error::from_raw_os_error(libc::EACCES)),
        ] {
            assert_eq!(
                classify_filter_error(error),
                HelperFailure::HostPolicyDenied
            );
        }
    }

    #[test]
    fn other_installation_failures_remain_socket_policy_failures() {
        assert_eq!(
            classify_filter_error(seccompiler::Error::Seccomp(io::Error::from_raw_os_error(
                libc::EINVAL
            ),)),
            HelperFailure::SocketPolicyInstall
        );
    }
}
