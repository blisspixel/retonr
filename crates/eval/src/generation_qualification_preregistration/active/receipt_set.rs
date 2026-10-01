//! Active-owned publication of a target receipt set from its exact sealed ledger.

use rewrite_model::{
    CandidateGenerationAttemptRecordV1, CandidateGenerationReceiptSetV1,
    CandidateGenerationReceiptSetV1Relations, CandidateGenerationReceiptV1,
    GenerationQualificationPhaseStatusV1,
};
use rewrite_model_store::WriteDisposition;
use rewrite_types::CancellationToken;
use thiserror::Error;

use super::ActiveGenerationQualificationOperation;
use crate::{
    VerifiedCandidateBatchSet,
    generation_qualification_preregistration::{
        GenerationQualificationPreparationError, GenerationQualificationPreregistrationRepository,
        check_gate, prepared::PreparedGenerationQualificationValidationError,
    },
};

/// Content-free failure of Active-owned target receipt-set publication.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ActiveGenerationQualificationReceiptSetError {
    /// The original absolute operation deadline was reached.
    #[error("active generation qualification receipt set deadline was reached")]
    DeadlineExceeded,
    /// Cancellation was observed before the original deadline.
    #[error("active generation qualification receipt set was cancelled")]
    Cancelled,
    /// The original deadline and mandatory final validation both failed.
    #[error("active generation qualification receipt set deadline and finalization failed")]
    DeadlineAndFinalization,
    /// Cancellation and mandatory final validation both failed.
    #[error("active generation qualification receipt set cancellation and finalization failed")]
    CancelledAndFinalization,
    /// Candidate execution or its Passed sealed ledger is unavailable.
    #[error("active generation qualification receipt set is not ready")]
    NotReady,
    /// Live authority belongs to another operation or differs from the ledger.
    #[error("active generation qualification receipt set scope does not match")]
    OperationScope,
    /// Live candidate evidence failed fresh validation.
    #[error("active generation qualification receipt set authority failed")]
    BatchAuthority,
    /// Prepared validation or durable publication failed.
    #[error("active generation qualification receipt set publication failed")]
    Publication,
    /// Mandatory independent final validation failed.
    #[error("active generation qualification receipt set finalization failed")]
    MandatoryFinalization,
    /// Primary failure and independent final validation both failed.
    #[error("active generation qualification receipt set and finalization failed")]
    PrimaryAndFinalization,
}

impl ActiveGenerationQualificationOperation<'_, '_, '_, '_, '_> {
    /// Publishes one target repetition's receipt set from current live authority.
    ///
    /// The batch set remains available for deterministic evaluation and judging.
    /// Its complete record is independently reconstructed from the sealed Passed
    /// target ledger before schema 14 publication and fresh canonical readback.
    /// Stored evidence grants no qualification, activation, or live-use authority.
    /// Every attempted publication receives independent uncancelled batch and
    /// Prepared final validation, including after cancellation or storage failure.
    /// A committed row may remain after later validation fails; that row stays inert.
    ///
    /// # Errors
    ///
    /// Returns a content-free error for incomplete candidates, foreign authority,
    /// ledger disagreement, expiry, cancellation, storage or final validation failure.
    pub fn persist_target_receipt_set(
        &mut self,
        repository: &mut GenerationQualificationPreregistrationRepository,
        batch_set: &VerifiedCandidateBatchSet,
        cancellation: &CancellationToken,
    ) -> Result<WriteDisposition, ActiveGenerationQualificationReceiptSetError> {
        if self.terminal
            || self.started_candidate_attempt.is_some()
            || self.pending_completed_candidate.is_some()
        {
            return Err(ActiveGenerationQualificationReceiptSetError::NotReady);
        }
        let Some(ledger) = self.attempt_ledger.as_ref() else {
            return Err(ActiveGenerationQualificationReceiptSetError::NotReady);
        };
        let selection_policy = self
            .prepared
            .plan_foundation()
            .candidate_selection_policy()
            .clone();
        let subject_matches = ledger.matches_active_subject(&self.subject)
            && batch_set.matches_active_subject(&self.subject);
        let primary = self
            .prepared
            .with_validated_view(cancellation, |view| {
                if !subject_matches {
                    return Err(ActiveGenerationQualificationReceiptSetError::OperationScope);
                }
                if self.next_candidate_attempt
                    != view.operation_policy_relations.planned_attempts.len()
                    || ledger.manifest().status() != GenerationQualificationPhaseStatusV1::Passed
                {
                    return Err(ActiveGenerationQualificationReceiptSetError::NotReady);
                }
                let observed = batch_set
                    .receipt_set(cancellation)
                    .map_err(|_| ActiveGenerationQualificationReceiptSetError::BatchAuthority)?;
                let relations = view.operation_policy_relations;
                let repetition = relations
                    .repetitions
                    .iter()
                    .find(|repetition| repetition.repetition_id() == observed.repetition_id())
                    .ok_or(ActiveGenerationQualificationReceiptSetError::OperationScope)?;
                let rebuilt = rederive_from_ledger(
                    CandidateGenerationReceiptSetV1Relations {
                        qualification_plan: relations.plan,
                        suite: relations.suite,
                        repetition,
                        generation_system: relations.target_system.generation_system,
                        selection_policy: &selection_policy,
                        planned_attempts: relations.planned_attempts,
                        attempt_records: &[],
                        receipts: &[],
                    },
                    ledger.target_attempt_records(),
                    ledger.target_attempt_receipts(),
                )?;
                if &rebuilt != observed {
                    return Err(ActiveGenerationQualificationReceiptSetError::OperationScope);
                }
                repository
                    .persist_receipt_set(&rebuilt, view.deadline, cancellation)
                    .map_err(map_preparation_error)
            })
            .map_err(|error| match error {
                PreparedGenerationQualificationValidationError::Callback(error) => error,
                PreparedGenerationQualificationValidationError::Initial(error)
                | PreparedGenerationQualificationValidationError::Final(error) => {
                    map_preparation_error(error)
                }
                PreparedGenerationQualificationValidationError::InitialAndFinal { .. }
                | PreparedGenerationQualificationValidationError::CallbackAndFinal { .. } => {
                    check_gate(self.prepared.operation_deadline(), cancellation)
                        .err()
                        .map_or(
                            ActiveGenerationQualificationReceiptSetError::Publication,
                            map_preparation_error,
                        )
                }
            });
        let batch_failed = batch_set.revalidate(&CancellationToken::new()).is_err();
        let prepared_failed = self
            .prepared
            .revalidate_for_mandatory_finalization()
            .is_err();
        let result = finish_publication(
            primary,
            batch_failed || prepared_failed,
            check_gate(self.prepared.operation_deadline(), cancellation)
                .map_err(map_preparation_error),
        );
        if result.is_err() && result != Err(ActiveGenerationQualificationReceiptSetError::NotReady)
        {
            self.terminal = true;
        }
        result
    }
}

fn finish_publication<T>(
    primary: Result<T, ActiveGenerationQualificationReceiptSetError>,
    finalization_failed: bool,
    terminal_gate: Result<(), ActiveGenerationQualificationReceiptSetError>,
) -> Result<T, ActiveGenerationQualificationReceiptSetError> {
    use ActiveGenerationQualificationReceiptSetError as Error;
    match (terminal_gate, finalization_failed) {
        (Err(Error::DeadlineExceeded), true) => Err(Error::DeadlineAndFinalization),
        (Err(Error::Cancelled), true) => Err(Error::CancelledAndFinalization),
        (Err(error), _) => Err(error),
        (Ok(()), _) => combine_finalization(primary, finalization_failed),
    }
}

fn map_preparation_error(
    error: GenerationQualificationPreparationError,
) -> ActiveGenerationQualificationReceiptSetError {
    match error {
        GenerationQualificationPreparationError::DeadlineExceeded => {
            ActiveGenerationQualificationReceiptSetError::DeadlineExceeded
        }
        GenerationQualificationPreparationError::Cancelled => {
            ActiveGenerationQualificationReceiptSetError::Cancelled
        }
        _ => ActiveGenerationQualificationReceiptSetError::Publication,
    }
}

fn combine_finalization<T>(
    primary: Result<T, ActiveGenerationQualificationReceiptSetError>,
    finalization_failed: bool,
) -> Result<T, ActiveGenerationQualificationReceiptSetError> {
    match (primary, finalization_failed) {
        (Ok(_), true) => Err(ActiveGenerationQualificationReceiptSetError::MandatoryFinalization),
        (Err(_), true) => Err(ActiveGenerationQualificationReceiptSetError::PrimaryAndFinalization),
        (result, false) => result,
    }
}

pub(super) fn rederive_from_ledger(
    relations: CandidateGenerationReceiptSetV1Relations<'_>,
    records: &[CandidateGenerationAttemptRecordV1],
    receipts: &[CandidateGenerationReceiptV1],
) -> Result<CandidateGenerationReceiptSetV1, ActiveGenerationQualificationReceiptSetError> {
    let mut selected_records = Vec::with_capacity(relations.suite.case_ids().len());
    let mut selected_receipts = Vec::with_capacity(relations.suite.case_ids().len());
    for case in relations.suite.case_ids() {
        let matching: Vec<_> = receipts
            .iter()
            .filter(|receipt| {
                receipt.case_id() == case
                    && receipt.repetition_id() == relations.repetition.repetition_id()
                    && receipt.generation_system_id()
                        == relations.generation_system.generation_system_id()
            })
            .collect();
        let [receipt] = matching.as_slice() else {
            return Err(ActiveGenerationQualificationReceiptSetError::OperationScope);
        };
        let matching_records: Vec<_> = records.iter().filter(|record| matches!(
            record.outcome(), rewrite_model::CandidateGenerationAttemptOutcomeV1::Completed {
                planned_attempt_id, receipt_id, ..
            } if planned_attempt_id == receipt.planned_attempt_id() && receipt_id == receipt.receipt_id()
        )).collect();
        let [record] = matching_records.as_slice() else {
            return Err(ActiveGenerationQualificationReceiptSetError::OperationScope);
        };
        selected_records.push((*record).clone());
        selected_receipts.push((*receipt).clone());
    }
    CandidateGenerationReceiptSetV1::new(CandidateGenerationReceiptSetV1Relations {
        attempt_records: &selected_records,
        receipts: &selected_receipts,
        ..relations
    })
    .map_err(|_| ActiveGenerationQualificationReceiptSetError::OperationScope)
}

#[cfg(test)]
pub(crate) mod tests;
