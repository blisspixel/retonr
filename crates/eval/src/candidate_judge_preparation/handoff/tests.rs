use std::{cell::Cell, io, rc::Rc};

use rewrite_model::{
    CandidateJudgePresentationV1, CandidateJudgeRequestAggregateV1,
    StructuredCompletionRequestBindingId,
};
use rewrite_types::{CancellationToken, Digest};

use super::error::CandidateJudgeRunnerRequestError;
use super::*;
use crate::candidate_judge_preparation::tests::{prepare, ready_config};
use crate::generation_case_material::verified_material_test_support::Fixture;
use crate::{CandidateJudgePreparationOutcome, CandidateJudgePreparationSide};

#[test]
fn handoff_exposes_only_frozen_identities_and_exact_scoped_attempts() {
    let fixture = Fixture::judge_triple();
    let prepared = prepare(
        &fixture,
        "runner-handoff",
        ready_config(vec![0, 2]),
        &CancellationToken::new(),
    );
    let CandidateJudgePreparationOutcome::Ready(ready) = prepared.result.expect("ready") else {
        panic!("passed deterministic record must be ready")
    };
    let handoff = ready
        .into_runner_handoff(&CancellationToken::new())
        .expect("runner handoff");
    let handoff_debug = format!("{handoff:?}");
    assert!(handoff_debug.contains("compatibility_plan_digest"));
    assert!(!handoff_debug.contains("Acme 42 needs polish"));
    assert_eq!(handoff.judge_plan().cases().len(), 2);
    assert_eq!(handoff.judge_schedule().entry_count(), 4);
    assert_eq!(handoff.request_aggregate().entry_count(), 4);
    assert_eq!(handoff.compatibility_case_permutation(), [0, 1]);
    assert_eq!(
        handoff.judge_plan().judge_generation_system_id(),
        handoff.judge_system().generation_system_id()
    );
    assert_eq!(
        handoff.deterministic_evaluation().status(),
        rewrite_model::CandidateDeterministicEvaluationStatusV1::Passed
    );
    assert_eq!(
        handoff.compatibility_plan_digest().as_str(),
        "3852a74dc1aa35b8d7715ccd2797f9c6023afc8d29a0fb4788af91fb2f3ea62e"
    );

    assert_exact_attempts(&handoff);
}

fn assert_exact_attempts(handoff: &CandidateJudgeRunnerHandoff<'_>) {
    let expected = [
        (
            "eligible-rewrite",
            "Acme 42 needs polish.",
            "Acme, 42 needs polish!",
            "Acme 42 needs polish?",
        ),
        (
            "eligible-rewrite",
            "Acme 42 needs polish.",
            "Acme, 42 needs polish!",
            "Acme 42 needs polish?",
        ),
        (
            "eligible-zeta",
            "Zeta 7 needs polish.",
            "Zeta, 7 needs polish!",
            "Zeta 7 needs polish?",
        ),
        (
            "eligible-zeta",
            "Zeta 7 needs polish.",
            "Zeta, 7 needs polish!",
            "Zeta 7 needs polish?",
        ),
    ];
    for (cursor, (case_key, source, candidate_a, candidate_b)) in expected.into_iter().enumerate() {
        handoff
            .with_request_at(cursor, &CancellationToken::new(), |attempt| {
                assert_eq!(attempt.schedule_cursor(), cursor);
                assert_eq!(attempt.case_key(), case_key);
                assert_eq!(attempt.source(), source);
                assert_eq!(attempt.candidate_a(), candidate_a);
                assert_eq!(attempt.candidate_b(), candidate_b);
                assert_eq!(
                    attempt.request().structured_request_binding_id(),
                    handoff.request_aggregate().structured_request_binding_ids()[cursor]
                );
                assert_eq!(
                    attempt.rubric_clause_ids(),
                    ["fidelity", "protected-values"]
                );
                match attempt.presentation() {
                    CandidateJudgePresentationV1::CandidateAFirst => {
                        assert_eq!(attempt.presented_first(), candidate_a);
                        assert_eq!(attempt.presented_second(), candidate_b);
                    }
                    CandidateJudgePresentationV1::CandidateBFirst => {
                        assert_eq!(attempt.presented_first(), candidate_b);
                        assert_eq!(attempt.presented_second(), candidate_a);
                    }
                }
                Ok::<(), io::Error>(())
            })
            .expect("exact callback view");
        handoff
            .revalidate_request_at(cursor, &CancellationToken::new())
            .expect("content-free request verification");
        let traffic = handoff
            .prepare_traffic_request_at(cursor, &CancellationToken::new())
            .expect("traffic request");
        let debug = format!("{traffic:?}");
        assert!(debug.contains("request_binding_id"));
        assert!(!debug.contains(source));
        assert_eq!(traffic.schedule_cursor(), cursor);
        assert_eq!(
            traffic.request_binding_id(),
            handoff.request_aggregate().structured_request_binding_ids()[cursor]
        );
        let request = traffic.into_structured_request();
        request.validate().expect("traffic request remains valid");
    }
}

#[test]
fn invalid_cursor_and_substituted_binding_fail_before_callback() {
    let fixture = Fixture::judge_triple();
    let prepared = prepare(
        &fixture,
        "handoff-substitution",
        ready_config(vec![0, 2]),
        &CancellationToken::new(),
    );
    let CandidateJudgePreparationOutcome::Ready(ready) = prepared.result.expect("ready") else {
        panic!("expected ready")
    };
    let mut handoff = ready
        .into_runner_handoff(&CancellationToken::new())
        .expect("handoff");
    let mut callback_called = false;
    let out_of_range = handoff.with_request_at(
        handoff.judge_schedule().entries().len(),
        &CancellationToken::new(),
        |_attempt| {
            callback_called = true;
            Ok::<(), io::Error>(())
        },
    );
    assert!(matches!(
        out_of_range,
        Err(CandidateJudgeRunnerRequestError::CursorOutOfRange { .. })
    ));
    assert!(!callback_called);

    handoff.eligible_semantic_indices[0] = 1;
    let invalid_closure = handoff.with_request_at(0, &CancellationToken::new(), |_attempt| {
        callback_called = true;
        Ok::<(), io::Error>(())
    });
    assert!(matches!(
        invalid_closure,
        Err(CandidateJudgeRunnerRequestError::AttemptClosure { schedule_cursor: 0 })
    ));
    assert!(!callback_called);
    handoff.eligible_semantic_indices[0] = 0;

    let mut ids = handoff
        .request_aggregate
        .structured_request_binding_ids()
        .to_vec();
    ids[0] = StructuredCompletionRequestBindingId::from_derived_digest(Digest::sha256(
        b"substituted handoff request",
    ));
    handoff.request_aggregate =
        CandidateJudgeRequestAggregateV1::new(&handoff.judge_plan, &handoff.judge_schedule, ids)
            .expect("internally valid substituted aggregate");
    let substituted = handoff.with_request_at(0, &CancellationToken::new(), |_attempt| {
        callback_called = true;
        Ok::<(), io::Error>(())
    });
    assert!(matches!(
        substituted,
        Err(CandidateJudgeRunnerRequestError::RequestBindingMismatch { schedule_cursor: 0 })
    ));
    assert!(!callback_called);
}

#[test]
fn acquisition_and_terminal_cancellation_revalidate_before_release() {
    for side in [
        CandidateJudgePreparationSide::CandidateA,
        CandidateJudgePreparationSide::CandidateB,
    ] {
        let fixture = Fixture::judge_triple();
        let prepared = prepare(
            &fixture,
            "handoff-acquire-revalidation",
            ready_config(vec![0, 2]),
            &CancellationToken::new(),
        );
        let CandidateJudgePreparationOutcome::Ready(ready) = prepared.result.expect("ready") else {
            panic!("expected ready")
        };
        let control = match side {
            CandidateJudgePreparationSide::CandidateA => &prepared.candidate_a_control,
            CandidateJudgePreparationSide::CandidateB => &prepared.candidate_b_control,
        };
        control.fail_on_call(control.revalidation_calls() + 1);
        let error = ready
            .into_runner_handoff(&CancellationToken::new())
            .expect_err("acquisition revalidation must fail");
        let CandidateJudgePreparationError::AuthorityValidation {
            candidate_a,
            candidate_b,
            ..
        } = error
        else {
            panic!("expected authority validation")
        };
        assert_eq!(
            candidate_a.is_some(),
            side == CandidateJudgePreparationSide::CandidateA
        );
        assert_eq!(
            candidate_b.is_some(),
            side == CandidateJudgePreparationSide::CandidateB
        );
    }

    let fixture = Fixture::judge_triple();
    let prepared = prepare(
        &fixture,
        "handoff-cancelled",
        ready_config(vec![0, 2]),
        &CancellationToken::new(),
    );
    let CandidateJudgePreparationOutcome::Ready(ready) = prepared.result.expect("ready") else {
        panic!("expected ready")
    };
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(matches!(
        ready.into_runner_handoff(&cancelled),
        Err(CandidateJudgePreparationError::AuthorityValidation {
            cancelled: true,
            ..
        })
    ));
}

#[test]
fn callback_and_mandatory_final_validation_failures_are_both_retained() {
    let fixture = Fixture::judge_triple();
    let prepared = prepare(
        &fixture,
        "handoff-dual-validation",
        ready_config(vec![0, 2]),
        &CancellationToken::new(),
    );
    let CandidateJudgePreparationOutcome::Ready(ready) = prepared.result.expect("ready") else {
        panic!("expected ready")
    };
    let handoff = ready
        .into_runner_handoff(&CancellationToken::new())
        .expect("handoff");
    let control = &prepared.candidate_a_control;
    let error = handoff
        .with_request_at(0, &CancellationToken::new(), |_attempt| {
            control.fail_on_call(control.revalidation_calls() + 1);
            Err(io::Error::other("sensitive callback output"))
        })
        .expect_err("callback and final validation must both survive");
    let debug = format!("{error:?}");
    assert!(!debug.contains("sensitive callback output"));
    let CandidateJudgeRunnerRequestError::PrimaryAndFinalValidation {
        primary,
        final_validation,
    } = error
    else {
        panic!("expected dual failure")
    };
    assert!(matches!(
        *primary,
        CandidateJudgeRunnerRequestError::Callback {
            schedule_cursor: 0,
            ..
        }
    ));
    assert!(matches!(
        *final_validation,
        CandidateJudgePreparationError::AuthorityValidation {
            candidate_a: Some(_),
            ..
        }
    ));
}

#[test]
fn successful_callback_is_discarded_when_final_validation_fails() {
    let fixture = Fixture::judge_triple();
    let prepared = prepare(
        &fixture,
        "handoff-final-validation",
        ready_config(vec![0, 2]),
        &CancellationToken::new(),
    );
    let CandidateJudgePreparationOutcome::Ready(ready) = prepared.result.expect("ready") else {
        panic!("expected ready")
    };
    let handoff = ready
        .into_runner_handoff(&CancellationToken::new())
        .expect("handoff");
    let control = &prepared.candidate_a_control;
    let error = handoff
        .with_request_at(0, &CancellationToken::new(), |_attempt| {
            control.fail_on_call(control.revalidation_calls() + 1);
            Ok::<(), io::Error>(())
        })
        .expect_err("successful callback cannot survive failed final validation");
    assert!(matches!(
        error,
        CandidateJudgeRunnerRequestError::Preparation {
            source: CandidateJudgePreparationError::AuthorityValidation {
                candidate_a: Some(_),
                ..
            }
        }
    ));
}

struct DropSentinel {
    drops: Rc<Cell<u32>>,
}

impl Drop for DropSentinel {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

#[test]
fn generic_callback_value_is_released_only_after_complete_validation() {
    let fixture = Fixture::judge_triple();
    let prepared = prepare(
        &fixture,
        "handoff-generic-value",
        ready_config(vec![0, 2]),
        &CancellationToken::new(),
    );
    let CandidateJudgePreparationOutcome::Ready(ready) = prepared.result.expect("ready") else {
        panic!("expected ready")
    };
    let handoff = ready
        .into_runner_handoff(&CancellationToken::new())
        .expect("handoff");
    let drops = Rc::new(Cell::new(0));
    let returned = handoff
        .with_attempt_at(0, &CancellationToken::new(), |attempt| {
            assert_eq!(attempt.schedule_cursor(), 0);
            Ok::<_, io::Error>((
                attempt.presentation(),
                DropSentinel {
                    drops: Rc::clone(&drops),
                },
            ))
        })
        .expect("validated value");
    assert_eq!(
        returned.0,
        handoff.judge_schedule().entries()[0].presentation()
    );
    assert_eq!(drops.get(), 0);
    drop(returned);
    assert_eq!(drops.get(), 1);
}

#[test]
fn generic_callback_value_is_dropped_on_outer_final_validation_failure() {
    let fixture = Fixture::judge_triple();
    let prepared = prepare(
        &fixture,
        "handoff-generic-final-failure",
        ready_config(vec![0, 2]),
        &CancellationToken::new(),
    );
    let CandidateJudgePreparationOutcome::Ready(ready) = prepared.result.expect("ready") else {
        panic!("expected ready")
    };
    let handoff = ready
        .into_runner_handoff(&CancellationToken::new())
        .expect("handoff");
    let control = &prepared.candidate_a_control;
    let drops = Rc::new(Cell::new(0));
    let result = handoff.with_attempt_at(0, &CancellationToken::new(), |_attempt| {
        control.fail_on_call(control.revalidation_calls() + 1);
        Ok::<_, io::Error>(DropSentinel {
            drops: Rc::clone(&drops),
        })
    });
    assert!(matches!(
        result,
        Err(CandidateJudgeRunnerRequestError::Preparation {
            source: CandidateJudgePreparationError::AuthorityValidation {
                candidate_a: Some(_),
                ..
            }
        })
    ));
    assert_eq!(drops.get(), 1);
}

#[test]
fn generic_callback_value_is_dropped_when_callback_cancels_validation() {
    let fixture = Fixture::judge_triple();
    let prepared = prepare(
        &fixture,
        "handoff-generic-cancelled-final",
        ready_config(vec![0, 2]),
        &CancellationToken::new(),
    );
    let CandidateJudgePreparationOutcome::Ready(ready) = prepared.result.expect("ready") else {
        panic!("expected ready")
    };
    let handoff = ready
        .into_runner_handoff(&CancellationToken::new())
        .expect("handoff");
    let cancellation = CancellationToken::new();
    let drops = Rc::new(Cell::new(0));
    let result = handoff.with_attempt_at(0, &cancellation, |_attempt| {
        cancellation.cancel();
        Ok::<_, io::Error>(DropSentinel {
            drops: Rc::clone(&drops),
        })
    });
    let error = match result {
        Err(error) => error,
        Ok(value) => {
            drop(value);
            panic!("terminal cancellation must fail")
        }
    };
    assert!(matches!(
        &error,
        CandidateJudgeRunnerRequestError::PrimaryAndFinalValidation { .. }
    ));
    assert_eq!(drops.get(), 1);
    let debug = format!("{error:?}");
    assert!(debug.contains("primary_and_final_validation"));
    assert!(!debug.contains("Acme"));
}
