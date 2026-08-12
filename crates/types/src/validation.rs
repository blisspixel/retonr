use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{CandidateId, RewriteUnitId};

/// Result of one independent validation gate.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateStatus {
    /// Evidence satisfied the gate.
    Pass,
    /// Evidence violated the gate.
    Fail,
    /// Available evidence was insufficient for a decision.
    Uncertain,
    /// The gate does not apply to this candidate.
    NotApplicable,
}

/// Severity associated with a gate result.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Informational evidence that does not affect eligibility.
    Info,
    /// Evidence that may affect a permissive policy.
    Warning,
    /// Evidence that makes the candidate ineligible.
    Error,
}

/// Redacted, machine-readable evidence attached to a gate result.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct GateEvidence {
    /// Stable evidence category.
    pub code: String,
    /// Human-readable explanation without raw document content.
    pub message: String,
}

/// Versioned result emitted by one validation gate.
#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct GateResult {
    /// Stable gate identifier.
    pub gate_id: String,
    /// Independent gate implementation version.
    pub gate_version: String,
    /// Gate decision.
    pub status: GateStatus,
    /// Gate severity.
    pub severity: Severity,
    /// Redacted supporting evidence.
    pub evidence: Vec<GateEvidence>,
    /// Optional calibrated confidence in the inclusive range zero to one.
    pub confidence: Option<f32>,
}

impl GateResult {
    /// Creates a passing hard gate without exposing source content.
    #[must_use]
    pub fn pass(gate_id: impl Into<String>) -> Self {
        Self {
            gate_id: gate_id.into(),
            gate_version: "1".to_owned(),
            status: GateStatus::Pass,
            severity: Severity::Error,
            evidence: Vec::new(),
            confidence: None,
        }
    }

    /// Creates a failing hard gate with redacted evidence.
    #[must_use]
    pub fn fail(
        gate_id: impl Into<String>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            gate_id: gate_id.into(),
            gate_version: "1".to_owned(),
            status: GateStatus::Fail,
            severity: Severity::Error,
            evidence: vec![GateEvidence {
                code: code.into(),
                message: message.into(),
            }],
            confidence: None,
        }
    }
}

/// Independent semantic evaluator result.
#[derive(Clone, Copy, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct SemanticAssessment {
    /// Evaluator decision.
    pub status: GateStatus,
    /// Calibrated confidence when available.
    pub confidence: Option<f32>,
}

/// Complete validation result for one candidate.
#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct CandidateAssessment {
    /// Candidate being assessed.
    pub candidate_id: CandidateId,
    /// Unit targeted by the candidate.
    pub unit_id: RewriteUnitId,
    /// Whether the configured policy permits this candidate.
    pub eligible: bool,
    /// Ordered gate results.
    pub gates: Vec<GateResult>,
}

#[cfg(test)]
mod tests {
    use super::{GateResult, GateStatus};

    #[test]
    fn pass_and_fail_build_stable_shapes() {
        let pass = GateResult::pass("encoding");
        assert_eq!(pass.status, GateStatus::Pass);
        assert!(pass.evidence.is_empty());

        let fail = GateResult::fail("literal", "missing", "protected value missing");
        assert_eq!(fail.status, GateStatus::Fail);
        assert_eq!(fail.evidence[0].code, "missing");
    }
}
