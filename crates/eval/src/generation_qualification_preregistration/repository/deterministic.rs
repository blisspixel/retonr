//! Canonical durable settlement of one inert compiler-derived evaluation.

use rewrite_model::CandidateDeterministicEvaluationRecordV1;
use rewrite_model_store::{
    CandidateDeterministicEvaluationV1Input, CandidateDeterministicEvaluationV1ReadInput,
    CandidateDeterministicEvaluationV1TransactionError, WriteDisposition,
};
use rewrite_types::CancellationToken;
use std::time::Instant;

use super::{GenerationQualificationPreregistrationRepository, map_store_error};
use crate::generation_qualification_preregistration::{
    GenerationQualificationPreparationError, check_gate,
};

impl GenerationQualificationPreregistrationRepository {
    pub(crate) fn persist_deterministic_evaluation(
        &mut self,
        record: &CandidateDeterministicEvaluationRecordV1,
        deadline: Instant,
        cancellation: &CancellationToken,
    ) -> Result<WriteDisposition, GenerationQualificationPreparationError> {
        let disposition = self
            .store
            .transact_candidate_deterministic_evaluation_v1(
                CandidateDeterministicEvaluationV1Input { record },
                || check_gate(deadline, cancellation),
            )
            .map_err(|error| match error {
                CandidateDeterministicEvaluationV1TransactionError::Store(error) => {
                    map_store_error(&error)
                }
                CandidateDeterministicEvaluationV1TransactionError::Gate(error) => error,
            })?;
        check_gate(deadline, cancellation)?;
        let stored = self
            .store
            .candidate_deterministic_evaluation_v1(CandidateDeterministicEvaluationV1ReadInput {
                record,
            })
            .map_err(|error| map_store_error(&error))?;
        check_gate(deadline, cancellation)?;
        if stored.as_ref() != Some(record) {
            return Err(GenerationQualificationPreparationError::ReadbackMismatch);
        }
        Ok(disposition)
    }
}
