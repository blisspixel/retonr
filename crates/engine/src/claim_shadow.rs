use rewrite_types::{
    CLAIM_COMPARATOR_VERSION, ClaimComparisonEvidence, Digest, GateEvidence, GateEvidenceDetails,
    GateResult, GateStatus, RewriteUnitId, SemanticEvidenceCode, Severity,
};

/// Independently produced claim comparison observed after hard gates.
///
/// A present observation is recorded on a separate informational gate. It never
/// changes candidate eligibility. Literal-token failure still abstains.
pub trait ClaimShadowObserver: Send + Sync {
    /// Returns comparison evidence for one exact restored unit pair, if any.
    fn observe(
        &self,
        unit_id: &RewriteUnitId,
        source: &str,
        candidate: &str,
    ) -> Option<ClaimComparisonEvidence>;
}

pub(crate) const CLAIM_SHADOW_GATE: &str = "claim_comparison_shadow";

pub(crate) fn shadow_gate(
    observer: &dyn ClaimShadowObserver,
    unit_id: &RewriteUnitId,
    source: &str,
    candidate: &str,
) -> Option<GateResult> {
    let comparison = observer.observe(unit_id, source, candidate)?;
    Some(gate_from_comparison(comparison, unit_id, source, candidate))
}

fn gate_from_comparison(
    comparison: ClaimComparisonEvidence,
    unit_id: &RewriteUnitId,
    source: &str,
    candidate: &str,
) -> GateResult {
    if !comparison_matches(&comparison, unit_id, source, candidate)
        || comparison.validate().is_err()
    {
        return mismatched_shadow_gate();
    }
    let counts = comparison.counts();
    let (status, code) = if counts.has_difference() {
        (
            GateStatus::Fail,
            SemanticEvidenceCode::ClaimComparisonConflict,
        )
    } else if counts.has_uncertainty() {
        (
            GateStatus::Uncertain,
            SemanticEvidenceCode::ClaimComparisonUncertain,
        )
    } else {
        (
            GateStatus::Pass,
            SemanticEvidenceCode::ClaimComparisonPreserved,
        )
    };
    let message = match code {
        SemanticEvidenceCode::ClaimComparisonPreserved => {
            "typed claim comparison found no conflict"
        }
        SemanticEvidenceCode::ClaimComparisonConflict => "typed claim comparison found a conflict",
        SemanticEvidenceCode::ClaimComparisonUncertain => {
            "typed claim comparison retained uncertainty"
        }
        _ => "typed claim comparison was recorded without authority",
    };
    let gate = GateResult {
        gate_id: CLAIM_SHADOW_GATE.to_owned(),
        gate_version: shadow_gate_version(),
        status,
        severity: Severity::Info,
        evidence: vec![GateEvidence {
            code: code.as_str().to_owned(),
            message: message.to_owned(),
            details: Some(GateEvidenceDetails::ClaimComparison(Box::new(comparison))),
        }],
        confidence: None,
    };
    if gate.validate().is_err() {
        mismatched_shadow_gate()
    } else {
        gate
    }
}

fn comparison_matches(
    comparison: &ClaimComparisonEvidence,
    unit_id: &RewriteUnitId,
    source: &str,
    candidate: &str,
) -> bool {
    comparison.unit_id() == unit_id
        && comparison.source_text_digest() == &Digest::sha256(source.as_bytes())
        && comparison.candidate_text_digest() == &Digest::sha256(candidate.as_bytes())
        && comparison.source_text_bytes() == source.len() as u64
        && comparison.candidate_text_bytes() == candidate.len() as u64
}

fn mismatched_shadow_gate() -> GateResult {
    GateResult {
        gate_id: CLAIM_SHADOW_GATE.to_owned(),
        gate_version: shadow_gate_version(),
        status: GateStatus::Uncertain,
        severity: Severity::Info,
        evidence: vec![GateEvidence {
            code: "invalid_shadow_claim_evidence".to_owned(),
            message: "shadow claim comparison was malformed or bound to other text".to_owned(),
            details: None,
        }],
        confidence: None,
    }
}

fn shadow_gate_version() -> String {
    format!("claim-comparator-v{CLAIM_COMPARATOR_VERSION}")
}

#[cfg(test)]
mod tests;
