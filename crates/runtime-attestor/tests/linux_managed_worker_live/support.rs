use std::{
    ffi::{c_int, c_ulong},
    fs::{self, File, OpenOptions},
    io::Write as _,
    num::NonZeroU32,
    os::{
        fd::OwnedFd,
        unix::{
            fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _},
            process::CommandExt as _,
        },
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use rewrite_model::{
    ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath, PackageSource,
    PackageSourceKind, PackageTransformation, RuntimeAbi, RuntimeArchitecture,
    RuntimeOperatingSystem, RuntimePackageLoadPolicy, RuntimePackageManifest, RuntimePackageMember,
    RuntimePackageMemberRole, RuntimeTarget,
};
use rewrite_runtime_attestor::{
    ManagedGenerationWorkerError, ManagedLinuxProcessExpectation, RetainedModelWeightSink,
    RetainedModelWeightSource, RetainedNativePackageMember,
};
use rewrite_types::Digest;
use rustix::{
    net::{
        AddressFamily, Protocol, SocketFlags, SocketType, bind, netlink::SocketAddrNetlink,
        socket_with,
    },
    process::geteuid,
};

pub(super) const MODEL_BYTES: &[u8] =
    b"GGUF\0retonr-managed-worker-live-fixture-v1\0private-mapping";
const WORKER_MARKER: &[u8] = b"\nretonr-distinct-worker-object-v1\n";
const PR_CAPBSET_DROP: c_int = 24;
const PR_SET_NO_NEW_PRIVS: c_int = 38;
const PR_SET_SECCOMP: c_int = 22;
const PR_CAP_AMBIENT: c_int = 47;
const PR_CAP_AMBIENT_CLEAR_ALL: c_ulong = 4;
const SECCOMP_MODE_FILTER: c_ulong = 2;
const SECCOMP_RET_ALLOW: u32 = 0x7fff_0000;
const LINUX_CAPABILITY_VERSION_3: u32 = 0x2008_0522;

#[repr(C)]
struct CapabilityHeader {
    version: u32,
    pid: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CapabilityData {
    effective: u32,
    permitted: u32,
    inheritable: u32,
}

#[repr(C)]
struct SocketFilter {
    code: u16,
    jump_true: u8,
    jump_false: u8,
    value: u32,
}

#[repr(C)]
struct SocketFilterProgram {
    length: u16,
    filter: *const SocketFilter,
}

unsafe extern "C" {
    fn prctl(option: c_int, ...) -> c_int;
    fn capset(header: *const CapabilityHeader, data: *const CapabilityData) -> c_int;
}

pub(super) struct ModelSource {
    pub(super) artifact_id: ArtifactId,
    pub(super) byte_size: u64,
    pub(super) file: File,
}

impl<'lease> RetainedModelWeightSource<'lease> for ModelSource {
    fn transfer(
        self,
        sink: &mut RetainedModelWeightSink<'_, 'lease>,
    ) -> Result<(), ManagedGenerationWorkerError> {
        sink.retain(self.artifact_id, self.byte_size, self.file)
    }
}

pub(super) struct ChildGuard(pub(super) Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub(super) struct ModelTargetGuard {
    pub(super) file: PathBuf,
    blobs: PathBuf,
    root: PathBuf,
}

impl Drop for ModelTargetGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.file);
        let _ = fs::remove_dir(&self.blobs);
        let _ = fs::remove_dir(&self.root);
    }
}

pub(super) fn copy_distinct_worker(worker_path: &Path) {
    let fixture = std::env::current_exe()
        .expect("resolve live test executable")
        .parent()
        .and_then(Path::parent)
        .expect("live test target directory")
        .join("rewrite-runtime-attestor-worker-fixture");
    fs::copy(fixture, worker_path).expect("copy worker fixture");
    OpenOptions::new()
        .append(true)
        .open(worker_path)
        .expect("open copied worker fixture")
        .write_all(WORKER_MARKER)
        .expect("append distinct worker object marker");
    fs::set_permissions(worker_path, fs::Permissions::from_mode(0o500))
        .expect("make worker fixture executable");
}

pub(super) fn install_private_model_target(artifact_id: &ArtifactId) -> ModelTargetGuard {
    let root = PathBuf::from(rewrite_runtime_attestor::MANAGED_OLLAMA_MODEL_ROOT);
    let blobs = root.join("blobs");
    fs::create_dir_all(&blobs).expect("create fixed private model root");
    let file = blobs.join(format!("sha256-{}", artifact_id.digest().as_str()));
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o400)
        .open(&file)
        .expect("create exact private model target")
        .write_all(MODEL_BYTES)
        .expect("write exact private model target");
    ModelTargetGuard { file, blobs, root }
}

pub(super) fn assert_private_model_copy(source: &Path, target: &Path) {
    assert_eq!(
        fs::read(target).expect("read private target model"),
        MODEL_BYTES
    );
    let source = fs::metadata(source).expect("source model metadata");
    let target = fs::metadata(target).expect("target model metadata");
    assert_ne!((source.dev(), source.ino()), (target.dev(), target.ino()));
}

pub(super) fn spawn_worker(worker_path: &Path, model_path: &Path) -> ChildGuard {
    let mut command = Command::new(worker_path);
    command.args([
        "--model",
        model_path.to_str().expect("model path is UTF-8"),
        "--port",
        "49152",
        "--host",
        "127.0.0.1",
        "--no-webui",
        "--offline",
        "-c",
        "4096",
        "-np",
        "1",
        "--log-verbosity",
        "4",
        "--no-log-prefix",
        "--no-log-timestamps",
        "--flash-attn",
        "off",
        "-ngl",
        "0",
    ]);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // SAFETY: this closure uses only async-signal-safe Linux system calls and
    // returns an I/O error before exec if any required privilege transition fails.
    unsafe {
        command.pre_exec(drop_worker_privileges);
    }
    ChildGuard(command.spawn().expect("spawn exact worker fixture"))
}

fn drop_worker_privileges() -> std::io::Result<()> {
    for capability in 0_u64..64 {
        // SAFETY: prctl is called with the documented PR_CAPBSET_DROP scalar ABI.
        let result = unsafe {
            prctl(
                PR_CAPBSET_DROP,
                c_ulong::from(capability),
                c_ulong::from(0_u32),
                c_ulong::from(0_u32),
                c_ulong::from(0_u32),
            )
        };
        if result != 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() != Some(22) {
                return Err(error);
            }
        }
    }
    let header = CapabilityHeader {
        version: LINUX_CAPABILITY_VERSION_3,
        pid: 0,
    };
    let data = [CapabilityData {
        effective: 0,
        permitted: 0,
        inheritable: 0,
    }; 2];
    // SAFETY: capset receives the version 3 header and its required two data words.
    if unsafe { capset(&raw const header, data.as_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: prctl is called with the documented ambient-clear scalar ABI.
    if unsafe {
        prctl(
            PR_CAP_AMBIENT,
            PR_CAP_AMBIENT_CLEAR_ALL,
            c_ulong::from(0_u32),
            c_ulong::from(0_u32),
            c_ulong::from(0_u32),
        )
    } != 0
    {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: prctl is called with the documented no-new-privileges scalar ABI.
    if unsafe {
        prctl(
            PR_SET_NO_NEW_PRIVS,
            c_ulong::from(1_u32),
            c_ulong::from(0_u32),
            c_ulong::from(0_u32),
            c_ulong::from(0_u32),
        )
    } != 0
    {
        return Err(std::io::Error::last_os_error());
    }
    let filter = [SocketFilter {
        code: 0x06,
        jump_true: 0,
        jump_false: 0,
        value: SECCOMP_RET_ALLOW,
    }];
    let program = SocketFilterProgram {
        length: u16::try_from(filter.len()).expect("one filter instruction"),
        filter: filter.as_ptr(),
    };
    // SAFETY: the filter program remains live for the synchronous prctl call and
    // consists of one valid return-allow classic BPF instruction.
    if unsafe {
        prctl(
            PR_SET_SECCOMP,
            SECCOMP_MODE_FILTER,
            &raw const program,
            c_ulong::from(0_u32),
            c_ulong::from(0_u32),
        )
    } != 0
    {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

pub(super) fn wait_for_worker_ready(pid: u32, model_path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(5);
    let expected = model_path.to_str().expect("model path is UTF-8");
    loop {
        if let Ok(status) = fs::read_to_string(format!("/proc/{pid}/status"))
            && status.contains("NoNewPrivs:\t1")
            && status.contains("Seccomp:\t2")
            && ["CapInh", "CapPrm", "CapEff", "CapBnd", "CapAmb"]
                .into_iter()
                .all(|name| status.contains(&format!("{name}:\t0000000000000000")))
            && fs::read_to_string(format!("/proc/{pid}/maps"))
                .is_ok_and(|maps| maps.lines().any(|line| line.ends_with(expected)))
        {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "worker fixture did not become ready"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

pub(super) fn package(server: &[u8], worker: &[u8], legal: &[u8]) -> RuntimePackageManifest {
    let members = [
        ("bin/ollama", server),
        ("legal/evidence", legal),
        ("lib/ollama/llama-server", worker),
    ]
    .map(|(relative_path, bytes)| {
        let path = ArtifactSetRelativePath::new(relative_path).expect("valid fixture path");
        let artifact = ArtifactId::from_digest(Digest::sha256(bytes));
        (
            path,
            artifact,
            u64::try_from(bytes.len()).expect("fixture size fits u64"),
        )
    });
    let artifact_set = ArtifactSetManifest::new(
        members
            .iter()
            .map(|(path, artifact, bytes)| {
                ArtifactSetMember::new(artifact.clone(), *bytes, path.clone())
            })
            .collect(),
    )
    .expect("canonical fixture artifact set");
    RuntimePackageManifest::new(
        &artifact_set,
        "ollama",
        "0.32.15",
        None,
        RuntimeTarget::new(
            RuntimeOperatingSystem::Linux,
            RuntimeArchitecture::X86_64,
            RuntimeAbi::LinuxMusl,
        )
        .expect("valid fixture target"),
        PackageSource::new(
            PackageSourceKind::LocalArchive,
            "local:managed-worker-live-fixture",
            "0.32.15",
            Digest::sha256(b"managed-worker-live-fixture-source"),
        )
        .expect("valid fixture source"),
        PackageTransformation::Untransformed {
            evidence_digest: Digest::sha256(b"managed-worker-live-fixture-comparison"),
        },
        vec![
            RuntimePackageMember::new(
                members[0].1.clone(),
                members[0].2,
                members[0].0.clone(),
                vec![RuntimePackageMemberRole::Entrypoint],
                RuntimePackageLoadPolicy::RequiredAtReady,
            ),
            RuntimePackageMember::new(
                members[1].1.clone(),
                members[1].2,
                members[1].0.clone(),
                vec![
                    RuntimePackageMemberRole::LicenseText,
                    RuntimePackageMemberRole::ProvenanceRecord,
                ],
                RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
            ),
            RuntimePackageMember::new(
                members[2].1.clone(),
                members[2].2,
                members[2].0.clone(),
                vec![RuntimePackageMemberRole::WorkerExecutable],
                RuntimePackageLoadPolicy::BackendConditional,
            ),
        ],
    )
    .expect("valid fixture runtime package")
}

pub(super) fn retain_code(
    package: &RuntimePackageManifest,
    server_path: &Path,
    worker_path: &Path,
) -> Vec<RetainedNativePackageMember> {
    package
        .members()
        .iter()
        .filter(|member| {
            member.roles().iter().any(|role| {
                matches!(
                    role,
                    RuntimePackageMemberRole::Entrypoint
                        | RuntimePackageMemberRole::NativeDependency
                        | RuntimePackageMemberRole::HelperExecutable
                        | RuntimePackageMemberRole::WorkerExecutable
                )
            })
        })
        .map(|member| {
            let path = if member
                .roles()
                .contains(&RuntimePackageMemberRole::WorkerExecutable)
            {
                worker_path
            } else {
                server_path
            };
            RetainedNativePackageMember::new(
                member.relative_path().clone(),
                member.artifact_id().clone(),
                member.byte_size(),
                File::open(path).expect("open retained fixture code"),
            )
            .expect("retain fixture code object")
        })
        .collect()
}

pub(super) fn current_expectation() -> ManagedLinuxProcessExpectation {
    let pid = std::process::id();
    let executable = fs::metadata(format!("/proc/{pid}/exe")).expect("server executable metadata");
    let namespace = fs::metadata(format!("/proc/{pid}/ns/net")).expect("server namespace metadata");
    ManagedLinuxProcessExpectation::new(
        pid,
        process_start_token(pid),
        executable.dev(),
        executable.ino(),
        executable.len(),
        namespace.dev(),
        namespace.ino(),
        geteuid().as_raw(),
    )
    .expect("valid managed server expectation")
}

fn process_start_token(pid: u32) -> u64 {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).expect("server process stat");
    let end = stat.rfind(") ").expect("server process command terminator");
    stat[end + 2..]
        .split_ascii_whitespace()
        .nth(19)
        .expect("server process start token")
        .parse()
        .expect("numeric server process start token")
}

pub(super) fn socket_diagnostics() -> File {
    let descriptor: OwnedFd = socket_with(
        AddressFamily::NETLINK,
        SocketType::RAW,
        SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
        Some(Protocol::from_raw(
            NonZeroU32::new(4).expect("socket diagnostics protocol is nonzero"),
        )),
    )
    .expect("create namespace-local socket diagnostics");
    bind(&descriptor, &SocketAddrNetlink::new(0, 0)).expect("bind socket diagnostics");
    File::from(descriptor)
}
