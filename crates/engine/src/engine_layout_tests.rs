use rewrite_types::{CharacterBudget, LayoutConstraints, LineBudget, ReasonCode, RewriteStatus};

use crate::engine_test_support::{PassStructure, document, literal_options};
use crate::{
    CancellationToken, EngineError, LiteralSemanticEvaluator, ProvidedCandidateGenerator,
    RewriteEngine,
};

#[test]
fn candidate_exceeding_character_budget_is_rejected() {
    let generator = ProvidedCandidateGenerator::new(vec![
        "A very long candidate that exceeds budget".to_owned(),
    ]);
    let engine = RewriteEngine::new(&generator, &LiteralSemanticEvaluator, &PassStructure);
    let mut options = literal_options();
    options.layout = Some(LayoutConstraints {
        character_budget: Some(CharacterBudget {
            min_characters: None,
            max_characters: Some(15),
            max_expansion_percent: None,
        }),
        line_budget: None,
    });
    let outcome = engine
        .run(&document("Short text"), &options, &CancellationToken::new())
        .expect("deterministic ports succeed");
    assert_eq!(outcome.status, RewriteStatus::Abstained);
    assert_eq!(outcome.reason, Some(ReasonCode::CharacterBudgetExceeded));
}

#[test]
fn candidate_violating_character_expansion_is_rejected() {
    let generator = ProvidedCandidateGenerator::new(vec![
        "Expanded by much more than allowed percentage".to_owned(),
    ]);
    let engine = RewriteEngine::new(&generator, &LiteralSemanticEvaluator, &PassStructure);
    let mut options = literal_options();
    options.layout = Some(LayoutConstraints {
        character_budget: Some(CharacterBudget {
            min_characters: None,
            max_characters: None,
            max_expansion_percent: Some(10),
        }),
        line_budget: None,
    });
    let outcome = engine
        .run(&document("Small text"), &options, &CancellationToken::new())
        .expect("deterministic ports succeed");
    assert_eq!(outcome.status, RewriteStatus::Abstained);
    assert_eq!(outcome.reason, Some(ReasonCode::CharacterBudgetExceeded));
}

#[test]
fn candidate_violating_line_count_preservation_is_rejected() {
    let generator =
        ProvidedCandidateGenerator::new(vec!["Line one\nLine two\nLine three".to_owned()]);
    let engine = RewriteEngine::new(&generator, &LiteralSemanticEvaluator, &PassStructure);
    let mut options = literal_options();
    options.layout = Some(LayoutConstraints {
        character_budget: None,
        line_budget: Some(LineBudget {
            max_lines: None,
            preserve_line_count: true,
        }),
    });
    let outcome = engine
        .run(
            &document("Single line text"),
            &options,
            &CancellationToken::new(),
        )
        .expect("deterministic ports succeed");
    assert_eq!(outcome.status, RewriteStatus::Abstained);
    assert_eq!(outcome.reason, Some(ReasonCode::LineBudgetExceeded));
}

#[test]
fn invalid_layout_budget_bounds_fail_validation() {
    let mut options = literal_options();
    options.layout = Some(LayoutConstraints {
        character_budget: Some(CharacterBudget {
            min_characters: Some(100),
            max_characters: Some(50),
            max_expansion_percent: None,
        }),
        line_budget: None,
    });
    assert_eq!(
        crate::validate_rewrite_options(&options),
        Err(EngineError::InvalidLayoutBudget)
    );
}
