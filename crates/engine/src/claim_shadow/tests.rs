use rewrite_types::{
    ClaimComparisonCounts, ClaimComparisonEvidence, Digest, GateStatus, RewriteStatus, Severity,
};

use super::{CLAIM_SHADOW_GATE, ClaimShadowObserver};
use crate::engine_test_support::{PassStructure, document, literal_options};
use crate::{
    CancellationToken, LiteralSemanticEvaluator, ProvidedCandidateGenerator, RewriteEngine,
};

struct FixedShadow(Option<ClaimComparisonEvidence>);

impl ClaimShadowObserver for FixedShadow {
    fn observe(
        &self,
        _unit_id: &rewrite_types::RewriteUnitId,
        _source: &str,
        _candidate: &str,
    ) -> Option<ClaimComparisonEvidence> {
        self.0.clone()
    }
}

fn counts(difference: bool, uncertainty: bool) -> ClaimComparisonCounts {
    ClaimComparisonCounts {
        source_claims: 1,
        candidate_claims: if difference { 2 } else { 1 },
        aligned_claims: 1,
        missing_claims: 0,
        novel_claims: u32::from(difference),
        polarity_conflicts: 0,
        modality_conflicts: 0,
        relationship_conflicts: 0,
        source_unknown_polarity: u32::from(uncertainty),
        candidate_unknown_polarity: 0,
        source_unknown_modality: 0,
        candidate_unknown_modality: 0,
        source_below_confidence: 0,
        candidate_below_confidence: 0,
    }
}

fn comparison(
    document: &rewrite_types::DocumentIr,
    source: &str,
    candidate: &str,
    difference: bool,
    uncertainty: bool,
) -> ClaimComparisonEvidence {
    ClaimComparisonEvidence::new(
        Digest::sha256(b"manifest"),
        document.rewrite_units[0].id.clone(),
        Digest::sha256(source.as_bytes()),
        u64::try_from(source.len()).expect("fixture source fits"),
        Digest::sha256(candidate.as_bytes()),
        u64::try_from(candidate.len()).expect("fixture candidate fits"),
        Digest::sha256(b"source evidence"),
        Digest::sha256(b"candidate evidence"),
        900_000,
        counts(difference, uncertainty),
    )
    .expect("valid shadow fixture")
}

fn shadow_gate_of(outcome: &crate::EngineOutcome) -> Option<&rewrite_types::GateResult> {
    outcome.assessments.first().and_then(|assessment| {
        assessment
            .gates
            .iter()
            .find(|gate| gate.gate_id == CLAIM_SHADOW_GATE)
    })
}

#[test]
fn shadow_conflict_cannot_reject_a_literal_pass() {
    let source = "Hello world";
    let candidate = "Hello, world!";
    let parsed = document(source);
    let observer = FixedShadow(Some(comparison(&parsed, source, candidate, true, false)));
    let generator = ProvidedCandidateGenerator::new(vec![candidate.to_owned()]);
    let outcome = RewriteEngine::new(&generator, &LiteralSemanticEvaluator, &PassStructure)
        .with_claim_shadow(&observer)
        .run(&parsed, &literal_options(), &CancellationToken::new())
        .expect("shadow observation is not operational failure");
    assert_eq!(outcome.status, RewriteStatus::Rewritten);
    assert!(outcome.assessments[0].eligible);
    let shadow = shadow_gate_of(&outcome).expect("shadow gate is retained");
    assert_eq!(shadow.status, GateStatus::Fail);
    assert_eq!(shadow.severity, Severity::Info);
    assert_eq!(shadow.evidence[0].code, "claim_comparison_conflict");
}

#[test]
fn literal_failure_still_abstains_when_shadow_claims_pass() {
    let source = "Hello world";
    let candidate = "Hello there";
    let parsed = document(source);
    let observer = FixedShadow(Some(comparison(&parsed, source, candidate, false, false)));
    let generator = ProvidedCandidateGenerator::new(vec![candidate.to_owned()]);
    let outcome = RewriteEngine::new(&generator, &LiteralSemanticEvaluator, &PassStructure)
        .with_claim_shadow(&observer)
        .run(&parsed, &literal_options(), &CancellationToken::new())
        .expect("shadow observation is not operational failure");
    assert_eq!(outcome.status, RewriteStatus::Abstained);
    assert_eq!(
        outcome.reason,
        Some(rewrite_types::ReasonCode::SemanticUncertain)
    );
    assert!(!outcome.assessments[0].eligible);
    let shadow = shadow_gate_of(&outcome).expect("shadow gate is retained");
    assert_eq!(shadow.status, GateStatus::Pass);
    assert_eq!(shadow.severity, Severity::Info);
    assert_eq!(shadow.evidence[0].code, "claim_comparison_preserved");
}

#[test]
fn absent_shadow_observation_leaves_the_current_cascade_unchanged() {
    let generator = ProvidedCandidateGenerator::new(vec!["Hello, world!".to_owned()]);
    let with_idle = RewriteEngine::new(&generator, &LiteralSemanticEvaluator, &PassStructure)
        .with_claim_shadow(&FixedShadow(None))
        .run(
            &document("Hello world"),
            &literal_options(),
            &CancellationToken::new(),
        )
        .expect("idle observer is omitted");
    let without = RewriteEngine::new(&generator, &LiteralSemanticEvaluator, &PassStructure)
        .run(
            &document("Hello world"),
            &literal_options(),
            &CancellationToken::new(),
        )
        .expect("current cascade is unchanged");
    assert_eq!(with_idle.status, without.status);
    assert_eq!(with_idle.edits, without.edits);
    assert!(shadow_gate_of(&with_idle).is_none());
    assert!(shadow_gate_of(&without).is_none());
}

#[test]
fn mismatched_shadow_evidence_is_recorded_without_authority() {
    let source = "Hello world";
    let candidate = "Hello, world!";
    let parsed = document(source);
    let observer = FixedShadow(Some(comparison(
        &document("Other"),
        "Other",
        "Other.",
        false,
        false,
    )));
    let generator = ProvidedCandidateGenerator::new(vec![candidate.to_owned()]);
    let outcome = RewriteEngine::new(&generator, &LiteralSemanticEvaluator, &PassStructure)
        .with_claim_shadow(&observer)
        .run(&parsed, &literal_options(), &CancellationToken::new())
        .expect("mismatched shadow is recorded");
    assert_eq!(outcome.status, RewriteStatus::Rewritten);
    assert!(outcome.assessments[0].eligible);
    let shadow = shadow_gate_of(&outcome).expect("shadow gate is retained");
    assert_eq!(shadow.status, GateStatus::Uncertain);
    assert_eq!(shadow.severity, Severity::Info);
    assert_eq!(shadow.evidence[0].code, "invalid_shadow_claim_evidence");
    assert!(shadow.evidence[0].details.is_none());
}

#[test]
fn shadow_uncertainty_is_informational() {
    let source = "Hello world";
    let candidate = "Hello, world!";
    let parsed = document(source);
    let observer = FixedShadow(Some(comparison(&parsed, source, candidate, false, true)));
    let generator = ProvidedCandidateGenerator::new(vec![candidate.to_owned()]);
    let outcome = RewriteEngine::new(&generator, &LiteralSemanticEvaluator, &PassStructure)
        .with_claim_shadow(&observer)
        .run(&parsed, &literal_options(), &CancellationToken::new())
        .expect("shadow uncertainty is informational");
    assert_eq!(outcome.status, RewriteStatus::Rewritten);
    let shadow = shadow_gate_of(&outcome).expect("shadow gate is retained");
    assert_eq!(shadow.status, GateStatus::Uncertain);
    assert_eq!(shadow.evidence[0].code, "claim_comparison_uncertain");
}
