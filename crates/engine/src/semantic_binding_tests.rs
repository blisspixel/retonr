use rewrite_types::{
    ClaimComparisonCounts, ClaimComparisonEvidence, Digest, GateStatus, SemanticAssessment,
    SemanticEvidence, SemanticEvidenceCode, SemanticEvidenceDetails,
};

use crate::engine_test_support::{FixedSemantic, PassStructure, document, literal_options};
use crate::{CancellationToken, ProvidedCandidateGenerator, RewriteEngine};

#[test]
fn semantic_comparison_replay_across_inputs_fails_closed() {
    let source = "Hello";
    let candidate = "Hello.";
    let bound_to_other_input = ClaimComparisonEvidence::new(
        Digest::sha256(b"manifest"),
        document("Other").rewrite_units[0].id.clone(),
        Digest::sha256(b"Other"),
        5,
        Digest::sha256(b"Other."),
        6,
        Digest::sha256(b"source evidence"),
        Digest::sha256(b"candidate evidence"),
        900_000,
        ClaimComparisonCounts {
            source_claims: 1,
            candidate_claims: 1,
            aligned_claims: 1,
            missing_claims: 0,
            novel_claims: 0,
            polarity_conflicts: 0,
            modality_conflicts: 0,
            relationship_conflicts: 0,
            source_unknown_polarity: 0,
            candidate_unknown_polarity: 0,
            source_unknown_modality: 0,
            candidate_unknown_modality: 0,
            source_below_confidence: 0,
            candidate_below_confidence: 0,
        },
    )
    .expect("valid evidence for a different invocation");
    let semantic = FixedSemantic(SemanticAssessment {
        status: GateStatus::Pass,
        confidence: Some(1.0),
        evidence: vec![SemanticEvidence::new(
            SemanticEvidenceCode::ClaimComparisonPreserved,
            Some(SemanticEvidenceDetails::ClaimComparison(
                bound_to_other_input,
            )),
        )],
    });
    let generator = ProvidedCandidateGenerator::new(vec![candidate.to_owned()]);
    let outcome = RewriteEngine::new(&generator, &semantic, &PassStructure)
        .run(
            &document(source),
            &literal_options(),
            &CancellationToken::new(),
        )
        .expect("replayed evidence is a safe outcome");
    assert_eq!(
        outcome.reason,
        Some(rewrite_types::ReasonCode::SemanticUncertain)
    );
    let gate = outcome.assessments[0]
        .gates
        .last()
        .expect("semantic gate is retained");
    assert_eq!(gate.evidence[0].code, "invalid_evaluator_evidence");
    assert_eq!(gate.evidence.len(), 1);
}
