use std::{cell::RefCell, rc::Rc};

use rewrite_app::ManagedJudgePrecursorCompilationError;
use rewrite_types::CancellationToken;

use super::kernel::{ConfigurationSubject, validate_configuration};
use super::{
    CandidateJudgePreparationError, ManagedJudgeRunnerConfigurationError,
    ManagedJudgeRunnerConfigurationRelationship, ManagedJudgeRunnerConfigurationValidationPhase,
};
use crate::candidate_judge_preparation::CandidateJudgePreparationRelationship;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event {
    Eval,
    App,
    Configuration,
    Release,
}

#[derive(Default)]
struct FakeSubject {
    events: Rc<RefCell<Vec<Event>>>,
    eval_calls: usize,
    app_calls: usize,
    fail_eval_on: Vec<usize>,
    fail_app_on: Vec<usize>,
    configuration_failure: Option<ManagedJudgeRunnerConfigurationRelationship>,
    release_failure: Option<ManagedJudgeRunnerConfigurationRelationship>,
    cancel_on_eval: Option<usize>,
    cancel_on_app: Option<usize>,
    cancel_during_configuration: bool,
}

impl ConfigurationSubject for FakeSubject {
    type Output = Rc<RefCell<Vec<Event>>>;

    fn revalidate_eval(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<(), CandidateJudgePreparationError> {
        self.eval_calls += 1;
        self.events.borrow_mut().push(Event::Eval);
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
        self.events.borrow_mut().push(Event::App);
        if self.cancel_on_app == Some(self.app_calls) {
            cancellation.cancel();
        }
        if self.fail_app_on.contains(&self.app_calls) {
            Err(ManagedJudgePrecursorCompilationError::Cancelled)
        } else {
            Ok(())
        }
    }

    fn validate_caller_configuration(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<(), ManagedJudgeRunnerConfigurationError> {
        self.events.borrow_mut().push(Event::Configuration);
        if self.cancel_during_configuration {
            cancellation.cancel();
        }
        if cancellation.is_cancelled() {
            return Err(ManagedJudgeRunnerConfigurationError::Cancelled);
        }
        match self.configuration_failure {
            Some(relationship) => Err(ManagedJudgeRunnerConfigurationError::Relationship(
                relationship,
            )),
            None => Ok(()),
        }
    }

    fn release(
        self,
        _cancellation: &CancellationToken,
    ) -> Result<Self::Output, ManagedJudgeRunnerConfigurationError> {
        self.events.borrow_mut().push(Event::Release);
        match self.release_failure {
            Some(relationship) => Err(ManagedJudgeRunnerConfigurationError::Relationship(
                relationship,
            )),
            None => Ok(self.events),
        }
    }
}

#[test]
fn success_brackets_configuration_with_both_authority_families_before_release() {
    let subject = FakeSubject::default();
    let observed = subject.events.clone();
    let output = validate_configuration(subject, &CancellationToken::new()).expect("configure");
    assert!(Rc::ptr_eq(&observed, &output));
    assert_eq!(
        observed.borrow().as_slice(),
        [
            Event::Eval,
            Event::App,
            Event::Configuration,
            Event::Eval,
            Event::App,
            Event::Release,
        ]
    );
}

#[test]
fn initial_validation_preserves_each_independent_failure_and_stops_before_config() {
    for (eval_failed, app_failed) in [(true, false), (false, true), (true, true)] {
        let subject = FakeSubject {
            fail_eval_on: eval_failed.then_some(1).into_iter().collect(),
            fail_app_on: app_failed.then_some(1).into_iter().collect(),
            ..FakeSubject::default()
        };
        let observed = subject.events.clone();
        let error = validate_configuration(subject, &CancellationToken::new())
            .expect_err("initial failure");
        assert_authority_error(
            &error,
            ManagedJudgeRunnerConfigurationValidationPhase::Initial,
            eval_failed,
            app_failed,
            false,
        );
        assert_eq!(observed.borrow().as_slice(), [Event::Eval, Event::App]);
    }
}

#[test]
fn final_validation_preserves_each_independent_failure_and_prevents_release() {
    for (eval_failed, app_failed) in [(true, false), (false, true), (true, true)] {
        let subject = FakeSubject {
            fail_eval_on: eval_failed.then_some(2).into_iter().collect(),
            fail_app_on: app_failed.then_some(2).into_iter().collect(),
            ..FakeSubject::default()
        };
        let observed = subject.events.clone();
        let error =
            validate_configuration(subject, &CancellationToken::new()).expect_err("final failure");
        assert_authority_error(
            &error,
            ManagedJudgeRunnerConfigurationValidationPhase::Final,
            eval_failed,
            app_failed,
            false,
        );
        assert_eq!(
            observed.borrow().as_slice(),
            [
                Event::Eval,
                Event::App,
                Event::Configuration,
                Event::Eval,
                Event::App,
            ]
        );
    }
}

#[test]
fn configuration_and_final_failures_are_both_retained_and_debug_is_redacted() {
    let subject = FakeSubject {
        fail_eval_on: vec![2],
        fail_app_on: vec![2],
        configuration_failure: Some(ManagedJudgeRunnerConfigurationRelationship::Model),
        ..FakeSubject::default()
    };
    let error =
        validate_configuration(subject, &CancellationToken::new()).expect_err("combined failure");
    let ManagedJudgeRunnerConfigurationError::PrimaryAndFinalValidation {
        primary,
        final_validation,
    } = &error
    else {
        panic!("expected combined failure")
    };
    assert!(matches!(
        primary.as_ref(),
        ManagedJudgeRunnerConfigurationError::Relationship(
            ManagedJudgeRunnerConfigurationRelationship::Model
        )
    ));
    assert_authority_error(
        final_validation,
        ManagedJudgeRunnerConfigurationValidationPhase::Final,
        true,
        true,
        false,
    );
    assert_eq!(
        format!("{error:?}"),
        "ManagedJudgeRunnerConfigurationError { kind: \"primary_and_final_validation\", .. }"
    );
}

#[test]
fn cancellation_precedence_stops_unstarted_checks_and_retains_final_cancellation() {
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let subject = FakeSubject::default();
    let observed = subject.events.clone();
    assert!(matches!(
        validate_configuration(subject, &cancellation),
        Err(ManagedJudgeRunnerConfigurationError::Cancelled)
    ));
    assert!(observed.borrow().is_empty());

    let cancellation = CancellationToken::new();
    let subject = FakeSubject {
        cancel_on_eval: Some(1),
        ..FakeSubject::default()
    };
    let observed = subject.events.clone();
    let error = validate_configuration(subject, &cancellation).expect_err("cancelled initial");
    assert_authority_error(
        &error,
        ManagedJudgeRunnerConfigurationValidationPhase::Initial,
        false,
        false,
        true,
    );
    assert_eq!(observed.borrow().as_slice(), [Event::Eval]);

    let cancellation = CancellationToken::new();
    let subject = FakeSubject {
        cancel_during_configuration: true,
        ..FakeSubject::default()
    };
    let observed = subject.events.clone();
    let error = validate_configuration(subject, &cancellation).expect_err("cancelled primary");
    let ManagedJudgeRunnerConfigurationError::PrimaryAndFinalValidation {
        primary,
        final_validation,
    } = error
    else {
        panic!("expected combined cancellation")
    };
    assert!(matches!(
        primary.as_ref(),
        ManagedJudgeRunnerConfigurationError::Cancelled
    ));
    assert_authority_error(
        &final_validation,
        ManagedJudgeRunnerConfigurationValidationPhase::Final,
        false,
        false,
        true,
    );
    assert_eq!(
        observed.borrow().as_slice(),
        [Event::Eval, Event::App, Event::Configuration]
    );
}

#[test]
fn cancellation_after_final_eval_prevents_app_revalidation_and_release() {
    let cancellation = CancellationToken::new();
    let subject = FakeSubject {
        cancel_on_eval: Some(2),
        ..FakeSubject::default()
    };
    let observed = subject.events.clone();
    let error = validate_configuration(subject, &cancellation).expect_err("cancelled final");
    assert_authority_error(
        &error,
        ManagedJudgeRunnerConfigurationValidationPhase::Final,
        false,
        false,
        true,
    );
    assert_eq!(
        observed.borrow().as_slice(),
        [Event::Eval, Event::App, Event::Configuration, Event::Eval,]
    );
}

#[test]
fn release_failure_is_returned_without_an_extra_authority_bracket() {
    let subject = FakeSubject {
        release_failure: Some(ManagedJudgeRunnerConfigurationRelationship::Runtime),
        ..FakeSubject::default()
    };
    let observed = subject.events.clone();
    assert!(matches!(
        validate_configuration(subject, &CancellationToken::new()),
        Err(ManagedJudgeRunnerConfigurationError::Relationship(
            ManagedJudgeRunnerConfigurationRelationship::Runtime
        ))
    ));
    assert_eq!(observed.borrow().last(), Some(&Event::Release));
}

#[test]
fn retained_authority_error_debug_exposes_only_failure_facts() {
    let error = ManagedJudgeRunnerConfigurationError::RetainedAuthorityValidation {
        eval: Some(Box::new(CandidateJudgePreparationError::Relationship(
            CandidateJudgePreparationRelationship::JudgePlan,
        ))),
        runtime: None,
        cancelled: false,
    };
    assert_eq!(
        format!("{error:?}"),
        "ManagedJudgeRunnerConfigurationError { kind: \"retained_authority_validation\", eval_failed: true, runtime_failed: false, cancelled: false, .. }"
    );
}

#[test]
fn concrete_compiler_and_exact_app_tuple_destructuring_remain_type_checked() {
    std::hint::black_box(super::configure_managed_judge_runner);
}

fn assert_authority_error(
    error: &ManagedJudgeRunnerConfigurationError,
    expected_phase: ManagedJudgeRunnerConfigurationValidationPhase,
    eval_failed: bool,
    app_failed: bool,
    cancelled: bool,
) {
    let ManagedJudgeRunnerConfigurationError::AuthorityValidation {
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
