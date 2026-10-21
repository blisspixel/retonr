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
fn forward_corpora_only_overlap_the_shipped_phrase() {
    // These corpora annotate families the catalog does not implement yet.
    // A clean control must stay empty. A finding case may also match a shipped
    // family when the annotated phrase is already that family's evidence.
    let expected = [
        (
            "editorial_prose_v1.json",
            "finding-unsourced-quantified-claim",
            "vague_attribution",
            "Studies show",
        ),
        (
            "editorial_model_impressions_v1.json",
            "finding-sycophantic-affirmation",
            "conversational_residue",
            "Great question",
        ),
        (
            "editorial_model_impressions_v1.json",
            "finding-careful-hedge-open",
            "throat_clearing",
            "It is worth noting that",
        ),
        (
            "editorial_assistant_residue_v1.json",
            "finding-assistant-role-disclaimer",
            "conversational_residue",
            "As an AI language model",
        ),
    ];
    let mut actual = Vec::new();
    for name in [
        "editorial_prose_v1.json",
        "editorial_model_impressions_v1.json",
        "editorial_assistant_residue_v1.json",
    ] {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join(name);
        let content = std::fs::read_to_string(&path).expect("read corpus");
        let corpus = parse_editorial_corpus(&content).expect("parse corpus");
        for case in &corpus.cases {
            for finding in lint_text(&case.source) {
                actual.push((
                    name.to_owned(),
                    case.id.clone(),
                    finding.rule_id.clone(),
                    finding.evidence.clone(),
                ));
            }
        }
    }
    actual.sort_unstable();
    let mut expected_rows = expected
        .into_iter()
        .map(|(corpus, id, rule, evidence)| {
            (
                corpus.to_owned(),
                id.to_owned(),
                rule.to_owned(),
                evidence.to_owned(),
            )
        })
        .collect::<Vec<_>>();
    expected_rows.sort_unstable();
    assert_eq!(actual, expected_rows);
}

#[test]
fn clean_text_produces_no_findings() {
    let text = "The compiler verifies memory safety and reports typed errors.";
    let findings = lint_text(text);
    assert!(findings.is_empty());
}
