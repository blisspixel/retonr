//! Durable settlement of one managed candidate execution outcome.

use rewrite_model::{
    CandidateGenerationAttemptCleanupDispositionV1, CandidateGenerationAttemptFailureCategoryV1,
    CandidateGenerationAttemptFailurePhaseV1, CandidateGenerationAttemptFailureV1Input,
    CandidateGenerationAttemptOutcomeV1, CandidateGenerationAttemptPrecursorV1,
    CandidateGenerationAttemptRecordV1, PlannedCandidateAttemptV1,
};
use rewrite_model_store::CandidateGenerationExecutionV1Input;

use super::{
    ActiveGenerationQualificationCandidateRunError,
    ActiveGenerationQualificationCandidateRunOutcome,
};
use crate::{
    ManagedCandidateAttemptExecutionOutcome,
    generation_qualification_preregistration::{
        GenerationQualificationPreregistrationRepository,
        active::{
            ActiveGenerationQualificationOperation, attempt_ledger::PendingActiveCandidateAttempt,
        },
    },
};

impl ActiveGenerationQualificationOperation<'_, '_, '_, '_, '_> {
    pub(super) fn retain_candidate_outcome(
        &mut self,
        repository: &mut GenerationQualificationPreregistrationRepository,
        outcome: ManagedCandidateAttemptExecutionOutcome,
        finalization_failed: bool,
    ) -> Result<
        ActiveGenerationQualificationCandidateRunOutcome,
        ActiveGenerationQualificationCandidateRunError,
    > {
        if !outcome.matches_active_subject(&self.subject) {
            self.terminal = true;
            return Err(ActiveGenerationQualificationCandidateRunError::OperationScope);
        }
        let Some(started) = self.started_candidate_attempt.as_ref() else {
            self.terminal = true;
            return Err(ActiveGenerationQualificationCandidateRunError::OperationScope);
        };
        match outcome {
            ManagedCandidateAttemptExecutionOutcome::Completed(completed) => {
                if completed.planned_attempt() != &started.planned_attempt
                    || completed.precursor() != &started.precursor
                {
                    self.terminal = true;
                    return Err(ActiveGenerationQualificationCandidateRunError::OperationScope);
                }
                if finalization_failed {
                    let record = failed_after_completed_finalization(
                        &started.planned_attempt,
                        completed.precursor(),
                    )?;
                    self.persist_failed_candidate(repository, &record)?;
                    return Err(
                        ActiveGenerationQualificationCandidateRunError::MandatoryFinalization,
                    );
                }
                let started = self
                    .started_candidate_attempt
                    .take()
                    .ok_or(ActiveGenerationQualificationCandidateRunError::OperationScope)?;
                self.pending_completed_candidate =
                    Some(PendingActiveCandidateAttempt::new(started, completed));
                Ok(ActiveGenerationQualificationCandidateRunOutcome::CompletedPending)
            }
            ManagedCandidateAttemptExecutionOutcome::Failed(failed) => {
                let record_matches = matches!(
                    failed.attempt_record().outcome(),
                    CandidateGenerationAttemptOutcomeV1::Failed {
                        planned_attempt_id,
                        precursor_id: Some(precursor_id),
                        ..
                    } if planned_attempt_id == started.planned_attempt.planned_attempt_id()
                        && precursor_id == started.precursor.precursor_id()
                );
                if !record_matches || failed.precursor() != &started.precursor {
                    self.terminal = true;
                    return Err(ActiveGenerationQualificationCandidateRunError::OperationScope);
                }
                self.persist_failed_candidate(repository, failed.attempt_record())?;
                if finalization_failed {
                    Err(
                        ActiveGenerationQualificationCandidateRunError::PrimaryAndFinalization {
                            primary: Box::new(
                                ActiveGenerationQualificationCandidateRunError::MandatoryFinalization,
                            ),
                        },
                    )
                } else {
                    Ok(ActiveGenerationQualificationCandidateRunOutcome::Failed(
                        failed,
                    ))
                }
            }
        }
    }

    fn persist_failed_candidate(
        &mut self,
        repository: &mut GenerationQualificationPreregistrationRepository,
        record: &CandidateGenerationAttemptRecordV1,
    ) -> Result<(), ActiveGenerationQualificationCandidateRunError> {
        let Some(started) = self.started_candidate_attempt.as_ref() else {
            self.terminal = true;
            return Err(ActiveGenerationQualificationCandidateRunError::OperationScope);
        };
        repository
            .persist_candidate_execution(
                &CandidateGenerationExecutionV1Input::Failed {
                    preregistration: self.prepared.preregistration_read_input(),
                    precursor: Some(&started.precursor),
                    attempt: record,
                },
                |_| Ok(()),
            )
            .map_err(|error| {
                self.terminal = true;
                ActiveGenerationQualificationCandidateRunError::Durability(error)
            })?;
        let started = self
            .started_candidate_attempt
            .take()
            .ok_or(ActiveGenerationQualificationCandidateRunError::OperationScope)?;
        if started.target {
            self.target_attempt_records.push(record.clone());
        }
        self.next_candidate_attempt += 1;
        self.terminal = true;
        Ok(())
    }
}

fn failed_after_completed_finalization(
    planned_attempt: &PlannedCandidateAttemptV1,
    precursor: &CandidateGenerationAttemptPrecursorV1,
) -> Result<CandidateGenerationAttemptRecordV1, ActiveGenerationQualificationCandidateRunError> {
    CandidateGenerationAttemptRecordV1::failed(
        planned_attempt,
        Some(precursor),
        CandidateGenerationAttemptFailureV1Input {
            failure_phase: CandidateGenerationAttemptFailurePhaseV1::Cleanup,
            failure_category:
                CandidateGenerationAttemptFailureCategoryV1::PackageRevalidationFailed,
            traffic_observed: true,
            output_observed: true,
            cleanup_disposition: CandidateGenerationAttemptCleanupDispositionV1::Succeeded,
        },
    )
    .map_err(|_| ActiveGenerationQualificationCandidateRunError::OperationScope)
}

pub(super) fn combine_candidate_finalization<T>(
    primary: Result<T, ActiveGenerationQualificationCandidateRunError>,
    finalization_failed: bool,
) -> Result<T, ActiveGenerationQualificationCandidateRunError> {
    match (primary, finalization_failed) {
        (Ok(value), false) => Ok(value),
        (Err(error), false) => Err(error),
        (Ok(value), true) => {
            drop(value);
            Err(ActiveGenerationQualificationCandidateRunError::MandatoryFinalization)
        }
        (Err(primary), true) => Err(
            ActiveGenerationQualificationCandidateRunError::PrimaryAndFinalization {
                primary: Box::new(primary),
            },
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ActiveGenerationQualificationCandidateRunErrorKind;

    #[test]
    fn candidate_finalization_drops_success_and_aggregates_primary_failure() {
        assert_eq!(
            combine_candidate_finalization(Ok(7_u8), false).expect("successful finalization"),
            7
        );
        let finalization = combine_candidate_finalization(Ok(7_u8), true)
            .expect_err("finalization must suppress a successful value");
        assert_eq!(
            finalization.kind(),
            ActiveGenerationQualificationCandidateRunErrorKind::MandatoryFinalization
        );
        let aggregated = combine_candidate_finalization::<u8>(
            Err(ActiveGenerationQualificationCandidateRunError::OperationScope),
            true,
        )
        .expect_err("primary and finalization must aggregate");
        assert_eq!(
            aggregated.kind(),
            ActiveGenerationQualificationCandidateRunErrorKind::PrimaryAndFinalization
        );
        assert!(!format!("{aggregated:?}").contains("OperationScope"));
    }
}
