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
    seccompiler::apply_filter(&program).map_err(|_error| HelperFailure::SocketPolicy)?;
    if !socket_policy_is_active(std::process::id())? || !socket_policy_behaves_as_required() {
        return Err(HelperFailure::SocketPolicy);
    }
    Ok(())
}

pub(super) fn socket_policy_is_active(pid: u32) -> Result<bool, HelperFailure> {
    let status = fs::read_to_string(format!("/proc/{pid}/status"))
        .map_err(|_error| HelperFailure::SocketPolicy)?;
    Ok(status
        .lines()
        .find_map(|line| line.strip_prefix("Seccomp:"))
        .is_some_and(|value| value.trim() == SECCOMP_FILTER_MODE))
}

fn target_socket_filter() -> Result<BpfProgram, HelperFailure> {
    let socket_rule = SeccompRule::new(vec![
        SeccompCondition::new(
            0,
            SeccompCmpArgLen::Dword,
            SeccompCmpOp::Ne,
            libc::AF_INET as u64,
        )
        .map_err(|_error| HelperFailure::SocketPolicy)?,
        SeccompCondition::new(
            0,
            SeccompCmpArgLen::Dword,
            SeccompCmpOp::Ne,
            libc::AF_INET6 as u64,
        )
        .map_err(|_error| HelperFailure::SocketPolicy)?,
    ])
    .map_err(|_error| HelperFailure::SocketPolicy)?;
    let rules = BTreeMap::from([
        (libc::SYS_io_uring_setup, Vec::new()),
        (libc::SYS_socket, vec![socket_rule]),
    ]);
    let filter = SeccompFilter::new(
        rules,
        SeccompAction::Allow,
        SeccompAction::Errno(libc::EPERM as u32),
        ARCH.try_into()
            .map_err(|_error| HelperFailure::SocketPolicy)?,
    )
    .map_err(|_error| HelperFailure::SocketPolicy)?;
    filter
        .try_into()
        .map_err(|_error| HelperFailure::SocketPolicy)
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
    use super::target_socket_filter;

    #[test]
    fn target_filter_compiles_for_the_current_architecture() {
        assert!(!target_socket_filter().expect("compile filter").is_empty());
    }
}
