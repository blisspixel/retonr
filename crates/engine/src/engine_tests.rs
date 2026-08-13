use rewrite_types::{
    Atomicity, GateStatus, RewriteMode, RewriteOptions, RewriteStatus, SemanticAssessment,
};

use crate::RewriteEngine;
use crate::engine_test_support::{
    CancellingGenerator, DuplicateCandidateIdGenerator, EmptyGenerator, ErrorGenerator,
    FixedSemantic, InvalidRankGenerator, MaskedEchoGenerator, MismatchedCandidateIdGenerator,
    MismatchedUnitGenerator, NoNewlineChange, PassStructure, document, literal_options,
    two_unit_document,
};
use crate::{
    CancellationToken, GenerationError, LiteralSemanticEvaluator, ProvidedCandidateGenerator,
};

#[test]
fn accepts_literal_punctuation_change() {
    let generator = ProvidedCandidateGenerator::new(vec!["Hello, world!".to_owned()]);
    let engine = RewriteEngine::new(&generator, &LiteralSemanticEvaluator, &PassStructure);
    let outcome = engine
        .run(
            &document("Hello world"),
            &literal_options(),
            &CancellationToken::new(),
        )
        .expect("deterministic ports succeed");
    assert_eq!(outcome.status, RewriteStatus::Rewritten);
    assert_eq!(outcome.edits[0].replacement, "Hello, world!");
    assert!(outcome.assessments[0].eligible);
}

#[test]
fn rejects_changed_protected_number() {
    let generator = ProvidedCandidateGenerator::new(vec!["Version 3".to_owned()]);
    let engine = RewriteEngine::new(&generator, &LiteralSemanticEvaluator, &PassStructure);
    let outcome = engine
        .run(
            &document("Version 2"),
            &literal_options(),
            &CancellationToken::new(),
        )
        .expect("policy rejection is an outcome");
    assert_eq!(outcome.status, RewriteStatus::Abstained);
    assert!(outcome.edits.is_empty());
    assert_eq!(
        outcome.reason,
        Some(rewrite_types::ReasonCode::ProtectedValueChanged)
    );
}

#[test]
fn ambiguous_source_mapping_is_a_safe_abstention() {
    let engine = RewriteEngine::new(&EmptyGenerator, &LiteralSemanticEvaluator, &PassStructure);
    let source = "ada@example.com $12.ada@example.com $150";
    let outcome = engine
        .run(
            &document(source),
            &literal_options(),
            &CancellationToken::new(),
        )
        .expect("source ambiguity is a policy outcome");
    assert_eq!(outcome.status, RewriteStatus::Abstained);
    assert_eq!(
        outcome.reason,
        Some(rewrite_types::ReasonCode::SentinelIntegrity)
    );
    assert!(outcome.edits.is_empty());
    assert!(outcome.assessments.is_empty());
}

#[test]
fn rejects_lexical_or_structural_change() {
    let lexical = ProvidedCandidateGenerator::new(vec!["Hello there".to_owned()]);
    let lexical_engine = RewriteEngine::new(&lexical, &LiteralSemanticEvaluator, &PassStructure);
    let lexical_outcome = lexical_engine
        .run(
            &document("Hello world"),
            &literal_options(),
            &CancellationToken::new(),
        )
        .expect("policy rejection is an outcome");
    assert_eq!(
        lexical_outcome.reason,
        Some(rewrite_types::ReasonCode::SemanticUncertain)
    );

    let structural = ProvidedCandidateGenerator::new(vec!["Hello\nworld".to_owned()]);
    let structure_engine =
        RewriteEngine::new(&structural, &LiteralSemanticEvaluator, &NoNewlineChange);
    let structure_outcome = structure_engine
        .run(
            &document("Hello world"),
            &literal_options(),
            &CancellationToken::new(),
        )
        .expect("policy rejection is an outcome");
    assert_eq!(
        structure_outcome.reason,
        Some(rewrite_types::ReasonCode::StructureChanged)
    );
}

#[test]
fn cancellation_and_atomicity_abstain() {
    let generator = ProvidedCandidateGenerator::new(vec!["Hello.".to_owned()]);
    let engine = RewriteEngine::new(&generator, &LiteralSemanticEvaluator, &PassStructure);
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let cancelled = engine
        .run(&document("Hello"), &literal_options(), &cancellation)
        .expect("cancellation is an outcome");
    assert_eq!(cancelled.reason, Some(rewrite_types::ReasonCode::Cancelled));

    let options = RewriteOptions {
        atomicity: Atomicity::Unit,
        ..literal_options()
    };
    let unsupported = engine
        .run(&document("Hello"), &options, &CancellationToken::new())
        .expect("unsupported policy is an outcome");
    assert_eq!(
        unsupported.reason,
        Some(rewrite_types::ReasonCode::UnsupportedAtomicity)
    );
}

#[test]
fn invalid_confidence_is_an_operational_error() {
    let generator = ProvidedCandidateGenerator::new(vec!["Hello.".to_owned()]);
    let engine = RewriteEngine::new(&generator, &LiteralSemanticEvaluator, &PassStructure);
    let options = RewriteOptions {
        minimum_semantic_confidence: f32::NAN,
        ..literal_options()
    };
    let error = engine
        .run(&document("Hello"), &options, &CancellationToken::new())
        .expect_err("NaN confidence is invalid configuration");
    assert_eq!(error, super::EngineError::InvalidSemanticConfidence);
}

#[test]
fn invalid_protected_term_policy_is_an_operational_error() {
    let generator = ProvidedCandidateGenerator::new(vec!["Hello.".to_owned()]);
    let engine = RewriteEngine::new(&generator, &LiteralSemanticEvaluator, &PassStructure);
    let options = RewriteOptions {
        mode: RewriteMode::Literal,
        protected_terms: vec![String::new()],
        ..RewriteOptions::default()
    };
    let error = engine
        .run(&document("Hello"), &options, &CancellationToken::new())
        .expect_err("empty protected terms are invalid configuration");
    assert_eq!(error, super::EngineError::InvalidProtectedTerms);
}

#[test]
fn generation_outcomes_are_distinct() {
    let empty = RewriteEngine::new(&EmptyGenerator, &LiteralSemanticEvaluator, &PassStructure)
        .run(
            &document("Hello"),
            &literal_options(),
            &CancellationToken::new(),
        )
        .expect("empty generation is an outcome");
    assert_eq!(empty.reason, Some(rewrite_types::ReasonCode::NoCandidate));

    let cancelled_generator = ErrorGenerator(GenerationError::Cancelled);
    let cancelled = RewriteEngine::new(
        &cancelled_generator,
        &LiteralSemanticEvaluator,
        &PassStructure,
    )
    .run(
        &document("Hello"),
        &literal_options(),
        &CancellationToken::new(),
    )
    .expect("generation cancellation is an outcome");
    assert_eq!(cancelled.reason, Some(rewrite_types::ReasonCode::Cancelled));

    let failed_generator = ErrorGenerator(GenerationError::Provider("offline".to_owned()));
    let failed = RewriteEngine::new(&failed_generator, &LiteralSemanticEvaluator, &PassStructure)
        .run(
            &document("Hello"),
            &literal_options(),
            &CancellationToken::new(),
        )
        .expect_err("provider failure is operational");
    assert_eq!(
        failed,
        super::EngineError::Generation(GenerationError::Provider("offline".to_owned()))
    );
}

#[test]
fn excessive_candidate_count_abstains() {
    let candidates = (0..=super::MAX_GENERATED_CANDIDATES)
        .map(|_| "Hello.".to_owned())
        .collect();
    let generator = ProvidedCandidateGenerator::new(candidates);
    let outcome = RewriteEngine::new(&generator, &LiteralSemanticEvaluator, &PassStructure)
        .run(
            &document("Hello"),
            &literal_options(),
            &CancellationToken::new(),
        )
        .expect("excessive candidate count is an outcome");
    assert_eq!(
        outcome.reason,
        Some(rewrite_types::ReasonCode::InvalidCandidate)
    );
    assert!(outcome.assessments.is_empty());
}

#[test]
fn validates_units_and_masked_candidates() {
    let mismatched = RewriteEngine::new(
        &MismatchedUnitGenerator,
        &LiteralSemanticEvaluator,
        &PassStructure,
    )
    .run(
        &document("Hello"),
        &literal_options(),
        &CancellationToken::new(),
    )
    .expect("unit mismatch is an outcome");
    assert_eq!(
        mismatched.reason,
        Some(rewrite_types::ReasonCode::InvalidCandidate)
    );
    assert!(mismatched.assessments.is_empty());

    let masked = RewriteEngine::new(
        &MaskedEchoGenerator,
        &LiteralSemanticEvaluator,
        &PassStructure,
    )
    .run(
        &document("Version 2"),
        &literal_options(),
        &CancellationToken::new(),
    )
    .expect("valid masked candidate succeeds");
    assert_eq!(masked.status, RewriteStatus::Rewritten);
    assert_eq!(masked.edits[0].replacement, "Version 2!");
}

#[test]
fn invalid_candidate_identity_scope_is_a_validated_ineligible_record() {
    let outcome = RewriteEngine::new(
        &MismatchedCandidateIdGenerator,
        &LiteralSemanticEvaluator,
        &PassStructure,
    )
    .run(
        &document("Hello"),
        &literal_options(),
        &CancellationToken::new(),
    )
    .expect("invalid identity is a safe outcome");
    assert_eq!(
        outcome.reason,
        Some(rewrite_types::ReasonCode::InvalidCandidate)
    );
    assert!(outcome.assessments.is_empty());
    assert!(outcome.selected_candidates.is_empty());

    let duplicate = RewriteEngine::new(
        &DuplicateCandidateIdGenerator,
        &LiteralSemanticEvaluator,
        &PassStructure,
    )
    .run(
        &document("Hello"),
        &literal_options(),
        &CancellationToken::new(),
    )
    .expect("duplicate identities are a safe outcome");
    assert_eq!(
        duplicate.reason,
        Some(rewrite_types::ReasonCode::InvalidCandidate)
    );
    assert!(duplicate.assessments.is_empty());
    assert!(duplicate.selected_candidates.is_empty());
}

#[test]
fn protected_gate_reports_redacted_typed_counts() {
    let source = "Email Ada at ada@example.com and pay $12.";
    let generator = ProvidedCandidateGenerator::new(vec![format!("{source}!")]);
    let options = RewriteOptions {
        mode: RewriteMode::Literal,
        atomicity: Atomicity::Document,
        protected_terms: vec!["Ada".to_owned()],
        minimum_semantic_confidence: 0.95,
    };
    let outcome = RewriteEngine::new(&generator, &LiteralSemanticEvaluator, &PassStructure)
        .run(&document(source), &options, &CancellationToken::new())
        .expect("exact invariants pass");
    let gate = outcome.assessments[0]
        .gates
        .iter()
        .find(|gate| gate.gate_id == "protected_values")
        .expect("protected gate exists");
    let Some(rewrite_types::GateEvidenceDetails::InvariantSummary(summary)) =
        gate.evidence[0].details
    else {
        panic!("typed invariant counts are retained");
    };
    assert_eq!(summary.declared_terms, 1);
    assert_eq!(summary.emails, 1);
    assert_eq!(summary.numbers, 1);
    assert_eq!(summary.total, 3);
    let encoded = serde_json::to_string(gate).expect("gate serializes");
    assert!(!encoded.contains("Ada"));
    assert!(!encoded.contains("example.com"));
}

#[test]
fn reserved_source_token_abstains_without_generation() {
    let outcome = RewriteEngine::new(&EmptyGenerator, &LiteralSemanticEvaluator, &PassStructure)
        .run(
            &document("{{PROTECTED_URL_0001}}"),
            &literal_options(),
            &CancellationToken::new(),
        )
        .expect("reserved source token is a safe abstention");
    assert_eq!(
        outcome.reason,
        Some(rewrite_types::ReasonCode::SentinelIntegrity)
    );
    assert!(outcome.edits.is_empty());
}

#[test]
fn invalid_candidate_rank_abstains() {
    let outcome = RewriteEngine::new(
        &InvalidRankGenerator,
        &LiteralSemanticEvaluator,
        &PassStructure,
    )
    .run(
        &document("Hello"),
        &literal_options(),
        &CancellationToken::new(),
    )
    .expect("invalid candidate metadata is an outcome");
    assert_eq!(
        outcome.reason,
        Some(rewrite_types::ReasonCode::InvalidCandidate)
    );
    assert!(!outcome.assessments[0].eligible);
}

#[test]
fn semantic_failure_and_low_confidence_abstain() {
    let generator = ProvidedCandidateGenerator::new(vec!["Hello.".to_owned()]);
    let fail_semantic = FixedSemantic(SemanticAssessment {
        status: GateStatus::Fail,
        confidence: Some(1.0),
        evidence: vec![rewrite_types::SemanticEvidence::new(
            rewrite_types::SemanticEvidenceCode::LiteralTokensChanged,
            None,
        )],
    });
    let failed = RewriteEngine::new(&generator, &fail_semantic, &PassStructure)
        .run(
            &document("Hello"),
            &literal_options(),
            &CancellationToken::new(),
        )
        .expect("semantic rejection is an outcome");
    assert_eq!(
        failed.reason,
        Some(rewrite_types::ReasonCode::SemanticMismatch)
    );

    let low_confidence = FixedSemantic(SemanticAssessment {
        status: GateStatus::Pass,
        confidence: Some(0.5),
        evidence: vec![rewrite_types::SemanticEvidence::new(
            rewrite_types::SemanticEvidenceCode::LiteralTokensEqual,
            None,
        )],
    });
    let uncertain = RewriteEngine::new(&generator, &low_confidence, &PassStructure)
        .run(
            &document("Hello"),
            &literal_options(),
            &CancellationToken::new(),
        )
        .expect("low confidence is an outcome");
    assert_eq!(
        uncertain.reason,
        Some(rewrite_types::ReasonCode::SemanticUncertain)
    );
    assert_eq!(
        uncertain.assessments[0]
            .gates
            .last()
            .map(|gate| gate.status),
        Some(GateStatus::Uncertain)
    );
}

#[test]
fn malformed_semantic_evidence_fails_closed_without_retaining_supplied_findings() {
    let generator = ProvidedCandidateGenerator::new(vec!["Hello.".to_owned()]);
    let malformed = FixedSemantic(SemanticAssessment {
        status: GateStatus::Pass,
        confidence: Some(f32::NAN),
        evidence: vec![
            rewrite_types::SemanticEvidence::new(
                rewrite_types::SemanticEvidenceCode::LiteralTokensEqual,
                None,
            );
            rewrite_types::MAX_SEMANTIC_EVIDENCE_ITEMS + 1
        ],
    });
    let outcome = RewriteEngine::new(&generator, &malformed, &PassStructure)
        .run(
            &document("Hello"),
            &literal_options(),
            &CancellationToken::new(),
        )
        .expect("malformed evidence is a safe outcome");
    assert_eq!(
        outcome.reason,
        Some(rewrite_types::ReasonCode::SemanticUncertain)
    );
    let gate = outcome.assessments[0]
        .gates
        .last()
        .expect("semantic gate is retained");
    assert_eq!(gate.status, GateStatus::Uncertain);
    assert_eq!(gate.confidence, None);
    assert_eq!(gate.evidence.len(), 1);
    assert_eq!(gate.evidence[0].code, "invalid_evaluator_evidence");
    let encoded = serde_json::to_string(&outcome.assessments).expect("assessment serializes");
    assert!(!encoded.contains("literal_tokens_equal"));
    assert!(gate.validate().is_ok());
}

#[test]
fn cancellation_after_partial_work_discards_all_edits() {
    let outcome = RewriteEngine::new(
        &CancellingGenerator,
        &LiteralSemanticEvaluator,
        &PassStructure,
    )
    .run(
        &two_unit_document(),
        &literal_options(),
        &CancellationToken::new(),
    )
    .expect("cooperative cancellation is an outcome");
    assert_eq!(outcome.reason, Some(rewrite_types::ReasonCode::Cancelled));
    assert!(outcome.edits.is_empty());
}

#[test]
fn unchanged_candidate_has_no_edit() {
    let generator = ProvidedCandidateGenerator::new(vec!["Hello".to_owned()]);
    let engine = RewriteEngine::new(&generator, &LiteralSemanticEvaluator, &PassStructure);
    let outcome = engine
        .run(
            &document("Hello"),
            &literal_options(),
            &CancellationToken::new(),
        )
        .expect("deterministic ports succeed");
    assert_eq!(outcome.status, RewriteStatus::UnchangedNoEligibleContent);
    assert!(outcome.edits.is_empty());
    assert_eq!(
        outcome.assessments[0].gates.last().map(|gate| gate.status),
        Some(GateStatus::Pass)
    );
}

#[test]
fn abstention_reason_priority_is_order_independent() {
    let ordered = [
        rewrite_types::ReasonCode::SentinelIntegrity,
        rewrite_types::ReasonCode::ProtectedValueChanged,
        rewrite_types::ReasonCode::UnsafeText,
        rewrite_types::ReasonCode::StructureChanged,
        rewrite_types::ReasonCode::SemanticMismatch,
        rewrite_types::ReasonCode::SemanticUncertain,
        rewrite_types::ReasonCode::InvalidCandidate,
        rewrite_types::ReasonCode::NoCandidate,
        rewrite_types::ReasonCode::ReassemblyVerification,
        rewrite_types::ReasonCode::Cancelled,
        rewrite_types::ReasonCode::UnsupportedAtomicity,
    ];
    for pair in ordered.windows(2) {
        assert_eq!(
            crate::policy::preferred_reason(Some(pair[1]), pair[0]),
            pair[0]
        );
        assert_eq!(
            crate::policy::preferred_reason(Some(pair[0]), pair[1]),
            pair[0]
        );
        assert!(crate::policy::reason_priority(pair[0]) < crate::policy::reason_priority(pair[1]));
    }
}
