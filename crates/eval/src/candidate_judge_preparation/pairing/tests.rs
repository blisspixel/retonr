use rewrite_app::{ManagedJudgePrecursorCompilationError, ManagedJudgePrecursorRunnerHandoff};
use rewrite_types::CancellationToken;

use super::kernel::{PairingSubject, validate_pairing};
use super::{
    CandidateJudgePreparationError, CandidateJudgeRunnerHandoff, CandidateJudgeRunnerPairingError,
    CandidateJudgeRunnerPairingRelationship, CandidateJudgeRunnerPairingValidationPhase,
    PairedCandidateJudgeRunnerHandoff, pair_candidate_judge_runner_handoffs,
};
use crate::CandidateJudgePreparationRelationship;

#[derive(Default)]
struct FakeSubject {
    eval_calls: usize,
    app_calls: usize,
    relationships: Vec<CandidateJudgeRunnerPairingRelationship>,
    fail_eval_on: Vec<usize>,
    fail_app_on: Vec<usize>,
    mismatch: Option<CandidateJudgeRunnerPairingRelationship>,
    cancel_on_eval: Option<usize>,
    cancel_on_app: Option<usize>,
    cancel_on_relationship: Option<CandidateJudgeRunnerPairingRelationship>,
}

impl PairingSubject for FakeSubject {
    fn revalidate_eval(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<(), CandidateJudgePreparationError> {
        self.eval_calls += 1;
        if self.cancel_on_eval == Some(self.eval_calls) {
            cancellation.cancel();
        }
        if self.fail_eval_on.contains(&self.eval_calls) {
            Err(CandidateJudgePreparationError::Relationship(
                CandidateJudgePreparationRelationship::JudgePlan,
            ))
        } else {
            Ok(())
        }
    }

    fn revalidate_app(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<(), ManagedJudgePrecursorCompilationError> {
        self.app_calls += 1;
        if self.cancel_on_app == Some(self.app_calls) {
            cancellation.cancel();
        }
        if self.fail_app_on.contains(&self.app_calls) {
            Err(ManagedJudgePrecursorCompilationError::Cancelled)
        } else {
            Ok(())
        }
    }

    fn relationship_matches(
        &mut self,
        relationship: CandidateJudgeRunnerPairingRelationship,
        cancellation: &CancellationToken,
    ) -> bool {
        self.relationships.push(relationship);
        if self.cancel_on_relationship == Some(relationship) {
            cancellation.cancel();
        }
        self.mismatch != Some(relationship)
    }
}

const RELATIONSHIPS: [CandidateJudgeRunnerPairingRelationship; 4] = [
    CandidateJudgeRunnerPairingRelationship::JudgePlan,
    CandidateJudgeRunnerPairingRelationship::JudgeSchedule,
    CandidateJudgeRunnerPairingRelationship::RequestAggregate,
    CandidateJudgeRunnerPairingRelationship::JudgeSystem,
];

#[test]
fn success_revalidates_both_families_around_complete_ordered_comparison() {
    let mut subject = FakeSubject::default();
    validate_pairing(&mut subject, &CancellationToken::new()).expect("pair");
    assert_eq!(subject.eval_calls, 2);
    assert_eq!(subject.app_calls, 2);
    assert_eq!(subject.relationships, RELATIONSHIPS);
}

#[test]
fn every_complete_relationship_substitution_is_rejected_in_order() {
    for (mismatch_index, mismatch) in RELATIONSHIPS.into_iter().enumerate() {
        let mut subject = FakeSubject {
            mismatch: Some(mismatch),
            ..FakeSubject::default()
        };
        let error = validate_pairing(&mut subject, &CancellationToken::new())
            .expect_err("substitution must fail");
        assert!(matches!(
            error,
            CandidateJudgeRunnerPairingError::Relationship(observed) if observed == mismatch
        ));
        assert_eq!(subject.relationships, RELATIONSHIPS[..=mismatch_index]);
        assert_eq!((subject.eval_calls, subject.app_calls), (2, 2));
    }
}

#[test]
fn initial_validation_preserves_each_independent_authority_failure() {
    for (eval_failed, app_failed) in [(true, false), (false, true), (true, true)] {
        let mut subject = FakeSubject {
            fail_eval_on: eval_failed.then_some(1).into_iter().collect(),
            fail_app_on: app_failed.then_some(1).into_iter().collect(),
            ..FakeSubject::default()
        };
        let error = validate_pairing(&mut subject, &CancellationToken::new())
            .expect_err("initial validation must fail");
        assert_authority_error(
            &error,
            CandidateJudgeRunnerPairingValidationPhase::Initial,
            eval_failed,
            app_failed,
            false,
        );
        assert!(subject.relationships.is_empty());
        assert_eq!((subject.eval_calls, subject.app_calls), (1, 1));
    }
}

#[test]
fn final_validation_preserves_each_independent_authority_failure() {
    for (eval_failed, app_failed) in [(true, false), (false, true), (true, true)] {
        let mut subject = FakeSubject {
            fail_eval_on: eval_failed.then_some(2).into_iter().collect(),
            fail_app_on: app_failed.then_some(2).into_iter().collect(),
            ..FakeSubject::default()
        };
        let error = validate_pairing(&mut subject, &CancellationToken::new())
            .expect_err("final validation must fail");
        assert_authority_error(
            &error,
            CandidateJudgeRunnerPairingValidationPhase::Final,
            eval_failed,
            app_failed,
            false,
        );
        assert_eq!(subject.relationships, RELATIONSHIPS);
        assert_eq!((subject.eval_calls, subject.app_calls), (2, 2));
    }
}

#[test]
fn primary_and_final_failures_are_both_retained_without_nested_debug_details() {
    let mut subject = FakeSubject {
        mismatch: Some(CandidateJudgeRunnerPairingRelationship::JudgeSchedule),
        fail_eval_on: vec![2],
        fail_app_on: vec![2],
        ..FakeSubject::default()
    };
    let error = validate_pairing(&mut subject, &CancellationToken::new())
        .expect_err("dual failure must fail");
    let CandidateJudgeRunnerPairingError::PrimaryAndFinalValidation {
        primary,
        final_validation,
    } = &error
    else {
        panic!("expected dual failure")
    };
    assert!(matches!(
        primary.as_ref(),
        CandidateJudgeRunnerPairingError::Relationship(
            CandidateJudgeRunnerPairingRelationship::JudgeSchedule
        )
    ));
    assert_authority_error(
        final_validation,
        CandidateJudgeRunnerPairingValidationPhase::Final,
        true,
        true,
        false,
    );
    assert_eq!(
        format!("{error:?}"),
        "CandidateJudgeRunnerPairingError { kind: \"primary_and_final_validation\", .. }"
    );
}

#[test]
fn cancellation_is_checked_before_and_within_every_authority_bracket() {
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    let mut untouched = FakeSubject::default();
    assert!(matches!(
        validate_pairing(&mut untouched, &cancelled),
        Err(CandidateJudgeRunnerPairingError::Cancelled)
    ));
    assert_eq!((untouched.eval_calls, untouched.app_calls), (0, 0));

    let cancellation = CancellationToken::new();
    let mut after_eval = FakeSubject {
        cancel_on_eval: Some(1),
        ..FakeSubject::default()
    };
    let error = validate_pairing(&mut after_eval, &cancellation).expect_err("cancelled");
    assert_authority_error(
        &error,
        CandidateJudgeRunnerPairingValidationPhase::Initial,
        false,
        false,
        true,
    );
    assert_eq!((after_eval.eval_calls, after_eval.app_calls), (1, 0));

    let cancellation = CancellationToken::new();
    let mut after_app = FakeSubject {
        cancel_on_app: Some(1),
        ..FakeSubject::default()
    };
    let error = validate_pairing(&mut after_app, &cancellation).expect_err("cancelled");
    assert_authority_error(
        &error,
        CandidateJudgeRunnerPairingValidationPhase::Initial,
        false,
        false,
        true,
    );
    assert_eq!((after_app.eval_calls, after_app.app_calls), (1, 1));
}

#[test]
fn cancellation_at_each_relationship_boundary_stops_comparison_and_keeps_final_failure() {
    for (relationship_index, relationship) in RELATIONSHIPS.into_iter().enumerate() {
        let cancellation = CancellationToken::new();
        let mut subject = FakeSubject {
            cancel_on_relationship: Some(relationship),
            ..FakeSubject::default()
        };
        let error = validate_pairing(&mut subject, &cancellation).expect_err("cancelled");
        let CandidateJudgeRunnerPairingError::PrimaryAndFinalValidation {
            primary,
            final_validation,
        } = error
        else {
            panic!("expected cancellation plus final bracket")
        };
        assert!(matches!(
            primary.as_ref(),
            CandidateJudgeRunnerPairingError::Cancelled
        ));
        assert_authority_error(
            &final_validation,
            CandidateJudgeRunnerPairingValidationPhase::Final,
            false,
            false,
            true,
        );
        assert_eq!(subject.relationships, RELATIONSHIPS[..=relationship_index]);
        assert_eq!((subject.eval_calls, subject.app_calls), (1, 1));
    }
}

#[test]
fn cancellation_during_final_eval_skips_final_app_and_is_typed() {
    let cancellation = CancellationToken::new();
    let mut subject = FakeSubject {
        cancel_on_eval: Some(2),
        ..FakeSubject::default()
    };
    let error = validate_pairing(&mut subject, &cancellation).expect_err("cancelled");
    assert_authority_error(
        &error,
        CandidateJudgeRunnerPairingValidationPhase::Final,
        false,
        false,
        true,
    );
    assert_eq!((subject.eval_calls, subject.app_calls), (2, 1));
}

#[test]
fn redacted_debug_reports_only_closed_error_kinds() {
    let authority = CandidateJudgeRunnerPairingError::AuthorityValidation {
        phase: CandidateJudgeRunnerPairingValidationPhase::Initial,
        eval: Some(Box::new(CandidateJudgePreparationError::Relationship(
            CandidateJudgePreparationRelationship::JudgePolicyClosure,
        ))),
        app: Some(Box::new(ManagedJudgePrecursorCompilationError::Cancelled)),
        cancelled: true,
    };
    assert_eq!(
        format!("{authority:?}"),
        "CandidateJudgeRunnerPairingError { kind: \"authority_validation\", phase: Initial, eval_failed: true, app_failed: true, cancelled: true, .. }"
    );
    assert_eq!(
        format!("{:?}", CandidateJudgeRunnerPairingError::Cancelled),
        "CandidateJudgeRunnerPairingError { kind: \"cancelled\", .. }"
    );
    assert_eq!(
        format!(
            "{:?}",
            CandidateJudgeRunnerPairingError::Relationship(
                CandidateJudgeRunnerPairingRelationship::JudgeSystem
            )
        ),
        "CandidateJudgeRunnerPairingError { kind: \"relationship\", relationship: JudgeSystem, .. }"
    );
}

#[test]
fn concrete_handoff_types_and_lifetimes_wire_without_test_authorities() {
    fn wire<'store, 'records, 'model, 'runtime, 'characterized>(
        eval: CandidateJudgeRunnerHandoff<'store>,
        app: ManagedJudgePrecursorRunnerHandoff<'records, 'model, 'runtime, 'characterized>,
        cancellation: &CancellationToken,
    ) -> Result<
        PairedCandidateJudgeRunnerHandoff<'store, 'records, 'model, 'runtime, 'characterized>,
        CandidateJudgeRunnerPairingError,
    > {
        pair_candidate_judge_runner_handoffs(eval, app, cancellation)
    }

    fn consume(paired: PairedCandidateJudgeRunnerHandoff<'_, '_, '_, '_, '_>) {
        let _parts = paired.into_parts();
    }

    std::hint::black_box(wire);
    std::hint::black_box(consume);
}

fn assert_authority_error(
    error: &CandidateJudgeRunnerPairingError,
    expected_phase: CandidateJudgeRunnerPairingValidationPhase,
    eval_failed: bool,
    app_failed: bool,
    cancelled: bool,
) {
    let CandidateJudgeRunnerPairingError::AuthorityValidation {
        phase,
        eval,
        app,
        cancelled: observed_cancelled,
    } = error
    else {
        panic!("expected authority validation error")
    };
    assert_eq!(*phase, expected_phase);
    assert_eq!(eval.is_some(), eval_failed);
    assert_eq!(app.is_some(), app_failed);
    assert_eq!(*observed_cancelled, cancelled);
}
