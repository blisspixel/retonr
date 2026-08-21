use std::{
    fs::File,
    net::{Ipv4Addr, TcpListener, TcpStream},
    num::NonZeroU32,
    os::{
        fd::{AsRawFd, OwnedFd},
        unix::fs::MetadataExt,
    },
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use rewrite_types::CancellationToken;
use rustix::{
    io::{FdFlags, fcntl_setfd},
    net::{
        AddressFamily, Protocol, SocketFlags, SocketType, bind, connect, getpeername, getsockname,
        netlink::SocketAddrNetlink,
        socket_with,
        sockopt::{socket_domain, socket_protocol, socket_type},
    },
    process::geteuid,
};

#[test]
fn fresh_diagnostics_descriptor_has_the_required_native_shape() {
    let file = socket_diagnostics(true, true, false);
    assert_eq!(socket_domain(&file), Ok(AddressFamily::NETLINK));
    assert_eq!(socket_type(&file), Ok(SocketType::RAW));
    assert_eq!(
        socket_protocol(&file),
        Ok(Some(Protocol::from_raw(
            NonZeroU32::new(4).expect("SOCK_DIAG protocol is nonzero")
        )))
    );
    let local = getsockname(&file).expect("local socket address");
    let local = SocketAddrNetlink::try_from(local).expect("netlink local address");
    assert_ne!(local.pid(), 0);
    assert_eq!(local.groups(), 0);
    let peer = getpeername(&file).expect("peer socket address");
    let peer =
        SocketAddrNetlink::try_from(peer.expect("connected peer")).expect("netlink peer address");
    assert_eq!(peer.pid(), 0);
    assert_eq!(peer.groups(), 0);
}

use super::process_start_token;
use crate::{
    AttachedProcessEvidenceClass, AttachedProcessLaunchMode, AttachedProcessLease,
    AttachedProcessWitnessError, AttachedProcessWitnessLimits, ListenerEndpoint,
    ManagedLinuxProcessExpectation, NativeManagedLinuxProcessObserver, RetainedTcpConnection,
};

#[test]
fn managed_current_process_is_retained_and_reobserved() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind listener");
    let endpoint = ListenerEndpoint::new(listener.local_addr().expect("listener address"))
        .expect("valid endpoint");
    let mut lease = NativeManagedLinuxProcessObserver
        .attach(
            endpoint,
            socket_diagnostics(true, true, false),
            current_expectation(0, 0, 0, 0),
            AttachedProcessWitnessLimits::default(),
            &CancellationToken::new(),
        )
        .expect("attach managed process");
    assert_eq!(lease.initial_evidence().schema_version(), 2);
    assert_eq!(
        lease.initial_evidence().evidence_class(),
        AttachedProcessEvidenceClass::LinuxManagedNamespaceSockDiag
    );
    assert_eq!(
        lease.initial_evidence().launch_mode(),
        AttachedProcessLaunchMode::ManagedLinuxIsolation
    );
    let encoded = serde_json::to_string(lease.initial_evidence()).expect("serialize evidence");
    assert!(encoded.contains("linux_managed_namespace_sock_diag"));
    assert!(encoded.contains("managed_linux_isolation"));
    assert!(!encoded.contains("/proc/"));
    assert!(!encoded.contains("socket:["));
    assert_eq!(
        lease.reobserve(&CancellationToken::new()),
        Ok(lease.initial_evidence().clone())
    );
    drop(listener);
    assert!(matches!(
        lease.reobserve(&CancellationToken::new()),
        Err(AttachedProcessWitnessError::ListenerNotFound
            | AttachedProcessWitnessError::ListenerRebound)
    ));
}

#[test]
fn visible_holder_must_be_the_exact_expected_outer_pid() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind listener");
    let endpoint = ListenerEndpoint::new(listener.local_addr().expect("listener address"))
        .expect("valid endpoint");
    let mut child = Command::new("/bin/sleep")
        .arg("30")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn target-shaped child");
    let expected = expectation_for(child.id(), 0, 0, 0, 0);
    let result = NativeManagedLinuxProcessObserver.attach(
        endpoint,
        socket_diagnostics(true, true, false),
        expected,
        AttachedProcessWitnessLimits::default(),
        &CancellationToken::new(),
    );
    child.kill().expect("terminate child");
    child.wait().expect("reap child");
    assert!(matches!(
        result,
        Err(AttachedProcessWitnessError::ListenerRebound)
    ));
}

#[test]
fn host_diagnostics_cannot_replace_a_distinct_target_namespace_when_available() {
    let host_namespace = File::open("/proc/self/ns/net")
        .expect("open host network namespace")
        .metadata()
        .expect("host namespace metadata");
    let Ok(mut child) = Command::new("unshare")
        .args(["--user", "--map-root-user", "--net", "/bin/sleep", "30"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return;
    };
    let deadline = Instant::now() + Duration::from_secs(2);
    let isolated = loop {
        if child
            .try_wait()
            .expect("read unshare child status")
            .is_some()
        {
            return;
        }
        let metadata =
            File::open(format!("/proc/{}/ns/net", child.id())).and_then(|file| file.metadata());
        if metadata.is_ok_and(|metadata| {
            metadata.dev() != host_namespace.dev() || metadata.ino() != host_namespace.ino()
        }) {
            break true;
        }
        if Instant::now() >= deadline {
            break false;
        }
        thread::sleep(Duration::from_millis(5));
    };
    if !isolated {
        let _ = child.kill();
        let _ = child.wait();
        return;
    }

    let expected = expectation_for(child.id(), 0, 0, 0, 0);
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind host listener");
    let endpoint = ListenerEndpoint::new(listener.local_addr().expect("host listener address"))
        .expect("valid endpoint");
    let result = NativeManagedLinuxProcessObserver.attach(
        endpoint,
        socket_diagnostics(true, true, false),
        expected,
        AttachedProcessWitnessLimits::default(),
        &CancellationToken::new(),
    );
    child.kill().expect("terminate isolated child");
    child.wait().expect("reap isolated child");
    assert!(matches!(
        result,
        Err(AttachedProcessWitnessError::ListenerRebound
            | AttachedProcessWitnessError::ListenerNotFound)
    ));
}

#[test]
fn managed_connection_uses_the_supplied_diagnostics_session() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind listener");
    let endpoint = ListenerEndpoint::new(listener.local_addr().expect("listener address"))
        .expect("valid endpoint");
    let client = TcpStream::connect(endpoint.socket()).expect("connect client");
    let (server, _) = listener.accept().expect("accept connection");
    let connection = RetainedTcpConnection::new(
        client.local_addr().expect("client address"),
        server.local_addr().expect("server address"),
    )
    .expect("connection tuple");
    let mut lease = NativeManagedLinuxProcessObserver
        .attach(
            endpoint,
            socket_diagnostics(true, true, false),
            current_expectation(0, 0, 0, 0),
            AttachedProcessWitnessLimits::default(),
            &CancellationToken::new(),
        )
        .expect("attach managed process");
    let initial = lease
        .observe_connection(connection, &CancellationToken::new())
        .expect("observe exact connection");
    assert_eq!(
        lease.reobserve_connection(connection, &initial, &CancellationToken::new()),
        Ok(initial)
    );
    drop((client, server));
}

#[test]
fn expected_start_executable_namespace_uid_and_listener_are_exact() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind listener");
    let endpoint = ListenerEndpoint::new(listener.local_addr().expect("listener address"))
        .expect("valid endpoint");
    let limits = AttachedProcessWitnessLimits::default();
    let cancellation = CancellationToken::new();
    let current = current_expectation(0, 0, 0, 0);
    let wrong_pid = ManagedLinuxProcessExpectation::new(
        u32::MAX,
        current.process_start_token(),
        current.executable_device(),
        current.executable_inode(),
        current.executable_bytes(),
        current.network_namespace_device(),
        current.network_namespace_inode(),
        current.diagnostics_uid(),
    )
    .expect("wrong PID expectation");
    assert!(matches!(
        NativeManagedLinuxProcessObserver.attach(
            endpoint,
            socket_diagnostics(true, true, false),
            wrong_pid,
            limits,
            &cancellation,
        ),
        Err(AttachedProcessWitnessError::ProcessInstanceUnavailable)
    ));
    let cases = [
        (
            current_expectation(1, 0, 0, 0),
            AttachedProcessWitnessError::ProcessInstanceChanged,
        ),
        (
            current_expectation(0, 1, 0, 0),
            AttachedProcessWitnessError::EntrypointChanged,
        ),
        (
            current_expectation(0, 0, 1, 0),
            AttachedProcessWitnessError::ListenerSnapshotIncomplete,
        ),
        (
            current_expectation(0, 0, 0, 1),
            AttachedProcessWitnessError::ProcessAccessDenied,
        ),
    ];
    for (expected, error) in cases {
        let result = NativeManagedLinuxProcessObserver.attach(
            endpoint,
            socket_diagnostics(true, true, false),
            expected,
            limits,
            &cancellation,
        );
        assert!(matches!(result, Err(observed) if observed == error));
    }

    let unused = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind unused listener");
    let missing = ListenerEndpoint::new(unused.local_addr().expect("unused address"))
        .expect("valid unused endpoint");
    drop(unused);
    assert!(matches!(
        NativeManagedLinuxProcessObserver.attach(
            missing,
            socket_diagnostics(true, true, false),
            current_expectation(0, 0, 0, 0),
            limits,
            &cancellation,
        ),
        Err(AttachedProcessWitnessError::ListenerNotFound)
    ));
}

#[test]
fn descriptor_substitution_flags_binding_peer_and_consumption_fail_closed() {
    assert!(
        super::SockDiagSession::from_file(tempfile::tempfile().expect("temporary file")).is_err()
    );

    let tcp = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind TCP listener");
    let tcp: OwnedFd = tcp.into();
    assert!(super::SockDiagSession::from_file(File::from(tcp)).is_err());
    assert!(super::SockDiagSession::from_file(other_netlink_protocol()).is_err());
    assert!(super::SockDiagSession::from_file(socket_diagnostics(false, true, false)).is_err());
    assert!(super::SockDiagSession::from_file(socket_diagnostics(true, false, false)).is_err());
    assert!(super::SockDiagSession::from_file(socket_diagnostics(true, true, true)).is_ok());
    let without_close_on_exec = socket_diagnostics(true, true, false);
    fcntl_setfd(&without_close_on_exec, FdFlags::empty()).expect("clear close-on-exec flag");
    assert!(super::SockDiagSession::from_file(without_close_on_exec).is_err());

    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind listener");
    let endpoint = ListenerEndpoint::new(listener.local_addr().expect("listener address"))
        .expect("valid endpoint");
    let descriptor = socket_diagnostics(true, true, false);
    let token = CancellationToken::new();
    token.cancel();
    assert!(matches!(
        NativeManagedLinuxProcessObserver.attach(
            endpoint,
            descriptor,
            current_expectation(0, 0, 0, 0),
            AttachedProcessWitnessLimits::default(),
            &token,
        ),
        Err(AttachedProcessWitnessError::Cancelled)
    ));

    let invalid_limits = AttachedProcessWitnessLimits {
        maximum_processes: 0,
        ..AttachedProcessWitnessLimits::default()
    };
    assert!(matches!(
        NativeManagedLinuxProcessObserver.attach(
            endpoint,
            socket_diagnostics(true, true, false),
            current_expectation(0, 0, 0, 0),
            invalid_limits,
            &CancellationToken::new(),
        ),
        Err(AttachedProcessWitnessError::InvalidLimits)
    ));
}

#[test]
fn closed_diagnostics_number_cannot_authorize_a_reused_file() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind listener");
    let endpoint = ListenerEndpoint::new(listener.local_addr().expect("listener address"))
        .expect("valid endpoint");
    let descriptor = socket_diagnostics(true, true, false);
    let diagnostics_number = descriptor.as_raw_fd();
    let lease = NativeManagedLinuxProcessObserver
        .attach(
            endpoint,
            descriptor,
            current_expectation(0, 0, 0, 0),
            AttachedProcessWitnessLimits::default(),
            &CancellationToken::new(),
        )
        .expect("attach managed process");
    drop(lease);

    let mut retained = Vec::new();
    for _attempt in 0..32 {
        let replacement = tempfile::tempfile().expect("replacement file");
        if replacement.as_raw_fd() == diagnostics_number {
            assert!(super::SockDiagSession::from_file(replacement).is_err());
            return;
        }
        retained.push(replacement);
    }
    panic!("closed diagnostics number was not reused within the bounded test");
}

fn current_expectation(
    start_delta: u64,
    executable_inode_delta: u64,
    namespace_inode_delta: u64,
    diagnostics_uid_delta: u32,
) -> ManagedLinuxProcessExpectation {
    expectation_for(
        std::process::id(),
        start_delta,
        executable_inode_delta,
        namespace_inode_delta,
        diagnostics_uid_delta,
    )
}

fn expectation_for(
    pid: u32,
    start_delta: u64,
    executable_inode_delta: u64,
    namespace_inode_delta: u64,
    diagnostics_uid_delta: u32,
) -> ManagedLinuxProcessExpectation {
    let executable = File::open(format!("/proc/{pid}/exe")).expect("open executable");
    let executable = executable.metadata().expect("executable metadata");
    let namespace = File::open(format!("/proc/{pid}/ns/net")).expect("open network namespace");
    let namespace = namespace.metadata().expect("namespace metadata");
    ManagedLinuxProcessExpectation::new(
        pid,
        process_start_token(pid).expect("process start token") + start_delta,
        executable.dev(),
        executable.ino() + executable_inode_delta,
        executable.len(),
        namespace.dev(),
        namespace.ino() + namespace_inode_delta,
        geteuid().as_raw() + diagnostics_uid_delta,
    )
    .expect("managed expectation")
}

fn socket_diagnostics(nonblocking: bool, bound: bool, connected: bool) -> File {
    let flags = SocketFlags::CLOEXEC
        | if nonblocking {
            SocketFlags::NONBLOCK
        } else {
            SocketFlags::empty()
        };
    let descriptor = socket_with(
        AddressFamily::NETLINK,
        SocketType::RAW,
        flags,
        Some(Protocol::from_raw(
            NonZeroU32::new(4).expect("SOCK_DIAG protocol is nonzero"),
        )),
    )
    .expect("create socket diagnostics");
    if bound {
        bind(&descriptor, &SocketAddrNetlink::new(0, 0)).expect("bind socket diagnostics");
    }
    if connected {
        connect(&descriptor, &SocketAddrNetlink::new(0, 0)).expect("connect socket diagnostics");
    }
    File::from(descriptor)
}

fn other_netlink_protocol() -> File {
    let descriptor = socket_with(
        AddressFamily::NETLINK,
        SocketType::RAW,
        SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
        None,
    )
    .expect("create other netlink protocol");
    bind(&descriptor, &SocketAddrNetlink::new(0, 0)).expect("bind other netlink protocol");
    File::from(descriptor)
}
