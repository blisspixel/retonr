//! Recoverable in-place replacement fixtures for `check` and `rewrite`.

use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

fn binary() -> Command {
    Command::cargo_bin("retonr").expect("built binary")
}

#[test]
fn check_in_place_without_backup_is_usage() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("draft.txt");
    let candidate = directory.path().join("candidate.txt");
    fs::write(&source, "Hello world\n").expect("write source");
    fs::write(&candidate, "Hello, world!\n").expect("write candidate");

    binary()
        .args(["check", "--in-place"])
        .arg(&source)
        .arg(&candidate)
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("\"code\": \"invalid_invocation\""));
    assert_eq!(fs::read(&source).expect("source remains"), b"Hello world\n");
    assert!(!directory.path().join("draft.txt.retonr-backup").exists());
}

#[test]
fn check_backup_without_in_place_is_usage() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("draft.txt");
    let candidate = directory.path().join("candidate.txt");
    fs::write(&source, "Hello world\n").expect("write source");
    fs::write(&candidate, "Hello, world!\n").expect("write candidate");

    binary()
        .args(["check", "--backup"])
        .arg(&source)
        .arg(&candidate)
        .assert()
        .code(2)
        .stderr(predicate::str::contains("\"code\": \"invalid_invocation\""));
    assert_eq!(fs::read(&source).expect("source remains"), b"Hello world\n");
}

#[test]
fn check_in_place_with_output_is_usage() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("draft.txt");
    let candidate = directory.path().join("candidate.txt");
    let output = directory.path().join("out.txt");
    fs::write(&source, "Hello world\n").expect("write source");
    fs::write(&candidate, "Hello, world!\n").expect("write candidate");

    binary()
        .args(["check", "--in-place", "--backup", "--output"])
        .arg(&output)
        .arg(&source)
        .arg(&candidate)
        .assert()
        .code(2)
        .stderr(predicate::str::contains("\"code\": \"invalid_invocation\""));
    assert_eq!(fs::read(&source).expect("source remains"), b"Hello world\n");
    assert!(!output.exists());
}

#[test]
fn check_in_place_on_standard_input_is_usage() {
    let directory = tempdir().expect("temporary directory");
    let candidate = directory.path().join("candidate.txt");
    fs::write(&candidate, "Hello, world!\n").expect("write candidate");

    binary()
        .args([
            "check",
            "--in-place",
            "--backup",
            "-",
            candidate.to_str().expect("utf-8"),
        ])
        .write_stdin("Hello world\n")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("\"code\": \"invalid_invocation\""));
}

#[test]
fn check_in_place_replaces_the_source_and_retains_a_sibling_backup() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("draft.txt");
    let candidate = directory.path().join("candidate.txt");
    fs::write(&source, "Hello world\n").expect("write source");
    fs::write(&candidate, "Hello, world!\n").expect("write candidate");

    let output = binary()
        .args(["check", "--in-place", "--backup"])
        .arg(&source)
        .arg(&candidate)
        .output()
        .expect("run in-place check");
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).expect("UTF-8");
    assert!(text.contains("\"status\": \"rewritten\""));
    assert!(text.contains("\"backup\": \"draft.txt.retonr-backup\""));
    assert!(!text.contains(&directory.path().display().to_string()));
    assert!(!text.contains("Hello"));
    assert_eq!(
        fs::read(&source).expect("replaced source"),
        b"Hello, world!\n"
    );
    assert_eq!(
        fs::read(directory.path().join("draft.txt.retonr-backup")).expect("backup"),
        b"Hello world\n"
    );
    assert!(!directory.path().join("draft.txt.retonr-staging").exists());
}

#[test]
fn check_in_place_skips_backup_when_accepted_bytes_match() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("draft.txt");
    let candidate = directory.path().join("candidate.txt");
    fs::write(&source, "Hello, world!\n").expect("write source");
    fs::write(&candidate, "Hello, world!\n").expect("write candidate");

    binary()
        .args(["--format", "text", "check", "--in-place", "--backup"])
        .arg(&source)
        .arg(&candidate)
        .assert()
        .success()
        .stdout(predicate::str::contains("backup:").not());
    assert_eq!(
        fs::read(&source).expect("source remains"),
        b"Hello, world!\n"
    );
    assert!(!directory.path().join("draft.txt.retonr-backup").exists());
}

#[test]
fn check_in_place_abstention_does_not_write_a_backup() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("draft.txt");
    let candidate = directory.path().join("candidate.txt");
    fs::write(&source, "Version 2\n").expect("write source");
    fs::write(&candidate, "Version 3\n").expect("write candidate");

    binary()
        .args(["check", "--in-place", "--backup"])
        .arg(&source)
        .arg(&candidate)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"abstained\""))
        .stdout(predicate::str::contains("\"backup\"").not());
    assert_eq!(fs::read(&source).expect("source remains"), b"Version 2\n");
    assert!(!directory.path().join("draft.txt.retonr-backup").exists());
    assert!(!directory.path().join("draft.txt.retonr-staging").exists());
}

#[test]
fn check_in_place_dry_run_does_not_mutate() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("draft.txt");
    let candidate = directory.path().join("candidate.txt");
    fs::write(&source, "Hello world\n").expect("write source");
    fs::write(&candidate, "Hello, world!\n").expect("write candidate");

    binary()
        .args(["check", "--in-place", "--backup", "--dry-run"])
        .arg(&source)
        .arg(&candidate)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"rewritten\""))
        .stdout(predicate::str::contains("\"backup\"").not());
    assert_eq!(fs::read(&source).expect("source remains"), b"Hello world\n");
    assert!(!directory.path().join("draft.txt.retonr-backup").exists());
}

#[test]
fn check_in_place_refuses_an_existing_backup() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("draft.txt");
    let candidate = directory.path().join("candidate.txt");
    let backup = directory.path().join("draft.txt.retonr-backup");
    fs::write(&source, "Hello world\n").expect("write source");
    fs::write(&candidate, "Hello, world!\n").expect("write candidate");
    fs::write(&backup, "keep\n").expect("write backup");

    binary()
        .args(["check", "--in-place", "--backup"])
        .arg(&source)
        .arg(&candidate)
        .assert()
        .code(3)
        .stderr(predicate::str::contains("\"code\": \"output_exists\""));
    assert_eq!(fs::read(&source).expect("source remains"), b"Hello world\n");
    assert_eq!(fs::read(&backup).expect("backup remains"), b"keep\n");
}

#[test]
fn rewrite_in_place_fails_closed_without_mutation() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("draft.txt");
    fs::write(&source, "Hello world\n").expect("write source");

    binary()
        .args(["rewrite", "--in-place", "--backup"])
        .arg(&source)
        .assert()
        .code(4)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("\"command\": \"rewrite\""))
        .stderr(predicate::str::contains("\"code\": \"unsupported\""));
    assert_eq!(fs::read(&source).expect("source remains"), b"Hello world\n");
    assert!(!directory.path().join("draft.txt.retonr-backup").exists());
    assert!(!directory.path().join("draft.txt.retonr-staging").exists());
}
