use std::{fs, path::PathBuf};

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn checked_in_suite_passes_as_a_process() {
    let suite = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/core.json");
    Command::cargo_bin("rewrite-eval")
        .expect("compiled evaluation runner")
        .arg(suite)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"passed\": 35"))
        .stdout(predicate::str::contains("\"acceptable\": 9"))
        .stdout(predicate::str::contains("\"rewritten\": 4"))
        .stdout(predicate::str::contains("\"failures\": []"));
}

#[test]
fn checked_in_editorial_corpus_validates_as_a_process() {
    let corpus =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/editorial_quality_v1.json");
    Command::cargo_bin("rewrite-eval")
        .expect("compiled evaluation runner")
        .args(["--editorial-corpus"])
        .arg(corpus)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total\": 15"))
        .stdout(predicate::str::contains("\"finding_cases\": 10"))
        .stdout(predicate::str::contains("\"clean_controls\": 5"))
        .stdout(predicate::str::contains("Certainly").not());
}

#[test]
fn checked_in_slop_corpus_validates_as_a_process() {
    let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/editorial_slop_v1.json");
    Command::cargo_bin("rewrite-eval")
        .expect("compiled evaluation runner")
        .args(["--editorial-corpus"])
        .arg(corpus)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total\": 24"))
        .stdout(predicate::str::contains("\"finding_cases\": 12"))
        .stdout(predicate::str::contains("\"clean_controls\": 12"))
        .stdout(predicate::str::contains("rapidly evolving").not());
}

#[test]
fn mismatch_fails_without_printing_fixture_content() {
    let directory = tempdir().expect("temporary directory");
    let suite = directory.path().join("suite.json");
    fs::write(
        &suite,
        r#"{
            "schema_version": 2,
            "cases": [{
                "id": "expected-mismatch",
                "category": "fixture",
                "source": "private source",
                "candidate": "private source.",
                "reference_judgment": "acceptable",
                "expected_status": "abstained",
                "expected_reason": null,
                "expected_output": "source"
            }]
        }"#,
    )
    .expect("write suite fixture");

    Command::cargo_bin("rewrite-eval")
        .expect("compiled evaluation runner")
        .arg(suite)
        .assert()
        .failure()
        .stdout(predicate::str::contains("expected-mismatch"))
        .stdout(predicate::str::contains("private source").not());
}
