//! Receipt-and-readback-gated settlement of one completed candidate attempt.

use std::fmt;

use rewrite_model::{CandidateGenerationAttemptOutcomeV1, CandidateGenerationAttemptRecordV1};
use rewrite_types::CancellationToken;
use thiserror::Error;

use super::{
    ActiveGenerationQualificationOperation, GenerationQualificationActivationError,
    attempt_ledger::ActiveCandidateAttemptScope, ensure_traffic_eligible, map_preparation_error,
};
use crate::generation_qualification_preregistration::prepared::{
    PreparedGenerationQualificationValidationError, PreparedGenerationQualificationValidationView,
};
use crate::{VerifiedCandidateBatch, VerifiedCandidateBatchError};

/// Stable category for completed candidate settlement failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActiveGenerationQualificationCandidateSettlementErrorKind {
    /// A prior failure terminalized the operation.
    OperationTerminated,
    /// No completed candidate is waiting for settlement.
    NoPendingCandidate,
    /// Prepared or retained Active authority validation failed.
    ActiveAuthority,
    /// The verified batch failed fresh evidence-tree validation.
    Batch,
    /// The batch did not close the exact pending attempt and observation mode.
    OperationScope,
    /// Independent mandatory finalization failed.
    MandatoryFinalization,
    /// Settlement and mandatory finalization both failed.
    PrimaryAndFinalization,
}

/// Content-redacted failure while settling one completed candidate batch.
#[derive(Error)]
pub enum ActiveGenerationQualificationCandidateSettlementError {
    /// A prior failure terminalized the operation.
    #[error("active generation qualification operation is terminal")]
    OperationTerminated,
    /// No completed candidate is waiting for settlement.
    #[error("active generation qualification has no pending completed candidate")]
    NoPendingCandidate,
    /// Prepared or retained Active authority validation failed.
    #[error("active generation qualification authority validation failed")]
    ActiveAuthority(#[source] GenerationQualificationActivationError),
    /// The verified batch failed fresh evidence-tree validation.
    #[error("active generation qualification candidate batch validation failed")]
    Batch(#[source] Box<VerifiedCandidateBatchError>),
    /// The batch did not close the exact pending attempt and observation mode.
    #[error("active generation qualification candidate settlement scope does not match")]
    OperationScope,
    /// Independent mandatory finalization failed.
    #[error("active generation qualification mandatory finalization failed")]
    MandatoryFinalization,
    /// Settlement and mandatory finalization both failed.
    #[error("active generation qualification settlement and finalization both failed")]
    PrimaryAndFinalization {
        /// Content-redacted primary settlement failure.
        primary: Box<ActiveGenerationQualificationCandidateSettlementError>,
    },
}

impl ActiveGenerationQualificationCandidateSettlementError {
    /// Returns the stable content-free failure category.
    #[must_use]
    pub const fn kind(&self) -> ActiveGenerationQualificationCandidateSettlementErrorKind {
        match self {
            Self::OperationTerminated => {
                ActiveGenerationQualificationCandidateSettlementErrorKind::OperationTerminated
            }
            Self::NoPendingCandidate => {
                ActiveGenerationQualificationCandidateSettlementErrorKind::NoPendingCandidate
            }
            Self::ActiveAuthority(_) => {
                ActiveGenerationQualificationCandidateSettlementErrorKind::ActiveAuthority
            }
            Self::Batch(_) => ActiveGenerationQualificationCandidateSettlementErrorKind::Batch,
            Self::OperationScope => {
                ActiveGenerationQualificationCandidateSettlementErrorKind::OperationScope
            }
            Self::MandatoryFinalization => {
                ActiveGenerationQualificationCandidateSettlementErrorKind::MandatoryFinalization
            }
            Self::PrimaryAndFinalization { .. } => {
                ActiveGenerationQualificationCandidateSettlementErrorKind::PrimaryAndFinalization
            }
        }
    }
}

impl fmt::Debug for ActiveGenerationQualificationCandidateSettlementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ActiveGenerationQualificationCandidateSettlementError")
            .field("kind", &self.kind())
            .finish_non_exhaustive()
    }
}

impl ActiveGenerationQualificationOperation<'_, '_, '_, '_, '_> {
    /// Settles the pending completed attempt from one exact verified batch.
    ///
    /// The batch must carry this Active operation's private subject binding.
    /// Target settlement requires a resource-observed batch; baseline settlement
    /// requires a compatibility batch. Only target records enter the target ledger.
    /// The global plan ordinal advances only after this boundary succeeds.
    ///
    /// # Errors
    ///
    /// Returns a content-redacted error for missing pending state, authority or
    /// evidence drift, cross-operation replay, wrong attempt or role, or mandatory
    /// finalization failure. A failed settlement terminalizes the Active owner.
    pub(crate) fn prepare_completed_candidate_settlement(
        &mut self,
        batch: &VerifiedCandidateBatch,
        cancellation: &CancellationToken,
    ) -> Result<
        CandidateGenerationAttemptRecordV1,
        ActiveGenerationQualificationCandidateSettlementError,
    > {
        if self.terminal {
            return Err(ActiveGenerationQualificationCandidateSettlementError::OperationTerminated);
        }
        let pending = self
            .pending_completed_candidate
            .as_ref()
            .map(|pending| pending.scope.clone())
            .ok_or(ActiveGenerationQualificationCandidateSettlementError::NoPendingCandidate)?;
        let subject = &self.subject;
        let next_candidate_attempt = self.next_candidate_attempt;
        let primary = self
            .prepared
            .with_validated_view(cancellation, |view| {
                ensure_traffic_eligible(view.disposition).map_err(
                    ActiveGenerationQualificationCandidateSettlementError::ActiveAuthority,
                )?;
                validate_pending_batch(
                    &pending,
                    &view,
                    next_candidate_attempt,
                    subject,
                    batch,
                    cancellation,
                )
            })
            .map_err(map_validation_error);
        let finalization_failed = self
            .prepared
            .revalidate_for_mandatory_finalization()
            .is_err();
        combine_finalization(primary, finalization_failed)
    }

    pub(crate) fn commit_completed_candidate_settlement(
        &mut self,
        receipt: rewrite_model::CandidateGenerationReceiptV1,
        record: CandidateGenerationAttemptRecordV1,
    ) {
        let Some(pending) = self.pending_completed_candidate.take() else {
            unreachable!("completed settlement was prepared from pending Active state")
        };
        if pending.scope.target {
            self.target_attempt_receipts.push(receipt);
            self.target_attempt_records.push(record);
        }
        self.next_candidate_attempt += 1;
    }
}

fn validate_pending_batch(
    pending: &ActiveCandidateAttemptScope,
    view: &PreparedGenerationQualificationValidationView<'_>,
    next_candidate_attempt: usize,
    subject: &crate::active_generation_qualification_subject::ActiveGenerationQualificationSubject,
    batch: &VerifiedCandidateBatch,
    cancellation: &CancellationToken,
) -> Result<CandidateGenerationAttemptRecordV1, ActiveGenerationQualificationCandidateSettlementError>
{
    let expected = view
        .operation_policy_relations
        .planned_attempts
        .get(next_candidate_attempt)
        .ok_or(ActiveGenerationQualificationCandidateSettlementError::OperationScope)?;
    batch.revalidate(cancellation).map_err(map_batch_error)?;
    let completed = batch
        .completed_attempt(cancellation)
        .map_err(map_batch_error)?;
    let record = batch.attempt_record();
    let outcome_matches = matches!(
        record.outcome(),
        CandidateGenerationAttemptOutcomeV1::Completed {
            planned_attempt_id,
            precursor_id,
            ..
        } if planned_attempt_id == pending.planned_attempt.planned_attempt_id()
            && precursor_id == pending.precursor.precursor_id()
    );
    let resource_matches = batch.resource_result().is_some() == pending.target;
    if !batch.matches_active_subject(subject)
        || expected != &pending.planned_attempt
        || !outcome_matches
        || completed.planned_attempt() != expected
        || completed.precursor() != &pending.precursor
        || batch.receipt().planned_attempt_id() != pending.planned_attempt.planned_attempt_id()
        || batch.receipt().generation_system_id() != expected.generation_system_id()
        || !resource_matches
    {
        return Err(ActiveGenerationQualificationCandidateSettlementError::OperationScope);
    }
    batch.revalidate(cancellation).map_err(map_batch_error)?;
    Ok(record.clone())
}

fn map_batch_error(
    error: VerifiedCandidateBatchError,
) -> ActiveGenerationQualificationCandidateSettlementError {
    ActiveGenerationQualificationCandidateSettlementError::Batch(Box::new(error))
}

fn map_validation_error(
    error: PreparedGenerationQualificationValidationError<
        ActiveGenerationQualificationCandidateSettlementError,
    >,
) -> ActiveGenerationQualificationCandidateSettlementError {
    match error {
        PreparedGenerationQualificationValidationError::Initial(error)
        | PreparedGenerationQualificationValidationError::Final(error) => {
            ActiveGenerationQualificationCandidateSettlementError::ActiveAuthority(
                map_preparation_error(error),
            )
        }
        PreparedGenerationQualificationValidationError::Callback(error) => error,
        PreparedGenerationQualificationValidationError::InitialAndFinal { .. }
        | PreparedGenerationQualificationValidationError::CallbackAndFinal { .. } => {
            ActiveGenerationQualificationCandidateSettlementError::ActiveAuthority(
                GenerationQualificationActivationError::ValidationAggregation,
            )
        }
    }
}

fn combine_finalization<T>(
    primary: Result<T, ActiveGenerationQualificationCandidateSettlementError>,
    finalization_failed: bool,
) -> Result<T, ActiveGenerationQualificationCandidateSettlementError> {
    match (primary, finalization_failed) {
        (Ok(value), false) => Ok(value),
        (Err(error), false) => Err(error),
        (Ok(value), true) => {
            drop(value);
            Err(ActiveGenerationQualificationCandidateSettlementError::MandatoryFinalization)
        }
        (Err(primary), true) => Err(
            ActiveGenerationQualificationCandidateSettlementError::PrimaryAndFinalization {
                primary: Box::new(primary),
            },
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mandatory_finalization_suppresses_success_and_aggregates_failure() {
        assert_eq!(combine_finalization(Ok(7_u8), false).expect("success"), 7);
        assert_eq!(
            combine_finalization(Ok(7_u8), true)
                .expect_err("finalization suppresses settlement")
                .kind(),
            ActiveGenerationQualificationCandidateSettlementErrorKind::MandatoryFinalization
        );
        assert_eq!(
            combine_finalization(
                Err::<u8, _>(
                    ActiveGenerationQualificationCandidateSettlementError::OperationScope,
                ),
                true,
            )
            .expect_err("dual failure")
            .kind(),
            ActiveGenerationQualificationCandidateSettlementErrorKind::PrimaryAndFinalization
        );
    }
}
