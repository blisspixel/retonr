use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt as _;
use predicates::str::contains;
use std::{fs, path::PathBuf};
use tempfile::tempdir;

fn retonr() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("retonr"))
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/cli")
        .join(name)
}

#[test]
fn redirected_interactive_review_requires_explicit_linear_fallback() {
    retonr()
        .args(["tui", "missing.txt"])
        .assert()
        .code(2)
        .stderr(contains("\"command\": \"tui\"").and(contains("invalid_invocation")));
}

#[test]
fn linear_candidate_review_uses_existing_checks_without_mutating_inputs() {
    let root = tempdir().expect("input root");
    let source = root.path().join("source.txt");
    let candidate = root.path().join("candidate.txt");
    let original = fs::read(fixture("source.txt")).expect("source fixture");
    let supplied = fs::read(fixture("candidate.txt")).expect("candidate fixture");
    fs::write(&source, &original).expect("source");
    fs::write(&candidate, &supplied).expect("candidate");
    let result = retonr()
        .arg("tui")
        .arg(&source)
        .arg("--candidate")
        .arg(&candidate)
        .arg("--plain")
        .output()
        .expect("linear review");
    assert!(result.status.success());
    let value: serde_json::Value = serde_json::from_slice(&result.stdout).expect("safe JSON");
    assert_eq!(value["command"], "tui");
    assert!(
        value["result"]["summary"]
            .as_str()
            .expect("summary")
            .contains("Candidate check: rewritten;")
    );
    assert_eq!(fs::read(&source).expect("unchanged source"), original);
    assert_eq!(fs::read(&candidate).expect("unchanged candidate"), supplied);
    assert_eq!(fs::read_dir(root.path()).expect("inputs only").count(), 2);
}

#[test]
fn explicit_text_linear_review_is_safe_and_lists_current_findings() {
    let root = tempdir().expect("root");
    let source = root.path().join("draft.txt");
    fs::write(&source, "Moreover, let's delve into this.\u{1b}[2J\n").expect("source");
    let result = retonr()
        .args(["--format", "text", "tui"])
        .arg(&source)
        .arg("--plain")
        .output()
        .expect("linear text");
    assert!(result.status.success());
    let text = String::from_utf8(result.stdout).expect("text");
    assert!(text.contains("Retonr document review"));
    assert!(text.contains("Findings"));
    assert!(!text.contains('\u{1b}'));
}

#[test]
fn linear_review_rejects_stdin_and_directory_inputs_without_waiting() {
    retonr().args(["tui", "-", "--plain"]).assert().code(2);
    let root = tempdir().expect("root");
    retonr()
        .arg("tui")
        .arg(root.path())
        .arg("--plain")
        .assert()
        .failure();
}

#[test]
fn recursive_linear_review_selects_exact_nested_pair_and_preserves_files() {
    let source = tempdir().expect("source");
    let candidate = tempdir().expect("candidate");
    fs::create_dir(source.path().join("nested")).expect("nested source");
    fs::create_dir(candidate.path().join("nested")).expect("nested candidate");
    fs::write(source.path().join("a.txt"), b"First document.").expect("first");
    fs::write(
        source.path().join("nested/b.txt"),
        fs::read(fixture("source.txt")).expect("fixture"),
    )
    .expect("source");
    let supplied = fs::read(fixture("candidate.txt")).expect("candidate fixture");
    fs::write(candidate.path().join("nested/b.txt"), &supplied).expect("candidate");
    fs::write(candidate.path().join("unmatched.txt"), b"Extra draft.").expect("unmatched");
    let result = retonr()
        .arg("tui")
        .arg(source.path())
        .arg("--candidate")
        .arg(candidate.path())
        .args(["--recursive", "--document", "nested/b.txt", "--plain"])
        .output()
        .expect("folder review");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).expect("report");
    assert_eq!(report["result"]["directory"]["selected"], 1);
    assert_eq!(report["result"]["directory"]["total"], 2);
    assert_eq!(report["result"]["directory"]["unmatched_candidates"], 1);
    assert!(
        report["result"]["summary"]
            .as_str()
            .expect("summary")
            .contains("Candidate check: rewritten;")
    );
    assert_eq!(
        fs::read(candidate.path().join("nested/b.txt")).expect("unchanged"),
        supplied
    );
    assert_eq!(fs::read_dir(source.path()).expect("no outputs").count(), 2);
    retonr()
        .arg("tui")
        .arg(source.path())
        .args(["--recursive", "--document", "../escape.txt", "--plain"])
        .assert()
        .code(2);
}

#[test]
fn recursive_unpaired_source_does_not_claim_candidate_validation() {
    let source = tempdir().expect("source");
    let candidate = tempdir().expect("candidate");
    fs::write(source.path().join("draft.txt"), b"Moreover, hello.").expect("source");
    let result = retonr()
        .arg("tui")
        .arg(source.path())
        .arg("--candidate")
        .arg(candidate.path())
        .args(["--recursive", "--protect", "hello", "--plain"])
        .output()
        .expect("unpaired review");
    assert!(result.status.success());
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).expect("report");
    assert!(report["result"]["candidate_preview"].is_null());
    assert!(
        !report["result"]["summary"]
            .as_str()
            .expect("summary")
            .contains("Candidate check:")
    );
    assert!(
        report["result"]["findings"]
            .as_array()
            .expect("rows")
            .iter()
            .any(|row| row
                .as_str()
                .is_some_and(|text| text.contains("protected terms were not checked")))
    );
}

#[cfg(unix)]
#[test]
fn real_terminal_folder_navigation_reload_quit_and_interrupt_restore_modes() {
    let output = std::process::Command::new("python3")
        .arg(fixture("folder-review-pty.py"))
        .arg(assert_cmd::cargo::cargo_bin!("retonr"))
        .output()
        .expect("Python terminal fixture");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
