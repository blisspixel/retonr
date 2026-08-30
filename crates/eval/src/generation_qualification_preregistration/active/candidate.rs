//! Exact plan-order candidate traffic owned by one Active operation.

mod outcome;

use std::fmt;

use rewrite_app::{
    CandidateAttemptOperationScopeError, CandidateAttemptPrecursorRunnerHandoff,
    VerifiedGenerationQualificationResourcePolicy,
};
use rewrite_model::{
    GenerationQualificationOperationPolicyV1, GenerationQualificationRequestProjectionEntryV1,
    PlannedCandidateAttemptV1,
};
use rewrite_model_store::WriteDisposition;
use rewrite_types::CancellationToken;
use thiserror::Error;

use super::{
    ActiveGenerationQualificationOperation, GenerationQualificationActivationError,
    ensure_traffic_eligible, map_preparation_error,
};
use crate::{
    FailedManagedCandidateAttempt, ManagedCandidateAttemptExecutionError,
    ManagedCandidateAttemptExecutionOutcome, ManagedCandidateAttemptRunInput,
    active_generation_qualification_subject::ActiveGenerationQualificationSubject,
    generation_qualification_preregistration::prepared::{
        PreparedGenerationQualificationValidationError,
        PreparedGenerationQualificationValidationView,
    },
    generation_qualification_preregistration::{
        GenerationQualificationPreparationError, GenerationQualificationPreregistrationRepository,
    },
    local_ollama_managed_preflight::{
        GenerationQualificationLiveLifecycle, ResourceObservedManagedCandidateAttemptRunInput,
        run_resource_observed_verified_managed_candidate_attempt_until,
        run_verified_managed_candidate_attempt_until,
    },
};

use self::outcome::combine_candidate_finalization;

/// Durable outcome released after Active owns all live completion authority.
pub enum ActiveGenerationQualificationCandidateRunOutcome {
    /// The completed authority is retained by Active and awaits durable closeout.
    CompletedPending,
    /// Exact failed closure after its portable terminal record committed.
    Failed(Box<FailedManagedCandidateAttempt>),
}

impl fmt::Debug for ActiveGenerationQualificationCandidateRunOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CompletedPending => formatter
                .write_str("ActiveGenerationQualificationCandidateRunOutcome::CompletedPending"),
            Self::Failed(failed) => formatter
                .debug_tuple("ActiveGenerationQualificationCandidateRunOutcome::Failed")
                .field(failed)
                .finish(),
        }
    }
}

/// Exact owned inputs for the next Active managed candidate attempt.
pub struct ActiveGenerationQualificationCandidateRunInput<'input, 'model, 'runtime, 'characterized>
{
    /// Exact app-owned runner handoff for the next planned attempt.
    pub handoff: CandidateAttemptPrecursorRunnerHandoff<'model, 'runtime, 'characterized>,
    /// Exact managed runtime, model, isolation, and observation inputs.
    pub attempt: ManagedCandidateAttemptRunInput<'input>,
    /// Required for a target attempt and forbidden for a baseline attempt.
    pub resource_policy: Option<&'input VerifiedGenerationQualificationResourcePolicy>,
}

/// Stable category for an Active managed candidate-attempt failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActiveGenerationQualificationCandidateRunErrorKind {
    /// The retained Active or Prepared authority failed validation.
    ActiveAuthority,
    /// A prior run failure terminalized this Active owner.
    OperationTerminated,
    /// Every preregistered candidate attempt has already been consumed.
    AttemptSequenceComplete,
    /// A completed attempt still needs durable batch settlement.
    CandidateSettlementPending,
    /// A checkpointed attempt has no terminal durable outcome yet.
    CandidateExecutionUnresolved,
    /// An exact checkpoint already exists and cannot authorize repeated traffic.
    CheckpointReplay,
    /// The handoff or observation mode did not match the next exact request.
    OperationScope,
    /// The strict managed candidate-attempt runner failed to produce an outcome.
    Execution,
    /// Durable checkpoint or terminal outcome persistence failed.
    Durability,
    /// Independent mandatory finalization validation failed.
    MandatoryFinalization,
    /// A primary error and mandatory finalization both failed.
    PrimaryAndFinalization,
}

/// Content-redacted failure from the Active managed candidate-attempt boundary.
#[derive(Error)]
pub enum ActiveGenerationQualificationCandidateRunError {
    /// The retained Active or Prepared authority failed validation.
    #[error("active generation qualification authority validation failed")]
    ActiveAuthority(#[source] GenerationQualificationActivationError),
    /// A prior failed run permanently terminalized this owner.
    #[error("active generation qualification operation is terminal")]
    OperationTerminated,
    /// No preregistered candidate attempt remains.
    #[error("active generation qualification candidate-attempt sequence is complete")]
    AttemptSequenceComplete,
    /// A completed attempt still needs durable batch settlement.
    #[error("active generation qualification candidate settlement is pending")]
    CandidateSettlementPending,
    /// A prior checkpoint has no terminal outcome and forbids another launch.
    #[error("active generation qualification candidate execution is unresolved")]
    CandidateExecutionUnresolved,
    /// An exact durable checkpoint already exists and does not authorize replay.
    #[error("active generation qualification candidate checkpoint cannot be replayed")]
    CheckpointReplay,
    /// The handoff or resource mode did not match the next exact operation request.
    #[error("active generation qualification candidate operation scope does not match")]
    OperationScope,
    /// The strict managed candidate-attempt runner could not produce an exact outcome.
    #[error("active generation qualification candidate execution failed")]
    Execution(#[source] Box<ManagedCandidateAttemptExecutionError>),
    /// Durable checkpoint or terminal persistence failed.
    #[error("active generation qualification candidate durability failed")]
    Durability(#[source] GenerationQualificationPreparationError),
    /// Independent mandatory finalization validation failed.
    #[error("active generation qualification mandatory finalization failed")]
    MandatoryFinalization,
    /// A primary error and mandatory finalization both failed.
    #[error("active generation qualification candidate and finalization both failed")]
    PrimaryAndFinalization {
        /// Content-redacted primary failure.
        primary: Box<ActiveGenerationQualificationCandidateRunError>,
    },
}

impl ActiveGenerationQualificationCandidateRunError {
    /// Returns the stable failure category without exposing live authority detail.
    #[must_use]
    pub const fn kind(&self) -> ActiveGenerationQualificationCandidateRunErrorKind {
        match self {
            Self::ActiveAuthority(_) => {
                ActiveGenerationQualificationCandidateRunErrorKind::ActiveAuthority
            }
            Self::OperationTerminated => {
                ActiveGenerationQualificationCandidateRunErrorKind::OperationTerminated
            }
            Self::AttemptSequenceComplete => {
                ActiveGenerationQualificationCandidateRunErrorKind::AttemptSequenceComplete
            }
            Self::CandidateSettlementPending => {
                ActiveGenerationQualificationCandidateRunErrorKind::CandidateSettlementPending
            }
            Self::CandidateExecutionUnresolved => {
                ActiveGenerationQualificationCandidateRunErrorKind::CandidateExecutionUnresolved
            }
            Self::CheckpointReplay => {
                ActiveGenerationQualificationCandidateRunErrorKind::CheckpointReplay
            }
            Self::OperationScope => {
                ActiveGenerationQualificationCandidateRunErrorKind::OperationScope
            }
            Self::Execution(_) => ActiveGenerationQualificationCandidateRunErrorKind::Execution,
            Self::Durability(_) => ActiveGenerationQualificationCandidateRunErrorKind::Durability,
            Self::MandatoryFinalization => {
                ActiveGenerationQualificationCandidateRunErrorKind::MandatoryFinalization
            }
            Self::PrimaryAndFinalization { .. } => {
                ActiveGenerationQualificationCandidateRunErrorKind::PrimaryAndFinalization
            }
        }
    }
}

impl fmt::Debug for ActiveGenerationQualificationCandidateRunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ActiveGenerationQualificationCandidateRunError")
            .field("kind", &self.kind())
            .finish_non_exhaustive()
    }
}

impl ActiveGenerationQualificationOperation<'_, '_, '_, '_, '_> {
    /// Runs the next exact plan-order candidate attempt under the Prepared clock.
    ///
    /// The handoff must match the next planned attempt and its projected
    /// provider-neutral and structured request bindings. Target attempts require
    /// the exact source-approved resource policy. Baseline attempts forbid that
    /// policy and use the compatibility observation shape. The one Active lifecycle
    /// and original deadline apply to both roles. A failed attempt consumes its
    /// ordinal and terminalizes the operation without retry. A completed outcome
    /// leaves its ordinal pending until
    /// [`ActiveGenerationQualificationOperation::close_pending_candidate`] owns
    /// publication, readback, receipt compilation, batch verification, and exact
    /// Active-subject settlement.
    ///
    /// # Errors
    ///
    /// Returns a content-redacted error for authority drift, sequence completion,
    /// scope mismatch, inability to construct an exact outcome, or mandatory
    /// finalization failure.
    pub async fn run_next_candidate(
        &mut self,
        repository: &mut GenerationQualificationPreregistrationRepository,
        input: ActiveGenerationQualificationCandidateRunInput<'_, '_, '_, '_>,
        cancellation: &CancellationToken,
    ) -> Result<
        ActiveGenerationQualificationCandidateRunOutcome,
        ActiveGenerationQualificationCandidateRunError,
    > {
        if self.terminal {
            return Err(ActiveGenerationQualificationCandidateRunError::OperationTerminated);
        }
        if self.started_candidate_attempt.is_some() {
            return Err(
                ActiveGenerationQualificationCandidateRunError::CandidateExecutionUnresolved,
            );
        }
        if self.pending_completed_candidate.is_some() {
            return Err(ActiveGenerationQualificationCandidateRunError::CandidateSettlementPending);
        }
        let scope = self
            .prepared
            .with_validated_view(cancellation, |view| {
                ensure_traffic_eligible(view.disposition)
                    .map_err(ActiveGenerationQualificationCandidateRunError::ActiveAuthority)?;
                let scope = candidate_scope(&view, self.next_candidate_attempt)?;
                validate_scoped_candidate(&input, &scope, cancellation)?;
                Ok(scope)
            })
            .map_err(map_candidate_pretraffic_validation_error)?;
        let precursor = input.handoff.precursor().clone();
        let checkpoint = repository
            .checkpoint_candidate_attempt(
                self.prepared.preregistration_read_input(),
                &precursor,
                scope.deadline,
                cancellation,
            )
            .map_err(ActiveGenerationQualificationCandidateRunError::Durability)?;
        if checkpoint == WriteDisposition::AlreadyPresent {
            self.terminal = true;
            return Err(ActiveGenerationQualificationCandidateRunError::CheckpointReplay);
        }
        self.started_candidate_attempt =
            Some(super::attempt_ledger::ActiveCandidateAttemptScope::new(
                scope.planned_attempt.clone(),
                precursor,
                scope.role == ActiveCandidateRole::Target,
            ));
        let primary =
            run_validated_candidate(input, &scope, &self.subject, &self.lifecycle, cancellation)
                .await;
        let finalization_failed = self
            .prepared
            .revalidate_for_mandatory_finalization()
            .is_err();
        match primary {
            Ok(outcome) => self.retain_candidate_outcome(repository, outcome, finalization_failed),
            Err(error) => {
                self.terminal = true;
                Err(
                    combine_candidate_finalization::<()>(Err(error), finalization_failed)
                        .expect_err("a failed execution remains an error"),
                )
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ActiveCandidateRole {
    Target,
    Baseline,
}

struct ActiveCandidateOperationScope {
    operation_policy: GenerationQualificationOperationPolicyV1,
    planned_attempt: PlannedCandidateAttemptV1,
    projection: GenerationQualificationRequestProjectionEntryV1,
    role: ActiveCandidateRole,
    deadline: std::time::Instant,
}

fn candidate_scope(
    view: &PreparedGenerationQualificationValidationView<'_>,
    next_candidate_attempt: usize,
) -> Result<ActiveCandidateOperationScope, ActiveGenerationQualificationCandidateRunError> {
    let planned_attempt = view
        .operation_policy_relations
        .planned_attempts
        .get(next_candidate_attempt)
        .ok_or(ActiveGenerationQualificationCandidateRunError::AttemptSequenceComplete)?;
    let projection = view
        .request_projection
        .entries()
        .get(next_candidate_attempt)
        .ok_or(ActiveGenerationQualificationCandidateRunError::OperationScope)?;
    let role = if planned_attempt.generation_system_id()
        == view.operation_policy.target_generation_system_id()
    {
        ActiveCandidateRole::Target
    } else if planned_attempt.generation_system_id()
        == view.operation_policy.baseline_generation_system_id()
    {
        ActiveCandidateRole::Baseline
    } else {
        return Err(ActiveGenerationQualificationCandidateRunError::OperationScope);
    };
    Ok(ActiveCandidateOperationScope {
        operation_policy: view.operation_policy.clone(),
        planned_attempt: planned_attempt.clone(),
        projection: projection.clone(),
        role,
        deadline: view.deadline,
    })
}

fn validate_scoped_candidate(
    input: &ActiveGenerationQualificationCandidateRunInput<'_, '_, '_, '_>,
    scope: &ActiveCandidateOperationScope,
    cancellation: &CancellationToken,
) -> Result<(), ActiveGenerationQualificationCandidateRunError> {
    input
        .handoff
        .revalidate_operation_request_scope(
            &scope.operation_policy,
            &scope.planned_attempt,
            &scope.projection,
            cancellation,
        )
        .map_err(map_candidate_scope_error)?;
    ensure_candidate_mode(scope.role, input.resource_policy.is_some())
}

async fn run_validated_candidate(
    input: ActiveGenerationQualificationCandidateRunInput<'_, '_, '_, '_>,
    scope: &ActiveCandidateOperationScope,
    subject: &ActiveGenerationQualificationSubject,
    lifecycle: &GenerationQualificationLiveLifecycle,
    cancellation: &CancellationToken,
) -> Result<ManagedCandidateAttemptExecutionOutcome, ActiveGenerationQualificationCandidateRunError>
{
    let ActiveGenerationQualificationCandidateRunInput {
        handoff,
        attempt,
        resource_policy,
    } = input;
    match (scope.role, resource_policy) {
        (ActiveCandidateRole::Target, Some(resource_policy)) => {
            run_resource_observed_verified_managed_candidate_attempt_until(
                handoff,
                ResourceObservedManagedCandidateAttemptRunInput {
                    attempt,
                    resource_policy,
                    operation_policy: &scope.operation_policy,
                },
                subject.binding(),
                scope.deadline,
                lifecycle,
                cancellation,
            )
            .await
        }
        (ActiveCandidateRole::Baseline, None) => {
            run_verified_managed_candidate_attempt_until(
                handoff,
                attempt,
                subject.binding(),
                scope.deadline,
                lifecycle,
                cancellation,
            )
            .await
        }
        (ActiveCandidateRole::Target, None) | (ActiveCandidateRole::Baseline, Some(_)) => {
            unreachable!("candidate mode was validated")
        }
    }
    .map_err(|error| ActiveGenerationQualificationCandidateRunError::Execution(Box::new(error)))
    .and_then(|outcome| {
        if outcome.matches_active_subject(subject) {
            Ok(outcome)
        } else {
            Err(ActiveGenerationQualificationCandidateRunError::OperationScope)
        }
    })
}

const fn ensure_candidate_mode(
    role: ActiveCandidateRole,
    has_resource_policy: bool,
) -> Result<(), ActiveGenerationQualificationCandidateRunError> {
    match (role, has_resource_policy) {
        (ActiveCandidateRole::Target, true) | (ActiveCandidateRole::Baseline, false) => Ok(()),
        (ActiveCandidateRole::Target, false) | (ActiveCandidateRole::Baseline, true) => {
            Err(ActiveGenerationQualificationCandidateRunError::OperationScope)
        }
    }
}

pub(super) const fn candidate_sequence_complete(next: usize, total: usize) -> bool {
    next >= total
}

const fn map_candidate_scope_error(
    error: CandidateAttemptOperationScopeError,
) -> ActiveGenerationQualificationCandidateRunError {
    match error {
        CandidateAttemptOperationScopeError::Cancelled => {
            ActiveGenerationQualificationCandidateRunError::ActiveAuthority(
                GenerationQualificationActivationError::Cancelled,
            )
        }
        CandidateAttemptOperationScopeError::Mismatch => {
            ActiveGenerationQualificationCandidateRunError::OperationScope
        }
    }
}

fn map_candidate_pretraffic_validation_error(
    error: PreparedGenerationQualificationValidationError<
        ActiveGenerationQualificationCandidateRunError,
    >,
) -> ActiveGenerationQualificationCandidateRunError {
    match error {
        PreparedGenerationQualificationValidationError::Initial(error)
        | PreparedGenerationQualificationValidationError::Final(error) => {
            ActiveGenerationQualificationCandidateRunError::ActiveAuthority(map_preparation_error(
                error,
            ))
        }
        PreparedGenerationQualificationValidationError::Callback(error) => error,
        PreparedGenerationQualificationValidationError::InitialAndFinal { .. }
        | PreparedGenerationQualificationValidationError::CallbackAndFinal { .. } => {
            ActiveGenerationQualificationCandidateRunError::ActiveAuthority(
                GenerationQualificationActivationError::ValidationAggregation,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_scope_and_mode_fail_closed() {
        assert!(matches!(
            map_candidate_scope_error(CandidateAttemptOperationScopeError::Cancelled),
            ActiveGenerationQualificationCandidateRunError::ActiveAuthority(
                GenerationQualificationActivationError::Cancelled
            )
        ));
        assert_eq!(
            map_candidate_scope_error(CandidateAttemptOperationScopeError::Mismatch).kind(),
            ActiveGenerationQualificationCandidateRunErrorKind::OperationScope
        );
        assert!(ensure_candidate_mode(ActiveCandidateRole::Target, true).is_ok());
        assert!(ensure_candidate_mode(ActiveCandidateRole::Baseline, false).is_ok());
        assert_eq!(
            ensure_candidate_mode(ActiveCandidateRole::Target, false)
                .expect_err("target requires resource policy")
                .kind(),
            ActiveGenerationQualificationCandidateRunErrorKind::OperationScope
        );
        assert_eq!(
            ensure_candidate_mode(ActiveCandidateRole::Baseline, true)
                .expect_err("baseline forbids target resource policy")
                .kind(),
            ActiveGenerationQualificationCandidateRunErrorKind::OperationScope
        );
        assert!(!candidate_sequence_complete(1, 2));
        assert!(candidate_sequence_complete(2, 2));
    }
}
