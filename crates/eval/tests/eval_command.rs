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
        .stdout(predicate::str::contains("\"passed\": 20"))
        .stdout(predicate::str::contains("\"acceptable\": 8"))
        .stdout(predicate::str::contains("\"rewritten\": 3"))
        .stdout(predicate::str::contains("\"failures\": []"));
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
