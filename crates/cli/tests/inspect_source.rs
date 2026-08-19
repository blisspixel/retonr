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
fn inspect_inventories_a_directory_without_recursion_or_mutation() {
    let directory = tempdir().expect("temporary directory");
    let root = directory.path();
    fs::write(root.join("a.txt"), "alpha\n").expect("write a");
    fs::write(root.join("b.txt"), "bravo\n").expect("write b");
    fs::create_dir(root.join("nested")).expect("create nested");
    fs::write(root.join("nested").join("inner.txt"), "inner\n").expect("write nested");
    fs::write(root.join(".hidden.txt"), "secret\n").expect("write hidden");

    let output = binary()
        .arg("inspect")
        .arg(root)
        .output()
        .expect("run directory inspect");
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).expect("UTF-8");
    assert!(text.contains("\"scope\": \"directory\""));
    assert!(text.contains("\"recursion\": \"none\""));
    assert!(text.contains("\"links\": \"not_followed\""));
    assert!(text.contains("\"relative_path\": \"a.txt\""));
    assert!(text.contains("\"relative_path\": \"b.txt\""));
    assert!(!text.contains("inner.txt"));
    assert!(!text.contains("alpha"));
    assert!(!text.contains("bravo"));
    assert!(!text.contains("secret"));
    assert!(text.contains("\"reason\": \"directory\""));
    assert!(text.contains("\"reason\": \"hidden\""));
    assert!(!text.contains(&root.display().to_string()));
    assert_eq!(fs::read(root.join("a.txt")).expect("read a"), b"alpha\n");
    assert_eq!(
        fs::read(root.join("nested").join("inner.txt")).expect("read nested"),
        b"inner\n"
    );

    binary()
        .args(["--format", "text", "inspect"])
        .arg(root)
        .assert()
        .success()
        .stdout(predicate::str::contains("scope: directory"))
        .stdout(predicate::str::contains("document a.txt"))
        .stdout(predicate::str::contains("skipped nested reason=directory"))
        .stdout(predicate::str::contains(
            "skipped .hidden.txt reason=hidden",
        ));
}

#[test]
fn inspect_recursive_includes_nested_files_and_skips_ignored_trees() {
    let directory = tempdir().expect("temporary directory");
    let root = directory.path();
    fs::write(root.join("a.txt"), "alpha\n").expect("write a");
    fs::create_dir(root.join("nested")).expect("create nested");
    fs::write(root.join("nested").join("inner.txt"), "inner\n").expect("write nested");
    fs::create_dir(root.join("target")).expect("create target");
    fs::write(root.join("target").join("built.txt"), "built\n").expect("write ignored");

    let output = binary()
        .args(["inspect", "--recursive"])
        .arg(root)
        .output()
        .expect("run recursive inspect");
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).expect("UTF-8");
    assert!(text.contains("\"recursion\": \"bounded\""));
    assert!(text.contains("\"relative_path\": \"nested/inner.txt\""));
    assert!(text.contains("\"relative_path\": \"a.txt\""));
    assert!(!text.contains("built.txt"));
    assert!(!text.contains("inner\n"));
    assert!(text.contains("\"reason\": \"ignored\""));
    assert!(!text.contains(&root.display().to_string()));
    assert_eq!(
        fs::read(root.join("nested").join("inner.txt")).expect("read nested"),
        b"inner\n"
    );

    binary()
        .args(["--format", "text", "inspect", "--recursive"])
        .arg(root)
        .assert()
        .success()
        .stdout(predicate::str::contains("recursion: bounded"))
        .stdout(predicate::str::contains("document nested/inner.txt"))
        .stdout(predicate::str::contains("skipped target reason=ignored"));
}

#[test]
fn inspect_recursive_on_a_file_is_usage() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("a.txt");
    fs::write(&source, "alpha\n").expect("write a");
    binary()
        .args(["inspect", "--recursive"])
        .arg(&source)
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("\"code\": \"invalid_invocation\""));
}

#[test]
fn inspect_recursive_on_standard_input_is_usage() {
    binary()
        .args(["inspect", "--recursive", "-"])
        .write_stdin("alpha\n")
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("\"code\": \"invalid_invocation\""));

    binary()
        .args(["--format", "text", "inspect", "--recursive", "-"])
        .write_stdin("alpha\n")
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains(
            "recursive inspect requires a directory",
        ));
}

fn source_mutated(path: &std::path::Path, expected: &[u8]) -> bool {
    fs::read(path).expect("read source") != expected
}
