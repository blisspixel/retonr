use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, de::Error as _};

use super::{EvidenceContractError, GateStatus, MAX_SEMANTIC_EVIDENCE_ITEMS, validate_confidence};
use crate::ClaimComparisonEvidence;
use crate::bounded_serde::BoundedVec;

/// Closed semantic finding categories accepted from an evaluator.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticEvidenceCode {
    /// Literal-mode tokens were identical.
    LiteralTokensEqual,
    /// Literal-mode tokens changed.
    LiteralTokensChanged,
    /// The evaluator does not support the requested rewrite mode.
    UnsupportedMode,
    /// Typed claim evidence reported preservation without unresolved uncertainty.
    ClaimComparisonPreserved,
    /// Typed claim evidence reported a fidelity conflict.
    ClaimComparisonConflict,
    /// Typed claim evidence was incomplete or retained unresolved uncertainty.
    ClaimComparisonUncertain,
}

impl SemanticEvidenceCode {
    /// Returns the stable product-owned evidence code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LiteralTokensEqual => "literal_tokens_equal",
            Self::LiteralTokensChanged => "literal_tokens_changed",
            Self::UnsupportedMode => "unsupported_mode",
            Self::ClaimComparisonPreserved => "claim_comparison_preserved",
            Self::ClaimComparisonConflict => "claim_comparison_conflict",
            Self::ClaimComparisonUncertain => "claim_comparison_uncertain",
        }
    }
}

/// Typed detail variants a semantic evaluator is permitted to return.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SemanticEvidenceDetails {
    /// Redacted aggregate from independently produced claim evidence.
    ClaimComparison(ClaimComparisonEvidence),
}

/// Content-redacted evidence returned by an independent semantic evaluator.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticEvidence {
    /// Closed product-owned finding category.
    pub code: SemanticEvidenceCode,
    /// Optional permitted semantic aggregate without raw text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<SemanticEvidenceDetails>,
}

impl<'de> Deserialize<'de> for SemanticEvidence {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            code: SemanticEvidenceCode,
            #[serde(default)]
            details: Option<SemanticEvidenceDetails>,
        }
        let wire = Wire::deserialize(deserializer)?;
        let evidence = Self::new(wire.code, wire.details);
        evidence.validate().map_err(D::Error::custom)?;
        Ok(evidence)
    }
}

impl SemanticEvidence {
    /// Builds one semantic finding from a closed product-owned category.
    #[must_use]
    pub const fn new(code: SemanticEvidenceCode, details: Option<SemanticEvidenceDetails>) -> Self {
        Self { code, details }
    }

    fn validate(&self) -> Result<(), EvidenceContractError> {
        match (&self.code, &self.details) {
            (
                SemanticEvidenceCode::LiteralTokensEqual
                | SemanticEvidenceCode::LiteralTokensChanged
                | SemanticEvidenceCode::UnsupportedMode
                | SemanticEvidenceCode::ClaimComparisonUncertain,
                None,
            ) => {}
            (
                SemanticEvidenceCode::ClaimComparisonPreserved,
                Some(SemanticEvidenceDetails::ClaimComparison(comparison)),
            ) => {
                comparison.validate()?;
                if comparison.counts().has_uncertainty() || comparison.counts().has_difference() {
                    return Err(EvidenceContractError::InconsistentSemanticFinding);
                }
            }
            (
                SemanticEvidenceCode::ClaimComparisonConflict,
                Some(SemanticEvidenceDetails::ClaimComparison(comparison)),
            ) => {
                comparison.validate()?;
                if !comparison.counts().has_difference() {
                    return Err(EvidenceContractError::InconsistentSemanticFinding);
                }
            }
            (
                SemanticEvidenceCode::ClaimComparisonUncertain,
                Some(SemanticEvidenceDetails::ClaimComparison(comparison)),
            ) => {
                comparison.validate()?;
                if !comparison.counts().has_uncertainty() {
                    return Err(EvidenceContractError::InconsistentSemanticFinding);
                }
            }
            _ => return Err(EvidenceContractError::InconsistentSemanticFinding),
        }
        Ok(())
    }
}

/// Independent semantic evaluator result.
#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize)]
pub struct SemanticAssessment {
    /// Evaluator decision.
    pub status: GateStatus,
    /// Calibrated confidence when available.
    pub confidence: Option<f32>,
    /// Redacted evaluator findings.
    pub evidence: Vec<SemanticEvidence>,
}

impl SemanticAssessment {
    /// Validates the bounded evaluator response contract.
    ///
    /// # Errors
    ///
    /// Returns [`EvidenceContractError`] when the response violates the contract.
    pub fn validate(&self) -> Result<(), EvidenceContractError> {
        validate_confidence(self.confidence)?;
        if self.evidence.len() > MAX_SEMANTIC_EVIDENCE_ITEMS {
            return Err(EvidenceContractError::TooManyFindings);
        }
        self.evidence
            .iter()
            .try_for_each(SemanticEvidence::validate)?;
        if self.status == GateStatus::Pass
            && (self.evidence.is_empty()
                || self.evidence.iter().any(|finding| {
                    !matches!(
                        finding.code,
                        SemanticEvidenceCode::LiteralTokensEqual
                            | SemanticEvidenceCode::ClaimComparisonPreserved
                    )
                }))
        {
            return Err(EvidenceContractError::InconsistentSemanticFinding);
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for SemanticAssessment {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            status: GateStatus,
            confidence: Option<f32>,
            #[serde(default)]
            evidence: BoundedVec<SemanticEvidence, MAX_SEMANTIC_EVIDENCE_ITEMS>,
        }
        let wire = Wire::deserialize(deserializer)?;
        let assessment = Self {
            status: wire.status,
            confidence: wire.confidence,
            evidence: wire.evidence.0,
        };
        assessment.validate().map_err(D::Error::custom)?;
        Ok(assessment)
    }
}
