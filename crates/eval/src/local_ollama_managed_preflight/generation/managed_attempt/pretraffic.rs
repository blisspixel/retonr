use rewrite_app::{
    GenerationQualificationPhasePolicySourceDisposition,
    GenerationQualificationResourceAttemptClock,
    GenerationQualificationResourceAttemptObservationError,
};
use rewrite_model::{
    CandidateGenerationAttemptPrecursorV1, GenerationSystemRecordV1, PlannedCandidateAttemptV1,
};
use rewrite_types::CancellationToken;

use super::resource::{
    ResourceAttemptSequenceEvent, ResourceAttemptSequencingKernel, ResourceObservationRunContext,
    ResourcePretrafficDecision, resource_operation_scope_matches, resource_pretraffic_decision,
};
use crate::local_ollama_managed_preflight::generation::{
    LocalOllamaManagedGenerationError, deadline::CandidateOperationDeadline,
};

pub(super) struct CandidatePretrafficSuccess {
    pub(super) clock: Option<GenerationQualificationResourceAttemptClock>,
}

pub(super) enum CandidatePretrafficFailure {
    Gate(LocalOllamaManagedGenerationError),
    Observation(GenerationQualificationResourceAttemptObservationError),
    OperationScopeMismatch,
}

pub(super) fn start_candidate_pretraffic(
    resource: Option<ResourceObservationRunContext<'_>>,
    generation_system: &GenerationSystemRecordV1,
    precursor: &CandidateGenerationAttemptPrecursorV1,
    planned_attempt: &PlannedCandidateAttemptV1,
    operation_deadline: CandidateOperationDeadline,
    cancellation: &CancellationToken,
) -> Result<CandidatePretrafficSuccess, CandidatePretrafficFailure> {
    ensure_active(operation_deadline, cancellation)?;
    let Some(resource) = resource else {
        return Ok(CandidatePretrafficSuccess { clock: None });
    };
    let decision = resource_pretraffic_decision(
        cancellation.is_cancelled(),
        || {
            resource.policy.source_disposition()
                == GenerationQualificationPhasePolicySourceDisposition::Approved
        },
        || {
            resource
                .policy
                .revalidate_operation_policy(resource.operation_policy)
                .is_ok()
        },
        || {
            resource_operation_scope_matches(
                resource.operation_policy,
                generation_system,
                precursor,
                planned_attempt,
            )
        },
    );
    ensure_active(operation_deadline, cancellation)?;
    match decision {
        ResourcePretrafficDecision::Proceed => {}
        ResourcePretrafficDecision::Observation(error) => {
            return Err(CandidatePretrafficFailure::Observation(error));
        }
        ResourcePretrafficDecision::OperationScopeMismatch => {
            return Err(CandidatePretrafficFailure::OperationScopeMismatch);
        }
    }

    let mut sequence = ResourceAttemptSequencingKernel::new();
    observe_sequence(
        &mut sequence,
        ResourceAttemptSequenceEvent::PolicyAndScopeVerified,
        operation_deadline,
        cancellation,
    )?;
    ensure_active(operation_deadline, cancellation)?;
    let clock = GenerationQualificationResourceAttemptClock::start(
        resource.policy,
        resource.operation_policy,
        cancellation,
    );
    ensure_active(operation_deadline, cancellation)?;
    let clock = clock.map_err(CandidatePretrafficFailure::Observation)?;
    observe_sequence(
        &mut sequence,
        ResourceAttemptSequenceEvent::ClockStarted,
        operation_deadline,
        cancellation,
    )?;
    observe_sequence(
        &mut sequence,
        ResourceAttemptSequenceEvent::FirstAttemptValidationStarted,
        operation_deadline,
        cancellation,
    )?;
    Ok(CandidatePretrafficSuccess { clock: Some(clock) })
}

fn observe_sequence(
    sequence: &mut ResourceAttemptSequencingKernel,
    event: ResourceAttemptSequenceEvent,
    operation_deadline: CandidateOperationDeadline,
    cancellation: &CancellationToken,
) -> Result<(), CandidatePretrafficFailure> {
    ensure_active(operation_deadline, cancellation)?;
    let result = sequence.observe(event);
    ensure_active(operation_deadline, cancellation)?;
    result.map_err(CandidatePretrafficFailure::Observation)
}

fn ensure_active(
    operation_deadline: CandidateOperationDeadline,
    cancellation: &CancellationToken,
) -> Result<(), CandidatePretrafficFailure> {
    operation_deadline
        .ensure_active(cancellation)
        .map_err(CandidatePretrafficFailure::Gate)
}
