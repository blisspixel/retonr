//! Cleanup-gated execution of one app-verified managed candidate attempt.

use rewrite_app::{
    CandidateAttemptPrecursorRunnerHandoff, GenerationQualificationResourceAttemptObservationError,
    VerifiedGenerationEffectivePackagePlan,
};
use rewrite_model::{
    CandidateGenerationAttemptFailureCategoryV1, CandidateGenerationAttemptFailurePhaseV1,
};
use rewrite_types::CancellationToken;

use crate::active_generation_qualification_subject::ActiveGenerationQualificationBinding;

use super::{
    LocalOllamaManagedGenerationError, ManagedGenerationObservationMode,
    deadline::CandidateOperationDeadline, live_lifecycle::GenerationQualificationLiveLifecycle,
    run_live_generation,
};

mod error;
mod failure;
mod input;
mod launch;
mod outcome;
mod pretraffic;
mod resource;
mod validation;

pub(super) use error::ManagedCandidateAttemptProgress;
pub use error::{
    ManagedCandidateAttemptCleanupFailures, ManagedCandidateAttemptExecutionError,
    ManagedCandidateAttemptFailureRecordError, ManagedCandidateAttemptPrimaryFailure,
};
use error::{
    RetainedBracketCleanupFailures, effective_package_plan_failure_facts,
    effective_package_release_failure_facts, fixed_failure_facts,
};
use failure::{generation_failure_outcome, override_terminal_category};
pub use input::ManagedCandidateAttemptRunInput;
use launch::{CandidateLaunchOperation, validate_and_launch_candidate};
pub(crate) use outcome::ResourceObservedManagedCandidateAttemptClosure;
pub use outcome::{
    FailedManagedCandidateAttempt, ManagedCandidateAttemptExecutionOutcome,
    VerifiedCompletedManagedCandidateAttempt,
};
use outcome::{complete_managed_attempt, failed_managed_attempt, split_pending_attempt};
use pretraffic::{CandidatePretrafficFailure, start_candidate_pretraffic};
pub(crate) use resource::ResourceObservedManagedCandidateAttemptRunInput;
pub(crate) use resource::run_resource_observed_verified_managed_candidate_attempt_until;
use resource::{
    FinishResourceCompletionInput, ResourceObservationRunContext, finish_resource_completion,
    resource_failure_outcome,
};

pub(crate) async fn run_verified_managed_candidate_attempt_until(
    handoff: CandidateAttemptPrecursorRunnerHandoff<'_, '_, '_>,
    input: ManagedCandidateAttemptRunInput<'_>,
    active_binding: ActiveGenerationQualificationBinding,
    operation_deadline: std::time::Instant,
    lifecycle: &GenerationQualificationLiveLifecycle,
    cancellation: &CancellationToken,
) -> Result<ManagedCandidateAttemptExecutionOutcome, ManagedCandidateAttemptExecutionError> {
    Box::pin(run_managed_candidate_attempt(
        handoff,
        input,
        None,
        CandidateOperationDeadline::until(operation_deadline),
        lifecycle,
        cancellation,
    ))
    .await
    .map(|outcome| outcome.bind_active_subject(active_binding))
}

#[expect(
    clippy::too_many_lines,
    reason = "the single live authority bracket keeps launch, observation, package release, and cleanup ordering auditable"
)]
async fn run_managed_candidate_attempt(
    handoff: CandidateAttemptPrecursorRunnerHandoff<'_, '_, '_>,
    input: ManagedCandidateAttemptRunInput<'_>,
    resource: Option<ResourceObservationRunContext<'_>>,
    operation_deadline: CandidateOperationDeadline,
    lifecycle: &GenerationQualificationLiveLifecycle,
    cancellation: &CancellationToken,
) -> Result<ManagedCandidateAttemptExecutionOutcome, ManagedCandidateAttemptExecutionError> {
    let model_package = handoff.model_package();
    let (
        runtime_package,
        launch_plan,
        characterized_package,
        planned_attempt,
        generation_system,
        expected_runtime_state,
        generation_request,
        structured_request,
        precursor,
    ) = handoff.into_parts();

    let progress = ManagedCandidateAttemptProgress::new();
    macro_rules! generation_step {
        ($result:expr) => {{
            if let Err(error) = operation_deadline.ensure_active(cancellation) {
                return generation_failure_outcome(
                    &planned_attempt,
                    &precursor,
                    error,
                    &progress,
                    None,
                );
            }
            match operation_deadline.precedence($result, cancellation) {
                Ok(value) => value,
                Err(error) => {
                    return generation_failure_outcome(
                        &planned_attempt,
                        &precursor,
                        error,
                        &progress,
                        None,
                    );
                }
            }
        }};
    }

    let mut live_reservation = match lifecycle.reserve() {
        Ok(reservation) => reservation,
        Err(_error) => {
            return generation_failure_outcome(
                &planned_attempt,
                &precursor,
                LocalOllamaManagedGenerationError::InvalidGenerationAuthority,
                &progress,
                None,
            );
        }
    };

    if let Err(error) = operation_deadline.ensure_active(cancellation) {
        return generation_failure_outcome(&planned_attempt, &precursor, error, &progress, None);
    }

    let pretraffic = match start_candidate_pretraffic(
        resource,
        &generation_system,
        &precursor,
        &planned_attempt,
        operation_deadline,
        cancellation,
    ) {
        Ok(pretraffic) => pretraffic,
        Err(CandidatePretrafficFailure::Gate(error)) => {
            return generation_failure_outcome(
                &planned_attempt,
                &precursor,
                error,
                &progress,
                None,
            );
        }
        Err(CandidatePretrafficFailure::Observation(error)) => {
            return resource_failure_outcome(
                &planned_attempt,
                &precursor,
                error,
                CandidateGenerationAttemptFailurePhaseV1::PreTrafficRevalidation,
                &progress,
            );
        }
        Err(CandidatePretrafficFailure::OperationScopeMismatch) => {
            let facts = fixed_failure_facts(
                CandidateGenerationAttemptFailurePhaseV1::PreTrafficRevalidation,
                CandidateGenerationAttemptFailureCategoryV1::RelationshipMismatch,
                &progress,
            );
            return failed_managed_attempt(
                &planned_attempt,
                &precursor,
                ManagedCandidateAttemptPrimaryFailure::ResourceOperationScopeMismatch,
                None,
                facts,
            )
            .map_err(ManagedCandidateAttemptExecutionError::from);
        }
    };
    let resource_clock = pretraffic.clock;

    let prepared_launch = generation_step!(validate_and_launch_candidate(
        &input,
        runtime_package,
        launch_plan,
        &structured_request,
        CandidateLaunchOperation {
            progress: &progress,
            operation_deadline,
            live_reservation: &mut live_reservation,
            cancellation,
        },
    ));
    let helper = prepared_launch.helper;
    let cloud_status = prepared_launch.cloud_status;
    let isolation_lease = prepared_launch.isolation_lease;

    let pending = run_live_generation(
        input.runtime_manifest,
        runtime_package,
        input.isolation,
        input.preflight_plan,
        helper,
        input.admitted_runtime,
        input.generation_path,
        input.frozen_components,
        cloud_status,
        input.limits,
        input.worker_limits,
        input.static_model,
        input.model,
        structured_request.clone(),
        &isolation_lease,
        if resource.is_some() {
            ManagedGenerationObservationMode::ResourceObserved
        } else {
            ManagedGenerationObservationMode::Compatibility
        },
        &progress,
        operation_deadline,
        cancellation,
    )
    .await;

    let pending = operation_deadline.precedence(pending, cancellation);
    let (pending, candidates) = match pending.and_then(|pending| {
        operation_deadline.ensure_active(cancellation)?;
        let candidates = validation::validate_pending_attempt(
            &pending,
            &precursor,
            &planned_attempt,
            &generation_system,
            &expected_runtime_state,
            &generation_request,
            &structured_request,
            input.admitted_runtime,
            input.generation_path,
            input.frozen_components,
        )?;
        operation_deadline.precedence(Ok((pending, candidates)), cancellation)
    }) {
        Ok(result) => result,
        Err(operation) => {
            let (operation, (cleanup, runtime)) =
                operation_deadline.finalize_failure(operation, cancellation, || {
                    (
                        isolation_lease.close(&CancellationToken::new()),
                        runtime_package.revalidate(&CancellationToken::new()),
                    )
                });
            let retained_cleanup =
                RetainedBracketCleanupFailures::new(cleanup.err(), runtime.err());
            return generation_failure_outcome(
                &planned_attempt,
                &precursor,
                operation,
                &progress,
                retained_cleanup,
            );
        }
    };

    let observed_state = pending.effective_runtime_state.state().clone();
    let effective_state_join_id = pending
        .effective_runtime_state
        .effective_runtime_state_join_id();
    let runtime_build = pending.managed_build.runtime_build().clone();
    let (live_effective_state, mut bracket_records) = split_pending_attempt(pending);
    progress.set_phase(CandidateGenerationAttemptFailurePhaseV1::ManagedEvidenceCompilation);
    if let Some(operation) = operation_deadline.terminal_override(cancellation) {
        let (operation, (cleanup, runtime)) =
            operation_deadline.finalize_failure(operation, cancellation, || {
                (
                    isolation_lease.close(&CancellationToken::new()),
                    runtime_package.revalidate(&CancellationToken::new()),
                )
            });
        let retained_cleanup = RetainedBracketCleanupFailures::new(cleanup.err(), runtime.err());
        return generation_failure_outcome(
            &planned_attempt,
            &precursor,
            operation,
            &progress,
            retained_cleanup,
        );
    }
    let effective_package_plan = match VerifiedGenerationEffectivePackagePlan::prepare(
        input.runtime_manifest,
        runtime_package,
        model_package,
        &runtime_build,
        live_effective_state,
        input.admitted_runtime,
        input.generation_path,
        input.frozen_components,
        input.isolation,
        isolation_lease,
        cancellation,
    ) {
        Ok(plan) => plan,
        Err(error) => {
            let mut facts = effective_package_plan_failure_facts(&error, &progress);
            override_terminal_category(
                &mut facts,
                operation_deadline.terminal_override(cancellation).as_ref(),
                &progress,
            );
            return failed_managed_attempt(
                &planned_attempt,
                &precursor,
                ManagedCandidateAttemptPrimaryFailure::EffectivePackagePlan(error),
                None,
                facts,
            )
            .map_err(ManagedCandidateAttemptExecutionError::from);
        }
    };
    progress.set_phase(CandidateGenerationAttemptFailurePhaseV1::Cleanup);
    let release_result = effective_package_plan.release();
    let terminal_override = operation_deadline.terminal_override(cancellation);
    let effective_package = match release_result {
        Ok(package) => package,
        Err(error) => {
            let mut facts = effective_package_release_failure_facts(&error, &progress);
            override_terminal_category(&mut facts, terminal_override.as_ref(), &progress);
            return failed_managed_attempt(
                &planned_attempt,
                &precursor,
                ManagedCandidateAttemptPrimaryFailure::EffectivePackageRelease(error),
                None,
                facts,
            )
            .map_err(ManagedCandidateAttemptExecutionError::from);
        }
    };
    if let Some(error) = terminal_override {
        return generation_failure_outcome(&planned_attempt, &precursor, error, &progress, None);
    }

    if let Err(error) = operation_deadline.ensure_active(cancellation) {
        return generation_failure_outcome(&planned_attempt, &precursor, error, &progress, None);
    }
    let characterized_package_matches = effective_package.evidence()
        == characterized_package.evidence()
        && effective_package.foundation_id() == characterized_package.foundation_id()
        && effective_package.license_control_id() == characterized_package.license_control_id();
    if let Some(error) = operation_deadline.terminal_override(cancellation) {
        return generation_failure_outcome(&planned_attempt, &precursor, error, &progress, None);
    }
    if !characterized_package_matches {
        let facts = fixed_failure_facts(
            CandidateGenerationAttemptFailurePhaseV1::ManagedEvidenceCompilation,
            CandidateGenerationAttemptFailureCategoryV1::RelationshipMismatch,
            &progress,
        );
        return failed_managed_attempt(
            &planned_attempt,
            &precursor,
            ManagedCandidateAttemptPrimaryFailure::CharacterizedPackageMismatch,
            None,
            facts,
        )
        .map_err(ManagedCandidateAttemptExecutionError::from);
    }

    if let Err(error) = operation_deadline.ensure_active(cancellation) {
        return generation_failure_outcome(&planned_attempt, &precursor, error, &progress, None);
    }
    let pending_completion = bracket_records.take_completion();
    if let Some(error) = operation_deadline.terminal_override(cancellation) {
        return generation_failure_outcome(&planned_attempt, &precursor, error, &progress, None);
    }
    let Some(pending_completion) = pending_completion else {
        return resource_failure_outcome(
            &planned_attempt,
            &precursor,
            GenerationQualificationResourceAttemptObservationError::ObservationInconsistent,
            CandidateGenerationAttemptFailurePhaseV1::ManagedEvidenceCompilation,
            &progress,
        );
    };
    if let Err(error) = operation_deadline.ensure_active(cancellation) {
        return generation_failure_outcome(&planned_attempt, &precursor, error, &progress, None);
    }
    let completion_result = finish_resource_completion(FinishResourceCompletionInput {
        pending: pending_completion,
        clock: resource_clock,
        resource,
        released_package: &effective_package,
        runtime_package,
        model_package,
        cancellation,
    });
    if let Some(error) = operation_deadline.terminal_override(cancellation) {
        return generation_failure_outcome(&planned_attempt, &precursor, error, &progress, None);
    }
    let completion = match completion_result {
        Ok(completion) => completion,
        Err(error) => {
            return resource_failure_outcome(
                &planned_attempt,
                &precursor,
                error,
                CandidateGenerationAttemptFailurePhaseV1::ManagedEvidenceCompilation,
                &progress,
            );
        }
    };

    if let Err(error) = operation_deadline.ensure_active(cancellation) {
        return generation_failure_outcome(&planned_attempt, &precursor, error, &progress, None);
    }
    let complete_result = complete_managed_attempt(
        bracket_records,
        completion,
        observed_state,
        effective_state_join_id,
        effective_package,
        planned_attempt.clone(),
        generation_system,
        generation_request,
        structured_request,
        precursor.clone(),
        candidates,
    );
    if let Some(error) = operation_deadline.terminal_override(cancellation) {
        return generation_failure_outcome(&planned_attempt, &precursor, error, &progress, None);
    }
    match complete_result {
        Ok(completed) => Ok(ManagedCandidateAttemptExecutionOutcome::Completed(
            Box::new(completed),
        )),
        Err(failure) => {
            let (error, planned_attempt, precursor) = *failure;
            let facts = fixed_failure_facts(
                CandidateGenerationAttemptFailurePhaseV1::ManagedEvidenceCompilation,
                CandidateGenerationAttemptFailureCategoryV1::ManagedEvidenceInvalid,
                &progress,
            );
            failed_managed_attempt(
                &planned_attempt,
                &precursor,
                ManagedCandidateAttemptPrimaryFailure::Record(error),
                None,
                facts,
            )
            .map_err(ManagedCandidateAttemptExecutionError::from)
        }
    }
}

#[cfg(test)]
pub(in crate::local_ollama_managed_preflight::generation) fn failure_facts_for_test(
    phase: CandidateGenerationAttemptFailurePhaseV1,
    error: &super::LocalOllamaManagedGenerationError,
) -> (
    CandidateGenerationAttemptFailurePhaseV1,
    CandidateGenerationAttemptFailureCategoryV1,
    rewrite_model::CandidateGenerationAttemptCleanupDispositionV1,
) {
    let progress = ManagedCandidateAttemptProgress::new();
    progress.set_phase(phase);
    let facts = progress.generation_failure_facts(error);
    (
        facts.failure_phase,
        facts.failure_category,
        facts.cleanup_disposition,
    )
}

#[cfg(test)]
#[path = "managed_attempt/tests.rs"]
mod tests;
