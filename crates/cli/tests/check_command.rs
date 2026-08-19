use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn accepts_utf8_bom_on_both_source_and_candidate_files() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.txt");
    let candidate = directory.path().join("candidate.txt");
    fs::write(&source, b"\xEF\xBB\xBFHello world\n").expect("write source with BOM");
    fs::write(&candidate, b"\xEF\xBB\xBFHello, world!\n").expect("write candidate with BOM");

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["check"])
        .arg(source)
        .arg(candidate)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"rewritten\""));
}

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
fn short_format_and_output_flags_match_the_long_forms() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.txt");
    let candidate = directory.path().join("candidate.txt");
    let output = directory.path().join("accepted.txt");
    fs::write(&source, "Hello world\n").expect("write source fixture");
    fs::write(&candidate, "Hello, world!\n").expect("write candidate fixture");

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["check", "-f", "text", "-o"])
        .arg(&output)
        .arg(&source)
        .arg(&candidate)
        .assert()
        .success()
        .stdout(predicate::str::contains("status: rewritten"));
    assert_eq!(
        fs::read(&output).expect("accepted bytes"),
        b"Hello, world!\n"
    );
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

#[test]
fn directory_source_is_rejected_as_unreadable() {
    let directory = tempdir().expect("temporary directory");
    let candidate = directory.path().join("candidate.txt");
    fs::write(&candidate, "Hello\n").expect("write candidate fixture");

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["check"])
        .arg(directory.path())
        .arg(candidate)
        .assert()
        .code(2)
        .stderr(predicate::str::contains("\"category\": \"usage\""))
        .stderr(predicate::str::contains("\"code\": \"input_unreadable\""));
}

#[test]
fn missing_source_is_an_unreadable_input_not_a_retryable_failure() {
    let directory = tempdir().expect("temporary directory");
    let candidate = directory.path().join("candidate.txt");
    fs::write(&candidate, "Hello\n").expect("write candidate fixture");

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["check"])
        .arg(directory.path().join("missing.txt"))
        .arg(candidate)
        .assert()
        .code(1)
        .stderr(predicate::str::contains("\"category\": \"operational\""))
        .stderr(predicate::str::contains("\"code\": \"input_unreadable\""))
        .stderr(predicate::str::contains("\"retryable\": false"));
}

#[test]
fn oversized_candidate_is_a_resource_limit() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.txt");
    let candidate = directory.path().join("candidate.txt");
    fs::write(&source, "short\n").expect("write source fixture");
    fs::write(&candidate, "a".repeat(16 * 1024 * 1024 + 1)).expect("write oversized candidate");

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["check"])
        .arg(source)
        .arg(candidate)
        .assert()
        .code(4)
        .stderr(predicate::str::contains("\"category\": \"compatibility\""))
        .stderr(predicate::str::contains(
            "\"code\": \"resource_limit_exceeded\"",
        ))
        .stderr(predicate::str::contains("\"retryable\": false"));
}

#[test]
fn invalid_utf8_source_is_usage() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.txt");
    let candidate = directory.path().join("candidate.txt");
    fs::write(&source, b"a\xFF").expect("write invalid source");
    fs::write(&candidate, "a\n").expect("write candidate fixture");

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["check"])
        .arg(source)
        .arg(candidate)
        .assert()
        .code(2)
        .stderr(predicate::str::contains("\"category\": \"usage\""))
        .stderr(predicate::str::contains("\"code\": \"input_unreadable\""));
}

/// The checked-in fixtures back the documented reproduction and the retained
/// screenshots, so their bytes are a contract. Line-ending normalization on
/// checkout silently changed these digests once; `.gitattributes` now pins
/// `fixtures/**` with `-text` and this test fails if that protection regresses.
#[test]
fn checked_in_cli_fixtures_keep_their_documented_bytes() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("repository root")
        .to_path_buf();

    let cases: [(&str, &[u8]); 2] = [
        ("fixtures/cli/source.txt", b"Hello world\n"),
        ("fixtures/cli/candidate.txt", b"Hello, world!\n"),
    ];

    for (relative, expected) in cases {
        let actual = fs::read(root.join(relative)).expect("read checked-in fixture");
        assert_eq!(
            actual, expected,
            "{relative} lost its exact committed bytes; check .gitattributes"
        );
    }
}

#[test]
fn diff_dry_run_and_trace_are_safe_and_non_replacing() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.txt");
    let candidate = directory.path().join("candidate.txt");
    let output = directory.path().join("accepted.txt");
    let trace = directory.path().join("trace.json");
    fs::write(&source, "Hello world\n").expect("write source fixture");
    fs::write(&candidate, "Hello, world!\n").expect("write candidate fixture");

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["check"])
        .arg(&source)
        .arg(&candidate)
        .args(["--diff", "--dry-run", "--format", "text"])
        .arg("--output")
        .arg(&output)
        .arg("--trace")
        .arg(&trace)
        .assert()
        .success()
        .stdout(predicate::str::contains("status: rewritten"))
        .stderr(predicate::str::contains("diff: changed"))
        .stderr(predicate::str::contains("replace"))
        .stderr(predicate::str::contains("Hello, world!"));

    assert!(!output.exists(), "dry-run must not create --output");
    let trace_text = fs::read_to_string(&trace).expect("read trace");
    assert!(trace_text.contains("\"command\": \"check\""));
    assert!(trace_text.contains("\"status\": \"rewritten\""));
    assert!(!trace_text.contains("Hello world"));

    fs::write(&output, b"keep existing").expect("write existing destination");
    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["check"])
        .arg(&source)
        .arg(&candidate)
        .args(["--dry-run", "--output"])
        .arg(&output)
        .assert()
        .code(3)
        .stderr(predicate::str::contains("\"code\": \"output_exists\""));
    assert_eq!(
        fs::read(&output).expect("read destination"),
        b"keep existing"
    );
}

#[test]
fn diff_neutralizes_controls_and_trace_refuses_to_replace() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.txt");
    let candidate = directory.path().join("candidate.txt");
    let trace = directory.path().join("trace.json");
    fs::write(&source, "safe line\n").expect("write source fixture");
    fs::write(&candidate, "safe line.\n").expect("write candidate fixture");
    fs::write(&trace, b"existing").expect("write existing trace");

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["check"])
        .arg(&source)
        .arg(&candidate)
        .arg("--diff")
        .assert()
        .success()
        .stderr(predicate::str::contains("safe line."));

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["check"])
        .arg(source)
        .arg(candidate)
        .arg("--trace")
        .arg(&trace)
        .assert()
        .code(3)
        .stderr(predicate::str::contains("\"code\": \"output_exists\""));
    assert_eq!(fs::read(trace).expect("read existing trace"), b"existing");
}
