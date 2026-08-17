use super::{
    EDITORIAL_CORPUS_SCHEMA_VERSION, EditorialCaseKind, EditorialCorpusError,
    parse_editorial_corpus,
};

const EDITORIAL_CORPUS: &str = include_str!("../../fixtures/editorial_quality_v1.json");
const EDITORIAL_SLOP_CORPUS: &str = include_str!("../../fixtures/editorial_slop_v1.json");

#[test]
fn checked_in_editorial_corpus_is_valid_and_balanced() {
    let corpus = parse_editorial_corpus(EDITORIAL_CORPUS).expect("checked-in corpus is valid");
    let summary = corpus.summary();
    assert_eq!(summary.schema_version, EDITORIAL_CORPUS_SCHEMA_VERSION);
    assert_eq!(summary.total, 20);
    assert_eq!(summary.finding_cases, 10);
    assert_eq!(summary.clean_controls, 10);
    assert_eq!(summary.targeted_rules, 9);
    assert!(corpus.cases.iter().any(|case| {
        case.kind == EditorialCaseKind::CleanControl && case.expected_source_findings.is_empty()
    }));
    assert_every_rule_is_paired(&corpus);
}

/// Every targeted rule needs a neighboring clean counterexample, so the rule set
/// covered by findings and the rule set covered by clean controls must match.
fn assert_every_rule_is_paired(corpus: &super::EditorialCorpus) {
    let rules = |kind| {
        corpus
            .cases
            .iter()
            .filter(|case| case.kind == kind)
            .flat_map(|case| case.target_rules.iter())
            .collect::<std::collections::BTreeSet<_>>()
    };
    assert_eq!(
        rules(EditorialCaseKind::Finding),
        rules(EditorialCaseKind::CleanControl),
        "each targeted rule requires a paired clean control"
    );
}

#[test]
fn checked_in_slop_corpus_is_valid_and_balanced() {
    let corpus = parse_editorial_corpus(EDITORIAL_SLOP_CORPUS).expect("checked-in corpus is valid");
    let summary = corpus.summary();
    assert_eq!(summary.schema_version, EDITORIAL_CORPUS_SCHEMA_VERSION);
    assert_eq!(summary.total, 24);
    assert_eq!(summary.finding_cases, 12);
    assert_eq!(summary.clean_controls, 12);
    assert_eq!(summary.targeted_rules, 12);

    assert_every_rule_is_paired(&corpus);
}

#[test]
fn rejects_duplicate_ids_and_unknown_fields() {
    let duplicate = EDITORIAL_CORPUS.replacen(
        "\"clean-quoted-throat-clearing\"",
        "\"finding-conversational-opening\"",
        1,
    );
    assert!(matches!(
        parse_editorial_corpus(&duplicate),
        Err(EditorialCorpusError::DuplicateCaseId { index: 10 })
    ));

    let unknown = EDITORIAL_CORPUS.replacen(
        "\"rule_catalog_version\": 1,",
        "\"rule_catalog_version\": 1, \"authorship_label\": \"ai\",",
        1,
    );
    assert!(matches!(
        parse_editorial_corpus(&unknown),
        Err(EditorialCorpusError::InvalidJson(_))
    ));
}

#[test]
fn rejects_missing_evidence_and_unbalanced_finding_cases() {
    let missing = EDITORIAL_CORPUS.replacen(
        "\"evidence\": \"Certainly!\"",
        "\"evidence\": \"missing text\"",
        1,
    );
    assert!(matches!(
        parse_editorial_corpus(&missing),
        Err(EditorialCorpusError::InvalidCase { index: 0 })
    ));

    let no_finding = EDITORIAL_CORPUS.replacen(
        r#"{"rule_id": "conversational_residue", "evidence": "Certainly!", "occurrence": 0}"#,
        "",
        1,
    );
    assert!(matches!(
        parse_editorial_corpus(&no_finding),
        Err(EditorialCorpusError::InvalidCase { index: 0 })
    ));
}

#[test]
fn rejects_non_synthetic_origin_and_implicit_reference_findings() {
    let non_synthetic = EDITORIAL_CORPUS.replacen(
        "\"content_origin\": \"synthetic\"",
        "\"content_origin\": \"scraped_public\"",
        1,
    );
    assert!(matches!(
        parse_editorial_corpus(&non_synthetic),
        Err(EditorialCorpusError::InvalidJson(_))
    ));

    let implicit_reference = EDITORIAL_CORPUS.replacen(
        "\"expected_reference_findings\": []",
        r#""expected_reference_findings": [
          {"rule_id": "conversational_residue", "evidence": "Certainly!", "occurrence": 0}
        ]"#,
        1,
    );
    assert!(matches!(
        parse_editorial_corpus(&implicit_reference),
        Err(EditorialCorpusError::InvalidCase { index: 0 })
    ));
}
