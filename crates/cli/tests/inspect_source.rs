use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

fn binary() -> Command {
    Command::cargo_bin("retonr").expect("built binary")
}

#[test]
fn inspect_reports_utf8_facts_without_source_text() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("draft.txt");
    fs::write(&source, "Hello world\n").expect("write source");

    let output = binary()
        .arg("inspect")
        .arg(&source)
        .output()
        .expect("run inspect");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout.clone()).expect("UTF-8");
    assert!(!text.contains("Hello"));
    assert!(text.contains("\"command\": \"inspect\""));
    assert!(text.contains("\"encoding\": \"utf8\""));
    assert!(text.contains("\"utf8_bom\": false"));
    assert!(text.contains("\"c2pa_unstructured_text\": \"absent\""));
    assert!(text.contains("\"derivative\": \"not_required\""));
    assert!(text.contains("\"external_references\": \"not_checked\""));
    assert!(!source_mutated(&source, b"Hello world\n"));

    binary()
        .args(["--format", "text", "inspect"])
        .arg(&source)
        .assert()
        .success()
        .stdout(predicate::str::contains("encoding: utf8"))
        .stdout(predicate::str::contains("derivative: not_required"))
        .stdout(predicate::str::contains("Hello").not());
}

#[test]
fn inspect_names_a_sibling_sidecar_without_reading_it() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("draft.txt");
    let sidecar = directory.path().join("draft.txt.c2pa");
    fs::write(&source, "body\n").expect("write source");
    fs::write(&sidecar, "private credential bytes").expect("write sidecar");

    let output = binary()
        .arg("inspect")
        .arg(&source)
        .output()
        .expect("run inspect");
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).expect("UTF-8");
    assert!(text.contains("\"derivative\": \"explicit_decision_required\""));
    assert!(text.contains("draft.txt.c2pa"));
    assert!(!text.contains("private credential"));
    assert!(!text.contains(&directory.path().display().to_string()));
    assert_eq!(
        fs::read(&sidecar).expect("read sidecar"),
        b"private credential bytes"
    );
}

#[test]
fn inspect_records_utf16_without_decoding() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("utf16.txt");
    fs::write(&source, b"\xFF\xFEa\0").expect("write utf-16");
    binary()
        .arg("inspect")
        .arg(&source)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"encoding\": \"utf16_le\""))
        .stdout(predicate::str::contains(
            "\"c2pa_unstructured_text\": \"not_decoded\"",
        ))
        .stdout(predicate::str::contains("\"derivative\": \"not_checked\""));
}

#[test]
fn inspect_standard_input_skips_sidecar_scan() {
    binary()
        .args(["inspect", "-"])
        .write_stdin("Hello world\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"sidecar_scan\"").not())
        .stdout(predicate::str::contains("\"status\": \"not_applicable\""))
        .stdout(predicate::str::contains("Hello").not());
}

#[test]
fn inspect_rejects_a_directory() {
    let directory = tempdir().expect("temporary directory");
    binary()
        .arg("inspect")
        .arg(directory.path())
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("\"code\": \"input_unreadable\""));
}

fn source_mutated(path: &std::path::Path, expected: &[u8]) -> bool {
    fs::read(path).expect("read source") != expected
}
