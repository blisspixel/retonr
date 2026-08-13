use std::{
    fs::{self, File},
    path::Path,
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

use predicates::prelude::*;
use rewrite_model::{
    ARTIFACT_MANIFEST_SCHEMA_VERSION, ArtifactId, ArtifactManifest, ArtifactRole, ArtifactSource,
    DeclaredCapabilities, LicenseRecord,
};
use rewrite_types::Digest;
use tempfile::tempdir;

const SOURCE_BYTES: u64 = 512 * 1024 * 1024;

#[test]
fn interrupt_requests_typed_import_cancellation_before_registration() {
    let directory = tempdir().expect("temporary directory");
    let data = directory.path().join("repository");
    let source = directory.path().join("large-source.gguf");
    File::create(&source)
        .and_then(|file| file.set_len(SOURCE_BYTES))
        .expect("create bounded sparse source");
    let manifest_path = directory.path().join("manifest.json");
    fs::write(
        &manifest_path,
        serde_json::to_vec(&manifest()).expect("encode manifest"),
    )
    .expect("write manifest");

    let interrupt = InterruptSender::prepare(directory.path());
    let mut child = spawn_import(&data, &source, &manifest_path);
    wait_for_repository_lock(&mut child, &data.join(".artifact-repository.lock"));
    interrupt.send(&child);
    let output = wait_for_output(child);
    assert_eq!(
        output.status.code(),
        Some(130),
        "interrupted import returned an unexpected status\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 error envelope");
    assert!(predicate::str::contains("\"command\": \"model.import\"").eval(&stderr));
    assert!(predicate::str::contains("\"code\": \"operation_cancelled\"").eval(&stderr));

    let artifacts = data.join("artifact-storage").join("artifacts");
    assert_eq!(
        fs::read_dir(artifacts)
            .expect("read artifacts after cancellation")
            .count(),
        0
    );
}

fn manifest() -> ArtifactManifest {
    let digest = Digest::sha256(b"intentionally unmatched after cancellation");
    ArtifactManifest {
        schema_version: ARTIFACT_MANIFEST_SCHEMA_VERSION,
        artifact_id: ArtifactId::from_digest(digest.clone()),
        source: ArtifactSource {
            origin: "fixture/model".to_owned(),
            revision: "signal-cancellation".to_owned(),
        },
        artifact_digest: digest,
        byte_size: SOURCE_BYTES,
        format: "gguf".to_owned(),
        family: "fixture".to_owned(),
        architecture: Some("transformer".to_owned()),
        quantization: Some("q4".to_owned()),
        tokenizer: None,
        licenses: vec![LicenseRecord {
            component: "weights".to_owned(),
            identifier: "Apache-2.0".to_owned(),
            text_digest: Digest::sha256(b"license"),
        }],
        declared_capabilities: DeclaredCapabilities {
            roles: vec![ArtifactRole::Generation],
            languages: vec!["en".to_owned()],
            context_tokens: Some(8_192),
        },
    }
}

fn spawn_import(data: &Path, source: &Path, manifest: &Path) -> Child {
    let mut command = Command::new(env!("CARGO_BIN_EXE_retonr"));
    command
        .arg("--data-dir")
        .arg(data)
        .args(["model", "import"])
        .arg(source)
        .arg("--manifest")
        .arg(manifest)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_interruptible_child(&mut command);
    command.spawn().expect("start import child")
}

fn wait_for_repository_lock(child: &mut Child, lock: &Path) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(
            child.try_wait().expect("poll import child").is_none(),
            "import child exited before reaching its cancellable operation"
        );
        if lock.is_file() {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "import child did not initialize its repository before the deadline"
        );
        thread::sleep(Duration::from_millis(1));
    }
}

fn wait_for_output(mut child: Child) -> Output {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if child.try_wait().expect("poll interrupted child").is_some() {
            return child.wait_with_output().expect("collect interrupted child");
        }
        if Instant::now() >= deadline {
            child.kill().expect("terminate unresponsive child");
            let output = child.wait_with_output().expect("collect timed out child");
            panic!(
                "interrupted child did not exit\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(unix)]
fn configure_interruptible_child(_command: &mut Command) {}

#[cfg(unix)]
struct InterruptSender;

#[cfg(unix)]
impl InterruptSender {
    fn prepare(_directory: &Path) -> Self {
        Self
    }

    fn send(self, child: &Child) {
        let status = Command::new("kill")
            .arg("-INT")
            .arg(child.id().to_string())
            .status()
            .expect("invoke POSIX kill");
        assert!(status.success(), "POSIX interrupt request failed");
    }
}

#[cfg(windows)]
fn configure_interruptible_child(command: &mut Command) {
    use std::os::windows::process::CommandExt as _;

    const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    command.creation_flags(CREATE_NEW_CONSOLE | CREATE_NEW_PROCESS_GROUP);
}

#[cfg(windows)]
struct InterruptSender {
    child: Child,
    request: std::path::PathBuf,
}

#[cfg(windows)]
impl InterruptSender {
    fn prepare(directory: &Path) -> Self {
        let ready = directory.join("signal-sender-ready");
        let request = directory.join("signal-sender-request");
        let script = "Add-Type -TypeDefinition 'using System; using System.Runtime.InteropServices; public static class RetonrSignal { [DllImport(\"kernel32.dll\", SetLastError=true)] public static extern bool FreeConsole(); [DllImport(\"kernel32.dll\", SetLastError=true)] public static extern bool AttachConsole(uint p); [DllImport(\"kernel32.dll\", SetLastError=true)] public static extern bool SetConsoleCtrlHandler(IntPtr h, bool a); [DllImport(\"kernel32.dll\", SetLastError=true)] public static extern bool GenerateConsoleCtrlEvent(uint e, uint g); }'; Set-Content -LiteralPath $env:RETONR_SIGNAL_READY -Value ready -NoNewline; $deadline = [DateTime]::UtcNow.AddSeconds(30); while (-not (Test-Path -LiteralPath $env:RETONR_SIGNAL_REQUEST)) { if ([DateTime]::UtcNow -ge $deadline) { exit 2 }; Start-Sleep -Milliseconds 5 }; $target = [uint32](Get-Content -LiteralPath $env:RETONR_SIGNAL_REQUEST -Raw); [RetonrSignal]::FreeConsole() | Out-Null; if (-not [RetonrSignal]::AttachConsole($target)) { exit 3 }; if (-not [RetonrSignal]::SetConsoleCtrlHandler([IntPtr]::Zero, $true)) { exit 4 }; if (-not [RetonrSignal]::GenerateConsoleCtrlEvent(1, $target)) { exit 5 }; Start-Sleep -Milliseconds 100";
        let mut child = Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .env("RETONR_SIGNAL_READY", &ready)
            .env("RETONR_SIGNAL_REQUEST", &request)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("prepare Windows console interrupt sender");
        wait_for_sender(&mut child, &ready);
        Self { child, request }
    }

    fn send(self, child: &Child) {
        fs::write(&self.request, child.id().to_string()).expect("submit Windows interrupt target");
        let output = self
            .child
            .wait_with_output()
            .expect("collect Windows interrupt sender");
        assert!(
            output.status.success(),
            "Windows interrupt request failed with status {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[cfg(windows)]
fn wait_for_sender(child: &mut Child, path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !path.is_file() {
        assert!(
            child
                .try_wait()
                .expect("poll Windows interrupt sender")
                .is_none(),
            "Windows interrupt sender exited before becoming ready"
        );
        assert!(
            Instant::now() < deadline,
            "Windows interrupt sender did not become ready"
        );
        thread::sleep(Duration::from_millis(5));
    }
}
