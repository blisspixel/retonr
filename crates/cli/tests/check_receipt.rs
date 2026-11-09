use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use rewrite_types::Digest;
use serde_json::Value;
use tempfile::tempdir;

fn binary() -> Command {
    Command::cargo_bin("retonr").expect("built binary")
}

fn write_file(directory: &std::path::Path, name: &str, body: &str) -> std::path::PathBuf {
    let path = directory.join(name);
    fs::write(&path, body).expect("write file");
    path
}

#[test]
fn receipt_is_absent_unless_asked_and_does_not_change_output() {
    let directory = tempdir().expect("temporary directory");
    let source_body = "Hello world\n";
    let candidate_body = "hello world\n";
    let source = write_file(directory.path(), "source.txt", source_body);
    let candidate = write_file(directory.path(), "candidate.txt", candidate_body);
    let plain = directory.path().join("plain.txt");
    binary()
        .arg("check")
        .arg(&source)
        .arg(&candidate)
        .args(["--output"])
        .arg(&plain)
        .assert()
        .success();
    let kept = directory.path().join("kept.txt");
    let receipt = directory.path().join("receipt.json");
    binary()
        .arg("check")
        .arg(&source)
        .arg(&candidate)
        .args(["--output"])
        .arg(&kept)
        .args(["--receipt"])
        .arg(&receipt)
        .assert()
        .success();
    assert_eq!(
        fs::read(&plain).expect("plain"),
        fs::read(&kept).expect("kept")
    );
    assert_eq!(fs::read_to_string(&source).expect("source"), source_body);
    assert!(!directory.path().join("missing.json").exists());
    assert!(receipt.is_file());
}

#[test]
fn receipt_records_digests_level_and_change_share_without_source_text() {
    let directory = tempdir().expect("temporary directory");
    let source_body = "Hello world\n";
    let candidate_body = "hello world\n";
    let source = write_file(directory.path(), "source.txt", source_body);
    let candidate = write_file(directory.path(), "candidate.txt", candidate_body);
    let output = directory.path().join("accepted.txt");
    let receipt = directory.path().join("receipt.json");
    binary()
        .arg("check")
        .arg(&source)
        .arg(&candidate)
        .args(["--edit-level", "rewrite", "--output"])
        .arg(&output)
        .args(["--receipt"])
        .arg(&receipt)
        .assert()
        .success()
        .stdout(predicate::str::contains("hello world").not());

    let accepted = fs::read(&output).expect("accepted");
    assert_eq!(accepted, candidate_body.as_bytes());
    assert_eq!(fs::read_to_string(&source).expect("source"), source_body);
    let text = fs::read_to_string(&receipt).expect("receipt");
    assert!(!text.contains("Hello world"));
    assert!(!text.contains("hello world"));
    assert!(
        !String::from_utf8(accepted.clone())
            .expect("utf-8")
            .contains("schema_version")
    );
    let value: Value = serde_json::from_str(&text).expect("json");
    assert_eq!(value["schema_version"], 1);
    assert_eq!(
        value["notice"],
        "The report is evidence of Retonr's process. It is not an authorship certificate, legal opinion, publisher approval, or proof of compliance."
    );
    assert_eq!(value["product"], "retonr");
    assert_eq!(value["product_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(value["commit"], env!("RETONR_COMMIT"));
    let created_at = value["created_at"].as_str().expect("time");
    assert_eq!(created_at.len(), "1970-01-01T00:00:00Z".len());
    assert!(created_at.ends_with('Z'));
    let source_digest = Digest::sha256(source_body.as_bytes());
    let output_digest = Digest::sha256(&accepted);
    assert_eq!(
        value["record"]["source_digest"].as_str(),
        Some(source_digest.as_str())
    );
    assert_eq!(
        value["record"]["output_digest"].as_str(),
        Some(output_digest.as_str())
    );
    assert_eq!(value["record"]["schema_version"], 2);
    assert!(value["record"].get("generation").is_none());
    assert!(!text.contains("marking_assurance"));
    assert_eq!(value["edit_level_effective"], "touch-up");
    assert_eq!(value["edit_level_requested"], "rewrite");
    assert!(
        value["edit_level_note"]
            .as_str()
            .expect("note")
            .contains("touch-up")
    );
    assert!(value["changed_token_pct"].is_number());
    assert!(value["changed_char_pct"].is_number());
    assert!(value.get("style_distance").is_none());
}

#[test]
fn occupied_receipt_and_directory_check_write_nothing() {
    let directory = tempdir().expect("temporary directory");
    let source_body = "Hello world\n";
    let source = write_file(directory.path(), "source.txt", source_body);
    let candidate = write_file(directory.path(), "candidate.txt", "hello world\n");
    let output = directory.path().join("accepted.txt");
    let receipt = write_file(directory.path(), "receipt.json", "keep\n");
    binary()
        .arg("check")
        .arg(&source)
        .arg(&candidate)
        .args(["--output"])
        .arg(&output)
        .args(["--receipt"])
        .arg(&receipt)
        .assert()
        .code(3)
        .stderr(predicate::str::contains(
            "output destination already exists",
        ));
    assert_eq!(fs::read_to_string(&receipt).expect("receipt"), "keep\n");
    assert!(!output.exists());
    assert_eq!(fs::read_to_string(&source).expect("source"), source_body);

    let source_dir = directory.path().join("sources");
    let candidate_dir = directory.path().join("candidates");
    fs::create_dir(&source_dir).expect("source dir");
    fs::create_dir(&candidate_dir).expect("candidate dir");
    write_file(&source_dir, "a.txt", source_body);
    write_file(&candidate_dir, "a.txt", "hello world\n");
    let directory_receipt = directory.path().join("directory.json");
    binary()
        .args(["check", "--dry-run", "--receipt"])
        .arg(&directory_receipt)
        .arg(&source_dir)
        .arg(&candidate_dir)
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "a directory check does not write a receipt",
        ));
    assert!(!directory_receipt.exists());
}

#[test]
fn dry_run_writes_the_receipt_and_not_the_document() {
    let directory = tempdir().expect("temporary directory");
    let source_body = "Hello world\n";
    let candidate_body = "Hello there\n";
    let source = write_file(directory.path(), "source.txt", source_body);
    let candidate = write_file(directory.path(), "candidate.txt", candidate_body);
    let output = directory.path().join("accepted.txt");
    let receipt = directory.path().join("receipt.json");
    binary()
        .args(["check", "--dry-run", "--fail-on-abstain", "--output"])
        .arg(&output)
        .arg(&source)
        .arg(&candidate)
        .args(["--receipt"])
        .arg(&receipt)
        .assert()
        .code(3);
    assert!(!output.exists());
    assert_eq!(fs::read_to_string(&source).expect("source"), source_body);
    assert_eq!(
        fs::read_to_string(&candidate).expect("candidate"),
        candidate_body
    );
    let value: Value =
        serde_json::from_str(&fs::read_to_string(&receipt).expect("receipt")).expect("json");
    assert_eq!(value["record"]["reason"], "semantic_uncertain");
    let source_digest = Digest::sha256(source_body.as_bytes());
    assert_eq!(
        value["record"]["source_digest"].as_str(),
        Some(source_digest.as_str())
    );
    assert_eq!(
        value["record"]["output_digest"].as_str(),
        Some(source_digest.as_str())
    );
    let text = fs::read_to_string(&receipt).expect("receipt text");
    assert!(!text.contains("Hello there"));
    assert!(!text.contains(candidate_body.trim()));
}

#[test]
fn receipt_help_names_the_file_and_refuses_a_shared_output_path() {
    binary()
        .args(["check", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--receipt"))
        .stdout(predicate::str::contains("authorship certificate"));

    let directory = tempdir().expect("temporary directory");
    let source = write_file(directory.path(), "source.txt", "Hello world\n");
    let candidate = write_file(directory.path(), "candidate.txt", "hello world\n");
    let shared = directory.path().join("shared.txt");
    binary()
        .arg("check")
        .arg(&source)
        .arg(&candidate)
        .args(["--output"])
        .arg(&shared)
        .args(["--receipt"])
        .arg(&shared)
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "a receipt path must differ from the output",
        ));
    assert!(!shared.exists());
}
