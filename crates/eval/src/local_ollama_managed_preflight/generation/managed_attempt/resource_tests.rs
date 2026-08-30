use rewrite_app::GenerationQualificationResourceAttemptObservationError;
use rewrite_model::CandidateGenerationAttemptFailureCategoryV1;

use super::{
    ResourceAttemptSequenceEvent as Event, ResourceAttemptSequencingKernel,
    ResourceOperationScopeFacts, ResourcePretrafficDecision, resource_failure_category,
    resource_pretraffic_decision,
};

#[test]
fn exact_resource_attempt_start_sequence_passes() {
    let mut sequence = ResourceAttemptSequencingKernel::new();
    for event in [
        Event::PolicyAndScopeVerified,
        Event::ClockStarted,
        Event::FirstAttemptValidationStarted,
    ] {
        sequence.observe(event).expect("exact sequence");
    }
}

#[test]
fn every_resource_attempt_start_reordering_fails_closed() {
    for events in [
        [
            Event::ClockStarted,
            Event::PolicyAndScopeVerified,
            Event::FirstAttemptValidationStarted,
        ],
        [
            Event::PolicyAndScopeVerified,
            Event::FirstAttemptValidationStarted,
            Event::ClockStarted,
        ],
        [
            Event::FirstAttemptValidationStarted,
            Event::PolicyAndScopeVerified,
            Event::ClockStarted,
        ],
    ] {
        let mut sequence = ResourceAttemptSequencingKernel::new();
        assert!(events.into_iter().any(|event| matches!(
            sequence.observe(event),
            Err(GenerationQualificationResourceAttemptObservationError::ObservationInconsistent)
        )));
    }
}

#[test]
fn repeated_or_post_validation_events_fail_closed() {
    let mut sequence = ResourceAttemptSequencingKernel::new();
    sequence
        .observe(Event::PolicyAndScopeVerified)
        .expect("first event");
    assert!(sequence.observe(Event::PolicyAndScopeVerified).is_err());

    let mut complete = ResourceAttemptSequencingKernel::new();
    for event in [
        Event::PolicyAndScopeVerified,
        Event::ClockStarted,
        Event::FirstAttemptValidationStarted,
    ] {
        complete.observe(event).expect("exact sequence");
    }
    assert!(
        complete
            .observe(Event::FirstAttemptValidationStarted)
            .is_err()
    );
}

#[test]
fn resource_operation_scope_rejects_target_plan_and_suite_substitution() {
    let exact = ResourceOperationScopeFacts {
        target_matches: true,
        plan_matches: true,
        suite_matches: true,
    };
    assert!(exact.is_exact());
    for substituted in [
        ResourceOperationScopeFacts {
            target_matches: false,
            ..exact
        },
        ResourceOperationScopeFacts {
            plan_matches: false,
            ..exact
        },
        ResourceOperationScopeFacts {
            suite_matches: false,
            ..exact
        },
    ] {
        assert!(!substituted.is_exact());
    }
}

#[test]
fn resource_pretraffic_precedence_checks_cancellation_before_policy_and_scope() {
    assert!(matches!(
        resource_pretraffic_decision(
            true,
            || panic!("cancelled input must not inspect source disposition"),
            || panic!("cancelled input must not inspect policy binding"),
            || panic!("cancelled input must not inspect operation scope"),
        ),
        ResourcePretrafficDecision::Observation(
            GenerationQualificationResourceAttemptObservationError::Cancelled
        )
    ));
    assert!(matches!(
        resource_pretraffic_decision(
            false,
            || false,
            || panic!("denied input must not inspect policy binding"),
            || panic!("denied input must not inspect operation scope"),
        ),
        ResourcePretrafficDecision::Observation(
            GenerationQualificationResourceAttemptObservationError::PolicyDenied
        )
    ));
    assert!(matches!(
        resource_pretraffic_decision(
            false,
            || true,
            || false,
            || panic!("binding mismatch must not inspect operation scope"),
        ),
        ResourcePretrafficDecision::Observation(
            GenerationQualificationResourceAttemptObservationError::PolicyBindingMismatch
        )
    ));
}

#[test]
fn active_resource_pretraffic_preserves_scope_substitution_and_success() {
    assert_eq!(
        resource_pretraffic_decision(false, || true, || true, || false),
        ResourcePretrafficDecision::OperationScopeMismatch
    );
    assert_eq!(
        resource_pretraffic_decision(false, || true, || true, || true),
        ResourcePretrafficDecision::Proceed
    );
}

#[test]
fn every_resource_observation_failure_has_a_frozen_attempt_category() {
    for (error, expected) in [
        (
            GenerationQualificationResourceAttemptObservationError::Cancelled,
            CandidateGenerationAttemptFailureCategoryV1::Cancelled,
        ),
        (
            GenerationQualificationResourceAttemptObservationError::PolicyDenied,
            CandidateGenerationAttemptFailureCategoryV1::RelationshipMismatch,
        ),
        (
            GenerationQualificationResourceAttemptObservationError::PolicyBindingMismatch,
            CandidateGenerationAttemptFailureCategoryV1::RelationshipMismatch,
        ),
        (
            GenerationQualificationResourceAttemptObservationError::ResponseBindingMismatch,
            CandidateGenerationAttemptFailureCategoryV1::RelationshipMismatch,
        ),
        (
            GenerationQualificationResourceAttemptObservationError::WorkerBindingMismatch,
            CandidateGenerationAttemptFailureCategoryV1::RelationshipMismatch,
        ),
        (
            GenerationQualificationResourceAttemptObservationError::PackageBindingMismatch,
            CandidateGenerationAttemptFailureCategoryV1::RelationshipMismatch,
        ),
        (
            GenerationQualificationResourceAttemptObservationError::PackageRevalidationFailed,
            CandidateGenerationAttemptFailureCategoryV1::PackageRevalidationFailed,
        ),
        (
            GenerationQualificationResourceAttemptObservationError::MeasurementOverflow,
            CandidateGenerationAttemptFailureCategoryV1::ObservationMismatch,
        ),
        (
            GenerationQualificationResourceAttemptObservationError::ClockOrderInvalid,
            CandidateGenerationAttemptFailureCategoryV1::ObservationMismatch,
        ),
        (
            GenerationQualificationResourceAttemptObservationError::ObservationInconsistent,
            CandidateGenerationAttemptFailureCategoryV1::ObservationMismatch,
        ),
    ] {
        assert_eq!(resource_failure_category(error), expected);
    }
}
