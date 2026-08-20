use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

fn binary() -> Command {
    let mut command = Command::cargo_bin("retonr").expect("built binary");
    command.env_remove("RETONR_DATA_DIR");
    command
}

#[test]
fn rewrite_fails_closed_without_a_qualified_artifact() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.txt");
    let output = directory.path().join("out.txt");
    fs::write(&source, "Hello world\n").expect("write source");

    binary()
        .args(["rewrite"])
        .arg(&source)
        .arg("--output")
        .arg(&output)
        .assert()
        .code(4)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("\"command\": \"rewrite\""))
        .stderr(predicate::str::contains("\"code\": \"unsupported\""));
    assert!(!output.exists(), "failure must not create --output");
    assert_eq!(fs::read(&source).expect("source remains"), b"Hello world\n");
}

#[test]
fn rewrite_refuses_an_existing_destination_before_generation() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.txt");
    let output = directory.path().join("out.txt");
    fs::write(&source, "Hello world\n").expect("write source");
    fs::write(&output, "keep\n").expect("write existing dest");

    binary()
        .args(["rewrite"])
        .arg(&source)
        .arg("--output")
        .arg(&output)
        .assert()
        .code(3)
        .stderr(predicate::str::contains("\"command\": \"rewrite\""))
        .stderr(predicate::str::contains("\"code\": \"output_exists\""));
    assert_eq!(fs::read(&output).expect("dest remains"), b"keep\n");
}

#[test]
fn rewrite_artifact_id_without_data_dir_is_usage() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.txt");
    fs::write(&source, "Hello world\n").expect("write source");
    let artifact_id = "0".repeat(64);

    binary()
        .args(["rewrite"])
        .arg(&source)
        .args(["--artifact-id", &artifact_id])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("\"command\": \"rewrite\""))
        .stderr(predicate::str::contains("\"code\": \"invalid_invocation\""));
}

#[test]
fn rewrite_inspects_a_repository_without_starting_a_runtime() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.txt");
    fs::write(&source, "Hello world\n").expect("write source");
    let data = directory.path().join("missing-data");

    binary()
        .arg("--data-dir")
        .arg(&data)
        .args(["rewrite"])
        .arg(&source)
        .assert()
        .code(4)
        .stderr(predicate::str::contains("\"command\": \"rewrite\""))
        .stderr(predicate::str::contains("\"code\": \"unsupported\""));
    assert!(!data.exists(), "rewrite must not initialize a repository");
}

#[test]
fn rewrite_directory_requires_output_dir_and_dry_run() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("docs");
    fs::create_dir(&source).expect("create docs");
    fs::write(source.join("a.txt"), "alpha\n").expect("write a");

    binary()
        .args(["rewrite"])
        .arg(&source)
        .assert()
        .code(2)
        .stderr(predicate::str::contains("\"code\": \"invalid_invocation\""));

    binary()
        .args(["rewrite"])
        .arg(&source)
        .arg("--output-dir")
        .arg(directory.path().join("out"))
        .assert()
        .code(4)
        .stderr(predicate::str::contains("\"code\": \"unsupported\""));
    assert!(!directory.path().join("out").exists());
}

#[test]
fn rewrite_directory_dry_run_maps_destinations_without_mutation() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("docs");
    let output = directory.path().join("rewritten");
    fs::create_dir(&source).expect("create docs");
    fs::write(source.join("a.txt"), "alpha\n").expect("write a");
    fs::create_dir(source.join("nested")).expect("create nested");
    fs::write(source.join("nested").join("inner.txt"), "inner\n").expect("write nested");
    fs::create_dir(source.join("target")).expect("create target");
    fs::write(source.join("target").join("built.txt"), "built\n").expect("write ignored");

    let result = binary()
        .args(["rewrite", "-r", "--dry-run", "--output-dir"])
        .arg(&output)
        .arg(&source)
        .output()
        .expect("run directory dry-run");
    assert!(result.status.success());
    let text = String::from_utf8(result.stdout).expect("UTF-8");
    assert!(text.contains("\"command\": \"rewrite\""));
    assert!(text.contains("\"mode\": \"dry_run\""));
    assert!(text.contains("\"source\": \"a.txt\""));
    assert!(text.contains("\"destination\": \"nested/inner.txt\""));
    assert!(text.contains("\"reason\": \"ignored\""));
    assert!(!text.contains("built.txt"));
    assert!(!text.contains("alpha"));
    assert!(!text.contains(&source.display().to_string()));
    assert!(!output.exists(), "dry-run must not create output-dir");
    assert_eq!(
        fs::read(source.join("a.txt")).expect("source remains"),
        b"alpha\n"
    );

    binary()
        .args([
            "--format",
            "text",
            "rewrite",
            "-r",
            "--dry-run",
            "--output-dir",
        ])
        .arg(&output)
        .arg(&source)
        .assert()
        .success()
        .stdout(predicate::str::contains("planned a.txt destination=a.txt"))
        .stdout(predicate::str::contains(
            "planned nested/inner.txt destination=nested/inner.txt",
        ))
        .stdout(predicate::str::contains("skipped target reason=ignored"));
}

#[test]
fn rewrite_directory_dry_run_reports_collisions_and_refuses_nested_roots() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("docs");
    let output = directory.path().join("rewritten");
    fs::create_dir(&source).expect("create docs");
    fs::create_dir(&output).expect("create output");
    fs::write(source.join("a.txt"), "alpha\n").expect("write a");
    fs::write(output.join("a.txt"), "keep\n").expect("write collision");

    binary()
        .args(["rewrite", "--dry-run", "--output-dir"])
        .arg(&output)
        .arg(&source)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"reason\": \"collision\""))
        .stdout(predicate::str::contains("\"planned_count\": \"0\""));
    assert_eq!(
        fs::read(output.join("a.txt")).expect("dest remains"),
        b"keep\n"
    );

    binary()
        .args(["rewrite", "--dry-run", "--output-dir"])
        .arg(source.join("nested-out"))
        .arg(&source)
        .assert()
        .code(3)
        .stderr(predicate::str::contains("\"code\": \"policy_refusal\""));
}

#[test]
fn rewrite_directory_rejects_in_place_and_file_output_dir() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("docs");
    let file = directory.path().join("draft.txt");
    fs::create_dir(&source).expect("create docs");
    fs::write(source.join("a.txt"), "alpha\n").expect("write a");
    fs::write(&file, "Hello world\n").expect("write file");

    binary()
        .args(["rewrite", "-i", "--dry-run", "--output-dir"])
        .arg(directory.path().join("out"))
        .arg(&source)
        .assert()
        .code(2)
        .stderr(predicate::str::contains("\"code\": \"invalid_invocation\""));

    binary()
        .args(["rewrite", "--dry-run", "--output-dir"])
        .arg(directory.path().join("out"))
        .arg(&file)
        .assert()
        .code(2)
        .stderr(predicate::str::contains("\"code\": \"invalid_invocation\""));
}

#[test]
fn rewrite_standard_input_is_accepted_then_refused() {
    binary()
        .args(["rewrite", "-"])
        .write_stdin(b"Hello world\n".to_vec())
        .assert()
        .code(4)
        .stderr(predicate::str::contains("\"command\": \"rewrite\""));
}

fn fake_generation_qualification(
    installed: &rewrite_model::InstalledArtifact,
) -> rewrite_model::QualificationRecord {
    use rewrite_model::{
        ArtifactRole, HardwareTier, LicenseDecision, QUALIFICATION_SCHEMA_VERSION,
        QualificationRecord, QualificationStatus, RuntimeIdentity,
    };
    use rewrite_types::Digest;

    QualificationRecord {
        schema_version: QUALIFICATION_SCHEMA_VERSION,
        artifact_id: installed.artifact_id.clone(),
        artifact_digest: installed.artifact_digest.clone(),
        runtime: RuntimeIdentity {
            backend: "fake".to_owned(),
            version: "1.0.0".to_owned(),
            digest: Some(Digest::sha256(b"runtime")),
        },
        operating_system: "test".to_owned(),
        hardware_tier: HardwareTier {
            id: "test".to_owned(),
            memory_mib: 8_192,
            accelerator: "none".to_owned(),
        },
        supported_roles: vec![ArtifactRole::Generation],
        source_byte_limit: 4_096,
        context_token_limit: 8_192,
        prompt_template_digest: Digest::sha256(b"prompt"),
        request_policy_digest: Digest::sha256(b"request"),
        threshold_policy_digest: Digest::sha256(b"threshold"),
        license_decision: LicenseDecision::LocalUseOnly,
        status: QualificationStatus::Qualified,
    }
}

fn activate_imported_fake(root: &std::path::Path) -> std::path::PathBuf {
    use rewrite_model::{
        ARTIFACT_MANIFEST_SCHEMA_VERSION, ActivationId, ArtifactId, ArtifactManifest, ArtifactRole,
        ArtifactSource, DeclaredCapabilities, InstalledArtifact, LicenseRecord,
    };
    use rewrite_model_store::ArtifactStateStore;
    use rewrite_types::Digest;

    let artifact_bytes = b"cli conformance artifact";
    let artifact_file = root.join("model.gguf");
    fs::write(&artifact_file, artifact_bytes).expect("write artifact");
    let digest = Digest::sha256(artifact_bytes);
    let artifact_id = ArtifactId::from_digest(digest.clone());
    let manifest = ArtifactManifest {
        schema_version: ARTIFACT_MANIFEST_SCHEMA_VERSION,
        artifact_id: artifact_id.clone(),
        source: ArtifactSource {
            origin: "fixture/model".to_owned(),
            revision: "fixture".to_owned(),
        },
        artifact_digest: digest.clone(),
        byte_size: u64::try_from(artifact_bytes.len()).expect("fixture size"),
        format: "gguf".to_owned(),
        family: "fixture".to_owned(),
        architecture: None,
        quantization: None,
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
    let manifest_path = root.join("manifest.json");
    fs::write(
        &manifest_path,
        serde_json::to_vec(&manifest).expect("encode manifest"),
    )
    .expect("write manifest");
    let data = root.join("data");
    binary()
        .arg("--data-dir")
        .arg(&data)
        .args(["model", "import"])
        .arg(&artifact_file)
        .arg("--manifest")
        .arg(&manifest_path)
        .assert()
        .success();
    let installed = InstalledArtifact {
        artifact_id: artifact_id.clone(),
        artifact_digest: digest,
        byte_size: u64::try_from(artifact_bytes.len()).expect("fixture size"),
        storage_key: format!("artifacts/{}", artifact_id.digest().as_str()),
    };
    let qualification = fake_generation_qualification(&installed);
    let qualification_id = qualification
        .qualification_id()
        .expect("fixture qualification");
    let mut store =
        ArtifactStateStore::open_existing_writable_exact(&data.join("artifact-state.sqlite3"))
            .expect("open store");
    store
        .put_qualification(&qualification)
        .expect("store qualification");
    store
        .activate(
            ActivationId::from_digest(Digest::sha256(b"cli-conformance")),
            ArtifactRole::Generation,
            &installed,
            &qualification_id,
        )
        .expect("activate");
    data
}

#[test]
fn rewrite_attaches_fake_conformance_to_a_recovered_binding() {
    let directory = tempdir().expect("temporary directory");
    let source_doc = directory.path().join("source.txt");
    let output = directory.path().join("out.txt");
    fs::write(&source_doc, "Hello world\n").expect("write source");
    let data = activate_imported_fake(directory.path());
    let rewritten = binary()
        .arg("--data-dir")
        .arg(&data)
        .args(["rewrite"])
        .arg(&source_doc)
        .arg("--output")
        .arg(&output)
        .output()
        .expect("run rewrite");
    assert!(
        rewritten.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&rewritten.stderr)
    );
    assert_eq!(fs::read(&output).expect("read output"), b"Hello world\n");
    let report = String::from_utf8(rewritten.stdout).expect("utf8 report");
    assert!(report.contains("\"command\": \"rewrite\""));
    assert!(report.contains("\"backend\": \"fake\""));
    assert!(!report.contains("Hello world"));
}

#[test]
fn rewrite_dry_run_diff_and_trace_are_safe_and_non_replacing() {
    let directory = tempdir().expect("temporary directory");
    let source_doc = directory.path().join("source.txt");
    let output = directory.path().join("out.txt");
    let trace = directory.path().join("trace.json");
    fs::write(&source_doc, "Hello world\n").expect("write source");
    let data = activate_imported_fake(directory.path());

    binary()
        .arg("-D")
        .arg(&data)
        .args(["rewrite", "--diff", "--dry-run"])
        .arg(&source_doc)
        .arg("-o")
        .arg(&output)
        .arg("--trace")
        .arg(&trace)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"command\": \"rewrite\""))
        .stdout(predicate::str::contains("Hello world").not())
        .stderr(predicate::str::contains("diff: unchanged"));
    assert!(!output.exists(), "dry-run must not create --output");
    let trace_text = fs::read_to_string(&trace).expect("read trace");
    assert!(trace_text.contains("\"command\": \"rewrite\""));
    assert!(!trace_text.contains("Hello world"));
    assert_eq!(
        fs::read(&source_doc).expect("source remains"),
        b"Hello world\n"
    );

    fs::write(&output, b"keep existing").expect("write existing destination");
    binary()
        .arg("-D")
        .arg(&data)
        .args(["rewrite"])
        .arg(&source_doc)
        .arg("-o")
        .arg(&output)
        .assert()
        .code(3)
        .stderr(predicate::str::contains("\"command\": \"rewrite\""))
        .stderr(predicate::str::contains("\"code\": \"output_exists\""));
    assert_eq!(fs::read(&output).expect("dest remains"), b"keep existing");
}

#[test]
fn rewrite_in_place_dry_run_does_not_mutate() {
    let directory = tempdir().expect("temporary directory");
    let source_doc = directory.path().join("draft.txt");
    fs::write(&source_doc, "Hello world\n").expect("write source");
    let data = activate_imported_fake(directory.path());

    binary()
        .arg("-D")
        .arg(&data)
        .args(["rewrite", "-i", "--dry-run"])
        .arg(&source_doc)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"command\": \"rewrite\""));
    assert_eq!(
        fs::read(&source_doc).expect("source remains"),
        b"Hello world\n"
    );
    assert!(!directory.path().join("draft.txt.retonr-backup").exists());
}
