use std::path::PathBuf;

use super::{evaluate_corpus_against_linter, lint_text};
use crate::editorial_corpus::parse_editorial_corpus;

#[test]
fn linter_passes_all_cases_in_editorial_quality_corpus() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/editorial_quality_v1.json");
    let content = std::fs::read_to_string(&path).expect("read editorial quality fixture");
    let corpus = parse_editorial_corpus(&content).expect("parse editorial quality corpus");
    let report = evaluate_corpus_against_linter(&corpus);
    assert_eq!(report.total_cases, 20);
    assert_eq!(report.passed_cases, 20);
    assert_eq!(report.failed_cases, 0);
    assert!(report.failures.is_empty());
}

#[test]
fn linter_passes_all_cases_in_editorial_slop_corpus() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/editorial_slop_v1.json");
    let content = std::fs::read_to_string(&path).expect("read editorial slop fixture");
    let corpus = parse_editorial_corpus(&content).expect("parse editorial slop corpus");
    let report = evaluate_corpus_against_linter(&corpus);

    assert_eq!(report.total_cases, 24);
    assert_eq!(report.passed_cases, 24);
    assert_eq!(report.failed_cases, 0);
    assert!(report.failures.is_empty());
}

#[test]
fn clean_text_produces_no_findings() {
    let text = "The compiler verifies memory safety and reports typed errors.";
    let findings = lint_text(text);
    assert!(findings.is_empty());
}
