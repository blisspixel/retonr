use rewrite_types::{ReasonCode, RewriteStatus};
use serde::{Deserialize, Serialize};

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
