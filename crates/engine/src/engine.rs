use rewrite_types::{
    AcceptedEdit, Atomicity, CandidateAssessment, CandidateTextKind, DocumentError, DocumentIr,
    GateEvidence, GateEvidenceDetails, GateResult, GateStatus, GeneratedCandidate,
    InvariantEvidenceSummary, ReasonCode, RewriteOptions, RewriteStatus, RewriteUnit,
    RewriteUnitId, Severity,
};
use thiserror::Error;

use crate::policy::{
    candidate_batch_reason, candidate_contract_is_valid, preferred_reason, validate_rewrite_options,
};
use crate::selection::compare_candidates;
use crate::{
    CancellationToken, CandidateGenerator, GenerationError, GenerationRequest, ProtectionError,
    ProtectionPlan, SemanticEvaluator, StructureAssessment, StructureValidator,
};

const UNIT_GATE: &str = "candidate_unit";
const CANDIDATE_GATE: &str = "candidate_contract";
const SENTINEL_GATE: &str = "sentinel_integrity";
const PROTECTED_GATE: &str = "protected_values";
const SEMANTIC_GATE: &str = "semantic_fidelity";

/// Maximum candidates accepted from one generation request.
pub const MAX_GENERATED_CANDIDATES: usize = 16;
/// Maximum generated candidate text size for one rewrite unit.
pub const MAX_GENERATED_TEXT_BYTES: usize = 16 * 1024 * 1024;
/// Deterministic result produced before an adapter commits edits.
#[derive(Clone, Debug, PartialEq)]
pub struct EngineOutcome {
    /// Transaction status.
    pub status: RewriteStatus,
    /// Stable reason when the engine abstained.
    pub reason: Option<ReasonCode>,
    /// Fully restored edits accepted by every required gate.
    pub edits: Vec<AcceptedEdit>,
    /// Redacted assessment records for all generated candidates.
    pub assessments: Vec<CandidateAssessment>,
    /// Selected candidate identifiers in rewrite-unit order.
    pub selected_candidates: Vec<rewrite_types::CandidateId>,
}

impl EngineOutcome {
    fn abstained(reason: ReasonCode, assessments: Vec<CandidateAssessment>) -> Self {
        Self {
            status: RewriteStatus::Abstained,
            reason: Some(reason),
            edits: Vec::new(),
            assessments,
            selected_candidates: Vec::new(),
        }
    }
}

/// Operational failure that prevents a safe engine decision.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum EngineError {
    /// Minimum semantic confidence is not finite or outside zero to one.
    #[error("minimum semantic confidence must be finite and between zero and one")]
    InvalidSemanticConfidence,
    /// Protected-term configuration is empty, oversized, or too numerous.
    #[error("protected terms violate count or byte limits")]
    InvalidProtectedTerms,
    /// Document IR violates its versioned structural contract.
    #[error(transparent)]
    InvalidDocument(#[from] DocumentError),
    /// Candidate generation backend failed.
    #[error(transparent)]
    Generation(#[from] GenerationError),
    /// Source protection planning failed.
    #[error(transparent)]
    Protection(#[from] ProtectionError),
}

/// Orchestrates candidate generation, independent gates, and deterministic
/// selection without owning document-format reconstruction.
pub struct RewriteEngine<'a> {
    generator: &'a dyn CandidateGenerator,
    semantic: &'a dyn SemanticEvaluator,
    structure: &'a dyn StructureValidator,
}

impl<'a> RewriteEngine<'a> {
    /// Creates an engine from independently replaceable ports.
    #[must_use]
    pub const fn new(
        generator: &'a dyn CandidateGenerator,
        semantic: &'a dyn SemanticEvaluator,
        structure: &'a dyn StructureValidator,
    ) -> Self {
        Self {
            generator,
            semantic,
            structure,
        }
    }

    /// Evaluates a document and returns edits only when all units are eligible.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError`] when protection planning or the generation port
    /// fails operationally. Policy rejections are represented as abstentions.
    pub fn run(
        &self,
        document: &DocumentIr,
        options: &RewriteOptions,
        cancellation: &CancellationToken,
    ) -> Result<EngineOutcome, EngineError> {
        validate_rewrite_options(options)?;
        document.validate()?;
        if options.atomicity != Atomicity::Document {
            return Ok(EngineOutcome::abstained(
                ReasonCode::UnsupportedAtomicity,
                Vec::new(),
            ));
        }
        if cancellation.is_cancelled() {
            return Ok(EngineOutcome::abstained(ReasonCode::Cancelled, Vec::new()));
        }
        if document.rewrite_units.is_empty() {
            return Ok(EngineOutcome {
                status: RewriteStatus::UnchangedNoEligibleContent,
                reason: None,
                edits: Vec::new(),
                assessments: Vec::new(),
                selected_candidates: Vec::new(),
            });
        }

        let mut edits = Vec::with_capacity(document.rewrite_units.len());
        let mut assessments = Vec::new();
        let mut selected_candidates = Vec::with_capacity(document.rewrite_units.len());

        for unit in &document.rewrite_units {
            if cancellation.is_cancelled() {
                return Ok(EngineOutcome::abstained(ReasonCode::Cancelled, assessments));
            }

            let protection = match ProtectionPlan::build(&unit.text, &options.protected_terms) {
                Ok(protection) => protection,
                Err(
                    ProtectionError::ReservedTokenInSource
                    | ProtectionError::AmbiguousSurfaceMapping,
                ) => {
                    return Ok(EngineOutcome::abstained(
                        ReasonCode::SentinelIntegrity,
                        assessments,
                    ));
                }
                Err(error) => return Err(error.into()),
            };
            let request = GenerationRequest {
                unit_id: unit.id.clone(),
                masked_source: protection.masked_source().to_owned(),
                protected_values: protection.values().to_vec(),
                mode: options.mode,
            };
            let candidates = match self.generator.generate(&request, cancellation) {
                Ok(candidates) => candidates,
                Err(GenerationError::Cancelled) => {
                    return Ok(EngineOutcome::abstained(ReasonCode::Cancelled, assessments));
                }
                Err(error) => return Err(error.into()),
            };
            if let Some(reason) = candidate_batch_reason(&unit.id, &candidates) {
                return Ok(EngineOutcome::abstained(reason, assessments));
            }

            let mut eligible = Vec::new();
            let mut unit_reason = None;
            for candidate in candidates {
                let evaluated = self.assess_candidate(unit, candidate, &protection, options);
                if evaluated.assessment.eligible {
                    eligible.push(evaluated.candidate);
                } else if let Some(reason) = evaluated.reason {
                    unit_reason = Some(preferred_reason(unit_reason, reason));
                }
                assessments.push(evaluated.assessment);
            }

            let Some(selected_item) = eligible
                .iter()
                .max_by(|left, right| compare_candidates(&left.generated, &right.generated))
            else {
                return Ok(EngineOutcome::abstained(
                    unit_reason.unwrap_or(ReasonCode::NoCandidate),
                    assessments,
                ));
            };
            let selected_id = selected_item.generated.id.clone();
            if selected_item.restored != unit.text {
                edits.push(AcceptedEdit {
                    unit_id: unit.id.clone(),
                    replacement: selected_item.restored.clone(),
                });
            }
            selected_candidates.push(selected_id);
        }

        let status = if edits.is_empty() {
            RewriteStatus::UnchangedNoEligibleContent
        } else {
            RewriteStatus::Rewritten
        };
        Ok(EngineOutcome {
            status,
            reason: None,
            edits,
            assessments,
            selected_candidates,
        })
    }

    fn assess_candidate(
        &self,
        unit: &RewriteUnit,
        candidate: GeneratedCandidate,
        protection: &ProtectionPlan,
        options: &RewriteOptions,
    ) -> EvaluatedCandidate {
        let mut gates = Vec::new();
        let mut candidate = match validate_candidate_metadata(unit, candidate, &mut gates) {
            Ok(candidate) => candidate,
            Err(evaluated) => return *evaluated,
        };

        let masked = match candidate.text_kind {
            CandidateTextKind::Masked => protection
                .validate_masked(&candidate.text)
                .map(|()| candidate.text.clone()),
            CandidateTextKind::Raw => protection.mask_raw_candidate(&candidate.text),
        };
        let masked = match masked {
            Ok(masked) => {
                gates.push(GateResult::pass(SENTINEL_GATE));
                masked
            }
            Err(error) => {
                gates.push(protection_failure(SENTINEL_GATE, &error));
                return ineligible(
                    candidate,
                    unit.id.clone(),
                    String::new(),
                    gates,
                    protection_reason(&error),
                );
            }
        };

        let restored = match protection.restore(&masked) {
            Ok(restored) => {
                gates.push(protected_values_pass(protection));
                restored
            }
            Err(error) => {
                gates.push(protection_failure(PROTECTED_GATE, &error));
                return ineligible(
                    candidate,
                    unit.id.clone(),
                    String::new(),
                    gates,
                    protection_reason(&error),
                );
            }
        };

        let structure = self.structure.validate(unit, &restored);
        let (structure_gate, structure_reason) = crate::structure::retained_gate(structure);
        let structure_passed = structure == StructureAssessment::Preserved;
        gates.push(structure_gate);
        if !structure_passed {
            return ineligible(
                candidate,
                unit.id.clone(),
                restored,
                gates,
                structure_reason,
            );
        }

        let (semantic_gate, semantic_passed, semantic_reason) =
            self.semantic_gate(&unit.id, &unit.text, &restored, options);
        gates.push(semantic_gate);
        if !semantic_passed {
            return ineligible(candidate, unit.id.clone(), restored, gates, semantic_reason);
        }

        candidate.rank.edit_cost = surface_edit_cost(&unit.text, &restored);
        EvaluatedCandidate {
            assessment: CandidateAssessment {
                candidate_id: candidate.id.clone(),
                unit_id: candidate.unit_id.clone(),
                eligible: true,
                gates,
            },
            candidate: EligibleCandidate {
                generated: candidate,
                restored,
            },
            reason: None,
        }
    }

    fn semantic_gate(
        &self,
        unit_id: &RewriteUnitId,
        source: &str,
        candidate: &str,
        options: &RewriteOptions,
    ) -> (GateResult, bool, ReasonCode) {
        let semantic = self.semantic.evaluate(source, candidate, options.mode);
        if semantic.validate().is_err()
            || !crate::semantic::evidence_matches(&semantic, unit_id, source, candidate)
        {
            return invalid_semantic_gate();
        }
        let confidence_passed = semantic
            .confidence
            .is_some_and(|value| value >= options.minimum_semantic_confidence);
        let passed = semantic.status == GateStatus::Pass && confidence_passed;
        let status = if semantic.status == GateStatus::Pass && !confidence_passed {
            GateStatus::Uncertain
        } else {
            semantic.status
        };
        let reason = if status == GateStatus::Fail {
            ReasonCode::SemanticMismatch
        } else {
            ReasonCode::SemanticUncertain
        };
        let gate = GateResult {
            gate_id: SEMANTIC_GATE.to_owned(),
            gate_version: self.semantic.id().to_owned(),
            status,
            severity: Severity::Error,
            evidence: semantic
                .evidence
                .into_iter()
                .map(semantic_evidence)
                .collect(),
            confidence: semantic.confidence,
        };
        if gate.validate().is_err() {
            invalid_semantic_gate()
        } else {
            (gate, passed, reason)
        }
    }
}

fn semantic_evidence(evidence: rewrite_types::SemanticEvidence) -> GateEvidence {
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

fn invalid_semantic_gate() -> (GateResult, bool, ReasonCode) {
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

fn protected_values_pass(protection: &ProtectionPlan) -> GateResult {
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

struct EligibleCandidate {
    generated: GeneratedCandidate,
    restored: String,
}

struct EvaluatedCandidate {
    assessment: CandidateAssessment,
    candidate: EligibleCandidate,
    reason: Option<ReasonCode>,
}

fn validate_candidate_metadata(
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

fn ineligible(
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

fn protection_failure(gate_id: &str, error: &ProtectionError) -> GateResult {
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

const fn protection_reason(error: &ProtectionError) -> ReasonCode {
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

fn surface_edit_cost(source: &str, candidate: &str) -> u64 {
    let substitutions = source
        .chars()
        .zip(candidate.chars())
        .filter(|(left, right)| left != right)
        .count();
    let length_delta = source.chars().count().abs_diff(candidate.chars().count());
    u64::try_from(substitutions.saturating_add(length_delta)).unwrap_or(u64::MAX)
}
