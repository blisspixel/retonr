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
