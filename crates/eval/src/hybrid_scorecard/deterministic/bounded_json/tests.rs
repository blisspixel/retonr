use rewrite_model::MAX_CANDIDATE_DETERMINISTIC_REPORT_JSON_BYTES;
use rewrite_types::RewriteStatus;

use crate::{
    EVALUATION_SCHEMA_VERSION, EvaluationCase, EvaluationFailure, EvaluationReport,
    EvaluationSuite, ExpectedOutput, ReferenceJudgment, TransformationCoverage,
};

use super::super::{canonical_suite_bytes, encode_report_json};
use crate::hybrid_scorecard::HybridScorecardError;

#[test]
fn oversized_in_memory_suite_is_rejected_by_the_bounded_writer() {
    let suite = EvaluationSuite {
        schema_version: EVALUATION_SCHEMA_VERSION,
        cases: vec![EvaluationCase {
            id: "oversized".to_owned(),
            category: "identity".to_owned(),
            source: "x".repeat(crate::MAX_EVALUATION_SUITE_BYTES),
            candidate: "x".to_owned(),
            protected_terms: Vec::new(),
            reference_judgment: ReferenceJudgment::NotApplicable,
            expected_status: RewriteStatus::UnchangedNoEligibleContent,
            expected_reason: None,
            expected_output: ExpectedOutput::Source,
        }],
    };
    assert_eq!(
        canonical_suite_bytes(&suite),
        Err(HybridScorecardError::InvalidDeterministicSuites)
    );
}

#[test]
fn oversized_report_is_rejected_by_its_smaller_fixed_writer_bound() {
    let report = EvaluationReport {
        schema_version: EVALUATION_SCHEMA_VERSION,
        total: 1,
        passed: 0,
        categories: Vec::new(),
        transformation_coverage: TransformationCoverage {
            acceptable: 0,
            rewritten: 0,
        },
        failures: vec![EvaluationFailure {
            id: "oversized".to_owned(),
            category: "identity".to_owned(),
            expected_status: RewriteStatus::UnchangedNoEligibleContent,
            actual_status: None,
            expected_reason: None,
            actual_reason: None,
            output_mismatch: false,
            error: Some("x".repeat(MAX_CANDIDATE_DETERMINISTIC_REPORT_JSON_BYTES)),
        }],
    };
    assert_eq!(
        encode_report_json(&report),
        Err(HybridScorecardError::InvalidDeterministicReport)
    );
}
