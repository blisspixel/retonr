use rewrite_types::{
    AcceptedEdit, Atomicity, CandidateAssessment, CandidateTextKind, DocumentError, DocumentIr,
    GateResult, GateStatus, GeneratedCandidate, ReasonCode, RewriteOptions, RewriteStatus,
    RewriteUnit, RewriteUnitId, Severity,
};
use thiserror::Error;

use crate::policy::{candidate_batch_reason, preferred_reason, validate_rewrite_options};
use crate::selection::compare_candidates;
use crate::{
    CancellationToken, CandidateGenerator, ClaimShadowObserver, GenerationError, GenerationRequest,
    ProtectionError, ProtectionPlan, SemanticEvaluator, StructureAssessment, StructureValidator,
};

mod support;

use support::{
    EligibleCandidate, EvaluatedCandidate, PROTECTED_GATE, SEMANTIC_GATE, SENTINEL_GATE,
    UnitProgress, ineligible, invalid_semantic_gate, protected_values_pass, protection_failure,
    protection_reason, semantic_evidence, surface_edit_cost, validate_candidate_metadata,
};

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
    claim_shadow: Option<&'a dyn ClaimShadowObserver>,
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
            claim_shadow: None,
        }
    }

    /// Records independently produced claim comparison without changing eligibility.
    ///
    /// The observer cannot authorize a rewrite. Literal-token failure still
    /// abstains, and a claim conflict cannot reject a candidate that already
    /// passed the hard gates.
    #[must_use]
    pub const fn with_claim_shadow(mut self, observer: &'a dyn ClaimShadowObserver) -> Self {
        self.claim_shadow = Some(observer);
        self
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
            match self.evaluate_unit(unit, options, cancellation, assessments)? {
                Err(outcome) => return Ok(outcome),
                Ok(progress) => {
                    assessments = progress.assessments;
                    if let Some(replacement) = progress.replacement {
                        edits.push(AcceptedEdit {
                            unit_id: unit.id.clone(),
                            replacement,
                        });
                    }
                    selected_candidates.push(progress.selected);
                }
            }
        }

        if cancellation.is_cancelled() {
            return Ok(EngineOutcome::abstained(ReasonCode::Cancelled, assessments));
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

    fn evaluate_unit(
        &self,
        unit: &RewriteUnit,
        options: &RewriteOptions,
        cancellation: &CancellationToken,
        mut assessments: Vec<CandidateAssessment>,
    ) -> Result<Result<UnitProgress, EngineOutcome>, EngineError> {
        if cancellation.is_cancelled() {
            return Ok(Err(EngineOutcome::abstained(
                ReasonCode::Cancelled,
                assessments,
            )));
        }

        let protection = match ProtectionPlan::build(&unit.text, &options.protected_terms) {
            Ok(protection) => protection,
            Err(
                ProtectionError::ReservedTokenInSource | ProtectionError::AmbiguousSurfaceMapping,
            ) => {
                return Ok(Err(EngineOutcome::abstained(
                    ReasonCode::SentinelIntegrity,
                    assessments,
                )));
            }
            Err(error) => return Err(error.into()),
        };
        if cancellation.is_cancelled() {
            return Ok(Err(EngineOutcome::abstained(
                ReasonCode::Cancelled,
                assessments,
            )));
        }
        let request = GenerationRequest {
            unit_id: unit.id.clone(),
            masked_source: protection.masked_source().to_owned(),
            protected_values: protection.values().to_vec(),
            mode: options.mode,
        };
        let candidates = match self.generator.generate(&request, cancellation) {
            Ok(candidates) => candidates,
            Err(GenerationError::Cancelled) => {
                return Ok(Err(EngineOutcome::abstained(
                    ReasonCode::Cancelled,
                    assessments,
                )));
            }
            Err(error) => return Err(error.into()),
        };
        if cancellation.is_cancelled() {
            return Ok(Err(EngineOutcome::abstained(
                ReasonCode::Cancelled,
                assessments,
            )));
        }
        if let Some(reason) = candidate_batch_reason(&unit.id, &candidates) {
            return Ok(Err(EngineOutcome::abstained(reason, assessments)));
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
            if cancellation.is_cancelled() {
                return Ok(Err(EngineOutcome::abstained(
                    ReasonCode::Cancelled,
                    assessments,
                )));
            }
        }

        let Some(selected_item) = eligible
            .iter()
            .max_by(|left, right| compare_candidates(&left.generated, &right.generated))
        else {
            return Ok(Err(EngineOutcome::abstained(
                unit_reason.unwrap_or(ReasonCode::NoCandidate),
                assessments,
            )));
        };
        Ok(Ok(UnitProgress {
            selected: selected_item.generated.id.clone(),
            replacement: (selected_item.restored != unit.text)
                .then(|| selected_item.restored.clone()),
            assessments,
        }))
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
            self.attach_claim_shadow(&unit.id, &unit.text, &restored, &mut gates);
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
        self.attach_claim_shadow(&unit.id, &unit.text, &restored, &mut gates);
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

    fn attach_claim_shadow(
        &self,
        unit_id: &RewriteUnitId,
        source: &str,
        candidate: &str,
        gates: &mut Vec<GateResult>,
    ) {
        if let Some(observer) = self.claim_shadow
            && let Some(gate) =
                crate::claim_shadow::shadow_gate(observer, unit_id, source, candidate)
        {
            gates.push(gate);
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
