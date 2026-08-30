//! Exact no-launch pairing of eval and app managed-judge handoffs.

use std::fmt;

use rewrite_app::{ManagedJudgePrecursorCompilationError, ManagedJudgePrecursorRunnerHandoff};
use rewrite_model::{
    CandidateJudgePlanId, CandidateJudgeRequestAggregateId, CandidateJudgeScheduleId,
    GenerationQualificationOperationPolicyV1, GenerationRepetitionId, GenerationSystemId,
};
use rewrite_types::CancellationToken;
use thiserror::Error;

use crate::active_generation_qualification_subject::ActiveGenerationQualificationSubject;

use super::{
    CandidateJudgePreparationError, CandidateJudgeRunnerHandoff,
    handoff::CandidateJudgeResourceViewError,
};

mod kernel;

use kernel::{PairingSubject, validate_pairing};

#[cfg(test)]
mod tests;

/// Exact relationship compared before a paired runner authority can be released.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateJudgeRunnerPairingRelationship {
    /// The complete portable judge plans differed.
    JudgePlan,
    /// The complete portable judge schedules differed.
    JudgeSchedule,
    /// The complete request identity aggregates differed.
    RequestAggregate,
    /// The complete judge generation-system records differed.
    JudgeSystem,
}

/// Authority-validation bracket that failed during no-launch pairing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateJudgeRunnerPairingValidationPhase {
    /// Validation before any relationship comparison.
    Initial,
    /// Mandatory validation after the relationship comparison attempt.
    Final,
}

/// Failure to pair the exact eval and app managed-judge handoffs.
#[derive(Error)]
pub enum CandidateJudgeRunnerPairingError {
    /// Pairing was cancelled before an authority-validation bracket began.
    #[error("candidate judge runner pairing was cancelled")]
    Cancelled,
    /// One validation bracket retained every independently observed failure.
    #[error("candidate judge runner authority validation failed during {phase:?}")]
    AuthorityValidation {
        /// Exact bracket in which validation failed.
        phase: CandidateJudgeRunnerPairingValidationPhase,
        /// Eval-owned validation failure, when observed before cancellation.
        eval: Option<Box<CandidateJudgePreparationError>>,
        /// App-owned validation failure, when observed before cancellation.
        app: Option<Box<ManagedJudgePrecursorCompilationError>>,
        /// Whether cancellation stopped or invalidated this validation bracket.
        cancelled: bool,
    },
    /// One complete portable relationship differed.
    #[error("candidate judge runner relationship does not match: {0:?}")]
    Relationship(CandidateJudgeRunnerPairingRelationship),
    /// Comparison and mandatory final validation both failed.
    #[error("candidate judge runner pairing and final validation both failed")]
    PrimaryAndFinalValidation {
        /// Primary comparison or cancellation failure.
        primary: Box<CandidateJudgeRunnerPairingError>,
        /// Independent final authority-validation failure.
        final_validation: Box<CandidateJudgeRunnerPairingError>,
    },
}

impl fmt::Debug for CandidateJudgeRunnerPairingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("CandidateJudgeRunnerPairingError");
        match self {
            Self::Cancelled => debug.field("kind", &"cancelled"),
            Self::AuthorityValidation {
                phase,
                eval,
                app,
                cancelled,
            } => debug
                .field("kind", &"authority_validation")
                .field("phase", phase)
                .field("eval_failed", &eval.is_some())
                .field("app_failed", &app.is_some())
                .field("cancelled", cancelled),
            Self::Relationship(relationship) => debug
                .field("kind", &"relationship")
                .field("relationship", relationship),
            Self::PrimaryAndFinalValidation { .. } => {
                debug.field("kind", &"primary_and_final_validation")
            }
        };
        debug.finish_non_exhaustive()
    }
}

/// Noncloneable, nonserializable no-launch authority for the exact managed runner.
///
/// This value retains both nonforgeable handoffs privately. Its public surface is
/// content-free: it exposes only portable identities and the checked attempt count.
/// Runtime launch and request traffic are not available at this boundary.
///
/// ```compile_fail
/// use rewrite_eval::PairedCandidateJudgeRunnerHandoff;
///
/// fn clone_capability(
///     value: &PairedCandidateJudgeRunnerHandoff<'_, '_, '_, '_, '_>,
/// ) {
///     let _forged: PairedCandidateJudgeRunnerHandoff<'_, '_, '_, '_, '_> = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_eval::PairedCandidateJudgeRunnerHandoff;
///
/// fn serialize_capability(
///     value: &PairedCandidateJudgeRunnerHandoff<'_, '_, '_, '_, '_>,
/// ) {
///     let _bytes = serde_json::to_vec(value).expect("capability must not serialize");
/// }
/// ```
pub struct PairedCandidateJudgeRunnerHandoff<'store, 'records, 'model, 'runtime, 'characterized> {
    eval: CandidateJudgeRunnerHandoff<'store>,
    app: ManagedJudgePrecursorRunnerHandoff<'records, 'model, 'runtime, 'characterized>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CandidateJudgeOperationScopeError {
    Cancelled,
    Mismatch,
}

impl PairedCandidateJudgeRunnerHandoff<'_, '_, '_, '_, '_> {
    pub(crate) fn matches_active_subject(
        &self,
        subject: &ActiveGenerationQualificationSubject,
    ) -> bool {
        self.eval.matches_active_subject(subject)
    }

    /// Returns the exact common judge-plan identity.
    #[must_use]
    pub fn judge_plan_id(&self) -> &CandidateJudgePlanId {
        self.eval.judge_plan().candidate_judge_plan_id()
    }

    /// Returns the exact common judge-schedule identity.
    #[must_use]
    pub fn judge_schedule_id(&self) -> &CandidateJudgeScheduleId {
        self.eval.judge_schedule().candidate_judge_schedule_id()
    }

    /// Returns the exact common request-aggregate identity.
    #[must_use]
    pub fn request_aggregate_id(&self) -> &CandidateJudgeRequestAggregateId {
        self.eval.request_aggregate().request_aggregate_id()
    }

    /// Returns the exact common judge generation-system identity.
    #[must_use]
    pub fn judge_generation_system_id(&self) -> &GenerationSystemId {
        self.eval.judge_system().generation_system_id()
    }

    /// Returns the exact checked schedule attempt count.
    #[must_use]
    pub fn attempt_count(&self) -> u32 {
        self.eval.judge_schedule().entry_count()
    }

    pub(crate) fn revalidate_operation_scope(
        &self,
        operation_policy: &GenerationQualificationOperationPolicyV1,
        repetition_id: &GenerationRepetitionId,
        cancellation: &CancellationToken,
    ) -> Result<(), CandidateJudgeOperationScopeError> {
        self.eval
            .revalidate_operation_scope(operation_policy, repetition_id, cancellation)
            .map_err(|error| map_operation_scope_error(&error))
    }
}

fn map_operation_scope_error(
    error: &CandidateJudgeResourceViewError,
) -> CandidateJudgeOperationScopeError {
    if error.is_cancelled() {
        CandidateJudgeOperationScopeError::Cancelled
    } else {
        CandidateJudgeOperationScopeError::Mismatch
    }
}

impl<'store, 'records, 'model, 'runtime, 'characterized>
    PairedCandidateJudgeRunnerHandoff<'store, 'records, 'model, 'runtime, 'characterized>
{
    pub(crate) fn into_parts(
        self,
    ) -> (
        CandidateJudgeRunnerHandoff<'store>,
        ManagedJudgePrecursorRunnerHandoff<'records, 'model, 'runtime, 'characterized>,
    ) {
        (self.eval, self.app)
    }
}

impl fmt::Debug for PairedCandidateJudgeRunnerHandoff<'_, '_, '_, '_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PairedCandidateJudgeRunnerHandoff")
            .field("judge_plan_id", self.judge_plan_id())
            .field("judge_schedule_id", self.judge_schedule_id())
            .field("request_aggregate_id", self.request_aggregate_id())
            .field(
                "judge_generation_system_id",
                self.judge_generation_system_id(),
            )
            .field("attempt_count", &self.attempt_count())
            .finish_non_exhaustive()
    }
}

/// Consumes and pairs exact eval and app managed-judge handoffs without launching.
///
/// Complete plan, schedule, request aggregate, and judge-system values must be
/// equal. Both authority families are revalidated before and after comparison.
/// Each bracket attempts eval validation before app validation, while retaining
/// both failures unless cancellation stops the later check. Relationships are
/// compared in the enum's documented order and the first mismatch is primary.
///
/// # Errors
///
/// Returns [`CandidateJudgeRunnerPairingError`] for cancellation, either retained
/// authority family's drift, or any complete portable relationship substitution.
pub fn pair_candidate_judge_runner_handoffs<'store, 'records, 'model, 'runtime, 'characterized>(
    eval: CandidateJudgeRunnerHandoff<'store>,
    mut app: ManagedJudgePrecursorRunnerHandoff<'records, 'model, 'runtime, 'characterized>,
    cancellation: &CancellationToken,
) -> Result<
    PairedCandidateJudgeRunnerHandoff<'store, 'records, 'model, 'runtime, 'characterized>,
    CandidateJudgeRunnerPairingError,
> {
    let mut subject = ConcretePairingSubject {
        eval: &eval,
        app: &mut app,
    };
    validate_pairing(&mut subject, cancellation)?;
    Ok(PairedCandidateJudgeRunnerHandoff { eval, app })
}

struct ConcretePairingSubject<'borrow, 'store, 'records, 'model, 'runtime, 'characterized> {
    eval: &'borrow CandidateJudgeRunnerHandoff<'store>,
    app:
        &'borrow mut ManagedJudgePrecursorRunnerHandoff<'records, 'model, 'runtime, 'characterized>,
}

impl PairingSubject for ConcretePairingSubject<'_, '_, '_, '_, '_, '_> {
    fn revalidate_eval(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<(), CandidateJudgePreparationError> {
        self.eval.revalidate(cancellation)
    }

    fn revalidate_app(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<(), ManagedJudgePrecursorCompilationError> {
        self.app.revalidate(cancellation)
    }

    fn relationship_matches(
        &mut self,
        relationship: CandidateJudgeRunnerPairingRelationship,
        _cancellation: &CancellationToken,
    ) -> bool {
        match relationship {
            CandidateJudgeRunnerPairingRelationship::JudgePlan => {
                self.eval.judge_plan() == self.app.judge_plan()
            }
            CandidateJudgeRunnerPairingRelationship::JudgeSchedule => {
                self.eval.judge_schedule() == self.app.judge_schedule()
            }
            CandidateJudgeRunnerPairingRelationship::RequestAggregate => {
                self.eval.request_aggregate() == self.app.request_aggregate()
            }
            CandidateJudgeRunnerPairingRelationship::JudgeSystem => {
                self.eval.judge_system() == self.app.judge_system()
            }
        }
    }
}
