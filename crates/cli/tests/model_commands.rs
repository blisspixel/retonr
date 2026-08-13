use std::{fs, path::Path};

use assert_cmd::Command;
use predicates::prelude::*;
use rewrite_model::{
    ARTIFACT_MANIFEST_SCHEMA_VERSION, ArtifactId, ArtifactManifest, ArtifactRole, ArtifactSource,
    DeclaredCapabilities, LicenseRecord,
};
use rewrite_types::Digest;
use serde_json::Value;
use tempfile::tempdir;

const ARTIFACT_BYTES: &[u8] = b"private model bytes for CLI tests";

fn manifest() -> ArtifactManifest {
    let digest = Digest::sha256(ARTIFACT_BYTES);
    ArtifactManifest {
        schema_version: ARTIFACT_MANIFEST_SCHEMA_VERSION,
        artifact_id: ArtifactId::from_digest(digest.clone()),
        source: ArtifactSource {
            origin: "private/provider/model".to_owned(),
            revision: "secret-revision".to_owned(),
        },
        artifact_digest: digest,
        byte_size: u64::try_from(ARTIFACT_BYTES.len()).expect("fixture size"),
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
    }
}

fn write_fixture(root: &Path) -> (std::path::PathBuf, std::path::PathBuf, String) {
    let source = root.join("private-source.gguf");
    let manifest_path = root.join("private-manifest.json");
    let manifest = manifest();
    fs::write(&source, ARTIFACT_BYTES).expect("write source");
    fs::write(
        &manifest_path,
        serde_json::to_vec(&manifest).expect("encode manifest"),
    )
    .expect("write manifest");
    (
        source,
        manifest_path,
        manifest.artifact_id.digest().as_str().to_owned(),
    )
}

fn import(data: &Path, source: &Path, manifest: &Path) -> Value {
    let output = Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .arg("--data-dir")
        .arg(data)
        .args(["model", "import"])
        .arg(source)
        .arg("--manifest")
        .arg(manifest)
        .output()
        .expect("run import");
    assert!(output.status.success(), "import stderr was not empty");
    assert!(output.stderr.is_empty());
    serde_json::from_slice(&output.stdout).expect("parse import JSON")
}

#[test]
fn imports_inventories_and_removes_one_exact_generation() {
    let directory = tempdir().expect("temporary directory");
    let data = directory.path().join("repository");
    let (source, manifest_path, artifact_id) = write_fixture(directory.path());
    let imported = import(&data, &source, &manifest_path);
    assert_eq!(imported["command"], "model.import");
    assert_eq!(imported["result"]["selection"]["artifact_id"], artifact_id);
    assert_eq!(
        imported["result"]["selection"]["installation_generation"],
        "1"
    );
    assert_eq!(imported["result"]["disposition"], "imported");

    let inventory = Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .arg("--data-dir")
        .arg(&data)
        .args(["model", "inventory"])
        .output()
        .expect("run inventory");
    assert!(inventory.status.success());
    assert!(inventory.stderr.is_empty());
    let inventory_text = String::from_utf8(inventory.stdout.clone()).expect("UTF-8 output");
    for private in [
        "private-source",
        "private/provider",
        "secret-revision",
        "private-family",
        "artifact-storage",
        "private model bytes",
    ] {
        assert!(!inventory_text.contains(private));
    }
    let inventory: Value = serde_json::from_slice(&inventory.stdout).expect("parse inventory");
    assert_eq!(inventory["result"]["health"], "clean");
    assert_eq!(
        inventory["result"]["registered"][0]["selection"]["artifact_id"],
        artifact_id
    );
    assert!(inventory["result"]["storage_entry_count"].is_string());
    assert!(inventory["result"]["verified_bytes"].is_string());

    let removal = Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .arg("--data-dir")
        .arg(&data)
        .args(["model", "remove", "--artifact-id", &artifact_id])
        .args(["--installation-generation", "1", "--yes"])
        .output()
        .expect("run removal");
    assert!(removal.status.success());
    let removal: Value = serde_json::from_slice(&removal.stdout).expect("parse removal");
    assert_eq!(removal["result"]["disposition"], "removed");
}

#[test]
fn removal_confirmation_and_recovery_refusal_are_typed_and_non_destructive() {
    let directory = tempdir().expect("temporary directory");
    let data = directory.path().join("repository");
    let (source, manifest_path, artifact_id) = write_fixture(directory.path());
    import(&data, &source, &manifest_path);

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .arg("--data-dir")
        .arg(&data)
        .args(["model", "remove", "--artifact-id", &artifact_id])
        .args(["--installation-generation", "1"])
        .assert()
        .code(3)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains(
            "\"code\": \"confirmation_required\"",
        ));

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .arg("--data-dir")
        .arg(&data)
        .args(["model", "recover-removal", "--artifact-id", &artifact_id])
        .args(["--installation-generation", "1", "--yes"])
        .assert()
        .code(3)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains(
            "\"code\": \"removal_recovery_not_pending\"",
        ));

    let inventory = Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .arg("--data-dir")
        .arg(&data)
        .args(["model", "inventory"])
        .output()
        .expect("run inventory");
    let inventory: Value = serde_json::from_slice(&inventory.stdout).expect("parse inventory");
    assert_eq!(
        inventory["result"]["registered"].as_array().map(Vec::len),
        Some(1)
    );
}

#[test]
fn missing_repository_and_invalid_invocation_use_stderr_only() {
    let directory = tempdir().expect("temporary directory");
    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .arg("--data-dir")
        .arg(directory.path().join("missing"))
        .args(["model", "inventory"])
        .assert()
        .code(1)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("repository_not_initialized"));

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["model", "inventory"])
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("invalid_invocation"));
}

#[test]
fn help_lists_only_the_implemented_offline_model_surface() {
    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["model", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("import"))
        .stdout(predicate::str::contains("inventory"))
        .stdout(predicate::str::contains("reconcile"))
        .stdout(predicate::str::contains("remove"))
        .stdout(predicate::str::contains("recover-removal"))
        .stdout(predicate::str::contains("download").not())
        .stdout(predicate::str::contains("activate").not())
        .stdout(predicate::str::contains("qualify").not());
}

#[test]
fn reconcile_text_inventory_findings_and_completed_recovery_are_actionable() {
    let directory = tempdir().expect("temporary directory");
    let data = directory.path().join("repository");
    let (source, manifest_path, artifact_id) = write_fixture(directory.path());
    import(&data, &source, &manifest_path);

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .arg("--data-dir")
        .arg(&data)
        .args(["model", "reconcile", "--manifest"])
        .arg(&manifest_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("already_registered"));

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["--format", "text", "--data-dir"])
        .arg(&data)
        .args(["model", "inventory"])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "registered {artifact_id} generation=1"
        )))
        .stdout(predicate::str::contains("private-family").not());

    let unexpected = data
        .join("artifact-storage")
        .join("artifacts")
        .join("unexpected-name");
    fs::write(&unexpected, b"unexpected").expect("write unexpected entry");
    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .arg("--data-dir")
        .arg(&data)
        .args(["model", "inventory", "--fail-on-findings"])
        .assert()
        .code(3)
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::contains("\"health\": \"findings\""))
        .stdout(predicate::str::contains("unexpected-name").not());
    fs::remove_file(unexpected).expect("remove unexpected fixture");

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .arg("--data-dir")
        .arg(&data)
        .args(["model", "remove", "--artifact-id", &artifact_id])
        .args(["--installation-generation", "1", "--yes"])
        .assert()
        .success();
    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .arg("--data-dir")
        .arg(&data)
        .args(["model", "recover-removal", "--artifact-id", &artifact_id])
        .args(["--installation-generation", "1", "--yes"])
        .assert()
        .success()
        .stdout(predicate::str::contains("already_removed"));
}

#[test]
fn unsupported_manifest_and_generation_overflow_are_compatibility_and_usage_errors() {
    let directory = tempdir().expect("temporary directory");
    let data = directory.path().join("repository");
    let (source, manifest_path, artifact_id) = write_fixture(directory.path());
    let mut unsupported: Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("read manifest"))
            .expect("parse manifest");
    unsupported["schema_version"] = Value::from(2);
    fs::write(
        &manifest_path,
        serde_json::to_vec(&unsupported).expect("encode unsupported manifest"),
    )
    .expect("write unsupported manifest");
    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .arg("--data-dir")
        .arg(&data)
        .args(["model", "import"])
        .arg(&source)
        .arg("--manifest")
        .arg(&manifest_path)
        .assert()
        .code(4)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("\"code\": \"unsupported\""));

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .arg("--data-dir")
        .arg(&data)
        .args(["model", "remove", "--artifact-id", &artifact_id])
        .args(["--installation-generation", "9223372036854775808", "--yes"])
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("invalid_invocation"));
}

#[test]
fn format_prescan_stops_at_the_option_terminator() {
    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["check", "--", "--format=text"])
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("\"command\": \"cli\""));
}

#[test]
fn import_never_claims_or_mutates_a_nonempty_uninitialized_directory() {
    let directory = tempdir().expect("temporary directory");
    let data = directory.path().join("existing-project");
    fs::create_dir(&data).expect("create existing project");
    let sentinel = data.join("keep.txt");
    fs::write(&sentinel, b"keep this").expect("write sentinel");
    let (source, manifest_path, _) = write_fixture(directory.path());

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .arg("--data-dir")
        .arg(&data)
        .args(["model", "import"])
        .arg(source)
        .arg("--manifest")
        .arg(manifest_path)
        .assert()
        .code(3)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("artifact_conflict"));

    assert_eq!(fs::read(sentinel).expect("read sentinel"), b"keep this");
    assert_eq!(fs::read_dir(data).expect("read project").count(), 1);
}
