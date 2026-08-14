//! Deterministic evaluation fixtures and reporting for fidelity gates.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use rewrite_app::{AppError, CandidateCheckRequest, CandidateCheckService};
use rewrite_types::{ReasonCode, RewriteStatus};
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod baseline;
mod editorial_corpus;

pub use baseline::{
    BASELINE_SCHEMA_VERSION, BaselineCaseError, BaselineCaseResult, BaselineDefinition,
    BaselineError, BaselineInferencePolicy, BaselineKind, BaselineReport, BaselineStatusCounts,
    run_baseline,
};
pub use editorial_corpus::{
    EDITORIAL_CORPUS_SCHEMA_VERSION, EditorialCase, EditorialCaseKind, EditorialCorpus,
    EditorialCorpusError, EditorialCorpusOrigin, EditorialCorpusSummary,
    EditorialFindingExpectation, MAX_EDITORIAL_CASES, MAX_EDITORIAL_CORPUS_BYTES,
    parse_editorial_corpus,
};

/// Current evaluation-suite contract version.
pub const EVALUATION_SCHEMA_VERSION: u32 = 2;
/// Maximum serialized evaluation suite size accepted by the runner.
pub const MAX_EVALUATION_SUITE_BYTES: usize = 64 * 1024 * 1024;
/// Maximum cases accepted in one evaluation suite.
pub const MAX_EVALUATION_CASES: usize = 10_000;

/// A collection of synthetic, reviewable candidate cases.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluationSuite {
    /// Evaluation contract version.
    pub schema_version: u32,
    /// Independently reported cases.
    pub cases: Vec<EvaluationCase>,
}

/// One deterministic source and candidate expectation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluationCase {
    /// Stable fixture identifier.
    pub id: String,
    /// Stable category used for risk-stratified reports.
    pub category: String,
    /// Synthetic source text.
    pub source: String,
    /// Synthetic candidate text.
    pub candidate: String,
    /// Exact caller-declared terms.
    #[serde(default)]
    pub protected_terms: Vec<String>,
    /// Human reference judgment for aggregate transformation coverage.
    pub reference_judgment: ReferenceJudgment,
    /// Required transaction status.
    pub expected_status: RewriteStatus,
    /// Required reason when the case should abstain.
    pub expected_reason: Option<ReasonCode>,
    /// Which complete byte sequence must be returned.
    pub expected_output: ExpectedOutput,
}

/// Human reference judgment for a complete candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceJudgment {
    /// The changed candidate is acceptable under the fixture's intended meaning and
    /// document contract.
    Acceptable,
    /// The candidate violates meaning, structure, safety, or another required
    /// contract.
    Unacceptable,
    /// The candidate is intentionally identical to the source.
    Identity,
    /// Transformation coverage does not apply to this fixture.
    NotApplicable,
}

/// Expected output identity for an evaluation case.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedOutput {
    /// The exact source bytes must be returned.
    Source,
    /// The exact candidate bytes must be returned.
    Candidate,
}

/// Redacted aggregate result for one suite execution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EvaluationReport {
    /// Evaluation contract version.
    pub schema_version: u32,
    /// Total cases executed.
    pub total: usize,
    /// Cases matching every expectation.
    pub passed: usize,
    /// Cases grouped by category.
    pub categories: Vec<CategoryResult>,
    /// Coverage over changed candidates independently judged acceptable.
    pub transformation_coverage: TransformationCoverage,
    /// Expectation mismatches without raw document content.
    pub failures: Vec<EvaluationFailure>,
}

/// Integer transformation-coverage counts without hiding the denominator.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct TransformationCoverage {
    /// Changed candidates independently judged acceptable.
    pub acceptable: usize,
    /// Acceptable changed candidates returned by the transaction.
    pub rewritten: usize,
}

impl EvaluationReport {
    /// Returns whether all evaluated expectations passed.
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.failures.is_empty()
    }
}

/// Aggregate totals for one evaluation category.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CategoryResult {
    /// Stable category name.
    pub category: String,
    /// Total category cases.
    pub total: usize,
    /// Passing category cases.
    pub passed: usize,
}

/// Redacted mismatch for one evaluation case.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EvaluationFailure {
    /// Stable fixture identifier.
    pub id: String,
    /// Stable risk category.
    pub category: String,
    /// Expected transaction status.
    pub expected_status: RewriteStatus,
    /// Actual transaction status, if the transaction completed.
    pub actual_status: Option<RewriteStatus>,
    /// Expected machine reason.
    pub expected_reason: Option<ReasonCode>,
    /// Actual machine reason, if the transaction completed.
    pub actual_reason: Option<ReasonCode>,
    /// Whether the returned bytes violated the output identity expectation.
    pub output_mismatch: bool,
    /// Redacted operational error category, when present.
    pub error: Option<String>,
}

/// Evaluation parsing or schema failure.
#[derive(Debug, Error)]
pub enum EvaluationError {
    /// Suite JSON is invalid.
    #[error("invalid evaluation suite: {0}")]
    InvalidJson(#[from] serde_json::Error),
    /// Suite contract version is unsupported.
    #[error("unsupported evaluation schema version {0}")]
    UnsupportedSchema(u32),
    /// Suite contains more cases than the evaluation bound.
    #[error("evaluation suite exceeds the case-count limit")]
    TooManyCases,
    /// Case identifier is empty, oversized, or contains unsupported characters.
    #[error("evaluation case {index} has an invalid identifier")]
    InvalidCaseId {
        /// Zero-based case position.
        index: usize,
    },
    /// Case identifier is not unique within the suite.
    #[error("evaluation case {index} has a duplicate identifier")]
    DuplicateCaseId {
        /// Zero-based case position.
        index: usize,
    },
    /// Case category is empty, oversized, or contains unsupported characters.
    #[error("evaluation case {index} has an invalid category")]
    InvalidCategory {
        /// Zero-based case position.
        index: usize,
    },
}

/// Parses a suite from JSON and validates its contract version.
///
/// # Errors
///
/// Returns [`EvaluationError`] for invalid JSON or an unsupported schema.
pub fn parse_suite(input: &str) -> Result<EvaluationSuite, EvaluationError> {
    let suite: EvaluationSuite = serde_json::from_str(input)?;
    if suite.schema_version != EVALUATION_SCHEMA_VERSION {
        return Err(EvaluationError::UnsupportedSchema(suite.schema_version));
    }
    if suite.cases.len() > MAX_EVALUATION_CASES {
        return Err(EvaluationError::TooManyCases);
    }
    let mut identifiers = std::collections::BTreeSet::new();
    for (index, case) in suite.cases.iter().enumerate() {
        if !valid_label(&case.id) {
            return Err(EvaluationError::InvalidCaseId { index });
        }
        if !identifiers.insert(case.id.as_str()) {
            return Err(EvaluationError::DuplicateCaseId { index });
        }
        if !valid_label(&case.category) {
            return Err(EvaluationError::InvalidCategory { index });
        }
    }
    Ok(suite)
}

fn valid_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-_".contains(&byte))
}

/// Executes all suite cases through the same application service as the CLI.
#[must_use]
pub fn run_suite(suite: &EvaluationSuite) -> EvaluationReport {
    let mut categories = std::collections::BTreeMap::<String, CategoryResult>::new();
    let mut failures = Vec::new();
    let mut transformation_coverage = TransformationCoverage {
        acceptable: 0,
        rewritten: 0,
    };

    for case in &suite.cases {
        let category = categories
            .entry(case.category.clone())
            .or_insert_with(|| CategoryResult {
                category: case.category.clone(),
                total: 0,
                passed: 0,
            });
        category.total += 1;

        let result = CandidateCheckService::check(CandidateCheckRequest {
            source: case.source.as_bytes().to_vec(),
            candidate: case.candidate.clone(),
            protected_terms: case.protected_terms.clone(),
        });
        match result {
            Ok(result) => {
                if case.reference_judgment == ReferenceJudgment::Acceptable
                    && case.source.as_bytes() != case.candidate.as_bytes()
                {
                    transformation_coverage.acceptable += 1;
                    if result.record.status == RewriteStatus::Rewritten
                        && result.output == case.candidate.as_bytes()
                    {
                        transformation_coverage.rewritten += 1;
                    }
                }
                let expected_bytes = match case.expected_output {
                    ExpectedOutput::Source => case.source.as_bytes(),
                    ExpectedOutput::Candidate => case.candidate.as_bytes(),
                };
                let output_mismatch = result.output != expected_bytes;
                if result.record.status == case.expected_status
                    && result.record.reason == case.expected_reason
                    && !output_mismatch
                {
                    category.passed += 1;
                } else {
                    failures.push(EvaluationFailure {
                        id: case.id.clone(),
                        category: case.category.clone(),
                        expected_status: case.expected_status,
                        actual_status: Some(result.record.status),
                        expected_reason: case.expected_reason,
                        actual_reason: result.record.reason,
                        output_mismatch,
                        error: None,
                    });
                }
            }
            Err(error) => failures.push(operational_failure(case, &error)),
        }
    }

    EvaluationReport {
        schema_version: EVALUATION_SCHEMA_VERSION,
        total: suite.cases.len(),
        passed: suite.cases.len().saturating_sub(failures.len()),
        categories: categories.into_values().collect(),
        transformation_coverage,
        failures,
    }
}

fn operational_failure(case: &EvaluationCase, error: &AppError) -> EvaluationFailure {
    let error = match error {
        AppError::CandidateTooLarge { .. } => "candidate_too_large",
        AppError::TextAdapter(_) => "text_adapter",
        AppError::Engine(_) => "engine",
        AppError::Protection(_) => "protection",
        AppError::Grounded(_) => "grounded",
    };
    EvaluationFailure {
        id: case.id.clone(),
        category: case.category.clone(),
        expected_status: case.expected_status,
        actual_status: None,
        expected_reason: case.expected_reason,
        actual_reason: None,
        output_mismatch: false,
        error: Some(error.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use rewrite_types::RewriteStatus;

    use super::{EVALUATION_SCHEMA_VERSION, EvaluationError, parse_suite, run_suite};

    const CORE_SUITE: &str = include_str!("../fixtures/core.json");

    #[test]
    fn core_regression_suite_has_zero_failures() {
        let suite = parse_suite(CORE_SUITE).expect("checked-in suite is valid");
        let report = run_suite(&suite);
        assert!(report.is_success(), "failures: {:?}", report.failures);
        assert_eq!(report.total, 25);
        assert_eq!(report.passed, report.total);
        assert_eq!(report.transformation_coverage.acceptable, 9);
        assert_eq!(report.transformation_coverage.rewritten, 4);
        assert!(
            report
                .categories
                .iter()
                .any(|category| category.category == "semantic_hard_negative")
        );
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
}
