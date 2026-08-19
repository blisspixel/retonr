use std::fmt::Write;

use rewrite_types::RewriteStatus;

use super::{
    CLAIM_SHADOW_CALIBRATION_SCHEMA_VERSION, ClaimShadowCalibrationError, ExpectedShadowOutcome,
    parse_claim_shadow_calibration, run_claim_shadow_calibration,
};

const CORPUS: &str = include_str!("../../fixtures/claim_shadow_calibration_v1.json");

#[test]
fn checked_in_corpus_matches_and_never_changes_acceptance() {
    let corpus = parse_claim_shadow_calibration(CORPUS).expect("checked-in corpus is valid");
    let report = run_claim_shadow_calibration(&corpus);
    assert!(report.is_success(), "failures: {:?}", report.failures);
    assert_eq!(
        report.schema_version,
        CLAIM_SHADOW_CALIBRATION_SCHEMA_VERSION
    );
    assert_eq!(report.corpus_id, "claim-shadow-calibration-v1");
    assert_eq!(report.total, 12);
    assert_eq!(report.passed, 12);
    assert_eq!(report.authority_violations, 0);
    assert!(report.failures.is_empty());
    assert_eq!(
        corpus
            .cases
            .iter()
            .filter(|case| case.expected_shadow == ExpectedShadowOutcome::Absent)
            .count(),
        4
    );
    assert!(corpus.cases.iter().any(|case| {
        case.expected_status == RewriteStatus::Rewritten
            && case.expected_shadow == ExpectedShadowOutcome::Conflict
    }));
    assert!(corpus.cases.iter().any(|case| {
        case.expected_status == RewriteStatus::Abstained
            && case.expected_shadow == ExpectedShadowOutcome::Conflict
    }));
}

#[test]
fn report_omits_fixture_text() {
    let corpus = parse_claim_shadow_calibration(CORPUS).expect("checked-in corpus is valid");
    let report = run_claim_shadow_calibration(&corpus);
    let serialized = serde_json::to_string(&report).expect("report serializes");
    assert!(!serialized.contains("Hello world"));
    assert!(!serialized.contains("Version 2"));
    assert!(!serialized.contains("available"));
}

#[test]
fn rejects_untrusted_schema_and_case_contracts() {
    let unsupported = r#"{
        "schema_version": 2,
        "corpus_id": "fixture",
        "cases": [{
            "id": "punctuation-absent",
            "source": "Hello world",
            "candidate": "Hello, world!",
            "expected_status": "rewritten",
            "expected_reason": null,
            "expected_shadow": "absent"
        }]
    }"#;
    assert!(matches!(
        parse_claim_shadow_calibration(unsupported),
        Err(ClaimShadowCalibrationError::UnsupportedSchema(2))
    ));

    let empty = r#"{"schema_version":1,"corpus_id":"fixture","cases":[]}"#;
    assert!(matches!(
        parse_claim_shadow_calibration(empty),
        Err(ClaimShadowCalibrationError::InvalidCorpus)
    ));

    let failed = CORPUS.replacen("\"rewritten\"", "\"failed\"", 1);
    assert!(matches!(
        parse_claim_shadow_calibration(&failed),
        Err(ClaimShadowCalibrationError::InvalidCase { index: 0 })
    ));

    let same_conflict = CORPUS.replacen("\"farewell\"", "\"greeting\"", 1);
    assert!(matches!(
        parse_claim_shadow_calibration(&same_conflict),
        Err(ClaimShadowCalibrationError::InvalidCase { index: 2 })
    ));

    let control = CORPUS.replacen("Hello world", "Hello\\u0000world", 1);
    assert!(matches!(
        parse_claim_shadow_calibration(&control),
        Err(ClaimShadowCalibrationError::InvalidCase { index: 0 })
    ));
}

#[test]
fn rejects_duplicate_ids_and_unknown_fields() {
    let duplicate = CORPUS.replacen("\"punctuation-preserved\"", "\"punctuation-absent\"", 1);
    assert!(matches!(
        parse_claim_shadow_calibration(&duplicate),
        Err(ClaimShadowCalibrationError::DuplicateCaseId { index: 1 })
    ));

    let unknown = CORPUS.replacen(
        "\"corpus_id\": \"claim-shadow-calibration-v1\"",
        "\"corpus_id\": \"claim-shadow-calibration-v1\", \"notes\": \"secret\"",
        1,
    );
    assert!(matches!(
        parse_claim_shadow_calibration(&unknown),
        Err(ClaimShadowCalibrationError::InvalidJson)
    ));
    assert!(matches!(
        parse_claim_shadow_calibration("{"),
        Err(ClaimShadowCalibrationError::InvalidJson)
    ));
}

#[test]
fn rejects_oversized_and_overlong_corpora() {
    assert!(matches!(
        parse_claim_shadow_calibration(&"x".repeat(super::MAX_CLAIM_SHADOW_CALIBRATION_BYTES + 1)),
        Err(ClaimShadowCalibrationError::TooLarge)
    ));

    let mut cases = String::new();
    for index in 0..=super::MAX_CLAIM_SHADOW_CALIBRATION_CASES {
        if index > 0 {
            cases.push(',');
        }
        let _ = write!(
            cases,
            "{{\"id\":\"case-{index}\",\"source\":\"Hello world\",\
              \"candidate\":\"Hello, world!\",\"expected_status\":\"rewritten\",\
              \"expected_reason\":null,\"expected_shadow\":\"absent\"}}"
        );
    }
    let input = format!("{{\"schema_version\":1,\"corpus_id\":\"too-many\",\"cases\":[{cases}]}}");
    assert!(matches!(
        parse_claim_shadow_calibration(&input),
        Err(ClaimShadowCalibrationError::TooManyCases)
    ));
}

#[test]
fn mismatch_fails_without_changing_acceptance_or_leaking_text() {
    let input = r#"{
        "schema_version": 1,
        "corpus_id": "mismatch-fixture",
        "cases": [{
            "id": "expected-mismatch",
            "source": "private source",
            "candidate": "private source.",
            "expected_status": "rewritten",
            "expected_reason": null,
            "expected_shadow": "conflict",
            "source_predicate": "same-claim",
            "candidate_predicate": "other-claim"
        }]
    }"#;
    let mut corpus = parse_claim_shadow_calibration(input).expect("valid mismatch fixture");
    corpus.cases[0].expected_shadow = ExpectedShadowOutcome::Preserved;
    let report = run_claim_shadow_calibration(&corpus);
    assert!(!report.is_success());
    assert_eq!(report.passed, 0);
    assert_eq!(report.authority_violations, 0);
    assert_eq!(report.failures[0].id, "expected-mismatch");
    assert_eq!(
        report.failures[0].actual_shadow,
        ExpectedShadowOutcome::Conflict
    );
    assert_eq!(
        report.failures[0].expected_shadow,
        ExpectedShadowOutcome::Preserved
    );
    assert!(!report.failures[0].authority_violation);
    let serialized = serde_json::to_string(&report).expect("report serializes");
    assert!(!serialized.contains("private source"));
}
