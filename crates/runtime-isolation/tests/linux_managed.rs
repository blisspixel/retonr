#![cfg(target_os = "linux")]

use std::{
    fs::{self, File},
    io::Write as _,
    net::SocketAddr,
    os::unix::{fs::PermissionsExt as _, net::UnixListener},
    path::Path,
    time::{Duration, Instant},
};

use rewrite_runtime_isolation::{IsolationError, IsolationPolicy, LaunchSpec, PreparedIsolation};
use rewrite_types::CancellationToken;
use rustix::io::{FdFlags, fcntl_setfd};

const SOCKET_POLICY_SCRIPT: &str = r#"
import ctypes, errno, os, socket, time

try:
    connection = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    connection.connect(os.environ["HOST_UNIX_SOCKET"])
except OSError as error:
    if error.errno != errno.EPERM:
        raise
    print("AF_UNIX_BLOCKED", flush=True)
else:
    raise RuntimeError("host pathname Unix socket was reachable")

try:
    socket.socket(socket.AF_VSOCK, socket.SOCK_STREAM)
except OSError as error:
    if error.errno != errno.EPERM:
        raise
    print("AF_VSOCK_BLOCKED", flush=True)
else:
    raise RuntimeError("VSOCK creation was allowed")

library = ctypes.CDLL(None, use_errno=True)
parameters = (ctypes.c_byte * 256)()
result = library.syscall(
    int(os.environ["IO_URING_SETUP_SYSCALL"]),
    1,
    ctypes.byref(parameters),
)
if result != -1 or ctypes.get_errno() != errno.EPERM:
    if result >= 0:
        os.close(result)
    raise RuntimeError("io_uring setup was allowed")
print("IO_URING_BLOCKED", flush=True)

listener = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
listener.bind(("127.0.0.1", 43199))
listener.listen(1)
print("LOOPBACK_READY", flush=True)
connection, _ = listener.accept()
connection.recv(1)
time.sleep(60)
"#;

fn prepare_or_skip(
    helper: &str,
    policy: IsolationPolicy,
    cancellation: &CancellationToken,
) -> Option<PreparedIsolation> {
    match PreparedIsolation::prepare(helper, policy, cancellation) {
        Ok(prepared) => Some(prepared),
        Err(IsolationError::HostPolicyDenied)
            if std::env::var_os("REWRITE_ISOLATION_REQUIRE_NATIVE").is_none() =>
        {
            None
        }
        Err(error) => panic!("unexpected isolation preparation failure: {error}"),
    }
}

fn retained_replaced_python(python: &Path) -> (std::path::PathBuf, File) {
    let retained_path = std::env::temp_dir().join(format!(
        "rewrite-isolation-retained-target-{}",
        std::process::id()
    ));
    fs::copy(python, &retained_path).expect("copy retained executable");
    fs::set_permissions(&retained_path, fs::Permissions::from_mode(0o755))
        .expect("make retained executable runnable");
    let retained = File::open(&retained_path).expect("open retained executable");
    fs::remove_file(&retained_path).expect("unlink retained executable path");
    fs::copy("/bin/false", &retained_path).expect("replace executable path");
    fs::set_permissions(&retained_path, fs::Permissions::from_mode(0o755))
        .expect("make replacement executable runnable");
    (retained_path, retained)
}

#[test]
fn managed_launch_is_verified_or_host_policy_denies_it_exactly() {
    let helper = env!("CARGO_BIN_EXE_rewrite-runtime-isolation-helper");
    let inherited = File::open("/dev/null").expect("open inherited descriptor fixture");
    fcntl_setfd(&inherited, FdFlags::empty())
        .expect("clear close-on-exec on inherited descriptor fixture");
    let cancellation = CancellationToken::new();
    let policy = IsolationPolicy::new(
        Duration::from_secs(10),
        Duration::from_secs(5),
        8,
        8,
        4_096,
        256,
        64,
    )
    .expect("valid test policy");
    let Some(prepared) = prepare_or_skip(helper, policy, &cancellation) else {
        return;
    };
    assert_eq!(prepared.policy_digest(), policy.redacted_digest());
    assert!(prepared.preparation_evidence().all_canaries_passed());
    assert!(prepared.preparation_evidence().helper_bytes() > 0);
    assert_eq!(
        prepared
            .preparation_evidence()
            .helper_digest()
            .as_str()
            .len(),
        64
    );

    let mut launch = LaunchSpec::new("/bin/sh");
    launch.push_argument("-c");
    launch.push_argument("while :; do sleep 1; done");
    let mut lease = prepared
        .launch(&launch, &cancellation)
        .expect("launch isolated process tree");
    let initial = lease.initial_evidence();
    assert!(initial.guardian_pid() > 0);
    assert!(initial.network_namespace().inode() > 0);
    assert!(initial.user_namespace().inode() > 0);
    assert!(initial.process_namespace().inode() > 0);
    assert!(initial.target().outer_pid() > 0);
    assert!(initial.target().namespace_pid() > 1);
    assert!(initial.target().process_start_token() > 0);
    assert_eq!(initial.target().namespace_user_id(), 0);
    assert!(initial.target().executable_inode() > 0);
    assert!(initial.target().executable_bytes() > 0);
    assert_eq!(
        lease.reobserve(&cancellation).expect("reobserve lease"),
        initial
    );
    lease.close(&cancellation).expect("close process tree");
}

#[test]
fn cancelled_preparation_never_starts_the_helper() {
    let helper = env!("CARGO_BIN_EXE_rewrite-runtime-isolation-helper");
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert!(matches!(
        PreparedIsolation::prepare(helper, IsolationPolicy::default(), &cancellation),
        Err(IsolationError::Cancelled)
    ));
}

#[test]
fn retained_channel_transfers_exact_capabilities_and_bounded_capture() {
    let python = Path::new("/usr/bin/python3");
    if !python.is_file() {
        assert!(
            std::env::var_os("REWRITE_ISOLATION_REQUIRE_NATIVE").is_none(),
            "forced native isolation test requires /usr/bin/python3"
        );
        return;
    }
    let helper = env!("CARGO_BIN_EXE_rewrite-runtime-isolation-helper");
    let cancellation = CancellationToken::new();
    let policy = IsolationPolicy::new(
        Duration::from_secs(10),
        Duration::from_secs(5),
        8,
        8,
        256 * 1024,
        256,
        64,
    )
    .expect("valid test policy");
    let Some(prepared) = prepare_or_skip(helper, policy, &cancellation) else {
        return;
    };
    let endpoint = "127.0.0.1:43197"
        .parse::<SocketAddr>()
        .expect("literal endpoint");
    let script = r#"
import os, socket, sys, time
leaks = []
for descriptor in range(3, 256):
    try:
        os.fstat(descriptor)
        leaks.append(descriptor)
    except OSError:
        pass
if leaks:
    print("FD_LEAK", leaks, flush=True)
else:
    print("FD_OK", flush=True)
sys.stdout.buffer.write(b"x" * 131072)
sys.stdout.flush()
print("ERR_OK", file=sys.stderr, flush=True)
time.sleep(0.3)
listener = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
listener.bind(("127.0.0.1", 43197))
listener.listen(1)
connection, _ = listener.accept()
connection.recv(1)
time.sleep(60)
"#;
    let (retained_path, retained) = retained_replaced_python(python);
    let mut launch = LaunchSpec::new(&retained_path);
    launch.push_argument("-c");
    launch.push_argument(script);
    let mut lease = prepared
        .launch_retained(&launch, retained, &cancellation)
        .expect("launch retained isolated fixture");
    let connected_at = Instant::now();
    let channel = lease
        .connect_loopback(endpoint, &cancellation)
        .expect("connect exact isolated loopback channel");
    assert!(connected_at.elapsed() >= Duration::from_millis(200));
    assert_eq!(
        channel.stream().peer_addr().expect("peer endpoint"),
        endpoint
    );
    assert!(
        channel
            .startup_output()
            .standard_output()
            .starts_with(b"FD_OK\n")
    );
    assert!(channel.startup_output().standard_output_truncated());
    assert_eq!(channel.startup_output().standard_error(), b"ERR_OK\n");
    assert!(!channel.startup_output().standard_error_truncated());
    let debug = format!("{:?}", channel.startup_output());
    assert!(!debug.contains("FD_OK"));
    let (mut stream, diagnostics, _capture) = channel.into_parts();
    stream.write_all(&[1]).expect("write connected stream");
    let diagnostics = diagnostics.into_file();
    assert!(
        rustix::fs::fcntl_getfl(&diagnostics)
            .expect("diagnostics status flags")
            .contains(rustix::fs::OFlags::NONBLOCK)
    );
    assert!(matches!(
        lease.connect_loopback(endpoint, &cancellation),
        Err(IsolationError::ChannelAlreadyRequested)
    ));
    drop(diagnostics);
    drop(stream);
    lease.close(&cancellation).expect("close process tree");
    fs::remove_file(retained_path).expect("remove replacement executable");
}

#[test]
fn target_socket_policy_blocks_host_local_families_and_keeps_loopback() {
    let python = Path::new("/usr/bin/python3");
    if !python.is_file() {
        assert!(
            std::env::var_os("REWRITE_ISOLATION_REQUIRE_NATIVE").is_none(),
            "forced native isolation test requires /usr/bin/python3"
        );
        return;
    }
    let socket_path = std::env::temp_dir().join(format!(
        "rewrite-isolation-host-socket-{}",
        std::process::id()
    ));
    let _missing = fs::remove_file(&socket_path);
    let host_listener = UnixListener::bind(&socket_path).expect("bind host Unix listener");
    host_listener
        .set_nonblocking(true)
        .expect("make host Unix listener nonblocking");

    let helper = env!("CARGO_BIN_EXE_rewrite-runtime-isolation-helper");
    let cancellation = CancellationToken::new();
    let policy = IsolationPolicy::new(
        Duration::from_secs(10),
        Duration::from_secs(5),
        8,
        8,
        4_096,
        256,
        64,
    )
    .expect("valid test policy");
    let Some(prepared) = prepare_or_skip(helper, policy, &cancellation) else {
        fs::remove_file(socket_path).expect("remove host Unix socket");
        return;
    };
    let endpoint = "127.0.0.1:43199"
        .parse::<SocketAddr>()
        .expect("literal endpoint");
    let mut launch = LaunchSpec::new(python);
    launch.push_argument("-c");
    launch.push_argument(SOCKET_POLICY_SCRIPT);
    launch.insert_environment("HOST_UNIX_SOCKET", socket_path.as_os_str());
    launch.insert_environment(
        "IO_URING_SETUP_SYSCALL",
        libc::SYS_io_uring_setup.to_string(),
    );
    let mut lease = prepared
        .launch(&launch, &cancellation)
        .expect("launch socket-policy fixture");
    let channel = lease
        .connect_loopback(endpoint, &cancellation)
        .expect("connect allowed isolated loopback channel");
    assert_eq!(
        channel.startup_output().standard_output(),
        b"AF_UNIX_BLOCKED\nAF_VSOCK_BLOCKED\nIO_URING_BLOCKED\nLOOPBACK_READY\n"
    );
    assert!(matches!(
        host_listener.accept(),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
    ));
    let (mut stream, diagnostics, _capture) = channel.into_parts();
    stream.write_all(&[1]).expect("write connected stream");
    drop(diagnostics);
    drop(stream);
    lease.close(&cancellation).expect("close process tree");
    drop(host_listener);
    fs::remove_file(socket_path).expect("remove host Unix socket");
}

#[test]
fn never_listening_target_fails_within_the_channel_deadline() {
    let helper = env!("CARGO_BIN_EXE_rewrite-runtime-isolation-helper");
    let cancellation = CancellationToken::new();
    let policy = IsolationPolicy::new(
        Duration::from_secs(5),
        Duration::from_secs(5),
        8,
        8,
        4_096,
        256,
        64,
    )
    .expect("valid test policy");
    let Some(prepared) = prepare_or_skip(helper, policy, &cancellation) else {
        return;
    };
    let mut launch = LaunchSpec::new("/bin/sleep");
    launch.push_argument("60");
    let mut lease = prepared
        .launch(&launch, &cancellation)
        .expect("launch isolated non-listener");
    let started = Instant::now();
    let result = lease.connect_loopback(
        "127.0.0.1:43198".parse().expect("literal endpoint"),
        &cancellation,
    );
    assert!(
        matches!(
            result,
            Err(IsolationError::HelperProtocol | IsolationError::StartupTimeout)
        ),
        "unexpected channel result: {result:?}"
    );
    assert!(started.elapsed() >= Duration::from_secs(4));
    assert!(started.elapsed() < Duration::from_secs(6));
    lease
        .close(&cancellation)
        .expect("close failed channel lease");
}
