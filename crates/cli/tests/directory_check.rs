use assert_cmd::Command;
use predicates::prelude::*;
use std::{fs, path::Path};
use tempfile::{TempDir, tempdir};

fn fixture() -> serde_json::Value {
    serde_json::from_str(include_str!("../../../fixtures/cli/directory-check.json"))
        .expect("directory-check fixtures")
}

fn roots() -> (TempDir, TempDir) {
    (
        tempdir().expect("source root"),
        tempdir().expect("candidate root"),
    )
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().expect("parent")).expect("create parents");
    fs::write(path, text).expect("write document");
}

fn command(source: &Path, candidate: &Path) -> Command {
    let mut command = Command::cargo_bin("retonr").expect("binary");
    command
        .arg("check")
        .arg(source)
        .arg(candidate)
        .arg("--dry-run");
    command
}

fn result(source: &Path, candidate: &Path, recursive: bool) -> serde_json::Value {
    let mut command = command(source, candidate);
    if recursive {
        command.arg("-r");
    }
    let output = command.assert().success().get_output().stdout.clone();
    serde_json::from_slice::<serde_json::Value>(&output).expect("JSON")["result"].clone()
}

#[test]
fn checked_in_pairs_are_sorted_read_only_and_report_acceptance_or_abstention() {
    let (source, candidate) = roots();
    for case in fixture().as_array().expect("cases") {
        let path = case["path"].as_str().expect("path");
        write(
            &source.path().join(path),
            case["source"].as_str().expect("source"),
        );
        write(
            &candidate.path().join(path),
            case["candidate"].as_str().expect("candidate"),
        );
    }
    let before = fs::read(source.path().join("z.txt")).expect("original");
    let flat = result(source.path(), candidate.path(), false);
    assert_eq!(flat["checked_count"], 2);
    assert_eq!(flat["recursion"], "none");
    let report = result(source.path(), candidate.path(), true);
    assert_eq!(report["checked_count"], 3);
    assert_eq!(report["abstained_count"], 1);
    assert_eq!(report["mode"], "dry_run");
    for (index, case) in fixture().as_array().expect("cases").iter().enumerate() {
        assert_eq!(report["pairs"][index]["relative_path"], case["path"]);
        assert_eq!(report["pairs"][index]["record"]["status"], case["status"]);
    }
    assert_eq!(
        before,
        fs::read(source.path().join("z.txt")).expect("unchanged source")
    );
    assert_eq!(
        fs::read(candidate.path().join("z.txt")).expect("unchanged candidate"),
        b"Version 3\n"
    );
    command(source.path(), candidate.path())
        .args(["-r", "--fail-on-abstain"])
        .assert()
        .code(3);
    command(source.path(), candidate.path())
        .args(["-r", "-f", "text"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "pair nested/b.txt status=rewritten",
        ))
        .stdout(predicate::str::contains("reason: protected_value_changed"));
}

#[test]
fn exact_relative_counterparts_are_required_and_missing_sides_are_visible() {
    let (source, candidate) = roots();
    write(&source.path().join("A.txt"), "Hello world\n");
    write(&candidate.path().join("a.txt"), "Hello, world!\n");
    let report = result(source.path(), candidate.path(), true);
    assert_eq!(report["checked_count"], 0);
    assert_eq!(report["unmatched"][0]["relative_path"], "A.txt");
    assert_eq!(report["unmatched"][0]["reason"], "missing_candidate");
    assert_eq!(report["unmatched"][1]["relative_path"], "a.txt");
    assert_eq!(report["unmatched"][1]["reason"], "missing_source");
    command(source.path(), candidate.path())
        .arg("--fail-on-abstain")
        .assert()
        .code(3);
}

#[test]
fn unsupported_pairs_derivative_markers_and_skipped_entries_are_visible() {
    let (source, candidate) = roots();
    for root in [source.path(), candidate.path()] {
        fs::write(root.join("utf16.txt"), [0xff, 0xfe, 0x41, 0x00]).expect("UTF-16");
        write(&root.join("carrier.txt"), "Hello world\n");
        write(&root.join("carrier.txt.xmp"), "sidecar");
        write(&root.join(".hidden.txt"), "Hello world\n");
        write(&root.join("target/ignored.txt"), "Hello world\n");
    }
    let report = result(source.path(), candidate.path(), true);
    assert_eq!(report["checked_count"], 1);
    assert_eq!(report["skipped_count"], 4);
    let unmatched = report["unmatched"].as_array().expect("unmatched");
    assert!(
        unmatched
            .iter()
            .any(|path| path["reason"] == "unsupported_encoding")
    );
    assert!(
        unmatched
            .iter()
            .any(|path| path["reason"] == "explicit_derivative_decision_required")
    );
    command(source.path(), candidate.path())
        .arg("--fail-on-abstain")
        .assert()
        .code(3);
}

#[test]
fn directory_checks_refuse_mutation_and_incompatible_document_flags() {
    let (source, candidate) = roots();
    Command::cargo_bin("retonr")
        .expect("binary")
        .arg("check")
        .arg(source.path())
        .arg(candidate.path())
        .assert()
        .code(4)
        .stdout(predicate::str::is_empty());
    for flag in [
        "--in-place",
        "--backup",
        "--diff",
        "--raw-terminal",
        "--yes",
    ] {
        command(source.path(), candidate.path())
            .arg(flag)
            .assert()
            .code(2)
            .stdout(predicate::str::is_empty());
    }
    for flag in ["--output", "--trace"] {
        let output = source.path().join("unwritten.txt");
        command(source.path(), candidate.path())
            .arg(flag)
            .arg(&output)
            .assert()
            .code(2)
            .stdout(predicate::str::is_empty());
        assert!(!output.exists());
    }
    command(source.path(), source.path()).assert().code(3);
    assert_eq!(
        result(source.path(), candidate.path(), true)["checked_count"],
        0
    );
    let file = source.path().join("file.txt");
    write(&file, "Hello world\n");
    command(&file, &file).arg("--recursive").assert().code(2);
}

#[test]
fn hard_link_aliases_are_refused_without_partial_results_or_mutation() {
    let (source, candidate) = roots();
    write(&source.path().join("linked.txt"), "Hello world\n");
    fs::hard_link(
        source.path().join("linked.txt"),
        candidate.path().join("linked.txt"),
    )
    .expect("hard link");
    command(source.path(), candidate.path())
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty());
    assert_eq!(
        fs::read(source.path().join("linked.txt")).expect("unchanged"),
        b"Hello world\n"
    );
}

#[test]
fn directory_reports_escape_bidi_names_and_never_include_document_text() {
    let (source, candidate) = roots();
    let name = "draft\u{202e}.txt";
    write(&source.path().join(name), "Private source 32\n");
    write(&candidate.path().join(name), "Private source 33\n");
    for format in ["text", "json"] {
        let output = command(source.path(), candidate.path())
            .args(["-f", format])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let text = String::from_utf8(output).expect("UTF-8");
        assert!(!text.contains('\u{202e}'));
        assert!(!text.contains("Private source"));
        if format == "json" {
            let value: serde_json::Value = serde_json::from_str(&text).expect("JSON");
            assert_eq!(value["result"]["pairs"][0]["relative_path"], name);
        } else {
            assert!(text.contains("draft\\u{202e}.txt"));
        }
    }
}

#[test]
fn layout_constraints_apply_to_each_supplied_candidate() {
    let (source, candidate) = roots();
    write(&source.path().join("draft.txt"), "Hello world\n");
    write(&candidate.path().join("draft.txt"), "Hello, world!\n");
    command(source.path(), candidate.path())
        .args(["--max-chars", "5", "--fail-on-abstain"])
        .assert()
        .code(3)
        .stdout(predicate::str::contains("character_budget_exceeded"));
}

#[cfg(unix)]
#[test]
fn directory_links_are_skipped_and_linked_roots_are_refused() {
    use std::os::unix::fs::symlink;
    let (source, candidate) = roots();
    let outside = tempdir().expect("outside");
    write(&outside.path().join("draft.txt"), "Hello world\n");
    symlink(outside.path(), source.path().join("linked")).expect("directory link");
    symlink(
        outside.path().join("draft.txt"),
        candidate.path().join("linked.txt"),
    )
    .expect("file link");
    let report = result(source.path(), candidate.path(), true);
    assert_eq!(report["checked_count"], 0);
    assert_eq!(report["skipped_count"], 2);
    command(&source.path().join("linked"), candidate.path())
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty());
    command(candidate.path(), &source.path().join("linked"))
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty());
}
