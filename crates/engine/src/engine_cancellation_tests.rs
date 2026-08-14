use rewrite_types::RewriteStatus;

use crate::RewriteEngine;
use crate::engine_test_support::{
    CancelOnEvaluate, CancellingGenerator, PassStructure, document, literal_options,
    two_unit_document,
};
use crate::{CancellationToken, LiteralSemanticEvaluator, ProvidedCandidateGenerator};

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
fn cancellation_during_single_unit_generation_does_not_rewrite() {
    let outcome = RewriteEngine::new(
        &CancellingGenerator,
        &LiteralSemanticEvaluator,
        &PassStructure,
    )
    .run(
        &document("Hello"),
        &literal_options(),
        &CancellationToken::new(),
    )
    .expect("cooperative cancellation is an outcome");
    assert_eq!(outcome.status, RewriteStatus::Abstained);
    assert_eq!(outcome.reason, Some(rewrite_types::ReasonCode::Cancelled));
    assert!(outcome.edits.is_empty());
}

#[test]
fn cancellation_during_assessment_does_not_rewrite() {
    let cancellation = CancellationToken::new();
    let generator = ProvidedCandidateGenerator::new(vec!["Hello.".to_owned()]);
    let semantic = CancelOnEvaluate {
        token: cancellation.clone(),
    };
    let outcome = RewriteEngine::new(&generator, &semantic, &PassStructure)
        .run(&document("Hello"), &literal_options(), &cancellation)
        .expect("cooperative cancellation is an outcome");
    assert_eq!(outcome.status, RewriteStatus::Abstained);
    assert_eq!(outcome.reason, Some(rewrite_types::ReasonCode::Cancelled));
    assert!(outcome.edits.is_empty());
}
