use std::{fs, path::PathBuf};

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn checked_in_no_rewrite_baseline_runs_offline() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    Command::cargo_bin("rewrite-eval")
        .expect("compiled evaluation runner")
        .args([
            "--baseline",
            root.join("fixtures/no_rewrite_baseline_v1.json")
                .to_str()
                .expect("utf-8 fixture path"),
        ])
        .arg(root.join("fixtures/core.json"))
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "\"baseline_id\": \"no-rewrite-v1\"",
        ))
        .stdout(predicate::str::contains("\"kind\": \"no_rewrite\""))
        .stdout(predicate::str::contains("\"unchanged\": 49"))
        .stdout(predicate::str::contains("\"failed\": 0"))
        .stdout(predicate::str::contains("Hello world").not());
}

#[test]
fn incomplete_generative_baseline_fails_closed() {
    let directory = tempdir().expect("temporary directory");
    let definition = directory.path().join("direct.json");
    fs::write(
        &definition,
        r#"{
            "schema_version": 1,
            "id": "direct-prompt-v1",
            "kind": "direct_prompt"
        }"#,
    )
    .expect("write baseline definition");
    let suite = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/core.json");
    Command::cargo_bin("rewrite-eval")
        .expect("compiled evaluation runner")
        .arg("--baseline")
        .arg(&definition)
        .arg(suite)
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid baseline configuration"))
        .stderr(predicate::str::contains("Hello world").not());
}

#[test]
fn checked_in_suite_passes_as_a_process() {
    let suite = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/core.json");
    Command::cargo_bin("rewrite-eval")
        .expect("compiled evaluation runner")
        .arg(suite)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"passed\": 49"))
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
        .stdout(predicate::str::contains("\"total\": 20"))
        .stdout(predicate::str::contains("\"finding_cases\": 10"))
        .stdout(predicate::str::contains("\"clean_controls\": 10"))
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
fn checked_in_prose_corpus_validates_as_a_process() {
    let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/editorial_prose_v1.json");
    Command::cargo_bin("rewrite-eval")
        .expect("compiled evaluation runner")
        .args(["--editorial-corpus"])
        .arg(corpus)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total\": 40"))
        .stdout(predicate::str::contains("\"finding_cases\": 20"))
        .stdout(predicate::str::contains("\"clean_controls\": 20"))
        .stdout(predicate::str::contains("\"targeted_rules\": 20"))
        .stdout(predicate::str::contains("Everyone knows").not());
}

#[test]
fn checked_in_model_impression_corpus_validates_as_a_process() {
    let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/editorial_model_impressions_v1.json");
    Command::cargo_bin("rewrite-eval")
        .expect("compiled evaluation runner")
        .args(["--editorial-corpus"])
        .arg(corpus)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total\": 16"))
        .stdout(predicate::str::contains("\"finding_cases\": 8"))
        .stdout(predicate::str::contains("\"clean_controls\": 8"))
        .stdout(predicate::str::contains("Great question").not());
}

#[test]
fn checked_in_assistant_residue_corpus_validates_as_a_process() {
    let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/editorial_assistant_residue_v1.json");
    Command::cargo_bin("rewrite-eval")
        .expect("compiled evaluation runner")
        .args(["--editorial-corpus"])
        .arg(corpus)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total\": 20"))
        .stdout(predicate::str::contains("\"finding_cases\": 10"))
        .stdout(predicate::str::contains("\"clean_controls\": 10"))
        .stdout(predicate::str::contains("knowledge update").not());
}

#[test]
fn writing_sample_libraries_validate_without_printing_excerpts() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/writing_samples");
    Command::cargo_bin("rewrite-eval")
        .expect("compiled evaluation runner")
        .args(["--writing-samples"])
        .arg(root.join("licensed_pre_ai_human_v1.json"))
        .assert()
        .success()
        .stdout(predicate::str::contains("\"human_controls\": 8"))
        .stdout(predicate::str::contains("datagrams").not());
    Command::cargo_bin("rewrite-eval")
        .expect("compiled evaluation runner")
        .args(["--writing-samples"])
        .arg(root.join("synthetic_model_impressions_v1.json"))
        .assert()
        .success()
        .stdout(predicate::str::contains("\"synthetic_impressions\": 7"))
        .stdout(predicate::str::contains("Certainly").not());
}

#[test]
fn checked_in_claim_shadow_calibration_passes_as_a_process() {
    let corpus =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/claim_shadow_calibration_v1.json");
    Command::cargo_bin("rewrite-eval")
        .expect("compiled evaluation runner")
        .args(["--claim-shadow-calibration"])
        .arg(corpus)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"passed\": 12"))
        .stdout(predicate::str::contains("\"authority_violations\": 0"))
        .stdout(predicate::str::contains("\"failures\": []"))
        .stdout(predicate::str::contains("Hello world").not())
        .stdout(predicate::str::contains("available").not());
}

#[test]
fn claim_shadow_calibration_mismatch_fails_without_fixture_text() {
    let directory = tempdir().expect("temporary directory");
    let corpus = directory.path().join("calibration.json");
    fs::write(
        &corpus,
        r#"{
            "schema_version": 1,
            "corpus_id": "process-mismatch",
            "cases": [{
                "id": "expected-mismatch",
                "source": "private source",
                "candidate": "private source.",
                "expected_status": "abstained",
                "expected_reason": "semantic_uncertain",
                "expected_shadow": "absent"
            }]
        }"#,
    )
    .expect("write calibration fixture");

    Command::cargo_bin("rewrite-eval")
        .expect("compiled evaluation runner")
        .args(["--claim-shadow-calibration"])
        .arg(corpus)
        .assert()
        .failure()
        .stdout(predicate::str::contains("expected-mismatch"))
        .stdout(predicate::str::contains("\"authority_violations\": 0"))
        .stdout(predicate::str::contains("private source").not());
}

#[test]
fn watermark_research_corpus_validates_without_mark_labels() {
    let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/watermark_research/style_is_not_a_watermark_v1.json");
    Command::cargo_bin("rewrite-eval")
        .expect("compiled evaluation runner")
        .args(["--watermark-research"])
        .arg(corpus)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"refused_style_as_mark\": 4"))
        .stdout(predicate::str::contains("\"unmarked_controls\": 6"))
        .stdout(predicate::str::contains("delves").not());
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
