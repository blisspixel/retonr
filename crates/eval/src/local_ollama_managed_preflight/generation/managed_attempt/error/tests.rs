use rewrite_app::effective_runtime_state_observation::EffectiveRuntimeStateObservationError;
use rewrite_app::{ManagedOllamaLaunchError, ManagedOllamaPostAcquisitionFailure};
use rewrite_inference::{CandidateOutputError, InferenceError, InferenceErrorKind};
use rewrite_runtime_attestor::ManagedGenerationWorkerError;
use rewrite_runtime_isolation::IsolationError;

use crate::LocalOllamaManagedPreflightError;

use super::classification::{post_acquisition_failure_category, post_acquisition_failure_phase};
use super::*;

#[test]
fn progress_uses_observed_generation_boundaries() {
    let progress = ManagedCandidateAttemptProgress::new();
    progress.observe_response(
        OllamaResponseObservationPhase::AfterResponse { ordinal: 7 },
        7,
        11,
    );
    let before = progress
        .generation_failure_facts(&LocalOllamaManagedGenerationError::InvalidGenerationAuthority);
    assert!(!before.traffic_observed);
    assert!(!before.output_observed);

    progress.observe_response(
        OllamaResponseObservationPhase::AfterFailedAttempt {
            completed_responses: 7,
        },
        7,
        11,
    );
    let traffic = progress.generation_failure_facts(&LocalOllamaManagedGenerationError::Session(
        InferenceError::new(InferenceErrorKind::Retryable, "closed"),
    ));
    assert!(traffic.traffic_observed);
    assert!(!traffic.output_observed);
    assert_eq!(
        traffic.failure_phase,
        CandidateGenerationAttemptFailurePhaseV1::PreTrafficRevalidation
    );

    progress.observe_response(
        OllamaResponseObservationPhase::AfterResponse { ordinal: 11 },
        7,
        11,
    );
    let output =
        progress.generation_failure_facts(&LocalOllamaManagedGenerationError::ResponseValidation(
            CandidateOutputError::InvalidEnvelope,
        ));
    assert!(output.traffic_observed);
    assert!(output.output_observed);
    assert_eq!(
        output.failure_phase,
        CandidateGenerationAttemptFailurePhaseV1::ResponseValidation
    );
    assert_eq!(
        output.failure_category,
        CandidateGenerationAttemptFailureCategoryV1::ResponseInvalid
    );
}

#[test]
fn final_observation_and_launch_are_not_collapsed() {
    let progress = ManagedCandidateAttemptProgress::new();
    progress.set_phase(CandidateGenerationAttemptFailurePhaseV1::Launch);
    let launch = progress.generation_failure_facts(&LocalOllamaManagedGenerationError::Launch(
        ManagedOllamaLaunchError::RelationshipMismatch,
    ));
    assert_eq!(
        launch.failure_phase,
        CandidateGenerationAttemptFailurePhaseV1::Launch
    );
    assert_eq!(
        launch.failure_category,
        CandidateGenerationAttemptFailureCategoryV1::RelationshipMismatch
    );

    progress.observe_response(
        OllamaResponseObservationPhase::AfterResponse { ordinal: 12 },
        7,
        11,
    );
    let observation = progress.generation_failure_facts(
        &LocalOllamaManagedGenerationError::InvalidCandidateAttemptRelationship,
    );
    assert_eq!(
        observation.failure_phase,
        CandidateGenerationAttemptFailurePhaseV1::FinalObservation
    );
    assert!(observation.traffic_observed);
    assert!(!observation.output_observed);
}

#[test]
fn post_acquisition_classification_preserves_primary_until_a_finalizer_fails() {
    for (primary, expected_category) in [
        (
            ManagedOllamaPostAcquisitionFailure::InputBindingMismatch,
            CandidateGenerationAttemptFailureCategoryV1::RelationshipMismatch,
        ),
        (
            ManagedOllamaPostAcquisitionFailure::Cancelled,
            CandidateGenerationAttemptFailureCategoryV1::Cancelled,
        ),
    ] {
        assert_eq!(
            post_acquisition_failure_phase(false),
            CandidateGenerationAttemptFailurePhaseV1::Launch
        );
        assert_eq!(
            post_acquisition_failure_category(primary, false),
            expected_category
        );
        assert_eq!(
            post_acquisition_failure_phase(true),
            CandidateGenerationAttemptFailurePhaseV1::Cleanup
        );
        assert_eq!(
            post_acquisition_failure_category(primary, true),
            CandidateGenerationAttemptFailureCategoryV1::CleanupFailed
        );
    }
}

#[test]
fn retained_bracket_preserves_dual_cleanup_failures() {
    let retained = RetainedBracketCleanupFailures::new(
        Some(ManagedOllamaCloseError::Isolation(
            IsolationError::Cancelled,
        )),
        Some(PackageAttestationError::InvalidLimits),
    )
    .expect("dual failure aggregate");
    let ManagedCandidateAttemptCleanupFailures::RetainedBracket {
        cleanup,
        runtime_package,
    } = retained.view()
    else {
        panic!("retained bracket view");
    };
    assert!(matches!(
        cleanup,
        Some(ManagedOllamaCloseError::Isolation(
            IsolationError::Cancelled
        ))
    ));
    assert!(matches!(
        runtime_package,
        Some(PackageAttestationError::InvalidLimits)
    ));
}

#[test]
fn primary_failure_survives_dual_cleanup_accounting() {
    let primary = ManagedCandidateAttemptPrimaryFailure::Generation(
        LocalOllamaManagedGenerationError::ResponseValidation(CandidateOutputError::CountMismatch),
    );
    let retained = RetainedBracketCleanupFailures::new(
        Some(ManagedOllamaCloseError::Isolation(
            IsolationError::Cancelled,
        )),
        Some(PackageAttestationError::InvalidLimits),
    );
    let view = cleanup_failures(&primary, retained.as_ref()).expect("cleanup failures");
    assert!(matches!(
        primary,
        ManagedCandidateAttemptPrimaryFailure::Generation(
            LocalOllamaManagedGenerationError::ResponseValidation(
                CandidateOutputError::CountMismatch
            )
        )
    ));
    assert!(matches!(
        view,
        ManagedCandidateAttemptCleanupFailures::RetainedBracket {
            cleanup: Some(_),
            runtime_package: Some(_),
        }
    ));
}

#[test]
fn embedded_cleanup_keeps_primary_operation_facts_and_failed_disposition() {
    let progress = ManagedCandidateAttemptProgress::new();
    progress.observe_response(
        OllamaResponseObservationPhase::AfterResponse { ordinal: 11 },
        7,
        11,
    );
    let error = LocalOllamaManagedGenerationError::CleanupAfterFailure {
        operation: Box::new(LocalOllamaManagedGenerationError::ResponseValidation(
            CandidateOutputError::InvalidEnvelope,
        )),
        cleanup: ManagedOllamaCloseError::Isolation(IsolationError::Cancelled),
    };
    let facts = progress.generation_failure_facts(&error);
    assert_eq!(
        facts.failure_phase,
        CandidateGenerationAttemptFailurePhaseV1::ResponseValidation
    );
    assert_eq!(
        facts.failure_category,
        CandidateGenerationAttemptFailureCategoryV1::ResponseInvalid
    );
    assert_eq!(
        facts.cleanup_disposition,
        CandidateGenerationAttemptCleanupDispositionV1::Failed
    );

    let primary = ManagedCandidateAttemptPrimaryFailure::Generation(error);
    assert!(matches!(
        cleanup_failures(&primary, None),
        Some(ManagedCandidateAttemptCleanupFailures::EmbeddedGeneration(
            LocalOllamaManagedGenerationError::CleanupAfterFailure { .. }
        ))
    ));
}

#[test]
fn embedded_and_outer_cleanup_failures_are_both_visible() {
    let primary = ManagedCandidateAttemptPrimaryFailure::Generation(
        LocalOllamaManagedGenerationError::CleanupAfterFailure {
            operation: Box::new(LocalOllamaManagedGenerationError::ResponseValidation(
                CandidateOutputError::InvalidEnvelope,
            )),
            cleanup: ManagedOllamaCloseError::Isolation(IsolationError::Cancelled),
        },
    );
    let retained = RetainedBracketCleanupFailures::new(
        Some(ManagedOllamaCloseError::Isolation(
            IsolationError::Cancelled,
        )),
        Some(PackageAttestationError::InvalidLimits),
    );
    assert!(matches!(
        cleanup_failures(&primary, retained.as_ref()),
        Some(
            ManagedCandidateAttemptCleanupFailures::RetainedAndEmbeddedGeneration {
                embedded_generation: LocalOllamaManagedGenerationError::CleanupAfterFailure { .. },
                cleanup: Some(_),
                runtime_package: Some(_),
            }
        )
    ));
}

#[test]
fn nested_managed_cleanup_variants_remain_consistent_and_visible() {
    let progress = ManagedCandidateAttemptProgress::new();
    let direct = LocalOllamaManagedGenerationError::Managed(
        LocalOllamaManagedPreflightError::Cleanup(IsolationError::Cancelled),
    );
    let direct_facts = progress.generation_failure_facts(&direct);
    assert_eq!(
        direct_facts.failure_phase,
        CandidateGenerationAttemptFailurePhaseV1::Cleanup
    );
    assert_eq!(
        direct_facts.failure_category,
        CandidateGenerationAttemptFailureCategoryV1::CleanupFailed
    );
    assert_eq!(
        direct_facts.cleanup_disposition,
        CandidateGenerationAttemptCleanupDispositionV1::Failed
    );

    let after = LocalOllamaManagedGenerationError::Managed(
        LocalOllamaManagedPreflightError::CleanupAfterFailure {
            operation: Box::new(LocalOllamaManagedPreflightError::InvalidInput),
            cleanup: IsolationError::Cancelled,
        },
    );
    let after_facts = progress.generation_failure_facts(&after);
    assert_eq!(
        after_facts.failure_phase,
        CandidateGenerationAttemptFailurePhaseV1::PreTrafficRevalidation
    );
    assert_eq!(
        after_facts.failure_category,
        CandidateGenerationAttemptFailureCategoryV1::RequestInvalid
    );
    assert_eq!(
        after_facts.cleanup_disposition,
        CandidateGenerationAttemptCleanupDispositionV1::Failed
    );

    let primary = ManagedCandidateAttemptPrimaryFailure::Generation(after);
    assert!(matches!(
        cleanup_failures(&primary, None),
        Some(ManagedCandidateAttemptCleanupFailures::EmbeddedGeneration(
            LocalOllamaManagedGenerationError::Managed(
                LocalOllamaManagedPreflightError::CleanupAfterFailure { .. }
            )
        ))
    ));
}

#[test]
fn worker_and_effective_state_categories_preserve_closed_causes() {
    for (error, expected) in [
        (
            ManagedGenerationWorkerError::InvalidRequest,
            CandidateGenerationAttemptFailureCategoryV1::RequestInvalid,
        ),
        (
            ManagedGenerationWorkerError::Cancelled,
            CandidateGenerationAttemptFailureCategoryV1::Cancelled,
        ),
        (
            ManagedGenerationWorkerError::DeadlineExceeded,
            CandidateGenerationAttemptFailureCategoryV1::DeadlineExceeded,
        ),
        (
            ManagedGenerationWorkerError::Unsupported,
            CandidateGenerationAttemptFailureCategoryV1::UnsupportedPlatform,
        ),
        (
            ManagedGenerationWorkerError::WorkerChanged,
            CandidateGenerationAttemptFailureCategoryV1::ObservationMismatch,
        ),
    ] {
        let actual = ManagedCandidateAttemptProgress::new()
            .generation_failure_facts(&LocalOllamaManagedGenerationError::Worker(error));
        assert_eq!(actual.failure_category, expected);
    }

    for (error, expected) in [
        (
            EffectiveRuntimeStateObservationError::Cancelled,
            CandidateGenerationAttemptFailureCategoryV1::Cancelled,
        ),
        (
            EffectiveRuntimeStateObservationError::UnsupportedProfile,
            CandidateGenerationAttemptFailureCategoryV1::UnsupportedPlatform,
        ),
        (
            EffectiveRuntimeStateObservationError::RelationshipMismatch,
            CandidateGenerationAttemptFailureCategoryV1::RelationshipMismatch,
        ),
        (
            EffectiveRuntimeStateObservationError::InvalidPlatformObservation,
            CandidateGenerationAttemptFailureCategoryV1::ObservationMismatch,
        ),
    ] {
        let actual = ManagedCandidateAttemptProgress::new()
            .generation_failure_facts(&LocalOllamaManagedGenerationError::EffectiveState(error));
        assert_eq!(actual.failure_category, expected);
    }
}

#[test]
fn isolation_categories_preserve_closed_causes_by_phase() {
    for (error, expected) in [
        (
            IsolationError::Cancelled,
            CandidateGenerationAttemptFailureCategoryV1::Cancelled,
        ),
        (
            IsolationError::UnsupportedPlatform,
            CandidateGenerationAttemptFailureCategoryV1::UnsupportedPlatform,
        ),
        (
            IsolationError::StartupTimeout,
            CandidateGenerationAttemptFailureCategoryV1::DeadlineExceeded,
        ),
        (
            IsolationError::InvalidPolicy("policy"),
            CandidateGenerationAttemptFailureCategoryV1::RequestInvalid,
        ),
        (
            IsolationError::NamespaceSetup,
            CandidateGenerationAttemptFailureCategoryV1::LaunchFailed,
        ),
    ] {
        let actual = ManagedCandidateAttemptProgress::new().generation_failure_facts(
            &LocalOllamaManagedGenerationError::Launch(ManagedOllamaLaunchError::Isolation(error)),
        );
        assert_eq!(actual.failure_category, expected);
    }

    let preflight = ManagedCandidateAttemptProgress::new().generation_failure_facts(
        &LocalOllamaManagedGenerationError::Managed(LocalOllamaManagedPreflightError::Isolation(
            IsolationError::EvidenceChanged,
        )),
    );
    assert_eq!(
        preflight.failure_category,
        CandidateGenerationAttemptFailureCategoryV1::ObservationMismatch
    );
}
