//! Bounded plain-text candidate evaluation suite contracts and runners.

use rewrite_model::EvaluationCase;
use rewrite_types::{ReasonCode, RewriteStatus};
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod runner;
#[cfg(test)]
mod tests;

pub use runner::{
    DEFAULT_EVALUATION_WORKER_CAP, MAX_EVALUATION_WORKER_CAP, run_suite, run_suite_with_concurrency,
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
