use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

fn binary() -> Command {
    Command::cargo_bin("retonr").expect("built binary")
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
fn rewrite_standard_input_is_accepted_then_refused() {
    binary()
        .args(["rewrite", "-"])
        .write_stdin(b"Hello world\n".to_vec())
        .assert()
        .code(4)
        .stderr(predicate::str::contains("\"command\": \"rewrite\""));
}
