use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn clean_document_reports_clean_status() {
    let directory = tempdir().expect("tempdir");
    let file = directory.path().join("clean.txt");
    fs::write(
        &file,
        "Retonr preserves layout bounds and protects factual claims.",
    )
    .expect("write clean document");

    Command::cargo_bin("retonr")
        .expect("compiled retonr binary")
        .args(["lint"])
        .arg(&file)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"clean\""))
        .stdout(predicate::str::contains("\"findings_count\": 0"));

    Command::cargo_bin("retonr")
        .expect("compiled retonr binary")
        .args(["--format", "text", "lint"])
        .arg(&file)
        .assert()
        .success()
        .stdout(predicate::str::contains("status: clean"))
        .stdout(predicate::str::contains("findings: 0"));
}

#[test]
fn document_with_findings_detects_patterns() {
    let directory = tempdir().expect("tempdir");
    let file = directory.path().join("slop.txt");
    fs::write(
        &file,
        "Certainly! In today's rapidly evolving digital landscape, we build solutions.",
    )
    .expect("write slop document");

    Command::cargo_bin("retonr")
        .expect("compiled retonr binary")
        .args(["lint"])
        .arg(&file)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "\"status\": \"findings_detected\"",
        ))
        .stdout(predicate::str::contains("\"findings_count\": 2"))
        .stdout(predicate::str::contains("conversational_residue"))
        .stdout(predicate::str::contains("prefabricated_scene_setting"));

    Command::cargo_bin("retonr")
        .expect("compiled retonr binary")
        .args(["lint", "--fail-on-findings"])
        .arg(&file)
        .assert()
        .code(3)
        .stdout(predicate::str::contains(
            "\"status\": \"findings_detected\"",
        ));
}

#[test]
fn comparative_lint_reports_resolution_and_improvement() {
    let directory = tempdir().expect("tempdir");
    let source = directory.path().join("source.txt");
    let candidate = directory.path().join("candidate.txt");

    fs::write(
        &source,
        "Certainly! In today's rapidly evolving digital landscape, we build software.",
    )
    .expect("write source");
    fs::write(
        &candidate,
        "We build software with deterministic fidelity verification.",
    )
    .expect("write candidate");

    Command::cargo_bin("retonr")
        .expect("compiled retonr binary")
        .args(["lint"])
        .arg(&source)
        .arg("--candidate")
        .arg(&candidate)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"clean\""))
        .stdout(predicate::str::contains("\"resolved_count\": 2"))
        .stdout(predicate::str::contains("\"introduced_count\": 0"))
        .stdout(predicate::str::contains("\"is_strict_improvement\": true"));

    Command::cargo_bin("retonr")
        .expect("compiled retonr binary")
        .args(["--format", "text", "lint"])
        .arg(&source)
        .arg("--candidate")
        .arg(&candidate)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "comparison: 2 resolved, 0 introduced, 0 retained (strict improvement: true)",
        ));
}

#[test]
fn comparative_lint_fails_when_defects_are_introduced() {
    let directory = tempdir().expect("tempdir");
    let source = directory.path().join("source.txt");
    let candidate = directory.path().join("candidate.txt");

    fs::write(
        &source,
        "We build software with deterministic verification.",
    )
    .expect("write source");
    fs::write(
        &candidate,
        "Certainly! Let's embark on this journey together and unlock the power.",
    )
    .expect("write candidate");

    Command::cargo_bin("retonr")
        .expect("compiled retonr binary")
        .args(["lint", "--fail-on-findings"])
        .arg(&source)
        .arg("--candidate")
        .arg(&candidate)
        .assert()
        .code(3)
        .stdout(predicate::str::contains(
            "\"status\": \"findings_detected\"",
        ))
        .stdout(predicate::str::contains("\"introduced_count\": 2"));
}

#[test]
fn lint_reads_from_stdin() {
    Command::cargo_bin("retonr")
        .expect("compiled retonr binary")
        .args(["lint", "-"])
        .write_stdin("Certainly! This came from stdin.")
        .assert()
        .success()
        .stdout(predicate::str::contains("conversational_residue"));
}

#[test]
fn rejects_duplicate_stdin_streams() {
    Command::cargo_bin("retonr")
        .expect("compiled retonr binary")
        .args(["lint", "-", "--candidate", "-"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("invalid_invocation"));

    Command::cargo_bin("retonr")
        .expect("compiled retonr binary")
        .args(["--format", "text", "lint", "-", "--candidate", "-"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("command input is invalid"));
}
