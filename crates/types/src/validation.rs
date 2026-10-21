use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use thiserror::Error;

use crate::bounded_serde::{BoundedString, BoundedVec};
use crate::{CandidateId, ClaimComparisonEvidence, RewriteUnitId};

#[path = "validation/semantic.rs"]
mod semantic;

pub use semantic::{
    SemanticAssessment, SemanticEvidence, SemanticEvidenceCode, SemanticEvidenceDetails,
};

/// Maximum findings accepted from one semantic evaluator call or gate.
pub const MAX_SEMANTIC_EVIDENCE_ITEMS: usize = 32;
/// Maximum gates retained for one candidate.
pub const MAX_GATES_PER_CANDIDATE: usize = 64;
/// Maximum bytes accepted in one evidence code.
pub const MAX_GATE_EVIDENCE_CODE_BYTES: usize = 64;
/// Maximum bytes accepted in one redacted evidence message.
pub const MAX_GATE_EVIDENCE_MESSAGE_BYTES: usize = 256;

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

/// Content-redacted counts for exact invariants implemented today.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize)]
pub struct InvariantEvidenceSummary {
    /// Caller-declared exact terms.
    pub declared_terms: u32,
    /// HTTP or HTTPS URLs.
    pub urls: u32,
    /// Email addresses.
    pub emails: u32,
    /// Numeric, currency, or percentage literals.
    pub numbers: u32,
    /// Total protected occurrences.
    pub total: u32,
}

impl InvariantEvidenceSummary {
    /// Validates that typed counts add to the declared total.
    ///
    /// # Errors
    ///
    /// Returns [`EvidenceContractError`] when addition overflows or counts differ.
    pub fn validate(self) -> Result<Self, EvidenceContractError> {
        let observed = self
            .declared_terms
            .checked_add(self.urls)
            .and_then(|count| count.checked_add(self.emails))
            .and_then(|count| count.checked_add(self.numbers))
            .ok_or(EvidenceContractError::InvalidInvariantSummary)?;
        if observed != self.total {
            return Err(EvidenceContractError::InvalidInvariantSummary);
        }
        Ok(self)
    }
}

impl<'de> Deserialize<'de> for InvariantEvidenceSummary {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            declared_terms: u32,
            urls: u32,
            emails: u32,
            numbers: u32,
            total: u32,
        }
        let wire = Wire::deserialize(deserializer)?;
        Self {
            declared_terms: wire.declared_terms,
            urls: wire.urls,
            emails: wire.emails,
            numbers: wire.numbers,
            total: wire.total,
        }
        .validate()
        .map_err(D::Error::custom)
    }
}

/// Maximum redacted span failures retained on one gate.
pub const MAX_SPAN_FAILURES: usize = 32;

/// How a protected or structural span differs, without its text.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpanChange {
    /// Both sides have a span and the surfaces differ.
    Altered,
    /// The source span has no counterpart.
    Missing,
    /// The candidate span has no counterpart.
    Added,
    /// A structural boundary moved.
    Moved,
    /// The candidate introduced an unsafe character.
    Introduced,
}

/// One-based line and byte column. Neither value is document text.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextPoint {
    /// One-based line number.
    pub line: u32,
    /// One-based byte column within the line.
    pub column: u32,
}

/// One redacted location for a failed protected, structural, or safety span.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpanFailure {
    /// Closed category such as `number`, `url`, `structure`, or `word_changed`.
    pub kind: String,
    /// One-based index in the caller-declared term list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ordinal: Option<u32>,
    /// Location in the source when the span is present there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<TextPoint>,
    /// Location in the candidate when the span is present there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate: Option<TextPoint>,
    /// Difference category.
    pub change: SpanChange,
}

/// Typed evidence payload attached to a retained gate finding.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GateEvidenceDetails {
    /// Counts of exact deterministic invariants protected for one unit.
    InvariantSummary(InvariantEvidenceSummary),
    /// Redacted aggregate from independently produced claim evidence.
    ClaimComparison(Box<ClaimComparisonEvidence>),
    /// Locations of spans that failed a hard gate, without their text.
    SpanFailures {
        /// Bounded redacted failures.
        failures: Vec<SpanFailure>,
    },
}

/// Redacted, machine-readable evidence attached to a gate result.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize)]
pub struct GateEvidence {
    /// Stable evidence category.
    pub code: String,
    /// Product-owned explanation without raw document content.
    pub message: String,
    /// Optional typed, content-redacted evidence for machine inspection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<GateEvidenceDetails>,
}

impl GateEvidence {
    /// Builds one bounded redacted finding.
    ///
    /// # Errors
    ///
    /// Returns [`EvidenceContractError`] when the finding violates the contract.
    pub fn new(
        code: impl Into<String>,
        message: impl Into<String>,
        details: Option<GateEvidenceDetails>,
    ) -> Result<Self, EvidenceContractError> {
        let evidence = Self {
            code: code.into(),
            message: message.into(),
            details,
        };
        evidence.validate()?;
        Ok(evidence)
    }

    /// Validates this finding without interpreting its conclusion.
    ///
    /// # Errors
    ///
    /// Returns [`EvidenceContractError`] when the finding violates the contract.
    pub fn validate(&self) -> Result<(), EvidenceContractError> {
        validate_code(&self.code)?;
        if self.message.is_empty()
            || self.message.len() > MAX_GATE_EVIDENCE_MESSAGE_BYTES
            || self.message.chars().any(char::is_control)
        {
            return Err(EvidenceContractError::InvalidMessage);
        }
        validate_gate_details(self.details.as_ref())
    }
}

impl<'de> Deserialize<'de> for GateEvidence {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            code: BoundedString<MAX_GATE_EVIDENCE_CODE_BYTES>,
            message: BoundedString<MAX_GATE_EVIDENCE_MESSAGE_BYTES>,
            #[serde(default)]
            details: Option<GateEvidenceDetails>,
        }
        let wire = Wire::deserialize(deserializer)?;
        Self::new(wire.code.0, wire.message.0, wire.details).map_err(D::Error::custom)
    }
}

/// Versioned result emitted by one validation gate.
#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize)]
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

    /// Creates a failing hard gate with redacted product-owned evidence.
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
                details: None,
            }],
            confidence: None,
        }
    }

    /// Validates the bounded gate-result contract without trusting its decision.
    ///
    /// # Errors
    ///
    /// Returns [`EvidenceContractError`] when the gate violates the contract.
    pub fn validate(&self) -> Result<(), EvidenceContractError> {
        validate_code(&self.gate_id)?;
        validate_version(&self.gate_version)?;
        validate_confidence(self.confidence)?;
        if self.evidence.len() > MAX_SEMANTIC_EVIDENCE_ITEMS {
            return Err(EvidenceContractError::TooManyFindings);
        }
        self.evidence.iter().try_for_each(GateEvidence::validate)
    }
}

impl<'de> Deserialize<'de> for GateResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            gate_id: BoundedString<MAX_GATE_EVIDENCE_CODE_BYTES>,
            gate_version: BoundedString<MAX_GATE_EVIDENCE_CODE_BYTES>,
            status: GateStatus,
            severity: Severity,
            evidence: BoundedVec<GateEvidence, MAX_SEMANTIC_EVIDENCE_ITEMS>,
            confidence: Option<f32>,
        }
        let wire = Wire::deserialize(deserializer)?;
        let result = Self {
            gate_id: wire.gate_id.0,
            gate_version: wire.gate_version.0,
            status: wire.status,
            severity: wire.severity,
            evidence: wire.evidence.0,
            confidence: wire.confidence,
        };
        result.validate().map_err(D::Error::custom)?;
        Ok(result)
    }
}

/// Malformed or unbounded redacted evidence from an evaluator.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum EvidenceContractError {
    /// A finding code is empty, oversized, or noncanonical.
    #[error("evidence code is invalid")]
    InvalidCode,
    /// A gate implementation version is empty, oversized, or noncanonical.
    #[error("evidence version is invalid")]
    InvalidVersion,
    /// A finding message is empty, oversized, or contains controls.
    #[error("evidence message is invalid")]
    InvalidMessage,
    /// Calibrated confidence is not finite or outside zero to one.
    #[error("evidence confidence must be finite and between zero and one")]
    InvalidConfidence,
    /// The evaluator emitted too many findings.
    #[error("semantic evidence exceeds the finding limit")]
    TooManyFindings,
    /// The candidate retained too many gates.
    #[error("candidate gate count exceeds the limit")]
    TooManyGates,
    /// Candidate and rewrite-unit identities do not share the same scope.
    #[error("candidate identifier does not belong to the assessed rewrite unit")]
    InvalidCandidateScope,
    /// Eligibility contradicts the retained hard-gate outcomes.
    #[error("candidate eligibility contradicts retained gate outcomes")]
    InconsistentEligibility,
    /// Typed claim comparison evidence is internally inconsistent.
    #[error(transparent)]
    InvalidClaimComparison(#[from] crate::ClaimEvidenceError),
    /// Exact invariant counts do not add to the declared total.
    #[error("invariant evidence counts are inconsistent")]
    InvalidInvariantSummary,
    /// A span-failure list is empty, unbounded, or not content-free.
    #[error("span failure evidence is invalid")]
    InvalidSpanFailure,
    /// A semantic conclusion conflicts with its typed finding category or details.
    #[error("semantic evidence conclusion is inconsistent")]
    InconsistentSemanticFinding,
}

fn validate_gate_details(
    details: Option<&GateEvidenceDetails>,
) -> Result<(), EvidenceContractError> {
    match details {
        Some(GateEvidenceDetails::InvariantSummary(summary)) => {
            summary.validate()?;
        }
        Some(GateEvidenceDetails::ClaimComparison(comparison)) => {
            comparison.validate()?;
        }
        Some(GateEvidenceDetails::SpanFailures { failures }) => {
            validate_span_failures(failures)?;
        }
        None => {}
    }
    Ok(())
}

fn validate_span_failures(failures: &[SpanFailure]) -> Result<(), EvidenceContractError> {
    if failures.is_empty() || failures.len() > MAX_SPAN_FAILURES {
        return Err(EvidenceContractError::InvalidSpanFailure);
    }
    failures.iter().try_for_each(validate_span_failure)
}

fn validate_span_failure(failure: &SpanFailure) -> Result<(), EvidenceContractError> {
    if !is_span_kind(&failure.kind) {
        return Err(EvidenceContractError::InvalidSpanFailure);
    }
    match (failure.kind.as_str(), failure.ordinal) {
        ("declared_term", Some(ordinal)) if ordinal > 0 => {}
        (_, None) => {}
        ("declared_term", _) | (_, Some(_)) => {
            return Err(EvidenceContractError::InvalidSpanFailure);
        }
    }
    if failure.source.is_none() && failure.candidate.is_none() {
        return Err(EvidenceContractError::InvalidSpanFailure);
    }
    validate_point(failure.source)?;
    validate_point(failure.candidate)
}

fn is_span_kind(kind: &str) -> bool {
    matches!(
        kind,
        "number" | "url" | "email" | "declared_term" | "unsafe_text" | "structure" | "word_changed"
    )
}

fn validate_point(point: Option<TextPoint>) -> Result<(), EvidenceContractError> {
    if point.is_some_and(|point| point.line == 0 || point.column == 0) {
        return Err(EvidenceContractError::InvalidSpanFailure);
    }
    Ok(())
}

fn validate_code(code: &str) -> Result<(), EvidenceContractError> {
    if code.is_empty()
        || code.len() > MAX_GATE_EVIDENCE_CODE_BYTES
        || !code
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        Err(EvidenceContractError::InvalidCode)
    } else {
        Ok(())
    }
}

fn validate_version(version: &str) -> Result<(), EvidenceContractError> {
    if version.is_empty()
        || version.len() > MAX_GATE_EVIDENCE_CODE_BYTES
        || !version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'+'))
    {
        Err(EvidenceContractError::InvalidVersion)
    } else {
        Ok(())
    }
}

fn validate_confidence(confidence: Option<f32>) -> Result<(), EvidenceContractError> {
    if confidence.is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value)) {
        Err(EvidenceContractError::InvalidConfidence)
    } else {
        Ok(())
    }
}

/// Complete validation result for one candidate.
#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize)]
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

impl CandidateAssessment {
    /// Validates the complete bounded candidate assessment.
    ///
    /// # Errors
    ///
    /// Returns [`EvidenceContractError`] when a gate or count is invalid.
    pub fn validate(&self) -> Result<(), EvidenceContractError> {
        if !self.candidate_id.is_scoped_to(&self.unit_id) {
            return Err(EvidenceContractError::InvalidCandidateScope);
        }
        if self.gates.len() > MAX_GATES_PER_CANDIDATE {
            return Err(EvidenceContractError::TooManyGates);
        }
        self.gates.iter().try_for_each(GateResult::validate)?;
        let hard_gates_pass = !self.gates.is_empty()
            && self.gates.iter().all(|gate| {
                gate.severity != Severity::Error
                    || matches!(gate.status, GateStatus::Pass | GateStatus::NotApplicable)
            });
        if self.eligible != hard_gates_pass {
            return Err(EvidenceContractError::InconsistentEligibility);
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for CandidateAssessment {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            candidate_id: CandidateId,
            unit_id: RewriteUnitId,
            eligible: bool,
            gates: BoundedVec<GateResult, MAX_GATES_PER_CANDIDATE>,
        }
        let wire = Wire::deserialize(deserializer)?;
        let assessment = Self {
            candidate_id: wire.candidate_id,
            unit_id: wire.unit_id,
            eligible: wire.eligible,
            gates: wire.gates.0,
        };
        assessment.validate().map_err(D::Error::custom)?;
        Ok(assessment)
    }
}

#[cfg(test)]
#[path = "validation_tests.rs"]
mod tests;
