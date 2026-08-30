use std::{collections::BTreeMap, env::consts::ARCH, fs, os::unix::net::UnixStream};

use rustix::{
    fs::{CWD, FileType, Mode, mknodat},
    io::Errno,
    mount::{MountFlags, mount},
    net::{AddressFamily, Protocol, SocketFlags, SocketType, ipproto, socket, socketpair},
    thread::UnshareFlags,
};
use seccompiler::{
    BpfProgram, SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition, SeccompFilter,
    SeccompRule,
};

use super::linux_helper_setup::HelperFailure;

const SECCOMP_FILTER_MODE: &str = "2";

pub(super) fn install_managed_target_policy() -> Result<(), HelperFailure> {
    let clone3_program =
        clone3_compatibility_filter(HelperFailure::ManagedContainmentPolicyCompile)?;
    seccompiler::apply_filter(&clone3_program).map_err(classify_managed_filter_error)?;
    let program = managed_target_filter()?;
    seccompiler::apply_filter(&program).map_err(classify_managed_filter_error)?;
    if !current_socket_policy_is_active()? {
        return Err(HelperFailure::ManagedContainmentPolicyInactive);
    }
    if !managed_policy_behaves_as_required() {
        return Err(HelperFailure::ManagedContainmentPolicyBehavior);
    }
    Ok(())
}

pub(super) fn install_build_network_policy() -> Result<(), HelperFailure> {
    let clone3_program = clone3_compatibility_filter(HelperFailure::SocketPolicyCompile)?;
    seccompiler::apply_filter(&clone3_program).map_err(classify_filter_error)?;
    let program = build_network_filter()?;
    seccompiler::apply_filter(&program).map_err(classify_filter_error)?;
    if !current_socket_policy_is_active()? {
        return Err(HelperFailure::SocketPolicyInactive);
    }
    if !build_network_policy_behaves_as_required() {
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

fn classify_managed_filter_error(error: seccompiler::Error) -> HelperFailure {
    match error {
        seccompiler::Error::Prctl(error) | seccompiler::Error::Seccomp(error)
            if matches!(error.raw_os_error(), Some(libc::EPERM | libc::EACCES)) =>
        {
            HelperFailure::HostPolicyDenied
        }
        _ => HelperFailure::ManagedContainmentPolicyInstall,
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

fn managed_target_filter() -> Result<BpfProgram, HelperFailure> {
    let socket_rule = SeccompRule::new(vec![
        SeccompCondition::new(
            0,
            SeccompCmpArgLen::Dword,
            SeccompCmpOp::Ne,
            libc::AF_INET as u64,
        )
        .map_err(|_error| HelperFailure::ManagedContainmentPolicyCompile)?,
        SeccompCondition::new(
            0,
            SeccompCmpArgLen::Dword,
            SeccompCmpOp::Ne,
            libc::AF_INET6 as u64,
        )
        .map_err(|_error| HelperFailure::ManagedContainmentPolicyCompile)?,
    ])
    .map_err(|_error| HelperFailure::ManagedContainmentPolicyCompile)?;
    let mut rules = BTreeMap::from([
        (libc::SYS_bpf, Vec::new()),
        (libc::SYS_fsconfig, Vec::new()),
        (libc::SYS_fsmount, Vec::new()),
        (libc::SYS_fsopen, Vec::new()),
        (libc::SYS_fspick, Vec::new()),
        (libc::SYS_io_uring_setup, Vec::new()),
        (libc::SYS_mknod, Vec::new()),
        (libc::SYS_mknodat, Vec::new()),
        (libc::SYS_mount, Vec::new()),
        (libc::SYS_mount_setattr, Vec::new()),
        (libc::SYS_move_mount, Vec::new()),
        (libc::SYS_open_tree, Vec::new()),
        (libc::SYS_pivot_root, Vec::new()),
        (libc::SYS_setns, Vec::new()),
        (libc::SYS_socket, vec![socket_rule]),
        (libc::SYS_umount2, Vec::new()),
        (libc::SYS_unshare, Vec::new()),
    ]);
    rules.insert(libc::SYS_clone, namespace_clone_rules()?);
    let filter = SeccompFilter::new(
        rules,
        SeccompAction::Allow,
        SeccompAction::Errno(libc::EPERM as u32),
        ARCH.try_into()
            .map_err(|_error| HelperFailure::ManagedContainmentPolicyCompile)?,
    )
    .map_err(|_error| HelperFailure::ManagedContainmentPolicyCompile)?;
    filter
        .try_into()
        .map_err(|_error| HelperFailure::ManagedContainmentPolicyCompile)
}

fn clone3_compatibility_filter(
    compile_failure: HelperFailure,
) -> Result<BpfProgram, HelperFailure> {
    let rules = BTreeMap::from([(libc::SYS_clone3, Vec::new())]);
    let filter = SeccompFilter::new(
        rules,
        SeccompAction::Allow,
        SeccompAction::Errno(libc::ENOSYS as u32),
        ARCH.try_into().map_err(|_error| compile_failure)?,
    )
    .map_err(|_error| compile_failure)?;
    filter.try_into().map_err(|_error| compile_failure)
}

fn namespace_clone_rules() -> Result<Vec<SeccompRule>, HelperFailure> {
    [
        libc::CLONE_NEWCGROUP,
        libc::CLONE_NEWIPC,
        libc::CLONE_NEWNET,
        libc::CLONE_NEWNS,
        libc::CLONE_NEWPID,
        libc::CLONE_NEWTIME,
        libc::CLONE_NEWUSER,
        libc::CLONE_NEWUTS,
    ]
    .into_iter()
    .map(|flag| {
        let flag =
            u64::try_from(flag).map_err(|_error| HelperFailure::ManagedContainmentPolicyCompile)?;
        SeccompRule::new(vec![
            SeccompCondition::new(
                0,
                SeccompCmpArgLen::Qword,
                SeccompCmpOp::MaskedEq(flag),
                flag,
            )
            .map_err(|_error| HelperFailure::ManagedContainmentPolicyCompile)?,
        ])
        .map_err(|_error| HelperFailure::ManagedContainmentPolicyCompile)
    })
    .collect()
}

fn build_network_filter() -> Result<BpfProgram, HelperFailure> {
    let mut rules = BTreeMap::from([
        (libc::SYS_io_uring_setup, Vec::new()),
        (libc::SYS_socket, Vec::new()),
    ]);
    rules.insert(libc::SYS_socketpair, build_socketpair_rules()?);
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

fn build_socketpair_rules() -> Result<Vec<SeccompRule>, HelperFailure> {
    let spawn_channel_type = u64::try_from(libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC)
        .map_err(|_error| HelperFailure::SocketPolicyCompile)?;
    [(0, libc::AF_UNIX as u64), (1, spawn_channel_type), (2, 0)]
        .into_iter()
        .map(|(argument, expected)| {
            SeccompRule::new(vec![
                SeccompCondition::new(
                    argument,
                    SeccompCmpArgLen::Dword,
                    SeccompCmpOp::Ne,
                    expected,
                )
                .map_err(|_error| HelperFailure::SocketPolicyCompile)?,
            ])
            .map_err(|_error| HelperFailure::SocketPolicyCompile)
        })
        .collect()
}

fn managed_policy_behaves_as_required() -> bool {
    denied_socket(AddressFamily::UNIX)
        && denied_socket(AddressFamily::VSOCK)
        && allowed_socket(AddressFamily::INET)
        && allowed_socket(AddressFamily::INET6)
        && denied_namespace_and_mount_operations()
}

fn denied_namespace_and_mount_operations() -> bool {
    #[expect(
        deprecated,
        reason = "the containment canary invokes the exact denied syscall"
    )]
    let unshare_denied =
        rustix::thread::unshare(UnshareFlags::empty()).is_err_and(|error| error == Errno::PERM);
    let mount_denied = mount("none", "/", "none", MountFlags::empty(), None)
        .is_err_and(|error| error == Errno::PERM);
    let mknod_denied = mknodat(
        CWD,
        "/dev/retonr-containment-canary",
        FileType::CharacterDevice,
        Mode::RUSR | Mode::WUSR,
        libc::makedev(1, 3),
    )
    .is_err_and(|error| error == Errno::PERM);
    unshare_denied && mount_denied && mknod_denied
}

fn build_network_policy_behaves_as_required() -> bool {
    [
        AddressFamily::UNIX,
        AddressFamily::VSOCK,
        AddressFamily::INET,
        AddressFamily::INET6,
    ]
    .into_iter()
    .all(denied_socket)
        && UnixStream::pair().is_err_and(|error| error.raw_os_error() == Some(libc::EPERM))
        && build_spawn_channel_policy_behaves_as_required()
}

fn build_spawn_channel_policy_behaves_as_required() -> bool {
    socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC,
        None,
    )
    .is_ok()
        && denied_socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::empty(),
            None,
        )
        && denied_socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )
        && denied_socketpair(
            AddressFamily::INET,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC,
            None,
        )
        && denied_socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC,
            Some(ipproto::TCP),
        )
}

fn denied_socketpair(
    family: AddressFamily,
    socket_type: SocketType,
    flags: SocketFlags,
    protocol: Option<Protocol>,
) -> bool {
    socketpair(family, socket_type, flags, protocol).is_err_and(|error| error == Errno::PERM)
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
        HelperFailure, build_network_filter, build_socketpair_rules, classify_filter_error,
        classify_managed_filter_error, clone3_compatibility_filter, managed_target_filter,
        namespace_clone_rules, status_reports_filter,
    };

    #[test]
    fn target_filter_compiles_for_the_current_architecture() {
        assert!(!managed_target_filter().expect("compile filter").is_empty());
        assert!(
            !clone3_compatibility_filter(HelperFailure::ManagedContainmentPolicyCompile)
                .expect("compile clone3 compatibility filter")
                .is_empty()
        );
        assert_eq!(namespace_clone_rules().expect("clone rules").len(), 8);
        assert_eq!(
            build_socketpair_rules()
                .expect("compile build socketpair rules")
                .len(),
            3
        );
        assert!(!build_network_filter().expect("compile filter").is_empty());
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
        assert_eq!(
            classify_managed_filter_error(seccompiler::Error::Seccomp(
                io::Error::from_raw_os_error(libc::EINVAL)
            )),
            HelperFailure::ManagedContainmentPolicyInstall
        );
    }
}
