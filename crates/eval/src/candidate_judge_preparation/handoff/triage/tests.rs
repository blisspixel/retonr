use rewrite_model::{
    CandidateJudgeChoiceV1, CandidateJudgeObservationBatchV1, CandidateJudgeObservationV1,
    CandidateJudgePresentationV1, OllamaRetainedSessionResponseId,
};
use rewrite_types::{CancellationToken, Digest};

use super::{
    CandidateJudgeTriageCompilationError, CandidateJudgeTriageCompilationRelationship,
    combine_preparation_results, map_observations,
};
use crate::candidate_judge_preparation::tests::{prepare, ready_config};
use crate::generation_case_material::verified_material_test_support::Fixture;
use crate::hybrid_scorecard::JudgeObservationEvidenceClass;
use crate::{
    CandidateJudgePreparationError, CandidateJudgePreparationOutcome,
    CandidateJudgePreparationRelationship, CandidateJudgeRunnerHandoff, HybridScorecardError,
    JudgeCaseOutcome, ReleaseReviewDisposition,
};

type ChoiceScenario = (
    fn(CandidateJudgePresentationV1) -> CandidateJudgeChoiceV1,
    JudgeCaseOutcome,
);

#[test]
fn reversed_eligible_subset_compiles_canonical_content_free_triage() {
    let fixture = Fixture::judge_triple_reversed_eligible_order();
    let prepared = prepare(
        &fixture,
        "triage-reversed",
        ready_config(vec![0, 2]),
        &CancellationToken::new(),
    );
    let handoff = into_handoff(prepared.result.expect("ready"));
    assert_eq!(handoff.compatibility_case_permutation(), [1, 0]);
    let batch = observation_batch_with(&handoff, |index, presentation| {
        if index < 2 {
            stable_a(presentation)
        } else {
            stable_b(presentation)
        }
    });
    let compiled = handoff
        .compile_compatibility_triage(&batch, &CancellationToken::new())
        .expect("compiled triage");

    assert_eq!(compiled.report().judge.total, 2);
    assert_eq!(compiled.report().judge.stable_a, 1);
    assert_eq!(compiled.report().judge.stable_b, 1);
    assert_eq!(compiled.report().judge.cases[0].case_id, "eligible-rewrite");
    assert_eq!(
        compiled.report().judge.cases[0].outcome,
        JudgeCaseOutcome::StableB
    );
    assert_eq!(compiled.report().judge.cases[1].case_id, "eligible-zeta");
    assert_eq!(
        compiled.report().judge.cases[1].outcome,
        JudgeCaseOutcome::StableA
    );
    assert_eq!(
        compiled.report().judge_observation_evidence_class,
        Some(JudgeObservationEvidenceClass::CallerDeclared)
    );
    assert_eq!(
        compiled.report().release_review,
        ReleaseReviewDisposition::RequiresHumanAdjudication
    );
    assert_eq!(
        compiled.canonical_json(),
        serde_json::to_vec(compiled.report())
            .expect("canonical report")
            .as_slice()
    );
    // Freezes the domain-framed canonical report after semantic schedule results
    // have been permuted into lexicographic compatibility-case order.
    assert_eq!(
        compiled.relationship().digest().as_str(),
        "44b680eb39761f946e1ea9b8aca7d5f307f5a99c4d602cbfc00663c2b180261b"
    );
    let json = std::str::from_utf8(compiled.canonical_json()).expect("UTF-8 JSON");
    let debug = format!("{compiled:?}");
    for content in [
        "Zeta 7 needs polish.",
        "Acme 42 needs polish.",
        "Zeta, 7 needs polish!",
        "Acme, 42 needs polish!",
        "Zeta 7 needs polish?",
        "Acme 42 needs polish?",
        "Retain Acme 42 exactly.",
        "selected candidate triage-reversed-a 1",
        "selected candidate triage-reversed-b 1",
    ] {
        assert!(!json.contains(content));
        assert!(!debug.contains(content));
    }
}

#[test]
fn equal_candidate_bytes_remain_admitted_only_by_exact_triage_path() {
    let fixture = Fixture::judge_pair();
    let prepared = prepare(
        &fixture,
        "equal-selected",
        ready_config(vec![0]),
        &CancellationToken::new(),
    );
    let handoff = into_handoff(prepared.result.expect("ready"));
    assert_eq!(
        handoff.compatibility_projection.plan.cases[0].candidate_a_digest,
        handoff.compatibility_projection.plan.cases[0].candidate_b_digest
    );
    assert!(matches!(
        crate::parse_hybrid_scorecard_plan(
            &serde_json::to_string(&handoff.compatibility_projection.plan)
                .expect("compatibility plan JSON")
        ),
        Err(HybridScorecardError::InvalidCase { index: 0 })
    ));
    let batch = observation_batch(&handoff, stable_tie);
    let compiled = handoff
        .compile_compatibility_triage(&batch, &CancellationToken::new())
        .expect("equal candidates remain triage eligible");
    assert_eq!(compiled.report().judge.stable_tie, 1);
    assert!(compiled.report().hard_gates_passed());
}

#[test]
fn all_choice_outcomes_map_relative_to_both_presentations() {
    let scenarios: [ChoiceScenario; 5] = [
        (stable_a, JudgeCaseOutcome::StableA),
        (stable_b, JudgeCaseOutcome::StableB),
        (stable_tie, JudgeCaseOutcome::StableTie),
        (abstained, JudgeCaseOutcome::Abstained),
        (order_sensitive, JudgeCaseOutcome::OrderSensitive),
    ];
    for (index, (choices, expected)) in scenarios.into_iter().enumerate() {
        let fixture = Fixture::judge_pair();
        let prepared = prepare(
            &fixture,
            &format!("triage-choice-{index}"),
            ready_config(vec![0]),
            &CancellationToken::new(),
        );
        let handoff = into_handoff(prepared.result.expect("ready"));
        let compiled = handoff
            .compile_compatibility_triage(
                &observation_batch(&handoff, choices),
                &CancellationToken::new(),
            )
            .expect("choice compiles");
        assert_eq!(compiled.report().judge.cases[0].outcome, expected);
    }
}

#[test]
fn missing_reordered_duplicate_and_invalid_permutation_are_rejected() {
    let fixture = Fixture::judge_triple_reversed_eligible_order();
    let prepared = prepare(
        &fixture,
        "triage-adversarial",
        ready_config(vec![0, 2]),
        &CancellationToken::new(),
    );
    let mut handoff = into_handoff(prepared.result.expect("ready"));
    let batch = observation_batch(&handoff, stable_a);
    let mut observations = batch.observations().to_vec();

    assert_relationship(
        &map_observations(&handoff, &observations[..observations.len() - 1]),
        CandidateJudgeTriageCompilationRelationship::ObservationSchedule,
    );
    observations.swap(0, 1);
    assert_relationship(
        &map_observations(&handoff, &observations),
        CandidateJudgeTriageCompilationRelationship::ObservationSchedule,
    );
    observations = batch.observations().to_vec();
    observations[1] = observations[0].clone();
    assert_relationship(
        &map_observations(&handoff, &observations),
        CandidateJudgeTriageCompilationRelationship::ObservationSchedule,
    );

    handoff.compatibility_projection.semantic_to_lexicographic[0] = u32::MAX;
    assert_relationship(
        &map_observations(&handoff, batch.observations()),
        CandidateJudgeTriageCompilationRelationship::CasePermutation,
    );
}

#[test]
fn initial_and_post_compilation_cancellation_cannot_release_output() {
    let fixture = Fixture::judge_pair();
    let prepared = prepare(
        &fixture,
        "triage-cancellation",
        ready_config(vec![0]),
        &CancellationToken::new(),
    );
    let handoff = into_handoff(prepared.result.expect("ready"));
    let batch = observation_batch(&handoff, stable_a);
    let initially_cancelled = CancellationToken::new();
    initially_cancelled.cancel();
    assert!(matches!(
        handoff.compile_compatibility_triage(&batch, &initially_cancelled),
        Err(CandidateJudgeTriageCompilationError::PrimaryAndFinalValidation { .. })
    ));

    let terminal = CancellationToken::new();
    assert!(matches!(
        handoff.compile_compatibility_triage_with_post_compilation(&batch, &terminal, || {
            terminal.cancel();
        }),
        Err(CandidateJudgeTriageCompilationError::Preparation(
            crate::CandidateJudgePreparationError::AuthorityValidation {
                cancelled: true,
                ..
            }
        ))
    ));
}

#[test]
fn terminal_authority_failure_and_crossed_primary_failure_are_lossless() {
    let fixture = Fixture::judge_pair();
    let prepared = prepare(
        &fixture,
        "triage-terminal-authority",
        ready_config(vec![0]),
        &CancellationToken::new(),
    );
    let candidate_a_control = prepared.candidate_a_control;
    let handoff = into_handoff(prepared.result.expect("ready"));
    let batch = observation_batch(&handoff, stable_a);
    let error = handoff
        .compile_compatibility_triage_with_post_compilation(
            &batch,
            &CancellationToken::new(),
            || {
                candidate_a_control.fail_on_call(candidate_a_control.revalidation_calls() + 1);
            },
        )
        .expect_err("terminal authority failure suppresses output");
    assert!(matches!(
        error,
        CandidateJudgeTriageCompilationError::Preparation(
            crate::CandidateJudgePreparationError::AuthorityValidation {
                candidate_a: Some(_),
                ..
            }
        )
    ));

    let primary_prepared = prepare(
        &fixture,
        "triage-crossed-primary",
        ready_config(vec![0]),
        &CancellationToken::new(),
    );
    let candidate_a_control = primary_prepared.candidate_a_control;
    let primary_handoff = into_handoff(primary_prepared.result.expect("primary ready"));
    let foreign_prepared = prepare(
        &fixture,
        "triage-crossed-foreign",
        ready_config(vec![0]),
        &CancellationToken::new(),
    );
    let foreign_handoff = into_handoff(foreign_prepared.result.expect("foreign ready"));
    let foreign_batch = observation_batch(&foreign_handoff, stable_a);
    let error = primary_handoff
        .compile_compatibility_triage_with_post_compilation(
            &foreign_batch,
            &CancellationToken::new(),
            || {
                candidate_a_control.fail_on_call(candidate_a_control.revalidation_calls() + 1);
            },
        )
        .expect_err("primary and terminal failures survive");
    let CandidateJudgeTriageCompilationError::PrimaryAndFinalValidation {
        primary,
        final_validation,
    } = error
    else {
        panic!("expected crossed primary and final validation failure")
    };
    assert!(matches!(
        *primary,
        CandidateJudgeTriageCompilationError::Relationship(
            CandidateJudgeTriageCompilationRelationship::ObservationBatch
        )
    ));
    assert!(matches!(
        *final_validation,
        crate::CandidateJudgePreparationError::AuthorityValidation {
            candidate_a: Some(_),
            ..
        }
    ));
}

#[test]
fn foreign_batch_and_errors_are_content_redacted() {
    let fixture = Fixture::judge_pair();
    let first = prepare(
        &fixture,
        "triage-first",
        ready_config(vec![0]),
        &CancellationToken::new(),
    );
    let second = prepare(
        &fixture,
        "triage-second",
        ready_config(vec![0]),
        &CancellationToken::new(),
    );
    let handoff = into_handoff(first.result.expect("first ready"));
    let foreign = into_handoff(second.result.expect("second ready"));
    let error = handoff
        .compile_compatibility_triage(
            &observation_batch(&foreign, stable_a),
            &CancellationToken::new(),
        )
        .expect_err("foreign batch");
    assert!(matches!(
        error,
        CandidateJudgeTriageCompilationError::Relationship(
            CandidateJudgeTriageCompilationRelationship::ObservationBatch
        )
    ));
    let debug = format!("{error:?}");
    assert!(!debug.contains("Acme 42 needs polish"));
    assert!(!debug.contains("selected candidate"));
}

#[test]
fn every_closed_error_debug_view_is_content_redacted() {
    let relationship_errors = [
        CandidateJudgeTriageCompilationRelationship::DeterministicEvaluation,
        CandidateJudgeTriageCompilationRelationship::ObservationBatch,
        CandidateJudgeTriageCompilationRelationship::ObservationSchedule,
        CandidateJudgeTriageCompilationRelationship::CasePermutation,
        CandidateJudgeTriageCompilationRelationship::Report,
    ]
    .map(CandidateJudgeTriageCompilationError::Relationship);
    let errors = relationship_errors.into_iter().chain([
        CandidateJudgeTriageCompilationError::Preparation(
            CandidateJudgePreparationError::Relationship(
                CandidateJudgePreparationRelationship::ScheduleClosure,
            ),
        ),
        CandidateJudgeTriageCompilationError::Scorecard(HybridScorecardError::InvalidPlan),
        CandidateJudgeTriageCompilationError::Portable(
            rewrite_model::GenerationQualificationContractError::InvalidEncoding,
        ),
        CandidateJudgeTriageCompilationError::Encoding,
        CandidateJudgeTriageCompilationError::PrimaryAndFinalValidation {
            primary: Box::new(CandidateJudgeTriageCompilationError::Encoding),
            final_validation: Box::new(CandidateJudgePreparationError::Relationship(
                CandidateJudgePreparationRelationship::ScheduleClosure,
            )),
        },
    ]);
    for error in errors {
        let debug = format!("{error:?}");
        assert!(debug.starts_with("CandidateJudgeTriageCompilationError"));
        assert!(!debug.contains("source content"));
        assert!(!debug.contains("candidate content"));
        assert!(!debug.contains("prompt content"));
        assert!(!debug.contains("rationale content"));
    }
}

#[test]
fn preparation_result_matrix_preserves_both_failures() {
    let failure = || {
        CandidateJudgePreparationError::Relationship(
            CandidateJudgePreparationRelationship::ScheduleClosure,
        )
    };
    assert!(combine_preparation_results(Ok(()), Ok(())).is_ok());
    assert!(matches!(
        combine_preparation_results(Err(failure()), Ok(())),
        Err(CandidateJudgePreparationError::Relationship(_))
    ));
    assert!(matches!(
        combine_preparation_results(Ok(()), Err(failure())),
        Err(CandidateJudgePreparationError::Relationship(_))
    ));
    assert!(matches!(
        combine_preparation_results(Err(failure()), Err(failure())),
        Err(CandidateJudgePreparationError::PrimaryAndFinalValidation { .. })
    ));
}

fn into_handoff(outcome: CandidateJudgePreparationOutcome<'_>) -> CandidateJudgeRunnerHandoff<'_> {
    let CandidateJudgePreparationOutcome::Ready(ready) = outcome else {
        panic!("passed deterministic record must prepare")
    };
    ready
        .into_runner_handoff(&CancellationToken::new())
        .expect("runner handoff")
}

fn observation_batch(
    handoff: &CandidateJudgeRunnerHandoff<'_>,
    choices: fn(CandidateJudgePresentationV1) -> CandidateJudgeChoiceV1,
) -> CandidateJudgeObservationBatchV1 {
    observation_batch_with(handoff, |_, presentation| choices(presentation))
}

fn observation_batch_with(
    handoff: &CandidateJudgeRunnerHandoff<'_>,
    choices: impl Fn(usize, CandidateJudgePresentationV1) -> CandidateJudgeChoiceV1,
) -> CandidateJudgeObservationBatchV1 {
    let observations = handoff
        .judge_schedule()
        .entries()
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let planned = handoff
                .judge_plan()
                .cases()
                .iter()
                .find(|case| case.case_id() == entry.case_id())
                .expect("planned case");
            CandidateJudgeObservationV1::new(
                handoff.judge_plan(),
                handoff.judge_schedule(),
                handoff.request_aggregate(),
                index,
                OllamaRetainedSessionResponseId::from_derived_digest(Digest::sha256(
                    format!("triage response {index}").as_bytes(),
                )),
                choices(index, entry.presentation()),
                planned.rubric_clause_ids().to_vec(),
            )
            .expect("observation")
        })
        .collect();
    CandidateJudgeObservationBatchV1::new(
        handoff.judge_plan(),
        handoff.judge_schedule(),
        handoff.request_aggregate(),
        observations,
    )
    .expect("observation batch")
}

const fn stable_a(presentation: CandidateJudgePresentationV1) -> CandidateJudgeChoiceV1 {
    match presentation {
        CandidateJudgePresentationV1::CandidateAFirst => CandidateJudgeChoiceV1::First,
        CandidateJudgePresentationV1::CandidateBFirst => CandidateJudgeChoiceV1::Second,
    }
}

const fn stable_b(presentation: CandidateJudgePresentationV1) -> CandidateJudgeChoiceV1 {
    match presentation {
        CandidateJudgePresentationV1::CandidateAFirst => CandidateJudgeChoiceV1::Second,
        CandidateJudgePresentationV1::CandidateBFirst => CandidateJudgeChoiceV1::First,
    }
}

const fn stable_tie(_presentation: CandidateJudgePresentationV1) -> CandidateJudgeChoiceV1 {
    CandidateJudgeChoiceV1::Tie
}

const fn abstained(presentation: CandidateJudgePresentationV1) -> CandidateJudgeChoiceV1 {
    match presentation {
        CandidateJudgePresentationV1::CandidateAFirst => CandidateJudgeChoiceV1::Abstain,
        CandidateJudgePresentationV1::CandidateBFirst => CandidateJudgeChoiceV1::Second,
    }
}

const fn order_sensitive(_presentation: CandidateJudgePresentationV1) -> CandidateJudgeChoiceV1 {
    CandidateJudgeChoiceV1::First
}

fn assert_relationship(
    result: &Result<Vec<crate::JudgeObservation>, CandidateJudgeTriageCompilationError>,
    expected: CandidateJudgeTriageCompilationRelationship,
) {
    assert!(matches!(
        result,
        Err(CandidateJudgeTriageCompilationError::Relationship(observed)) if *observed == expected
    ));
}
