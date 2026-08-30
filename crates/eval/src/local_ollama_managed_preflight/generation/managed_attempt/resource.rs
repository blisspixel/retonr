use std::time::Instant;

use rewrite_app::{
    CandidateAttemptPrecursorRunnerHandoff, GenerationQualificationResourceAttemptClock,
    GenerationQualificationResourceAttemptObservationError,
    GenerationQualificationResourceAttemptObservationInput, ReleasedGenerationEffectivePackageV2,
    RuntimePackageLease, VerifiedGenerationQualificationResourcePolicy,
    VerifiedManagedOllamaModelPackageLease,
};
use rewrite_model::{
    CandidateGenerationAttemptFailureCategoryV1, CandidateGenerationAttemptFailurePhaseV1,
    CandidateGenerationAttemptPrecursorV1, GenerationQualificationOperationPolicyV1,
    GenerationSystemRecordV1, PlannedCandidateAttemptV1,
};
use rewrite_types::CancellationToken;

use crate::active_generation_qualification_subject::ActiveGenerationQualificationBinding;

use super::error::{ManagedCandidateAttemptProgress, fixed_failure_facts};
use super::outcome::{
    ManagedCandidateAttemptExecutionOutcome, ResourceObservedManagedCandidateAttemptClosure,
    VerifiedManagedCandidateCompletion, failed_managed_attempt,
};
use super::{
    ManagedCandidateAttemptExecutionError, ManagedCandidateAttemptPrimaryFailure,
    ManagedCandidateAttemptRunInput, run_managed_candidate_attempt,
};
use crate::local_ollama_managed_preflight::generation::{
    GenerationQualificationLiveLifecycle, PendingManagedGenerationCompletion,
    PendingResourceObservedGenerationCompletion, deadline::CandidateOperationDeadline,
};

/// Exact resource policy plus ordinary managed-attempt inputs.
pub(crate) struct ResourceObservedManagedCandidateAttemptRunInput<'a> {
    /// Ordinary exact managed-attempt inputs.
    pub attempt: ManagedCandidateAttemptRunInput<'a>,
    /// Exact source-approved resource policy required before traffic.
    pub resource_policy: &'a VerifiedGenerationQualificationResourcePolicy,
    /// Exact operation policy designating the attempted target system.
    pub operation_policy: &'a GenerationQualificationOperationPolicyV1,
}

#[derive(Clone, Copy)]
pub(super) struct ResourceObservationRunContext<'a> {
    pub(super) policy: &'a VerifiedGenerationQualificationResourcePolicy,
    pub(super) operation_policy: &'a GenerationQualificationOperationPolicyV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ResourceAttemptSequenceEvent {
    PolicyAndScopeVerified,
    ClockStarted,
    FirstAttemptValidationStarted,
}

#[derive(Clone, Copy)]
pub(super) struct ResourceOperationScopeFacts {
    pub(super) target_matches: bool,
    pub(super) plan_matches: bool,
    pub(super) suite_matches: bool,
}

impl ResourceOperationScopeFacts {
    pub(super) const fn is_exact(self) -> bool {
        self.target_matches && self.plan_matches && self.suite_matches
    }
}

pub(super) fn resource_operation_scope_matches(
    operation_policy: &GenerationQualificationOperationPolicyV1,
    generation_system: &GenerationSystemRecordV1,
    precursor: &CandidateGenerationAttemptPrecursorV1,
    planned_attempt: &PlannedCandidateAttemptV1,
) -> bool {
    ResourceOperationScopeFacts {
        target_matches: operation_policy.target_generation_system_id()
            == generation_system.generation_system_id(),
        plan_matches: operation_policy.generation_qualification_plan_id()
            == precursor.qualification_plan_id(),
        suite_matches: operation_policy.suite_manifest_id() == planned_attempt.suite_manifest_id(),
    }
    .is_exact()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ResourcePretrafficDecision {
    Proceed,
    Observation(GenerationQualificationResourceAttemptObservationError),
    OperationScopeMismatch,
}

pub(super) fn resource_pretraffic_decision(
    cancelled: bool,
    source_approved: impl FnOnce() -> bool,
    policy_binding_matches: impl FnOnce() -> bool,
    operation_scope_matches: impl FnOnce() -> bool,
) -> ResourcePretrafficDecision {
    if cancelled {
        return ResourcePretrafficDecision::Observation(
            GenerationQualificationResourceAttemptObservationError::Cancelled,
        );
    }
    if !source_approved() {
        return ResourcePretrafficDecision::Observation(
            GenerationQualificationResourceAttemptObservationError::PolicyDenied,
        );
    }
    if !policy_binding_matches() {
        return ResourcePretrafficDecision::Observation(
            GenerationQualificationResourceAttemptObservationError::PolicyBindingMismatch,
        );
    }
    if !operation_scope_matches() {
        return ResourcePretrafficDecision::OperationScopeMismatch;
    }
    ResourcePretrafficDecision::Proceed
}

pub(super) struct ResourceAttemptSequencingKernel {
    next: u8,
}

impl ResourceAttemptSequencingKernel {
    pub(super) const fn new() -> Self {
        Self { next: 0 }
    }

    pub(super) fn observe(
        &mut self,
        event: ResourceAttemptSequenceEvent,
    ) -> Result<(), GenerationQualificationResourceAttemptObservationError> {
        let expected = match self.next {
            0 => ResourceAttemptSequenceEvent::PolicyAndScopeVerified,
            1 => ResourceAttemptSequenceEvent::ClockStarted,
            2 => ResourceAttemptSequenceEvent::FirstAttemptValidationStarted,
            _ => {
                return Err(
                    GenerationQualificationResourceAttemptObservationError::ObservationInconsistent,
                );
            }
        };
        if event != expected {
            return Err(
                GenerationQualificationResourceAttemptObservationError::ObservationInconsistent,
            );
        }
        self.next += 1;
        Ok(())
    }
}

pub(crate) async fn run_resource_observed_verified_managed_candidate_attempt_until(
    handoff: CandidateAttemptPrecursorRunnerHandoff<'_, '_, '_>,
    input: ResourceObservedManagedCandidateAttemptRunInput<'_>,
    active_binding: ActiveGenerationQualificationBinding,
    operation_deadline: Instant,
    lifecycle: &GenerationQualificationLiveLifecycle,
    cancellation: &CancellationToken,
) -> Result<ManagedCandidateAttemptExecutionOutcome, ManagedCandidateAttemptExecutionError> {
    let ResourceObservedManagedCandidateAttemptRunInput {
        attempt,
        resource_policy,
        operation_policy,
    } = input;
    Box::pin(run_managed_candidate_attempt(
        handoff,
        attempt,
        Some(ResourceObservationRunContext {
            policy: resource_policy,
            operation_policy,
        }),
        CandidateOperationDeadline::until(operation_deadline),
        lifecycle,
        cancellation,
    ))
    .await
    .map(|outcome| outcome.bind_active_subject(active_binding))
}

pub(super) struct FinishResourceCompletionInput<'a> {
    pub(super) pending: PendingManagedGenerationCompletion,
    pub(super) clock: Option<GenerationQualificationResourceAttemptClock>,
    pub(super) resource: Option<ResourceObservationRunContext<'a>>,
    pub(super) released_package: &'a ReleasedGenerationEffectivePackageV2,
    pub(super) runtime_package: &'a mut RuntimePackageLease,
    pub(super) model_package: &'a VerifiedManagedOllamaModelPackageLease,
    pub(super) cancellation: &'a CancellationToken,
}

pub(super) fn finish_resource_completion(
    input: FinishResourceCompletionInput<'_>,
) -> Result<
    VerifiedManagedCandidateCompletion,
    GenerationQualificationResourceAttemptObservationError,
> {
    match (input.pending, input.clock, input.resource) {
        (
            PendingManagedGenerationCompletion::Compatibility {
                response,
                residency_receipt,
            },
            None,
            None,
        ) => Ok(VerifiedManagedCandidateCompletion::Compatibility {
            response,
            residency_receipt,
        }),
        (
            PendingManagedGenerationCompletion::ResourceObserved(resource_observed),
            Some(clock),
            Some(resource),
        ) => {
            let PendingResourceObservedGenerationCompletion {
                completion,
                worker_observation,
                worker_evidence,
            } = *resource_observed;
            let observation =
                clock.finish(GenerationQualificationResourceAttemptObservationInput {
                    policy: resource.policy,
                    operation_policy: resource.operation_policy,
                    completion: *completion,
                    worker_observation: &worker_observation,
                    worker_evidence: &worker_evidence,
                    released_package: input.released_package,
                    runtime_package: input.runtime_package,
                    model_package: input.model_package,
                    cancellation: input.cancellation,
                })?;
            Ok(VerifiedManagedCandidateCompletion::ResourceObserved(
                Box::new(ResourceObservedManagedCandidateAttemptClosure {
                    observation,
                    worker_observation,
                    worker_evidence,
                }),
            ))
        }
        _ => Err(GenerationQualificationResourceAttemptObservationError::ObservationInconsistent),
    }
}

pub(super) fn resource_failure_outcome(
    planned_attempt: &PlannedCandidateAttemptV1,
    precursor: &CandidateGenerationAttemptPrecursorV1,
    error: GenerationQualificationResourceAttemptObservationError,
    phase: CandidateGenerationAttemptFailurePhaseV1,
    progress: &ManagedCandidateAttemptProgress,
) -> Result<ManagedCandidateAttemptExecutionOutcome, ManagedCandidateAttemptExecutionError> {
    let category = resource_failure_category(error);
    let facts = fixed_failure_facts(phase, category, progress);
    failed_managed_attempt(
        planned_attempt,
        precursor,
        ManagedCandidateAttemptPrimaryFailure::ResourceObservation(error),
        None,
        facts,
    )
    .map_err(ManagedCandidateAttemptExecutionError::from)
}

const fn resource_failure_category(
    error: GenerationQualificationResourceAttemptObservationError,
) -> CandidateGenerationAttemptFailureCategoryV1 {
    match error {
        GenerationQualificationResourceAttemptObservationError::Cancelled => {
            CandidateGenerationAttemptFailureCategoryV1::Cancelled
        }
        GenerationQualificationResourceAttemptObservationError::PolicyDenied
        | GenerationQualificationResourceAttemptObservationError::PolicyBindingMismatch
        | GenerationQualificationResourceAttemptObservationError::ResponseBindingMismatch
        | GenerationQualificationResourceAttemptObservationError::WorkerBindingMismatch
        | GenerationQualificationResourceAttemptObservationError::PackageBindingMismatch => {
            CandidateGenerationAttemptFailureCategoryV1::RelationshipMismatch
        }
        GenerationQualificationResourceAttemptObservationError::PackageRevalidationFailed => {
            CandidateGenerationAttemptFailureCategoryV1::PackageRevalidationFailed
        }
        GenerationQualificationResourceAttemptObservationError::MeasurementOverflow
        | GenerationQualificationResourceAttemptObservationError::ClockOrderInvalid
        | GenerationQualificationResourceAttemptObservationError::ObservationInconsistent => {
            CandidateGenerationAttemptFailureCategoryV1::ObservationMismatch
        }
    }
}

#[cfg(test)]
#[path = "resource_tests.rs"]
mod tests;
