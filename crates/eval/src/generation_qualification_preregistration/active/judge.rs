//! Exact repetition-order judge traffic owned by one Active operation.

use std::fmt;

use rewrite_model::{GenerationQualificationOperationPolicyV1, GenerationRepetitionId};
use rewrite_ollama::OllamaModelBinding;
use rewrite_runtime_attestor::ManagedGenerationWorkerLimits;
use rewrite_types::CancellationToken;
use thiserror::Error;

use super::{
    ActiveGenerationQualificationOperation, GenerationQualificationActivationError,
    candidate::candidate_sequence_complete, ensure_traffic_eligible, map_preparation_error,
};
use crate::{
    LocalOllamaBoundPreflightPlan, LocalOllamaManagedPreflightLimits,
    LocalOllamaModelBindingEvidence, ManagedCandidateJudgeRunError,
    PairedCandidateJudgeRunnerHandoff, VerifiedCandidateJudgeJoin,
    active_generation_qualification_subject::ActiveGenerationQualificationSubject,
    candidate_judge_preparation::CandidateJudgeOperationScopeError,
    generation_qualification_preregistration::prepared::PreparedGenerationQualificationValidationError,
    local_ollama_managed_preflight::{
        GenerationQualificationLiveLifecycle, run_verified_managed_candidate_judge_until,
    },
};

/// Exact owned inputs for the next Active managed candidate-judge run.
pub struct ActiveGenerationQualificationJudgeRunInput<
    'store,
    'records,
    'model,
    'runtime,
    'characterized,
> {
    /// Exact paired eval and app runner authority for one repetition.
    pub paired:
        PairedCandidateJudgeRunnerHandoff<'store, 'records, 'model, 'runtime, 'characterized>,
    /// Exact retained local Ollama preflight plan.
    pub preflight_plan: LocalOllamaBoundPreflightPlan,
    /// Exact managed preflight ceilings.
    pub preflight_limits: LocalOllamaManagedPreflightLimits,
    /// Exact generation-worker observation ceilings.
    pub worker_limits: ManagedGenerationWorkerLimits,
    /// Exact static package-to-Ollama model evidence.
    pub model_evidence: LocalOllamaModelBindingEvidence,
    /// Exact Ollama model and artifact binding.
    pub model: OllamaModelBinding,
}

/// Stable category for an Active managed candidate-judge failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActiveGenerationQualificationJudgeRunErrorKind {
    /// The retained Active or Prepared authority failed validation.
    ActiveAuthority,
    /// A prior run failure terminalized this Active owner.
    OperationTerminated,
    /// Exact plan-order candidate execution has not completed.
    CandidateSequenceIncomplete,
    /// The exact Active-owned target attempt ledger is not sealed.
    AttemptLedgerIncomplete,
    /// Every preregistered repetition has already been run.
    RepetitionSequenceComplete,
    /// The paired handoff did not match the next exact operation repetition.
    OperationScope,
    /// The strict managed candidate-judge runner failed.
    Execution,
    /// Independent mandatory finalization validation failed.
    MandatoryFinalization,
    /// A primary error and mandatory finalization both failed.
    PrimaryAndFinalization,
}

/// Content-redacted failure from the Active managed candidate-judge boundary.
#[derive(Error)]
pub enum ActiveGenerationQualificationJudgeRunError {
    /// The retained Active or Prepared authority failed validation.
    #[error("active generation qualification authority validation failed")]
    ActiveAuthority(#[source] GenerationQualificationActivationError),
    /// A prior failed run permanently terminalized this owner.
    #[error("active generation qualification operation is terminal")]
    OperationTerminated,
    /// Candidate execution must complete before judging begins.
    #[error("active generation qualification candidate sequence is incomplete")]
    CandidateSequenceIncomplete,
    /// The target attempt ledger must be sealed before judging begins.
    #[error("active generation qualification attempt ledger is incomplete")]
    AttemptLedgerIncomplete,
    /// No preregistered repetition remains.
    #[error("active generation qualification judge repetition sequence is complete")]
    RepetitionSequenceComplete,
    /// The paired handoff did not match the next exact operation repetition.
    #[error("active generation qualification judge operation scope does not match")]
    OperationScope,
    /// The strict managed candidate-judge runner failed.
    #[error("active generation qualification judge execution failed")]
    Execution(#[source] ManagedCandidateJudgeRunError),
    /// Independent mandatory finalization validation failed.
    #[error("active generation qualification mandatory finalization failed")]
    MandatoryFinalization,
    /// A primary error and mandatory finalization both failed.
    #[error("active generation qualification judge and finalization both failed")]
    PrimaryAndFinalization {
        /// Content-redacted primary failure.
        primary: Box<ActiveGenerationQualificationJudgeRunError>,
    },
}

impl ActiveGenerationQualificationJudgeRunError {
    /// Returns the stable failure category without exposing live authority detail.
    #[must_use]
    pub const fn kind(&self) -> ActiveGenerationQualificationJudgeRunErrorKind {
        match self {
            Self::ActiveAuthority(_) => {
                ActiveGenerationQualificationJudgeRunErrorKind::ActiveAuthority
            }
            Self::OperationTerminated => {
                ActiveGenerationQualificationJudgeRunErrorKind::OperationTerminated
            }
            Self::CandidateSequenceIncomplete => {
                ActiveGenerationQualificationJudgeRunErrorKind::CandidateSequenceIncomplete
            }
            Self::AttemptLedgerIncomplete => {
                ActiveGenerationQualificationJudgeRunErrorKind::AttemptLedgerIncomplete
            }
            Self::RepetitionSequenceComplete => {
                ActiveGenerationQualificationJudgeRunErrorKind::RepetitionSequenceComplete
            }
            Self::OperationScope => ActiveGenerationQualificationJudgeRunErrorKind::OperationScope,
            Self::Execution(_) => ActiveGenerationQualificationJudgeRunErrorKind::Execution,
            Self::MandatoryFinalization => {
                ActiveGenerationQualificationJudgeRunErrorKind::MandatoryFinalization
            }
            Self::PrimaryAndFinalization { .. } => {
                ActiveGenerationQualificationJudgeRunErrorKind::PrimaryAndFinalization
            }
        }
    }
}

impl fmt::Debug for ActiveGenerationQualificationJudgeRunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ActiveGenerationQualificationJudgeRunError")
            .field("kind", &self.kind())
            .finish_non_exhaustive()
    }
}

impl ActiveGenerationQualificationOperation<'_, '_, '_, '_, '_> {
    /// Runs the next exact preregistered repetition through the strict judge path.
    ///
    /// The paired handoff must retain the operation's exact target, baseline, plan,
    /// suite, and next repetition, including resource-observed target results and a
    /// compatibility-only baseline. The original Prepared deadline and this owner's
    /// single process-local lifecycle are used without replacement. Any failure
    /// terminalizes the owner and cannot be retried.
    ///
    /// # Errors
    ///
    /// Returns a content-redacted error for authority drift, incomplete candidate
    /// execution, sequence completion, scope mismatch, strict runner failure, or
    /// mandatory finalization failure.
    pub async fn run_next_judge<'store, 'records, 'model, 'runtime>(
        &mut self,
        input: ActiveGenerationQualificationJudgeRunInput<'store, 'records, 'model, 'runtime, '_>,
        cancellation: &CancellationToken,
    ) -> Result<
        VerifiedCandidateJudgeJoin<'store, 'records, 'model, 'runtime>,
        ActiveGenerationQualificationJudgeRunError,
    > {
        if self.terminal {
            return Err(ActiveGenerationQualificationJudgeRunError::OperationTerminated);
        }
        let ledger_ready = self
            .attempt_ledger
            .as_ref()
            .is_some_and(|ledger| ledger.matches_active_subject(&self.subject));
        let scope = self
            .prepared
            .with_validated_view(cancellation, |view| {
                ensure_traffic_eligible(view.disposition)
                    .map_err(ActiveGenerationQualificationJudgeRunError::ActiveAuthority)?;
                if !candidate_sequence_complete(
                    self.next_candidate_attempt,
                    view.operation_policy_relations.planned_attempts.len(),
                ) {
                    return Err(
                        ActiveGenerationQualificationJudgeRunError::CandidateSequenceIncomplete,
                    );
                }
                if !ledger_ready {
                    return Err(
                        ActiveGenerationQualificationJudgeRunError::AttemptLedgerIncomplete,
                    );
                }
                let repetition = view
                    .operation_policy_relations
                    .repetitions
                    .get(self.next_judge_repetition)
                    .ok_or(
                        ActiveGenerationQualificationJudgeRunError::RepetitionSequenceComplete,
                    )?;
                Ok(ActiveJudgeOperationScope {
                    operation_policy: view.operation_policy.clone(),
                    repetition_id: repetition.repetition_id().clone(),
                    deadline: view.deadline,
                })
            })
            .map_err(map_judge_pretraffic_validation_error);
        let primary = match scope {
            Ok(scope) => {
                run_scoped_judge(input, &scope, &self.subject, &self.lifecycle, cancellation).await
            }
            Err(error) => Err(error),
        };
        let finalization_failed = self
            .prepared
            .revalidate_for_mandatory_finalization()
            .is_err();
        let result = combine_judge_finalization(primary, finalization_failed);
        match &result {
            Ok(_) => self.next_judge_repetition += 1,
            Err(
                ActiveGenerationQualificationJudgeRunError::CandidateSequenceIncomplete
                | ActiveGenerationQualificationJudgeRunError::AttemptLedgerIncomplete
                | ActiveGenerationQualificationJudgeRunError::RepetitionSequenceComplete,
            ) => {}
            Err(_) => self.terminal = true,
        }
        result
    }
}

struct ActiveJudgeOperationScope {
    operation_policy: GenerationQualificationOperationPolicyV1,
    repetition_id: GenerationRepetitionId,
    deadline: std::time::Instant,
}

async fn run_scoped_judge<'store, 'records, 'model, 'runtime>(
    input: ActiveGenerationQualificationJudgeRunInput<'store, 'records, 'model, 'runtime, '_>,
    scope: &ActiveJudgeOperationScope,
    subject: &ActiveGenerationQualificationSubject,
    lifecycle: &GenerationQualificationLiveLifecycle,
    cancellation: &CancellationToken,
) -> Result<
    VerifiedCandidateJudgeJoin<'store, 'records, 'model, 'runtime>,
    ActiveGenerationQualificationJudgeRunError,
> {
    let ActiveGenerationQualificationJudgeRunInput {
        paired,
        preflight_plan,
        preflight_limits,
        worker_limits,
        model_evidence,
        model,
    } = input;
    if !paired.matches_active_subject(subject) {
        return Err(ActiveGenerationQualificationJudgeRunError::OperationScope);
    }
    paired
        .revalidate_operation_scope(&scope.operation_policy, &scope.repetition_id, cancellation)
        .map_err(map_judge_scope_error)?;
    run_verified_managed_candidate_judge_until(
        paired,
        preflight_plan,
        preflight_limits,
        worker_limits,
        model_evidence,
        model,
        scope.deadline,
        lifecycle,
        cancellation,
    )
    .await
    .map_err(ActiveGenerationQualificationJudgeRunError::Execution)
    .and_then(|join| {
        if join.matches_active_subject(subject) {
            Ok(join)
        } else {
            Err(ActiveGenerationQualificationJudgeRunError::OperationScope)
        }
    })
}

const fn map_judge_scope_error(
    error: CandidateJudgeOperationScopeError,
) -> ActiveGenerationQualificationJudgeRunError {
    match error {
        CandidateJudgeOperationScopeError::Cancelled => {
            ActiveGenerationQualificationJudgeRunError::ActiveAuthority(
                GenerationQualificationActivationError::Cancelled,
            )
        }
        CandidateJudgeOperationScopeError::Mismatch => {
            ActiveGenerationQualificationJudgeRunError::OperationScope
        }
    }
}

fn map_judge_pretraffic_validation_error(
    error: PreparedGenerationQualificationValidationError<
        ActiveGenerationQualificationJudgeRunError,
    >,
) -> ActiveGenerationQualificationJudgeRunError {
    match error {
        PreparedGenerationQualificationValidationError::Initial(error)
        | PreparedGenerationQualificationValidationError::Final(error) => {
            ActiveGenerationQualificationJudgeRunError::ActiveAuthority(map_preparation_error(
                error,
            ))
        }
        PreparedGenerationQualificationValidationError::Callback(error) => error,
        PreparedGenerationQualificationValidationError::InitialAndFinal { .. }
        | PreparedGenerationQualificationValidationError::CallbackAndFinal { .. } => {
            ActiveGenerationQualificationJudgeRunError::ActiveAuthority(
                GenerationQualificationActivationError::ValidationAggregation,
            )
        }
    }
}

fn combine_judge_finalization<T>(
    primary: Result<T, ActiveGenerationQualificationJudgeRunError>,
    finalization_failed: bool,
) -> Result<T, ActiveGenerationQualificationJudgeRunError> {
    match (primary, finalization_failed) {
        (Ok(value), false) => Ok(value),
        (Err(error), false) => Err(error),
        (Ok(value), true) => {
            drop(value);
            Err(ActiveGenerationQualificationJudgeRunError::MandatoryFinalization)
        }
        (Err(primary), true) => Err(
            ActiveGenerationQualificationJudgeRunError::PrimaryAndFinalization {
                primary: Box::new(primary),
            },
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn judge_scope_preserves_cancellation_and_redacts_mismatch() {
        assert!(matches!(
            map_judge_scope_error(CandidateJudgeOperationScopeError::Cancelled),
            ActiveGenerationQualificationJudgeRunError::ActiveAuthority(
                GenerationQualificationActivationError::Cancelled
            )
        ));
        assert_eq!(
            map_judge_scope_error(CandidateJudgeOperationScopeError::Mismatch).kind(),
            ActiveGenerationQualificationJudgeRunErrorKind::OperationScope
        );
    }

    #[test]
    fn judge_finalization_drops_success_and_aggregates_primary_failure() {
        assert_eq!(
            combine_judge_finalization(Ok(7_u8), false).expect("successful finalization"),
            7
        );
        let finalization = combine_judge_finalization(Ok(7_u8), true)
            .expect_err("finalization must suppress a successful value");
        assert_eq!(
            finalization.kind(),
            ActiveGenerationQualificationJudgeRunErrorKind::MandatoryFinalization
        );
        let aggregated = combine_judge_finalization::<u8>(
            Err(ActiveGenerationQualificationJudgeRunError::OperationScope),
            true,
        )
        .expect_err("primary and finalization must aggregate");
        assert_eq!(
            aggregated.kind(),
            ActiveGenerationQualificationJudgeRunErrorKind::PrimaryAndFinalization
        );
        assert!(!format!("{aggregated:?}").contains("OperationScope"));
    }
}
