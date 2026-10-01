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

fn folder_result(path: &std::path::Path, recursive: bool) -> serde_json::Value {
    let mut command = Command::cargo_bin("retonr").expect("binary");
    command.arg("lint").arg(path);
    if recursive {
        command.arg("--recursive");
    }
    let output = command.assert().success().get_output().stdout.clone();
    serde_json::from_slice::<serde_json::Value>(&output).expect("JSON")["result"].clone()
}

#[test]
fn folder_lint_is_sorted_read_only_and_recursion_is_explicit() {
    let root = tempdir().expect("temporary directory");
    fs::write(root.path().join("z.txt"), "Certainly! Draft.").expect("write draft");
    fs::write(root.path().join("a.txt"), "A clear sentence.").expect("write clean");
    fs::write(root.path().join("binary.dat"), [0xff, 0xfe, 0x00]).expect("write binary");
    fs::write(root.path().join(".hidden.txt"), "Certainly! Hidden.").expect("write hidden");
    fs::create_dir(root.path().join("nested")).expect("create nested");
    fs::write(root.path().join("nested/child.txt"), "Certainly! Child.").expect("write child");
    fs::create_dir(root.path().join("target")).expect("create ignored");
    fs::write(
        root.path().join("target/ignored.txt"),
        "Certainly! Ignored.",
    )
    .expect("write ignored");
    let before = fs::read(root.path().join("z.txt")).expect("read source");
    let flat = folder_result(root.path(), false);
    assert_eq!(flat["document_count"], 2);
    assert_eq!(flat["documents"][0]["relative_path"], "a.txt");
    assert_eq!(flat["documents"][1]["relative_path"], "z.txt");
    assert_eq!(flat["findings_count"], 1);
    let nested = folder_result(root.path(), true);
    assert_eq!(nested["document_count"], 3);
    assert_eq!(nested["findings_count"], 2);
    assert_eq!(nested["documents"][1]["relative_path"], "nested/child.txt");
    assert!(
        nested["skipped"]
            .as_array()
            .expect("skips")
            .iter()
            .any(|entry| entry["reason"] == "unsupported_encoding")
    );
    assert_eq!(
        before,
        fs::read(root.path().join("z.txt")).expect("source unchanged")
    );
    Command::cargo_bin("retonr")
        .expect("binary")
        .args(["lint", "--recursive", "--fail-on-findings"])
        .arg(root.path())
        .assert()
        .code(3);
    Command::cargo_bin("retonr")
        .expect("binary")
        .args(["-f", "text", "lint", "-r"])
        .arg(root.path())
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "document nested/child.txt status=findings_detected findings=1",
        ));
}

#[test]
fn folder_candidate_and_single_document_recursion_are_refused() {
    let root = tempdir().expect("temporary directory");
    let draft = root.path().join("draft.txt");
    fs::write(&draft, "A clear sentence.").expect("write draft");
    Command::cargo_bin("retonr")
        .expect("binary")
        .arg("lint")
        .arg(root.path())
        .arg("--candidate")
        .arg(&draft)
        .assert()
        .code(2);
    Command::cargo_bin("retonr")
        .expect("binary")
        .args(["lint", "--recursive"])
        .arg(&draft)
        .assert()
        .code(2);
    let empty = tempdir().expect("empty directory");
    assert_eq!(folder_result(empty.path(), true)["status"], "clean");
}

#[cfg(unix)]
#[test]
fn folder_lint_does_not_follow_links_and_escapes_report_paths() {
    use std::os::unix::fs::symlink;
    let root = tempdir().expect("temporary directory");
    let outside = tempdir().expect("outside directory");
    fs::write(outside.path().join("outside.txt"), "Certainly! Outside.").expect("write outside");
    symlink(outside.path(), root.path().join("linked")).expect("link directory");
    Command::cargo_bin("retonr")
        .expect("binary")
        .arg("lint")
        .arg(root.path().join("linked"))
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty());
    fs::write(root.path().join("draft\u{202e}\n.txt"), "Certainly! Draft.")
        .expect("write escaped path");
    let result = folder_result(root.path(), true);
    assert_eq!(result["document_count"], 1);
    assert!(
        result["skipped"]
            .as_array()
            .expect("skips")
            .iter()
            .any(|entry| entry["reason"] == "symlink")
    );
    let output = Command::cargo_bin("retonr")
        .expect("binary")
        .args(["-f", "text", "lint", "-r"])
        .arg(root.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).expect("UTF-8");
    assert!(text.contains("draft\\u{202e}\\n.txt"));
    assert!(!text.contains('\u{202e}'));
}

#[test]
fn folder_lint_escapes_bidi_filenames_in_text_and_json() {
    let root = tempdir().expect("temporary directory");
    let name = "draft\u{202e}.txt";
    fs::write(root.path().join(name), "Certainly! Draft.").expect("write bidi filename");
    for format in ["text", "json"] {
        let output = Command::cargo_bin("retonr")
            .expect("binary")
            .args(["-f", format, "lint"])
            .arg(root.path())
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let text = String::from_utf8(output).expect("UTF-8");
        assert!(!text.contains('\u{202e}'));
        if format == "json" {
            let value: serde_json::Value = serde_json::from_str(&text).expect("JSON");
            assert_eq!(value["result"]["documents"][0]["relative_path"], name);
        } else {
            assert!(text.contains("draft\\u{202e}.txt"));
        }
    }
}

#[test]
fn folder_lint_refuses_oversized_files_without_partial_results() {
    let root = tempdir().expect("temporary directory");
    fs::write(root.path().join("a.txt"), "A clear sentence.").expect("write valid file");
    let oversized = fs::File::create(root.path().join("z.txt")).expect("create oversized file");
    oversized
        .set_len(u64::try_from(rewrite_app::MAX_CANDIDATE_CHECK_BYTES).expect("limit") + 1)
        .expect("set file size");
    Command::cargo_bin("retonr")
        .expect("binary")
        .arg("lint")
        .arg(root.path())
        .assert()
        .code(4)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("resource_limit_exceeded"));
}

#[test]
fn folder_lint_refuses_excessive_findings_without_a_partial_clean_report() {
    let root = tempdir().expect("temporary directory");
    fs::write(root.path().join("dense.txt"), "!!! ".repeat(16_385))
        .expect("write repeated findings");
    Command::cargo_bin("retonr")
        .expect("binary")
        .arg("lint")
        .arg(root.path())
        .assert()
        .code(4)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("resource_limit_exceeded"));
}

#[test]
fn comparative_lint_counts_removed_and_introduced_repeated_findings() {
    let root = tempdir().expect("temporary directory");
    let source = root.path().join("source.txt");
    let candidate = root.path().join("candidate.txt");
    fs::write(&source, "First!!! Second!!! Third!!!").expect("write source");
    fs::write(&candidate, "First!!! Second. Third.").expect("write candidate");
    Command::cargo_bin("retonr")
        .expect("binary")
        .arg("lint")
        .arg(&source)
        .arg("--candidate")
        .arg(&candidate)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"resolved_count\": 2"))
        .stdout(predicate::str::contains("\"retained_count\": 1"));
    Command::cargo_bin("retonr")
        .expect("binary")
        .args(["lint", "--fail-on-findings"])
        .arg(&candidate)
        .arg("--candidate")
        .arg(&source)
        .assert()
        .code(3)
        .stdout(predicate::str::contains("\"introduced_count\": 2"));
}
