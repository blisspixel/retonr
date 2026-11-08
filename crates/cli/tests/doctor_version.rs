use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

fn binary() -> Command {
    let mut command = Command::cargo_bin("retonr").expect("built binary");
    command.env_remove("RETONR_DATA_DIR");
    command
}

#[test]
fn version_reports_local_identity_without_a_data_directory() {
    binary()
        .args(["--format", "json", "version"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::contains("\"command\": \"version\""))
        .stdout(predicate::str::contains("\"product\": \"retonr\""))
        .stdout(predicate::str::contains("\"product_version\": \"0.1.0\""))
        .stdout(predicate::str::contains("\"local_only\": true"))
        .stdout(predicate::str::contains("\"cli_schema_version\": 1"))
        .stdout(predicate::str::contains("store_schema_version"));

    binary()
        .args(["--format", "text", "version"])
        .assert()
        .success()
        .stdout(predicate::str::contains("product: retonr"))
        .stdout(predicate::str::contains("local_only: true"));
}

#[test]
fn doctor_without_data_dir_is_local_and_does_not_request_a_repository() {
    binary()
        .args(["--format", "json", "doctor"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::contains("\"command\": \"doctor\""))
        .stdout(predicate::str::contains("\"network\": \"denied\""))
        .stdout(predicate::str::contains("\"local_only\": true"))
        .stdout(predicate::str::contains("\"status\": \"not_requested\""))
        .stdout(predicate::str::contains("\"recovery_actions\": []"))
        .stdout(predicate::str::contains("pending_operations").not())
        .stdout(predicate::str::contains("active_generation").not());
}

#[test]
fn help_lists_completions_and_man() {
    binary()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("completions"))
        .stdout(predicate::str::contains("man"))
        .stdout(predicate::str::contains("inspect"))
        .stdout(predicate::str::contains("Examples:"))
        .stdout(predicate::str::contains(
            "retonr check original.txt candidate.txt --diff",
        ))
        .stdout(predicate::str::contains("not available in this build"))
        .stdout(predicate::str::contains("retonr rewrite draft.txt -i"))
        .stdout(predicate::str::contains(
            "retonr rewrite docs/ -r --output-dir rewritten --dry-run",
        ))
        .stdout(predicate::str::contains("-D .retonr model list"))
        .stdout(predicate::str::contains(
            "Retonr is a local-first copy editor for AI-assisted and rough drafts.",
        ))
        .stdout(predicate::str::contains(
            "Retonr makes small, checked copy edits toward your own style. It adds no watermark,",
        ))
        .stdout(predicate::str::contains(
            "tracking, or attribution. It does not test text against detectors and makes no claim",
        ))
        .stdout(predicate::str::contains(
            "about detection results, authorship, or disclosure duties.",
        ));
}

#[test]
fn completions_json_names_the_shell_and_script() {
    binary()
        .args(["--format", "json", "completions", "bash"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::contains("\"command\": \"completions\""))
        .stdout(predicate::str::contains("\"shell\": \"bash\""))
        .stdout(predicate::str::contains("\"script\":"));

    binary()
        .args(["--format", "text", "completions", "powershell"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::contains("retonr"))
        .stdout(predicate::str::contains("schema_version").not());
}

#[test]
fn completions_reject_an_unknown_shell() {
    binary()
        .args(["--format", "json", "completions", "cmd"])
        .assert()
        .failure()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("\"command\": \"completions\""))
        .stderr(predicate::str::contains("\"code\": \"invalid_invocation\""));
}

#[test]
fn man_writes_a_section_one_page() {
    binary()
        .args(["--format", "json", "man"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::contains("\"command\": \"man\""))
        .stdout(predicate::str::contains("\"name\": \"retonr\""))
        .stdout(predicate::str::contains("\"section\": \"1\""))
        .stdout(predicate::str::contains(".TH"));

    binary()
        .args(["--format", "text", "man"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::contains(".TH"))
        .stdout(predicate::str::contains("retonr"))
        .stdout(predicate::str::contains("schema_version").not());
}

#[test]
fn doctor_reports_a_missing_repository_without_creating_one() {
    let directory = tempdir().expect("temporary directory");
    let data = directory.path().join("missing-repository");
    binary()
        .arg("--data-dir")
        .arg(&data)
        .args(["--format", "json", "doctor"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"not_initialized\""))
        .stdout(predicate::str::contains("\"recovery_actions\": []"))
        .stdout(predicate::str::contains("pending_operations").not());
    assert!(!data.exists(), "doctor must not create a data directory");
}

#[test]
fn doctor_reports_a_current_repository_without_claiming_activation() {
    use std::fs;

    use rewrite_model::{
        ARTIFACT_MANIFEST_SCHEMA_VERSION, ArtifactId, ArtifactManifest, ArtifactRole,
        ArtifactSource, DeclaredCapabilities, LicenseRecord,
    };
    use rewrite_types::Digest;

    let directory = tempdir().expect("temporary directory");
    let data = directory.path().join("repository");
    let bytes = b"doctor current repository fixture";
    let digest = Digest::sha256(bytes);
    let manifest = ArtifactManifest {
        schema_version: ARTIFACT_MANIFEST_SCHEMA_VERSION,
        artifact_id: ArtifactId::from_digest(digest.clone()),
        source: ArtifactSource {
            origin: "private/provider/model".to_owned(),
            revision: "secret-revision".to_owned(),
        },
        artifact_digest: digest,
        byte_size: u64::try_from(bytes.len()).expect("fixture size"),
        format: "gguf".to_owned(),
        family: "private-family".to_owned(),
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
    };
    let source = directory.path().join("private-source.gguf");
    let manifest_path = directory.path().join("private-manifest.json");
    fs::write(&source, bytes).expect("write source");
    fs::write(
        &manifest_path,
        serde_json::to_vec(&manifest).expect("encode manifest"),
    )
    .expect("write manifest");
    binary()
        .arg("--data-dir")
        .arg(&data)
        .args(["model", "import"])
        .arg(&source)
        .arg("--manifest")
        .arg(&manifest_path)
        .assert()
        .success();

    let output = binary()
        .arg("--data-dir")
        .arg(&data)
        .args(["--format", "json", "doctor"])
        .output()
        .expect("run doctor");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout.clone()).expect("UTF-8 doctor output");
    for private in [
        "private-source",
        "private/provider",
        "secret-revision",
        "private-family",
        "artifact-storage",
        "doctor current repository fixture",
    ] {
        assert!(!text.contains(private), "doctor leaked {private}: {text}");
    }
    assert!(text.contains("\"status\": \"current\""));
    assert!(text.contains("\"recovery_actions\": []"));
    assert!(text.contains("\"status\": \"absent\""));
    assert!(text.contains("\"artifact_removals\": \"0\""));
    assert!(!text.contains("present"));

    binary()
        .args(["--format", "text", "--data-dir"])
        .arg(&data)
        .args(["doctor"])
        .assert()
        .success()
        .stdout(predicate::str::contains("repository: current"))
        .stdout(predicate::str::contains("pending_artifact_removals: 0"))
        .stdout(predicate::str::contains("active_generation: absent"))
        .stdout(predicate::str::contains("recovery_actions: none"));
}

#[test]
fn usage_errors_name_the_problem_and_exit_two() {
    binary()
        .args(["--bogus"])
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("unexpected argument '--bogus'"))
        .stderr(predicate::str::contains("usage:"))
        .stderr(predicate::str::contains("retonr --help"));
    binary()
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("a command is required"))
        .stderr(predicate::str::contains("usage:"))
        .stderr(predicate::str::contains("retonr --help"));
    binary()
        .args(["check", "only-one"])
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("missing required argument"))
        .stderr(predicate::str::contains("usage:"))
        .stderr(predicate::str::contains("retonr check --help"));
    binary()
        .args(["--format", "json", "--bogus"])
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("\"code\": \"invalid_invocation\""))
        .stderr(predicate::str::contains("\"diagnostic\": \"--bogus\""))
        .stderr(predicate::str::contains("\"command\": \"cli\""));
    binary()
        .args(["--format", "json", "--verbose", "check", "only-one"])
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("\"command\": \"check\""))
        .stderr(predicate::str::contains("\"code\": \"invalid_invocation\""));
}

#[cfg(unix)]
#[test]
fn help_and_check_reports_fit_a_sixty_column_terminal() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.txt");
    let candidate = directory.path().join("candidate.txt");
    std::fs::write(&source, "Hello world\n").expect("source");
    std::fs::write(&candidate, "Hello, world!\n").expect("candidate");
    let lint_source = directory.path().join("lint.txt");
    std::fs::write(
        &lint_source,
        "Certainly! This opening is conversational residue that runs past sixty columns easily.\n",
    )
    .expect("lint source");
    let script = r#"
import fcntl, os, pty, re, select, struct, subprocess, sys, termios
binary, source, candidate, lint_source = sys.argv[1:]

def run(args):
    leader, follower = pty.openpty()
    fcntl.ioctl(follower, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 60, 0, 0))
    env = os.environ.copy()
    env["COLUMNS"] = "60"
    env.pop("NO_COLOR", None)
    env["TERM"] = "xterm-256color"
    proc = subprocess.Popen(
        [binary, *args],
        stdin=follower,
        stdout=follower,
        stderr=follower,
        env=env,
        close_fds=True,
    )
    os.close(follower)
    chunks = []
    while True:
        ready, _, _ = select.select([leader], [], [], 5)
        if not ready:
            proc.kill()
            sys.stderr.write("timed out\n")
            return 1, ["timed out"], "", ""
        try:
            data = os.read(leader, 65536)
        except OSError:
            break
        if not data:
            break
        chunks.append(data)
    raw = b"".join(chunks).decode()
    text = re.sub(r"\x1b(?:[@-Z\\-_]|\[[0-?]*[ -/]*[@-~])", "", raw)
    long = [line for line in text.splitlines() if len(line) > 60]
    return proc.wait(), long, text, raw

def fail(label, code, long, text):
    sys.stderr.write(f"{label} code {code}\n")
    sys.stderr.write("\n".join(repr(line) for line in long[:30]) + "\n")
    sys.stderr.write(text[-2000:])
    sys.exit(1)

code, long, text, raw = run(["--help"])
if code != 0 or long:
    fail("help", code, long, text)
code, long, text, raw = run(
    ["--format", "text", "check", source, candidate, "--edit-level", "voice-pass"]
)
if code != 0 or long or "\x1b[32maccepted" not in raw:
    fail("check", code, long, text + "\n" + raw[-500:])
if not text.startswith("accepted"):
    fail("check verdict", code, long, text)
code, long, text, raw = run(["--format", "text", "lint", lint_source])
if code != 0 or long or not text.startswith("1 finding"):
    fail("lint", code, long, text)
"#;
    let output = std::process::Command::new("python3")
        .arg("-c")
        .arg(script)
        .arg(assert_cmd::cargo::cargo_bin("retonr"))
        .arg(&source)
        .arg(&candidate)
        .arg(&lint_source)
        .output()
        .expect("run terminal fixture");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
