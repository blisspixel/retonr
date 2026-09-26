use rewrite_types::{
    CandidateAssessment, GateEvidence, GateEvidenceDetails, GateResult, GateStatus,
    GeneratedCandidate, InvariantEvidenceSummary, ReasonCode, RewriteOptions, RewriteUnit,
    RewriteUnitId, Severity,
};

use crate::policy::candidate_contract_is_valid;
use crate::{ProtectionError, ProtectionPlan};

pub(super) const UNIT_GATE: &str = "candidate_unit";
pub(super) const CANDIDATE_GATE: &str = "candidate_contract";
pub(super) const SENTINEL_GATE: &str = "sentinel_integrity";
pub(super) const PROTECTED_GATE: &str = "protected_values";
pub(super) const LAYOUT_GATE: &str = "layout_constraints";
pub(super) const SEMANTIC_GATE: &str = "semantic_fidelity";

pub(super) struct UnitProgress {
    pub selected: rewrite_types::CandidateId,
    pub replacement: Option<String>,
    pub assessments: Vec<CandidateAssessment>,
}

pub(super) struct EligibleCandidate {
    pub generated: GeneratedCandidate,
    pub restored: String,
}

pub(super) struct EvaluatedCandidate {
    pub assessment: CandidateAssessment,
    pub candidate: EligibleCandidate,
    pub reason: Option<ReasonCode>,
}

pub(super) fn semantic_evidence(evidence: rewrite_types::SemanticEvidence) -> GateEvidence {
    use rewrite_types::{SemanticEvidenceCode, SemanticEvidenceDetails};
    let message = match evidence.code {
        SemanticEvidenceCode::LiteralTokensEqual => "literal token sequence was preserved",
        SemanticEvidenceCode::LiteralTokensChanged => "literal token sequence changed",
        SemanticEvidenceCode::UnsupportedMode => "semantic evaluator does not support this mode",
        SemanticEvidenceCode::ClaimComparisonPreserved => {
            "typed claim comparison found no conflict"
        }
        SemanticEvidenceCode::ClaimComparisonConflict => "typed claim comparison found a conflict",
        SemanticEvidenceCode::ClaimComparisonUncertain => {
            "typed claim comparison retained uncertainty"
        }
    };
    GateEvidence {
        code: evidence.code.as_str().to_owned(),
        message: message.to_owned(),
        details: evidence.details.map(|details| match details {
            SemanticEvidenceDetails::ClaimComparison(comparison) => {
                GateEvidenceDetails::ClaimComparison(Box::new(comparison))
            }
        }),
    }
}

pub(super) fn invalid_semantic_gate() -> (GateResult, bool, ReasonCode) {
    (
        GateResult {
            gate_id: SEMANTIC_GATE.to_owned(),
            gate_version: "semantic-evidence-contract-v1".to_owned(),
            status: GateStatus::Uncertain,
            severity: Severity::Error,
            evidence: vec![GateEvidence {
                code: "invalid_evaluator_evidence".to_owned(),
                message: "semantic evaluator returned malformed or unbounded evidence".to_owned(),
                details: None,
            }],
            confidence: None,
        },
        false,
        ReasonCode::SemanticUncertain,
    )
}

pub(super) fn protected_values_pass(protection: &ProtectionPlan) -> GateResult {
    let mut summary = InvariantEvidenceSummary {
        declared_terms: 0,
        urls: 0,
        emails: 0,
        numbers: 0,
        total: 0,
    };
    for value in protection.values() {
        let count = match value.kind {
            crate::ProtectedKind::DeclaredTerm => &mut summary.declared_terms,
            crate::ProtectedKind::Url => &mut summary.urls,
            crate::ProtectedKind::Email => &mut summary.emails,
            crate::ProtectedKind::Number => &mut summary.numbers,
        };
        *count = count.saturating_add(1);
        summary.total = summary.total.saturating_add(1);
    }
    GateResult {
        gate_id: PROTECTED_GATE.to_owned(),
        gate_version: "1".to_owned(),
        status: GateStatus::Pass,
        severity: Severity::Error,
        evidence: vec![GateEvidence {
            code: "invariant_counts".to_owned(),
            message: "exact protected invariant counts were preserved".to_owned(),
            details: Some(GateEvidenceDetails::InvariantSummary(summary)),
        }],
        confidence: None,
    }
}

pub(super) fn validate_candidate_metadata(
    unit: &RewriteUnit,
    candidate: GeneratedCandidate,
    gates: &mut Vec<GateResult>,
) -> Result<GeneratedCandidate, Box<EvaluatedCandidate>> {
    if candidate.unit_id != unit.id {
        gates.push(GateResult::fail(
            UNIT_GATE,
            "unit_mismatch",
            "candidate targets a different rewrite unit",
        ));
        return Err(Box::new(ineligible(
            candidate,
            unit.id.clone(),
            String::new(),
            core::mem::take(gates),
            ReasonCode::StructureChanged,
        )));
    }
    gates.push(GateResult::pass(UNIT_GATE));
    if !candidate_contract_is_valid(&candidate) {
        gates.push(GateResult::fail(
            CANDIDATE_GATE,
            "invalid_candidate_contract",
            "candidate size, identity, or ranking metadata violates engine limits",
        ));
        return Err(Box::new(ineligible(
            candidate,
            unit.id.clone(),
            String::new(),
            core::mem::take(gates),
            ReasonCode::InvalidCandidate,
        )));
    }
    gates.push(GateResult::pass(CANDIDATE_GATE));
    Ok(candidate)
}

pub(super) fn ineligible(
    candidate: GeneratedCandidate,
    assessed_unit_id: RewriteUnitId,
    restored: String,
    gates: Vec<GateResult>,
    reason: ReasonCode,
) -> EvaluatedCandidate {
    let assessment = CandidateAssessment {
        candidate_id: candidate.id.clone(),
        unit_id: assessed_unit_id,
        eligible: false,
        gates,
    };
    EvaluatedCandidate {
        assessment,
        candidate: EligibleCandidate {
            generated: candidate,
            restored,
        },
        reason: Some(reason),
    }
}

pub(super) fn protection_failure(gate_id: &str, error: &ProtectionError) -> GateResult {
    let code = match error {
        ProtectionError::ReservedTokenInSource => "reserved_token",
        ProtectionError::ProtectedOccurrenceCount => "protected_occurrence_count",
        ProtectionError::SentinelOccurrenceCount => "sentinel_occurrence_count",
        ProtectionError::UnknownSentinel => "unknown_sentinel",
        ProtectionError::MatcherBuild => "matcher_build",
        ProtectionError::ResourceLimit => "protection_resource_limit",
        ProtectionError::InvalidDeclaredTerms => "invalid_declared_terms",
        ProtectionError::AmbiguousSurfaceMapping => "ambiguous_surface_mapping",
    };
    GateResult {
        gate_id: gate_id.to_owned(),
        gate_version: "1".to_owned(),
        status: GateStatus::Fail,
        severity: Severity::Error,
        evidence: vec![GateEvidence {
            code: code.to_owned(),
            message: "candidate did not preserve protected-value integrity".to_owned(),
            details: None,
        }],
        confidence: None,
    }
}

pub(super) const fn protection_reason(error: &ProtectionError) -> ReasonCode {
    match error {
        ProtectionError::ProtectedOccurrenceCount => ReasonCode::ProtectedValueChanged,
        ProtectionError::ReservedTokenInSource
        | ProtectionError::SentinelOccurrenceCount
        | ProtectionError::UnknownSentinel
        | ProtectionError::MatcherBuild
        | ProtectionError::ResourceLimit
        | ProtectionError::InvalidDeclaredTerms
        | ProtectionError::AmbiguousSurfaceMapping => ReasonCode::SentinelIntegrity,
    }
}

pub(super) fn surface_edit_cost(source: &str, candidate: &str) -> u64 {
    let substitutions = source
        .chars()
        .zip(candidate.chars())
        .filter(|(left, right)| left != right)
        .count();
    let length_delta = source.chars().count().abs_diff(candidate.chars().count());
    u64::try_from(substitutions.saturating_add(length_delta)).unwrap_or(u64::MAX)
}

fn count_lines(text: &str) -> usize {
    if text.is_empty() {
        0
    } else {
        text.matches('\n').count() + 1
    }
}

pub(super) fn validate_layout_constraints(
    unit: &RewriteUnit,
    restored: &str,
    options: &RewriteOptions,
) -> Option<(GateResult, ReasonCode)> {
    let layout = options.layout.as_ref()?;

    if let Some(char_budget) = &layout.character_budget {
        let restored_chars = restored.chars().count();
        let source_chars = unit.text.chars().count();

        if let Some(min) = char_budget.min_characters
            && restored_chars < min
        {
            return Some((
                GateResult::fail(
                    LAYOUT_GATE,
                    "character_budget_underflow",
                    "candidate character count is below minimum budget",
                ),
                ReasonCode::CharacterBudgetExceeded,
            ));
        }

        if let Some(max) = char_budget.max_characters
            && restored_chars > max
        {
            return Some((
                GateResult::fail(
                    LAYOUT_GATE,
                    "character_budget_exceeded",
                    "candidate character count exceeds maximum budget",
                ),
                ReasonCode::CharacterBudgetExceeded,
            ));
        }

        if let Some(expansion_pct) = char_budget.max_expansion_percent {
            let max_allowed = source_chars.saturating_mul(100 + usize::from(expansion_pct)) / 100;
            if restored_chars > max_allowed {
                return Some((
                    GateResult::fail(
                        LAYOUT_GATE,
                        "character_expansion_exceeded",
                        "candidate character expansion exceeds allowed percentage",
                    ),
                    ReasonCode::CharacterBudgetExceeded,
                ));
            }
        }
    }

    if let Some(line_budget) = &layout.line_budget {
        let restored_lines = count_lines(restored);
        let source_lines = count_lines(&unit.text);

        if let Some(max_lines) = line_budget.max_lines
            && restored_lines > max_lines
        {
            return Some((
                GateResult::fail(
                    LAYOUT_GATE,
                    "line_budget_exceeded",
                    "candidate line count exceeds maximum line budget",
                ),
                ReasonCode::LineBudgetExceeded,
            ));
        }

        if line_budget.preserve_line_count && restored_lines != source_lines {
            return Some((
                GateResult::fail(
                    LAYOUT_GATE,
                    "line_count_mismatch",
                    "candidate line count does not match source line count",
                ),
                ReasonCode::LineBudgetExceeded,
            ));
        }
    }

    Some((
        GateResult::pass(LAYOUT_GATE),
        ReasonCode::CharacterBudgetExceeded,
    ))
}
