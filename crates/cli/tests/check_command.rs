use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn reports_rewritten_candidate_as_json() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.txt");
    let candidate = directory.path().join("candidate.txt");
    fs::write(&source, "Hello world\n").expect("write source fixture");
    fs::write(&candidate, "Hello, world!\n").expect("write candidate fixture");

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["check"])
        .arg(source)
        .arg(candidate)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"schema_version\": 2"))
        .stdout(predicate::str::contains("\"status\": \"rewritten\""))
        .stdout(predicate::str::contains("\"assessments\""))
        .stdout(predicate::str::contains("\"generation\"").not());
}

#[test]
fn abstention_can_be_used_as_a_ci_failure() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.txt");
    let candidate = directory.path().join("candidate.txt");
    fs::write(&source, "Version 2\n").expect("write source fixture");
    fs::write(&candidate, "Version 3\n").expect("write candidate fixture");

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["check"])
        .arg(source)
        .arg(candidate)
        .arg("--fail-on-abstain")
        .assert()
        .code(3)
        .stdout(predicate::str::contains("\"status\": \"abstained\""))
        .stdout(predicate::str::contains(
            "\"reason\": \"protected_value_changed\"",
        ));
}

#[test]
fn global_format_is_accepted_before_or_after_the_command() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.txt");
    let candidate = directory.path().join("candidate.txt");
    fs::write(&source, "Hello world\n").expect("write source fixture");
    fs::write(&candidate, "Hello, world!\n").expect("write candidate fixture");

    for arguments in [
        vec!["--format", "text", "check"],
        vec!["check", "--format", "text"],
    ] {
        Command::cargo_bin("retonr")
            .expect("compiled CLI")
            .args(arguments)
            .arg(&source)
            .arg(&candidate)
            .assert()
            .success()
            .stdout(predicate::str::contains("status: rewritten"));
    }
}

#[test]
fn text_report_never_contains_raw_document_content() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.txt");
    let candidate = directory.path().join("candidate.txt");
    fs::write(&source, "private phrase\n").expect("write source fixture");
    fs::write(&candidate, "private phrase.\n").expect("write candidate fixture");

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["check"])
        .arg(source)
        .arg(candidate)
        .args(["--format", "text"])
        .assert()
        .success()
        .stdout(predicate::str::contains("status: rewritten"))
        .stdout(predicate::str::contains("private phrase").not());
}
