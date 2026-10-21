use rewrite_types::RewriteStatus;

use super::{
    EVALUATION_SCHEMA_VERSION, EvaluationError, EvaluationSuite, parse_suite, run_suite,
    run_suite_with_concurrency,
};

const CORE_SUITE: &str = include_str!("../../fixtures/core.json");

#[test]
fn core_regression_suite_has_zero_failures() {
    let suite = parse_suite(CORE_SUITE).expect("checked-in suite is valid");
    let report = run_suite(&suite);
    assert!(report.is_success(), "failures: {:?}", report.failures);
    assert_eq!(report.total, 74);
    assert_eq!(report.passed, report.total);
    assert_eq!(report.transformation_coverage.acceptable, 12);
    assert_eq!(report.transformation_coverage.rewritten, 7);
    assert_eq!(
        report
            .categories
            .iter()
            .find(|category| category.category == "unsafe_text_negative")
            .map(|category| category.total),
        Some(21),
        "invisible and directional coverage must not regress"
    );
    assert!(
        report
            .categories
            .iter()
            .any(|category| category.category == "semantic_hard_negative")
    );
}

#[test]
fn concurrent_evaluation_produces_identical_reports() {
    let suite = parse_suite(CORE_SUITE).expect("checked-in suite is valid");
    let serial_report = run_suite(&suite);

    for worker_cap in [1, 2, 4, 8, 16] {
        let concurrent_report = run_suite_with_concurrency(&suite, worker_cap);
        assert_eq!(
            concurrent_report, serial_report,
            "worker cap {worker_cap} differed from serial report"
        );
    }

    // Zero worker cap clamps safely to 1
    let clamped_report = run_suite_with_concurrency(&suite, 0);
    assert_eq!(clamped_report, serial_report);
}

#[test]
fn empty_suite_produces_empty_report() {
    let empty_suite = EvaluationSuite {
        schema_version: EVALUATION_SCHEMA_VERSION,
        cases: Vec::new(),
    };
    let report = run_suite_with_concurrency(&empty_suite, 4);
    assert_eq!(report.total, 0);
    assert_eq!(report.passed, 0);
    assert!(report.categories.is_empty());
    assert_eq!(report.transformation_coverage.acceptable, 0);
    assert_eq!(report.transformation_coverage.rewritten, 0);
    assert!(report.failures.is_empty());
}

#[test]
fn abstentions_locate_failed_spans_without_echoing_them() {
    use rewrite_app::{CandidateCheckRequest, CandidateCheckService};
    use rewrite_types::ReasonCode;

    let suite = parse_suite(CORE_SUITE).expect("checked-in suite is valid");
    for case in &suite.cases {
        let Some(reason) = case.expected_reason else {
            continue;
        };
        if !matches!(
            reason,
            ReasonCode::ProtectedValueChanged
                | ReasonCode::StructureChanged
                | ReasonCode::UnsafeText
                | ReasonCode::SemanticUncertain
        ) {
            continue;
        }
        let result = CandidateCheckService::check(CandidateCheckRequest::new(
            case.source.as_bytes().to_vec(),
            case.candidate.clone(),
            case.protected_terms.clone(),
        ))
        .expect("check");
        let failures = span_failures(&result.record);
        assert!(!failures.is_empty(), "{}", case.id);
        let encoded = serde_json::to_string(&result.record).expect("record json");
        assert_locations_in_range(&case.source, &case.candidate, &failures, &case.id);
        assert!(
            !encoded.contains(&case.source) || case.source.chars().count() < 12,
            "{} record echoed source",
            case.id
        );
    }

    let marker = "zzMARKER9f3k";
    let source = format!("Visit https://example.com/{marker} today.");
    let candidate = format!("Visit https://example.org/{marker} today.");
    let result = CandidateCheckService::check(CandidateCheckRequest::new(
        source.as_bytes().to_vec(),
        candidate,
        Vec::new(),
    ))
    .expect("url check");
    let encoded = serde_json::to_string(&result.record).expect("url record");
    assert!(!encoded.contains(marker), "{encoded}");
    let url = span_failures(&result.record)
        .into_iter()
        .find(|failure| failure.kind == "url")
        .expect("url failure");
    let point = url.source.expect("url source point");
    assert_eq!(
        byte_index(&source, point.line, point.column),
        source.find("https")
    );

    let source = format!("Keep {marker} stable.");
    let result = CandidateCheckService::check(CandidateCheckRequest::new(
        source.as_bytes().to_vec(),
        "Keep other stable.".to_owned(),
        vec![marker.to_owned()],
    ))
    .expect("term check");
    let encoded = serde_json::to_string(&result.record).expect("term record");
    assert!(!encoded.contains(marker), "{encoded}");
    let term = span_failures(&result.record)
        .into_iter()
        .find(|failure| failure.kind == "declared_term")
        .expect("declared term failure");
    assert_eq!(term.ordinal, Some(1));
    let point = term.source.expect("term source point");
    assert_eq!(
        byte_index(&source, point.line, point.column),
        source.find(marker)
    );
}

fn span_failures(record: &rewrite_types::RewriteRecord) -> Vec<rewrite_types::SpanFailure> {
    let mut failures = Vec::new();
    for assessment in &record.assessments {
        for gate in &assessment.gates {
            for evidence in &gate.evidence {
                if let Some(rewrite_types::GateEvidenceDetails::SpanFailures { failures: found }) =
                    &evidence.details
                {
                    failures.extend(found.iter().cloned());
                }
            }
        }
    }
    failures
}

fn assert_locations_in_range(
    source: &str,
    candidate: &str,
    failures: &[rewrite_types::SpanFailure],
    id: &str,
) {
    for failure in failures {
        if let Some(point) = failure.source {
            let index = byte_index(source, point.line, point.column)
                .unwrap_or_else(|| panic!("{id} source location"));
            assert!(source.is_char_boundary(index), "{id}");
        }
        if let Some(point) = failure.candidate {
            let index = byte_index(candidate, point.line, point.column)
                .unwrap_or_else(|| panic!("{id} candidate location"));
            assert!(candidate.is_char_boundary(index), "{id}");
        }
    }
}

fn byte_index(text: &str, line: u32, column: u32) -> Option<usize> {
    let mut current = 1u32;
    let mut start = 0usize;
    for (index, byte) in text.bytes().enumerate() {
        if current == line && u32::try_from(index - start + 1).ok()? == column {
            return Some(index);
        }
        if byte == b'\n' {
            current = current.checked_add(1)?;
            start = index + 1;
        }
    }
    if current == line && u32::try_from(text.len() - start + 1).ok()? == column {
        Some(text.len())
    } else {
        None
    }
}

#[test]
fn rejects_unknown_suite_schema() {
    let input = format!(
        "{{\"schema_version\":{},\"cases\":[]}}",
        EVALUATION_SCHEMA_VERSION + 1
    );
    assert!(matches!(
        parse_suite(&input),
        Err(EvaluationError::UnsupportedSchema(_))
    ));
}

#[test]
fn rejects_untrusted_or_duplicate_report_labels() {
    let invalid = r#"{
        "schema_version": 2,
        "cases": [{
            "id": "raw content",
            "category": "fixture",
            "source": "source",
            "candidate": "source",
            "reference_judgment": "identity",
            "expected_status": "unchanged_no_eligible_content",
            "expected_reason": null,
            "expected_output": "source"
        }]
    }"#;
    assert!(matches!(
        parse_suite(invalid),
        Err(EvaluationError::InvalidCaseId { index: 0 })
    ));

    let duplicate = CORE_SUITE.replacen("\"literal-crlf\"", "\"literal-punctuation\"", 1);
    assert!(matches!(
        parse_suite(&duplicate),
        Err(EvaluationError::DuplicateCaseId { index: 1 })
    ));
}

#[test]
fn report_exposes_mismatch_without_raw_content() {
    let input = r#"{
        "schema_version": 2,
        "cases": [{
            "id": "mismatch",
            "category": "fixture",
            "source": "secret",
            "candidate": "secret.",
            "reference_judgment": "acceptable",
            "expected_status": "abstained",
            "expected_reason": null,
            "expected_output": "source"
        }]
    }"#;
    let suite = parse_suite(input).expect("valid fixture schema");
    let report = run_suite(&suite);
    assert!(!report.is_success());
    assert_eq!(
        report.failures[0].actual_status,
        Some(RewriteStatus::Rewritten)
    );
    let serialized = serde_json::to_string(&report).expect("report serializes");
    assert!(!serialized.contains("secret"));
}
