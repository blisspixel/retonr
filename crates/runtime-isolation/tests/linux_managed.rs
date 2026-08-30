#![cfg(target_os = "linux")]

use std::{
    fs::{self, File, OpenOptions},
    io::{Read as _, Write as _},
    net::{SocketAddr, TcpListener, TcpStream},
    os::unix::{
        fs::PermissionsExt as _,
        fs::{FileExt as _, FileTypeExt as _, MetadataExt as _},
        net::{UnixListener, UnixStream},
    },
    path::Path,
    process::Command,
    time::{Duration, Instant},
};

use rewrite_runtime_isolation::{
    ControlledBuildExecution, ControlledBuildInputFile, ControlledBuildLaunchSpec,
    ControlledBuildProcessStatus, IsolationError, IsolationPolicy, LaunchSpec,
    MANAGED_RUNTIME_INPUT_ROOT_V1, ManagedDeviceVisibilityPolicy, PreparedIsolation,
    RetainedRuntimeInputSink, RetainedRuntimeInputSource, RetainedRuntimeInputTree,
};
use rewrite_types::{CancellationToken, Digest};
use rustix::mount::{UnmountFlags, mount_bind, unmount};
use rustix::{
    fs::{CWD, Mode, mkfifoat},
    io::{FdFlags, fcntl_setfd},
};

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

const MANAGED_INPUT_FIXTURE: [(&str, &[u8]); 6] = [
    (
        "blobs/sha256-0000000000000000000000000000000000000000000000000000000000000001",
        b"config-v1",
    ),
    (
        "blobs/sha256-0000000000000000000000000000000000000000000000000000000000000002",
        b"weights",
    ),
    (
        "blobs/sha256-0000000000000000000000000000000000000000000000000000000000000003",
        b"template",
    ),
    (
        "blobs/sha256-0000000000000000000000000000000000000000000000000000000000000004",
        b"license",
    ),
    (
        "blobs/sha256-0000000000000000000000000000000000000000000000000000000000000005",
        b"parameters",
    ),
    (
        "manifests/registry.ollama.ai/library/granite3.3/2b",
        b"manifest",
    ),
];

struct ManagedInputSource(Vec<(String, Digest, u64, File)>);

impl RetainedRuntimeInputSource for ManagedInputSource {
    fn transfer(self, sink: &mut RetainedRuntimeInputSink<'_>) -> Result<(), IsolationError> {
        for (alias, digest, bytes, file) in self.0 {
            sink.retain(alias, digest, bytes, file)?;
        }
        Ok(())
    }
}

#[test]
fn managed_boundary_fixture_entrypoint() {
    if std::env::var_os("RETONR_TEST_MANAGED_BOUNDARY_FIXTURE").is_none() {
        return;
    }
    let listener = TcpListener::bind("127.0.0.1:43200").expect("bind isolated fixture");
    std::io::stdout()
        .write_all(b"BOUNDARY_READY\n")
        .expect("write fixture readiness");
    std::io::stdout().flush().expect("flush fixture readiness");
    let (mut connection, _) = listener.accept().expect("accept isolated fixture");
    let mut byte = [0_u8; 1];
    connection.read_exact(&mut byte).expect("read fixture byte");
    let mut entries = fs::read_dir("/dev")
        .expect("read private device root")
        .map(|entry| entry.expect("read private device entry").file_name())
        .collect::<Vec<_>>();
    entries.sort_unstable();
    assert_eq!(entries, ["null"]);
    let mut null = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/null")
        .expect("open retained null device");
    null.write_all(b"managed-boundary")
        .expect("write retained null device");
    assert_eq!(null.read(&mut byte).expect("read retained null device"), 0);
    drop(null);
    connection.write_all(b"D").expect("write device result");
    for denied in [
        "/dev/zero",
        "/dev/dri/renderD128",
        "/dev/nvidia0",
        "/dev/kfd",
        "/dev/dxg",
        "/dev/vfio/vfio",
    ] {
        assert_eq!(
            File::open(denied)
                .expect_err("device path must be absent")
                .kind(),
            std::io::ErrorKind::NotFound
        );
    }
    connection.write_all(b"A").expect("write absence result");
    let host_marker = std::env::var("RETONR_TEST_HOST_MARKER_PID")
        .expect("host marker")
        .parse::<u32>()
        .expect("numeric host marker");
    let mut numeric = fs::read_dir("/proc")
        .expect("read private procfs")
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_string_lossy().parse::<u32>().ok())
        .collect::<Vec<_>>();
    numeric.sort_unstable();
    assert!(numeric.contains(&1));
    assert!(numeric.contains(&std::process::id()));
    assert!(!numeric.contains(&host_marker));
    connection.write_all(b"P").expect("write proc result");
    let leaks = (3..256)
        .filter(|descriptor| {
            fs::metadata(format!("/proc/self/fd/{descriptor}"))
                .is_ok_and(|metadata| metadata.file_type().is_char_device())
        })
        .collect::<Vec<_>>();
    assert!(
        leaks.is_empty(),
        "unexpected inherited descriptors: {leaks:?}"
    );
    connection.write_all(b"L").expect("write descriptor result");
    assert_eq!(std::thread::spawn(|| 7).join().expect("ordinary thread"), 7);
    connection.write_all(b"T").expect("write thread result");
    assert!(
        Command::new("/bin/true")
            .status()
            .expect("ordinary child")
            .success()
    );
    connection.write_all(b"C").expect("write child result");
    connection.write_all(b"O").expect("write fixture result");
    drop(connection);
    std::thread::sleep(Duration::from_mins(1));
}

#[test]
fn managed_runtime_input_fixture_entrypoint() {
    if std::env::var_os("RETONR_TEST_MANAGED_INPUT_FIXTURE").is_none() {
        return;
    }
    let root = Path::new(MANAGED_RUNTIME_INPUT_ROOT_V1);
    let mut observed = Vec::new();
    collect_runtime_input_files(root, root, &mut observed);
    let expected = MANAGED_INPUT_FIXTURE
        .iter()
        .map(|(alias, _)| (*alias).to_owned())
        .collect::<Vec<_>>();
    assert_eq!(observed, expected);
    let materialized = fs::metadata(root.join(MANAGED_INPUT_FIXTURE[0].0))
        .expect("materialized runtime input metadata");
    let source_identity = (
        std::env::var("RETONR_TEST_SOURCE_DEVICE")
            .expect("source device")
            .parse::<u64>()
            .expect("numeric source device"),
        std::env::var("RETONR_TEST_SOURCE_INODE")
            .expect("source inode")
            .parse::<u64>()
            .expect("numeric source inode"),
    );
    assert_ne!((materialized.dev(), materialized.ino()), source_identity);
    for (alias, contents) in MANAGED_INPUT_FIXTURE {
        let target = root.join(alias);
        assert_eq!(
            fs::read(&target).expect("read retained runtime input"),
            contents
        );
        assert!(matches!(
            OpenOptions::new()
                .write(true)
                .open(&target)
                .expect_err("retained runtime input must be read only")
                .kind(),
            std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::ReadOnlyFilesystem
        ));
    }
    assert!(matches!(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(root.join("ambient-substitution"))
            .expect_err("retained runtime input root must be read only")
            .kind(),
        std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::ReadOnlyFilesystem
    ));
    assert_eq!(
        std::thread::spawn(|| 11).join().expect("ordinary thread"),
        11
    );
    assert!(
        Command::new("/bin/true")
            .status()
            .expect("ordinary child")
            .success()
    );
    let listener = TcpListener::bind("127.0.0.1:43201").expect("bind isolated input fixture");
    std::io::stdout()
        .write_all(b"INPUT_READY\n")
        .expect("write input readiness");
    std::io::stdout().flush().expect("flush input readiness");
    let (mut connection, _) = listener.accept().expect("accept isolated input fixture");
    connection.write_all(b"I").expect("write input result");
    let mut byte = [0_u8; 1];
    connection.read_exact(&mut byte).expect("read close signal");
    assert_eq!(byte, [b'Q']);
    connection.write_all(b"O").expect("write completion");
    drop(connection);
    std::thread::sleep(Duration::from_mins(1));
}

#[test]
fn never_listening_fixture_entrypoint() {
    if std::env::var_os("RETONR_TEST_NEVER_LISTENING_FIXTURE").is_some() {
        std::thread::sleep(Duration::from_mins(1));
    }
}

fn collect_runtime_input_files(root: &Path, directory: &Path, observed: &mut Vec<String>) {
    let mut entries = fs::read_dir(directory)
        .expect("read retained runtime input directory")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect retained runtime input directory");
    entries.sort_unstable_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let metadata = entry.metadata().expect("runtime input metadata");
        if metadata.is_dir() {
            collect_runtime_input_files(root, &path, observed);
        } else {
            assert!(metadata.is_file());
            observed.push(
                path.strip_prefix(root)
                    .expect("runtime input relative path")
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
}

#[test]
fn controlled_build_fixture_entrypoint() {
    if std::env::var_os("RETONR_TEST_CONTROLLED_BUILD_FIXTURE").is_none() {
        return;
    }
    let input = std::env::var("RETONR_CONTROLLED_BUILD_INPUT_FD").expect("input descriptor");
    let output = std::env::var("RETONR_CONTROLLED_BUILD_OUTPUT_FD").expect("output descriptor");
    assert_eq!(
        std::env::var("RETONR_CONTROLLED_BUILD_CAPABILITY_ABI").expect("capability ABI"),
        "2"
    );
    assert!(input.parse::<u32>().expect("input descriptor number") > 2);
    assert!(output.parse::<u32>().expect("output descriptor number") > 2);
    let input_root = std::env::var("RETONR_CONTROLLED_BUILD_INPUT_ROOT").expect("input root");
    let output_root = std::env::var("RETONR_CONTROLLED_BUILD_OUTPUT_ROOT").expect("output root");
    assert_eq!(input_root, "/tmp/retonr-controlled-build/input");
    assert_eq!(output_root, "/tmp/retonr-controlled-build/output");
    assert_controlled_build_network_denied();
    let input_path = Path::new(&input_root).join("allowed.txt");
    assert_eq!(
        fs::read(&input_path).expect("read retained input"),
        b"allowed"
    );
    assert!(matches!(
        OpenOptions::new()
            .write(true)
            .open(&input_path)
            .expect_err("private input member must be read only")
            .kind(),
        std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::ReadOnlyFilesystem
    ));
    let mut dev_null = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/null")
        .expect("open exact null device capability");
    dev_null.write_all(b"canary").expect("write null device");
    let mut byte = [0_u8; 1];
    assert_eq!(dev_null.read(&mut byte).expect("read null device"), 0);
    assert_eq!(
        File::open("/dev/zero")
            .expect_err("other device must be denied")
            .kind(),
        std::io::ErrorKind::PermissionDenied
    );
    assert_eq!(
        File::open("/etc/passwd")
            .expect_err("host read must be denied")
            .kind(),
        std::io::ErrorKind::PermissionDenied
    );
    let retained_metadata = fs::metadata(&output_root).expect("read mounted output");
    let current_metadata = fs::metadata(".").expect("read current output");
    assert_eq!(
        (retained_metadata.dev(), retained_metadata.ino()),
        (current_metadata.dev(), current_metadata.ino())
    );
    if std::env::var_os("RETONR_TEST_CONTROLLED_BUILD_SUBSTITUTION").is_some() {
        std::thread::sleep(Duration::from_millis(250));
        fs::write(
            "substitution-result.txt",
            fs::read(&input_path).expect("read retained substituted member"),
        )
        .expect("write substitution result");
        std::thread::sleep(Duration::from_millis(500));
        return;
    }
    if std::env::var_os("RETONR_TEST_CONTROLLED_BUILD_IN_PLACE_MUTATION").is_some() {
        std::thread::sleep(Duration::from_millis(250));
        fs::write(
            "mutation-result.txt",
            fs::read(&input_path).expect("read snapshotted mutated member"),
        )
        .expect("write mutation result");
        std::thread::sleep(Duration::from_millis(500));
        return;
    }
    if std::env::var_os("RETONR_TEST_CONTROLLED_BUILD_OUTPUT_FIFO").is_some() {
        mkfifoat(CWD, "blocked.fifo", Mode::RUSR | Mode::WUSR)
            .expect("create rejected output FIFO");
        return;
    }
    let descendant_program = Path::new(&output_root).join("descendant-fixture");
    fs::copy(
        format!("/proc/self/fd/{input}/build-fixture"),
        &descendant_program,
    )
    .expect("copy controlled descendant fixture");
    fs::set_permissions(&descendant_program, fs::Permissions::from_mode(0o755))
        .expect("make controlled descendant executable");
    let child = Command::new(descendant_program)
        .arg("--exact")
        .arg("controlled_build_descendant_fixture_entrypoint")
        .env("RETONR_TEST_CONTROLLED_BUILD_DESCENDANT", &output_root)
        .status()
        .expect("launch controlled descendant");
    assert!(child.success());
    fs::write("result.txt", b"built").expect("write retained output");
}

fn assert_controlled_build_network_denied() {
    assert_eq!(
        TcpStream::connect("127.0.0.1:9")
            .expect_err("controlled build cannot create an Internet socket")
            .kind(),
        std::io::ErrorKind::PermissionDenied
    );
    assert_eq!(
        UnixStream::pair()
            .expect_err("controlled build cannot create a local socket")
            .kind(),
        std::io::ErrorKind::PermissionDenied
    );
}

#[test]
fn controlled_build_descendant_fixture_entrypoint() {
    let Some(retained_cwd) = std::env::var_os("RETONR_TEST_CONTROLLED_BUILD_DESCENDANT") else {
        return;
    };
    fs::write(
        Path::new(&retained_cwd).join("descendant-result.txt"),
        b"descendant",
    )
    .expect("write through retained coordinator cwd");
}

#[test]
fn controlled_build_is_filesystem_confined_or_host_policy_denies_it() {
    if std::env::var_os("RETONR_TEST_CONTROLLED_BUILD_FIXTURE").is_some() {
        return;
    }
    let helper = helper_path();
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
    .expect("valid build policy");
    let Some(prepared) = prepare_retained_or_skip(&helper, policy, &cancellation) else {
        return;
    };
    let input = tempfile::tempdir_in("/tmp").expect("input root under host temporary root");
    let output = tempfile::tempdir_in("/tmp").expect("output root under host temporary root");
    fs::write(input.path().join("allowed.txt"), b"allowed").expect("write input fixture");
    let program_path = input.path().join("build-fixture");
    fs::copy(
        std::env::current_exe().expect("test executable"),
        &program_path,
    )
    .expect("copy build fixture");
    fs::set_permissions(&program_path, fs::Permissions::from_mode(0o755))
        .expect("make build fixture executable");
    let program_bytes = fs::read(&program_path).expect("read build fixture");
    let mut specification = ControlledBuildLaunchSpec::new(
        "build-fixture",
        Digest::sha256(&program_bytes),
        u64::try_from(program_bytes.len()).expect("fixture length"),
        Duration::from_secs(30),
    )
    .expect("valid controlled build");
    specification.push_argument("--exact");
    specification.push_argument("controlled_build_fixture_entrypoint");
    specification.insert_environment("RETONR_TEST_CONTROLLED_BUILD_FIXTURE", "1");
    assert_snapshot_boundaries(
        &prepared,
        &specification,
        input.path(),
        &program_path,
        &cancellation,
    );
    assert_nested_input_mount_rejected(
        &prepared,
        &specification,
        input.path(),
        &program_path,
        &cancellation,
    );
    assert_special_entries_fail_without_blocking(
        &prepared,
        &specification,
        input.path(),
        &program_path,
        &cancellation,
    );
    let input_files = retained_input_files(input.path());
    let expected_launch_digest = specification.redacted_digest_with_inputs(&input_files);
    let execution = prepared
        .run_controlled_build_retained(
            &specification,
            File::open(&program_path).expect("open retained program"),
            File::open(input.path()).expect("open retained input"),
            input_files,
            File::open(output.path()).expect("open retained output"),
            &cancellation,
        )
        .expect("run controlled build");
    assert_controlled_build_execution(&execution, &expected_launch_digest, output.path());
    assert_eq!(
        prepared.run_controlled_build_retained(
            &specification,
            File::open(&program_path).expect("reopen retained program"),
            File::open(input.path()).expect("reopen retained input"),
            retained_input_files(input.path()),
            File::open(output.path()).expect("reopen nonempty output"),
            &cancellation,
        ),
        Err(IsolationError::ControlledBuildOutputNotEmpty)
    );
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    let cancelled_output =
        tempfile::tempdir_in("/tmp").expect("cancelled output under host temporary root");
    assert_eq!(
        prepared.run_controlled_build_retained(
            &specification,
            File::open(&program_path).expect("reopen cancelled program"),
            File::open(input.path()).expect("reopen cancelled input"),
            retained_input_files(input.path()),
            File::open(cancelled_output.path()).expect("open cancelled output"),
            &cancelled,
        ),
        Err(IsolationError::Cancelled)
    );
}

fn assert_snapshot_boundaries(
    prepared: &PreparedIsolation,
    specification: &ControlledBuildLaunchSpec,
    input: &Path,
    program: &Path,
    cancellation: &CancellationToken,
) {
    assert_path_substitution_uses_retained_bytes(
        prepared,
        specification,
        input,
        program,
        cancellation,
    );
    assert_in_place_mutation_uses_snapshotted_bytes(
        prepared,
        specification,
        input,
        program,
        cancellation,
    );
    assert_digest_mismatch_is_rejected(prepared, specification, input, program, cancellation);
    assert_aggregate_input_limit_is_rejected(prepared, specification, input, program, cancellation);
}

fn assert_special_entries_fail_without_blocking(
    prepared: &PreparedIsolation,
    specification: &ControlledBuildLaunchSpec,
    input: &Path,
    program: &Path,
    cancellation: &CancellationToken,
) {
    let input_fifo = input.join("blocked-input.fifo");
    mkfifoat(CWD, &input_fifo, Mode::RUSR | Mode::WUSR).expect("create rejected input FIFO");
    let input_output = tempfile::tempdir_in("/tmp").expect("input-FIFO rejection output");
    let started = Instant::now();
    assert!(matches!(
        prepared.run_controlled_build_retained(
            specification,
            File::open(program).expect("open input-FIFO program"),
            File::open(input).expect("open input with FIFO"),
            retained_input_files(input),
            File::open(input_output.path()).expect("open input-FIFO output"),
            cancellation,
        ),
        Err(IsolationError::ControlledBuildObjectMismatch | IsolationError::HelperProtocol)
    ));
    assert!(started.elapsed() < Duration::from_secs(5));
    fs::remove_file(&input_fifo).expect("remove rejected input FIFO");

    let mut output_specification = specification.clone();
    output_specification.insert_environment("RETONR_TEST_CONTROLLED_BUILD_OUTPUT_FIFO", "1");
    let output = tempfile::tempdir_in("/tmp").expect("output-FIFO rejection output");
    let started = Instant::now();
    let result = prepared.run_controlled_build_retained(
        &output_specification,
        File::open(program).expect("open output-FIFO program"),
        File::open(input).expect("open output-FIFO input"),
        retained_input_files(input),
        File::open(output.path()).expect("open output-FIFO output"),
        cancellation,
    );
    assert!(
        matches!(result, Err(IsolationError::ControlledBuildObjectMismatch)),
        "unexpected output-FIFO result: {result:?}"
    );
    assert!(started.elapsed() < Duration::from_secs(5));
}

fn assert_controlled_build_execution(
    execution: &ControlledBuildExecution,
    expected_launch_digest: &Digest,
    output: &Path,
) {
    assert_eq!(
        execution.output().status(),
        ControlledBuildProcessStatus::Success,
        "controlled stdout: {}\ncontrolled stderr: {}",
        String::from_utf8_lossy(execution.output().streams().standard_output()),
        String::from_utf8_lossy(execution.output().streams().standard_error())
    );
    let isolation = execution.isolation();
    assert_eq!(isolation.landlock_abi(), 3);
    assert!(isolation.guardian_pid() > 0);
    assert!(isolation.namespace_init_pid() > 0);
    for namespace in [
        isolation.network_namespace(),
        isolation.user_namespace(),
        isolation.process_namespace(),
        isolation.mount_namespace(),
    ] {
        assert!(namespace.inode() > 0);
    }
    assert_eq!(isolation.launch_digest(), expected_launch_digest);
    assert_eq!(isolation.redacted_digest().as_str().len(), 64);
    let tree = execution.output().tree().expect("committed output tree");
    assert_eq!(tree.regular_file_count(), 3);
    assert_eq!(
        fs::read(output.join("result.txt")).expect("read build output"),
        b"built"
    );
    assert_eq!(
        fs::read(output.join("descendant-result.txt")).expect("read descendant output"),
        b"descendant"
    );
}

fn assert_path_substitution_uses_retained_bytes(
    prepared: &PreparedIsolation,
    specification: &ControlledBuildLaunchSpec,
    input: &Path,
    program: &Path,
    cancellation: &CancellationToken,
) {
    let replacements = tempfile::tempdir_in("/tmp").expect("substitution replacements");
    let replacement = replacements.path().join("replacement.txt");
    let backup = replacements.path().join("retained.txt");
    fs::write(&replacement, b"swapped").expect("write substituted bytes");
    let token = format!(
        "{}-{}",
        std::process::id(),
        Instant::now().elapsed().as_nanos()
    );
    let exact_environment = format!("RETONR_TEST_CONTROLLED_BUILD_SUBSTITUTION={token}");
    let allowed = input.join("allowed.txt");
    let watcher = std::thread::spawn({
        let allowed = allowed.clone();
        let replacement = replacement.clone();
        let backup = backup.clone();
        move || {
            let started = Instant::now();
            while !process_with_environment(exact_environment.as_bytes()) {
                assert!(started.elapsed() < Duration::from_secs(15));
                std::thread::sleep(Duration::from_millis(5));
            }
            fs::rename(&allowed, &backup).expect("detach retained input pathname");
            fs::rename(&replacement, &allowed).expect("substitute input pathname");
            std::thread::sleep(Duration::from_millis(350));
            fs::remove_file(&allowed).expect("remove substituted pathname");
            fs::rename(&backup, &allowed).expect("restore retained input pathname");
        }
    });
    let output = tempfile::tempdir_in("/tmp").expect("substitution output");
    let mut substituted = specification.clone();
    substituted.insert_environment("RETONR_TEST_CONTROLLED_BUILD_SUBSTITUTION", token);
    let execution = prepared
        .run_controlled_build_retained(
            &substituted,
            File::open(program).expect("open substitution program"),
            File::open(input).expect("open substitution input"),
            retained_input_files(input),
            File::open(output.path()).expect("open substitution output"),
            cancellation,
        )
        .expect("run substitution build");
    watcher.join().expect("substitution watcher");
    assert_eq!(
        execution.output().status(),
        ControlledBuildProcessStatus::Success
    );
    assert_eq!(
        fs::read(output.path().join("substitution-result.txt")).expect("read substitution result"),
        b"allowed"
    );
}

fn assert_in_place_mutation_uses_snapshotted_bytes(
    prepared: &PreparedIsolation,
    specification: &ControlledBuildLaunchSpec,
    input: &Path,
    program: &Path,
    cancellation: &CancellationToken,
) {
    let token = format!("{}-mutation", std::process::id());
    let exact_environment = format!("RETONR_TEST_CONTROLLED_BUILD_IN_PLACE_MUTATION={token}");
    let allowed = input.join("allowed.txt");
    let watcher = std::thread::spawn({
        let allowed = allowed.clone();
        move || {
            let started = Instant::now();
            while !process_with_environment(exact_environment.as_bytes()) {
                assert!(started.elapsed() < Duration::from_secs(15));
                std::thread::sleep(Duration::from_millis(5));
            }
            fs::write(&allowed, b"mutated").expect("mutate retained inode in place");
            std::thread::sleep(Duration::from_millis(350));
            fs::write(&allowed, b"allowed").expect("restore retained inode in place");
        }
    });
    let output = tempfile::tempdir_in("/tmp").expect("mutation output");
    let mut mutated = specification.clone();
    mutated.insert_environment("RETONR_TEST_CONTROLLED_BUILD_IN_PLACE_MUTATION", token);
    let execution = prepared
        .run_controlled_build_retained(
            &mutated,
            File::open(program).expect("open mutation program"),
            File::open(input).expect("open mutation input"),
            retained_input_files(input),
            File::open(output.path()).expect("open mutation output"),
            cancellation,
        )
        .expect("run in-place mutation build");
    watcher.join().expect("mutation watcher");
    assert_eq!(
        execution.output().status(),
        ControlledBuildProcessStatus::Success
    );
    assert_eq!(
        fs::read(output.path().join("mutation-result.txt")).expect("read mutation result"),
        b"allowed"
    );
}

fn process_with_environment(exact: &[u8]) -> bool {
    fs::read_dir("/proc").is_ok_and(|processes| {
        processes.filter_map(Result::ok).any(|process| {
            process
                .file_name()
                .to_str()
                .is_some_and(|name| name.bytes().all(|byte| byte.is_ascii_digit()))
                && fs::read(process.path().join("environ")).is_ok_and(|environment| {
                    environment
                        .split(|byte| *byte == 0)
                        .any(|entry| entry == exact)
                })
        })
    })
}

fn assert_nested_input_mount_rejected(
    prepared: &PreparedIsolation,
    specification: &ControlledBuildLaunchSpec,
    input: &Path,
    program: &Path,
    cancellation: &CancellationToken,
) {
    let mounted_source = tempfile::tempdir_in("/tmp").expect("nested mounted input source");
    fs::write(mounted_source.path().join("mounted.txt"), b"mounted")
        .expect("write mounted input fixture");
    let nested_mount = input.join("nested-mount");
    fs::create_dir(&nested_mount).expect("create nested input mount point");
    match mount_bind(mounted_source.path(), &nested_mount) {
        Ok(()) => {
            let output = tempfile::tempdir_in("/tmp").expect("nested-mount rejection output");
            assert!(matches!(
                prepared.run_controlled_build_retained(
                    specification,
                    File::open(program).expect("open nested-mount program"),
                    File::open(input).expect("open nested-mount input"),
                    retained_input_files(input),
                    File::open(output.path()).expect("open nested-mount output"),
                    cancellation,
                ),
                Err(IsolationError::ControlledBuildObjectMismatch
                    | IsolationError::FilesystemAliasSetup("input-invalid"))
            ));
            unmount(&nested_mount, UnmountFlags::empty()).expect("unmount nested input fixture");
        }
        Err(error) => assert!(
            std::env::var_os("REWRITE_ISOLATION_REQUIRE_NATIVE").is_none(),
            "forced native test could not create nested input mount: {error}"
        ),
    }
    fs::remove_dir(&nested_mount).expect("remove nested input mount point");
}

fn assert_digest_mismatch_is_rejected(
    prepared: &PreparedIsolation,
    specification: &ControlledBuildLaunchSpec,
    input: &Path,
    program: &Path,
    cancellation: &CancellationToken,
) {
    let output = tempfile::tempdir_in("/tmp").expect("digest-mismatch output");
    let mut input_files = retained_input_files(input);
    let allowed = input_files
        .iter()
        .position(|file| file.relative_path() == "allowed.txt")
        .expect("allowed declaration");
    input_files[allowed] = ControlledBuildInputFile::new(
        "allowed.txt",
        Digest::sha256(b"not-allowed"),
        7,
        File::open(input.join("allowed.txt")).expect("open digest-mismatch input"),
    )
    .expect("digest-mismatch declaration");
    assert_eq!(
        prepared.run_controlled_build_retained(
            specification,
            File::open(program).expect("open digest-mismatch program"),
            File::open(input).expect("open digest-mismatch input root"),
            input_files,
            File::open(output.path()).expect("open digest-mismatch output"),
            cancellation,
        ),
        Err(IsolationError::ControlledBuildObjectMismatch)
    );
}

fn assert_aggregate_input_limit_is_rejected(
    prepared: &PreparedIsolation,
    specification: &ControlledBuildLaunchSpec,
    input: &Path,
    program: &Path,
    cancellation: &CancellationToken,
) {
    let oversized_path = input.join("sparse-limit.bin");
    let oversized = File::create(&oversized_path).expect("create sparse input");
    oversized
        .set_len(rewrite_runtime_isolation::MAXIMUM_CONTROLLED_BUILD_INPUT_BYTES)
        .expect("size sparse input");
    drop(oversized);
    let mut input_files = retained_input_files(input);
    input_files.push(
        ControlledBuildInputFile::new(
            "sparse-limit.bin",
            Digest::sha256(b"unread-sparse-input"),
            rewrite_runtime_isolation::MAXIMUM_CONTROLLED_BUILD_INPUT_BYTES,
            File::open(&oversized_path).expect("open sparse input"),
        )
        .expect("sparse declaration"),
    );
    let output = tempfile::tempdir_in("/tmp").expect("aggregate-limit output");
    let started = Instant::now();
    assert_eq!(
        prepared.run_controlled_build_retained(
            specification,
            File::open(program).expect("open aggregate-limit program"),
            File::open(input).expect("open aggregate-limit input root"),
            input_files,
            File::open(output.path()).expect("open aggregate-limit output"),
            cancellation,
        ),
        Err(IsolationError::ControlledBuildObjectMismatch)
    );
    assert!(started.elapsed() < Duration::from_secs(5));
    fs::remove_file(oversized_path).expect("remove sparse input");
}

fn retained_input_files(input: &Path) -> Vec<ControlledBuildInputFile> {
    retained_input_files_for(input, &["allowed.txt", "build-fixture"])
}

fn retained_input_files_for(
    input: &Path,
    relative_paths: &[&str],
) -> Vec<ControlledBuildInputFile> {
    relative_paths
        .iter()
        .copied()
        .map(|relative| {
            let bytes = fs::read(input.join(relative)).expect("read retained input member");
            ControlledBuildInputFile::new(
                relative,
                Digest::sha256(&bytes),
                u64::try_from(bytes.len()).expect("retained input length"),
                File::open(input.join(relative)).expect("open retained input member"),
            )
            .expect("valid retained input member")
        })
        .collect()
}

fn prepare_retained_or_skip(
    helper: &Path,
    policy: IsolationPolicy,
    cancellation: &CancellationToken,
) -> Option<PreparedIsolation> {
    let (digest, bytes) = helper_identity(helper);
    match PreparedIsolation::prepare_retained(
        File::open(helper).expect("open retained isolation helper"),
        &digest,
        bytes,
        policy,
        cancellation,
    ) {
        Ok(prepared) => Some(prepared),
        Err(IsolationError::HostPolicyDenied)
            if std::env::var_os("REWRITE_ISOLATION_REQUIRE_NATIVE").is_none() =>
        {
            None
        }
        Err(error) => panic!("unexpected retained isolation preparation failure: {error}"),
    }
}

fn prepare_or_skip(
    helper: &Path,
    policy: IsolationPolicy,
    cancellation: &CancellationToken,
) -> Option<PreparedIsolation> {
    let (digest, bytes) = helper_identity(helper);
    match PreparedIsolation::prepare(helper, &digest, bytes, policy, cancellation) {
        Ok(prepared) => Some(prepared),
        Err(IsolationError::HostPolicyDenied)
            if std::env::var_os("REWRITE_ISOLATION_REQUIRE_NATIVE").is_none() =>
        {
            None
        }
        Err(error) => panic!("unexpected isolation preparation failure: {error}"),
    }
}

fn helper_identity(helper: &Path) -> (Digest, u64) {
    let bytes = fs::read(helper).expect("read isolation helper identity");
    (
        Digest::sha256(&bytes),
        u64::try_from(bytes.len()).expect("helper byte length"),
    )
}

fn helper_path() -> std::path::PathBuf {
    std::env::var_os("REWRITE_ISOLATION_TEST_HELPER").map_or_else(
        || std::path::PathBuf::from(env!("CARGO_BIN_EXE_rewrite-runtime-isolation-helper")),
        std::path::PathBuf::from,
    )
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
    let helper = helper_path();
    let host_mount = fs::metadata("/proc/self/ns/mnt").expect("host mount namespace");
    let inherited = File::open("/dev/zero").expect("open inherited descriptor fixture");
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
    let Some(prepared) = prepare_or_skip(&helper, policy, &cancellation) else {
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

    let mut launch = LaunchSpec::new(std::env::current_exe().expect("managed fixture executable"));
    launch.push_argument("--exact");
    launch.push_argument("managed_boundary_fixture_entrypoint");
    launch.push_argument("--nocapture");
    launch.insert_environment("RETONR_TEST_MANAGED_BOUNDARY_FIXTURE", "1");
    launch.insert_environment(
        "RETONR_TEST_HOST_MARKER_PID",
        std::process::id().to_string(),
    );
    let mut lease = prepared
        .launch(&launch, &cancellation)
        .expect("launch isolated process tree");
    assert_eq!(lease.launch_spec_digest(), &launch.redacted_digest());
    assert_eq!(lease.isolation_policy_digest(), &policy.redacted_digest());
    let initial = lease.initial_evidence();
    assert!(initial.guardian_pid() > 0);
    assert!(initial.network_namespace().inode() > 0);
    assert!(initial.user_namespace().inode() > 0);
    assert!(initial.process_namespace().inode() > 0);
    assert_ne!(
        (
            initial.mount_namespace().device(),
            initial.mount_namespace().inode()
        ),
        (host_mount.dev(), host_mount.ino())
    );
    assert_eq!(
        initial.device_boundary().policy(),
        ManagedDeviceVisibilityPolicy::LinuxCpuOnlyV1
    );
    assert!(initial.device_boundary().all_visibility_canaries_passed());
    assert_eq!(initial.device_boundary().visible_device_entries(), 1);
    assert_eq!(initial.device_boundary().null_device_numbers(), (1, 3));
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
    let channel = lease
        .connect_loopback(
            "127.0.0.1:43200".parse().expect("literal endpoint"),
            &cancellation,
        )
        .expect("connect managed boundary fixture");
    assert!(
        channel
            .startup_output()
            .standard_output()
            .ends_with(b"BOUNDARY_READY\n")
    );
    let (mut stream, diagnostics, _capture) = channel.into_parts();
    stream
        .write_all(&[1])
        .expect("release managed boundary fixture");
    let mut boundary_result = Vec::new();
    stream
        .read_to_end(&mut boundary_result)
        .expect("read managed boundary result");
    assert_eq!(&boundary_result, b"DAPLTCO");
    drop(diagnostics);
    drop(stream);
    lease.close(&cancellation).expect("close process tree");
}

#[test]
fn retained_runtime_inputs_are_private_exact_and_reobserved() {
    if std::env::var_os("RETONR_TEST_MANAGED_INPUT_FIXTURE").is_some() {
        return;
    }
    let helper = helper_path();
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
    .expect("valid input policy");
    let Some(prepared) = prepare_or_skip(&helper, policy, &cancellation) else {
        return;
    };
    let source_root = tempfile::tempdir_in("/tmp").expect("runtime input source root");
    let (inputs, mutation) = managed_runtime_input_tree(source_root.path(), &cancellation);
    let source_metadata = mutation.metadata().expect("retained source metadata");
    assert_eq!(inputs.member_count(), MANAGED_INPUT_FIXTURE.len());
    assert_eq!(
        inputs.total_bytes(),
        MANAGED_INPUT_FIXTURE
            .iter()
            .map(|(_, bytes)| u64::try_from(bytes.len()).expect("fixture length"))
            .sum::<u64>()
    );
    let mut launch = LaunchSpec::new(std::env::current_exe().expect("managed fixture executable"));
    launch.push_argument("--exact");
    launch.push_argument("managed_runtime_input_fixture_entrypoint");
    launch.push_argument("--nocapture");
    launch.insert_environment("RETONR_TEST_MANAGED_INPUT_FIXTURE", "1");
    launch.insert_environment(
        "RETONR_TEST_SOURCE_DEVICE",
        source_metadata.dev().to_string(),
    );
    launch.insert_environment(
        "RETONR_TEST_SOURCE_INODE",
        source_metadata.ino().to_string(),
    );
    let executable = File::open(std::env::current_exe().expect("managed fixture path"))
        .expect("open retained managed fixture");
    let mut lease = prepared
        .launch_retained_with_inputs(&launch, executable, inputs, &cancellation)
        .expect("launch retained runtime input fixture");
    let initial = lease.initial_evidence();
    assert_eq!(
        initial.runtime_inputs().member_count(),
        u32::try_from(MANAGED_INPUT_FIXTURE.len()).expect("fixture member count")
    );
    assert!(!initial.runtime_inputs().is_empty());
    assert!(initial.runtime_inputs().scratch_mount().inode() > 0);
    assert!(initial.runtime_inputs().input_mount().inode() > 0);
    assert_eq!(
        lease.reobserve(&cancellation).expect("stable input tree"),
        initial
    );
    let channel = lease
        .connect_loopback(
            "127.0.0.1:43201".parse().expect("literal endpoint"),
            &cancellation,
        )
        .expect("connect managed input fixture");
    assert!(
        channel
            .startup_output()
            .standard_output()
            .ends_with(b"INPUT_READY\n")
    );
    let (mut stream, diagnostics, _capture) = channel.into_parts();
    let mut byte = [0_u8; 1];
    stream
        .read_exact(&mut byte)
        .expect("read input fixture result");
    assert_eq!(byte, [b'I']);
    mutation
        .write_all_at(b"config-v2", 0)
        .expect("mutate retained inode in place");
    mutation.sync_all().expect("sync retained inode mutation");
    assert!(matches!(
        lease.reobserve(&cancellation),
        Err(IsolationError::RuntimeInputObjectMismatch | IsolationError::EvidenceChanged)
    ));
    stream.write_all(b"Q").expect("release input fixture");
    stream.read_exact(&mut byte).expect("read input completion");
    assert_eq!(byte, [b'O']);
    drop(diagnostics);
    drop(stream);
    lease
        .close(&cancellation)
        .expect("close input process tree");
}

fn managed_runtime_input_tree(
    source_root: &Path,
    cancellation: &CancellationToken,
) -> (RetainedRuntimeInputTree, File) {
    let mut members = Vec::new();
    let mut mutation = None;
    for (index, (alias, contents)) in MANAGED_INPUT_FIXTURE.iter().enumerate() {
        let source = source_root.join(format!("member-{index}"));
        fs::write(&source, contents).expect("write runtime input source");
        let retained = File::open(&source).expect("open retained runtime input source");
        if index == 0 {
            mutation = Some(
                OpenOptions::new()
                    .write(true)
                    .open(&source)
                    .expect("open retained mutation handle"),
            );
            let detached = source_root.join("retained-member-0");
            fs::rename(&source, detached).expect("detach retained runtime input pathname");
            fs::write(&source, b"ambient-v1").expect("install ambient path substitution");
        }
        members.push((
            (*alias).to_owned(),
            Digest::sha256(contents),
            u64::try_from(contents.len()).expect("runtime input member length"),
            retained,
        ));
    }
    let tree = RetainedRuntimeInputTree::from_source(ManagedInputSource(members), cancellation)
        .expect("construct retained runtime input tree");
    (tree, mutation.expect("retained mutation handle"))
}

#[test]
fn cancelled_preparation_never_starts_the_helper() {
    let helper = helper_path();
    let (digest, bytes) = helper_identity(&helper);
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert!(matches!(
        PreparedIsolation::prepare(
            &helper,
            &digest,
            bytes,
            IsolationPolicy::default(),
            &cancellation,
        ),
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
    let helper = helper_path();
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
    let Some(prepared) = prepare_or_skip(&helper, policy, &cancellation) else {
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
    assert_eq!(lease.launch_spec_digest(), &launch.redacted_digest());
    assert_eq!(lease.isolation_policy_digest(), &policy.redacted_digest());
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

    let helper = helper_path();
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
    let Some(prepared) = prepare_or_skip(&helper, policy, &cancellation) else {
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
    let helper = helper_path();
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
    let Some(prepared) = prepare_or_skip(&helper, policy, &cancellation) else {
        return;
    };
    let executable_path = std::env::current_exe().expect("non-listener fixture executable");
    let mut launch = LaunchSpec::new(&executable_path);
    launch.push_argument("--exact");
    launch.push_argument("never_listening_fixture_entrypoint");
    launch.push_argument("--nocapture");
    launch.insert_environment("RETONR_TEST_NEVER_LISTENING_FIXTURE", "1");
    let mut lease = prepared
        .launch_retained(
            &launch,
            File::open(executable_path).expect("open non-listener fixture"),
            &cancellation,
        )
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
