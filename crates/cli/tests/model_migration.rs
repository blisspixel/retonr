use std::{fs, path::Path};

use assert_cmd::Command;
use predicates::prelude::*;
use rewrite_model::{
    ARTIFACT_MANIFEST_SCHEMA_VERSION, ArtifactId, ArtifactManifest, ArtifactRole, ArtifactSource,
    DeclaredCapabilities, LicenseRecord,
};
use rewrite_types::Digest;
use rusqlite::Connection;
use serde_json::Value;
use tempfile::tempdir;

const ARTIFACT_BYTES: &[u8] = b"model bytes for migration process tests";

fn initialize_repository(root: &Path, data: &Path, fixture_name: &str) {
    let digest = Digest::sha256(ARTIFACT_BYTES);
    let manifest = ArtifactManifest {
        schema_version: ARTIFACT_MANIFEST_SCHEMA_VERSION,
        artifact_id: ArtifactId::from_digest(digest.clone()),
        source: ArtifactSource {
            origin: "fixture/model".to_owned(),
            revision: "fixture-revision".to_owned(),
        },
        artifact_digest: digest,
        byte_size: u64::try_from(ARTIFACT_BYTES.len()).expect("fixture size"),
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
    };
    let source = root.join(format!("{fixture_name}.gguf"));
    let manifest_path = root.join(format!("{fixture_name}.json"));
    fs::write(&source, ARTIFACT_BYTES).expect("write source");
    fs::write(
        &manifest_path,
        serde_json::to_vec(&manifest).expect("encode manifest"),
    )
    .expect("write manifest");
    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .arg("--data-dir")
        .arg(data)
        .args(["model", "import"])
        .arg(source)
        .arg("--manifest")
        .arg(manifest_path)
        .assert()
        .success();
}

fn downgrade_to_schema_two(data: &Path) {
    let connection =
        Connection::open(data.join("artifact-state.sqlite3")).expect("open current state fixture");
    connection
        .execute_batch(
            "DROP TABLE qualification_v2_records;
             DROP TABLE effective_package_evidence;
             DROP TABLE effective_runtime_states;
             DROP TABLE runtime_build_identities;
             DROP TABLE artifact_set_manifests;
             PRAGMA user_version = 2;",
        )
        .expect("restore canonical schema two shape");
}

fn assert_backup(data: &Path, backup_key: &str) {
    let relative = Path::new(backup_key);
    assert!(!relative.is_absolute());
    assert_eq!(relative.components().count(), 1);
    let metadata = fs::symlink_metadata(data.join(relative)).expect("retained backup exists");
    assert!(metadata.is_file());
    assert!(!metadata.file_type().is_symlink());
}

#[test]
fn migration_requires_confirmation_and_reports_current_state() {
    let directory = tempdir().expect("temporary directory");
    let data = directory.path().join("repository");
    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .arg("--data-dir")
        .arg(&data)
        .args(["model", "migrate"])
        .assert()
        .code(3)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("confirmation_required"));
    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .arg("--data-dir")
        .arg(&data)
        .args(["model", "migrate", "--yes"])
        .assert()
        .code(1)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("\"command\": \"model.migrate\""))
        .stderr(predicate::str::contains("repository_not_initialized"));

    initialize_repository(directory.path(), &data, "current");
    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["--format", "text", "--data-dir"])
        .arg(&data)
        .args(["model", "migrate", "--yes"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::eq(
            "disposition: already_current\nfrom_schema: 3\nto_schema: 3\n",
        ));
}

#[test]
fn migrates_schema_two_with_backup_then_inventory_opens_exact_state() {
    let directory = tempdir().expect("temporary directory");
    let data = directory.path().join("repository");
    initialize_repository(directory.path(), &data, "legacy");
    downgrade_to_schema_two(&data);

    let output = Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .arg("--data-dir")
        .arg(&data)
        .args(["model", "migrate", "--yes"])
        .output()
        .expect("migrate schema two");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout.clone()).expect("UTF-8 migration output");
    assert!(!text.contains(&data.display().to_string()));
    let output: Value = serde_json::from_slice(&output.stdout).expect("parse migration JSON");
    assert_eq!(output["command"], "model.migrate");
    assert_eq!(output["result"]["disposition"], "migrated");
    assert_eq!(output["result"]["from_schema"], 2);
    assert_eq!(output["result"]["to_schema"], 3);
    let backup_key = output["result"]["backup_key"]
        .as_str()
        .expect("opaque backup key");
    assert_backup(&data, backup_key);

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .arg("--data-dir")
        .arg(&data)
        .args(["model", "inventory"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::contains("\"health\": \"clean\""));
}
