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
        .args(["version"])
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
        .args(["doctor"])
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
        .stdout(predicate::str::contains("retonr rewrite draft.txt -i"))
        .stdout(predicate::str::contains("-D .retonr model list"));
}

#[test]
fn completions_json_names_the_shell_and_script() {
    binary()
        .args(["completions", "bash"])
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
        .args(["completions", "cmd"])
        .assert()
        .failure()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("\"command\": \"cli\""))
        .stderr(predicate::str::contains("\"code\": \"invalid_invocation\""));
}

#[test]
fn man_writes_a_section_one_page() {
    binary()
        .args(["man"])
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
        .args(["doctor"])
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
        .args(["doctor"])
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
