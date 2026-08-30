use rewrite_model::CandidateDeterministicEvaluationStatusV1;
use rewrite_types::Digest;

use super::*;

#[test]
fn deterministic_failure_returns_before_invalid_judge_inputs_are_examined() {
    let material = Fixture::pair();
    let mut config = ready_config(vec![0]);
    config.supplied_rubric = LocalJudgeRubric {
        schema_version: 999,
        clauses: Vec::new(),
    };
    config.prompt_digest = Digest::sha256(b"invalid prompt contract");
    config.output_schema_digest = Digest::sha256(b"invalid output schema");
    let outcome = prepare(
        &material,
        "hard-gate-failed",
        config,
        &CancellationToken::new(),
    )
    .result
    .expect("failed deterministic outcome");
    let debug = format!("{outcome:?}");
    assert!(debug.contains("DeterministicFailed"));
    assert!(!debug.contains("Retain Acme"));
    let CandidateJudgePreparationOutcome::DeterministicFailed(record) = outcome else {
        panic!("judge inputs must not be examined after failed hard gates")
    };
    assert_eq!(
        record.status(),
        CandidateDeterministicEvaluationStatusV1::Failed
    );
}

#[test]
fn equal_selected_candidate_bytes_still_form_two_order_requests() {
    let material = Fixture::judge_pair();
    let outcome = prepare(
        &material,
        "equal-selected",
        ready_config(vec![0]),
        &CancellationToken::new(),
    )
    .result
    .expect("equal candidates remain judgeable");
    let CandidateJudgePreparationOutcome::Ready(ready) = outcome else {
        panic!("equal selected bytes must not forge a deterministic failure")
    };
    assert_eq!(ready.request_aggregate().entry_count(), 2);
    assert_ne!(
        ready.request_aggregate().structured_request_binding_ids()[0],
        ready.request_aggregate().structured_request_binding_ids()[1]
    );
}

#[test]
fn eligible_case_and_clause_sets_require_exact_semantic_closure() {
    let material = Fixture::judge_triple();
    for indices in [vec![0], vec![1, 2]] {
        let error = prepare(
            &material,
            "eligible-closure",
            ready_config(indices),
            &CancellationToken::new(),
        )
        .result
        .expect_err("missing or foreign eligible case");
        assert!(matches!(
            error,
            CandidateJudgePreparationError::Relationship(
                CandidateJudgePreparationRelationship::EligibleCaseClosure
            )
        ));
    }

    let mut missing_clause = ready_config(vec![0, 2]);
    missing_clause.case_clauses = Some(vec![vec!["fidelity".to_owned()]; 2]);
    assert!(matches!(
        prepare(
            &material,
            "missing-clause",
            missing_clause,
            &CancellationToken::new(),
        )
        .result,
        Err(CandidateJudgePreparationError::Relationship(
            CandidateJudgePreparationRelationship::RubricClauseClosure
        ))
    ));

    let mut foreign_clause = ready_config(vec![0, 2]);
    foreign_clause.case_clauses = Some(vec![
        vec!["fidelity".to_owned(), "meaning".to_owned()],
        vec!["fidelity".to_owned(), "meaning".to_owned()],
    ]);
    assert!(matches!(
        prepare(
            &material,
            "foreign-clause",
            foreign_clause,
            &CancellationToken::new(),
        )
        .result,
        Err(CandidateJudgePreparationError::Relationship(
            CandidateJudgePreparationRelationship::RubricClauseClosure
        ))
    ));

    let clauses = vec!["fidelity".to_owned(), "protected-values".to_owned()];
    assert!(try_plan(&material, &[2, 0], vec![clauses.clone(), clauses]).is_err());
    assert!(
        try_plan(
            &material,
            &[0],
            vec![vec!["protected-values".to_owned(), "fidelity".to_owned()]],
        )
        .is_err()
    );
}

#[test]
fn rubric_prompt_schema_and_model_substitution_fail_closed() {
    let material = Fixture::judge_pair();
    let mut rubric_substitution = ready_config(vec![0]);
    rubric_substitution.supplied_rubric.clauses[0]
        .instruction
        .push_str(" Substituted.");
    assert_policy_error(&prepare(
        &material,
        "rubric-substitution",
        rubric_substitution,
        &CancellationToken::new(),
    ));

    let mut prompt_substitution = ready_config(vec![0]);
    prompt_substitution.prompt_digest = Digest::sha256(b"foreign prompt");
    assert_policy_error(&prepare(
        &material,
        "prompt-substitution",
        prompt_substitution,
        &CancellationToken::new(),
    ));

    let mut schema_substitution = ready_config(vec![0]);
    schema_substitution.output_schema_digest = Digest::sha256(b"foreign schema");
    assert_policy_error(&prepare(
        &material,
        "schema-substitution",
        schema_substitution,
        &CancellationToken::new(),
    ));

    let mut model_substitution = ready_config(vec![0]);
    model_substitution.foreign_relation_model = true;
    assert!(matches!(
        prepare(
            &material,
            "model-substitution",
            model_substitution,
            &CancellationToken::new(),
        )
        .result,
        Err(CandidateJudgePreparationError::PortableContract { .. })
    ));
}

#[test]
fn source_candidate_prompt_limits_and_cancellation_stop_preparation() {
    let material = Fixture::judge_pair();
    for (suffix, limits) in [
        ("source-limit", limits(1, 8, 128, 512)),
        ("candidate-limit", limits(1, 128, 8, 512)),
        ("prompt-limit", limits(1, 128, 128, 128)),
    ] {
        let mut config = ready_config(vec![0]);
        config.limits = limits;
        assert!(matches!(
            prepare(&material, suffix, config, &CancellationToken::new()).result,
            Err(CandidateJudgePreparationError::Request {
                schedule_index: 0,
                failure: CandidateJudgePreparationRequestFailure::InputLimitExceeded,
            })
        ));
    }

    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert!(matches!(
        prepare(&material, "cancelled", ready_config(vec![0]), &cancellation,).result,
        Err(CandidateJudgePreparationError::Deterministic(
            CandidateDeterministicCompilerError::Cancelled
        ))
    ));
}

#[test]
fn mandatory_final_validation_and_dual_failure_are_lossless() {
    let material = Fixture::judge_pair();
    let mut final_failure = ready_config(vec![0]);
    final_failure.fail_candidate_a_on_call = Some(24);
    let final_result = prepare(
        &material,
        "final-failure",
        final_failure,
        &CancellationToken::new(),
    );
    assert!(matches!(
        final_result.result,
        Err(CandidateJudgePreparationError::AuthorityValidation {
            candidate_a: Some(_),
            ..
        })
    ));

    let mut dual = ready_config(vec![0]);
    dual.foreign_relation_model = true;
    dual.fail_candidate_a_on_call = Some(13);
    let dual_result = prepare(&material, "dual-failure", dual, &CancellationToken::new());
    let error = dual_result
        .result
        .expect_err("primary and final failures survive");
    let CandidateJudgePreparationError::PrimaryAndFinalValidation {
        primary,
        final_validation,
    } = error
    else {
        panic!("expected lossless primary and final error")
    };
    assert!(matches!(
        *primary,
        CandidateJudgePreparationError::PortableContract { .. }
    ));
    assert!(matches!(
        *final_validation,
        CandidateJudgePreparationError::AuthorityValidation {
            candidate_a: Some(_),
            ..
        }
    ));
}

fn assert_policy_error(result: &PreparationResult<'_>) {
    assert!(matches!(
        result.result,
        Err(CandidateJudgePreparationError::Relationship(
            CandidateJudgePreparationRelationship::JudgePolicyClosure
        ))
    ));
}
