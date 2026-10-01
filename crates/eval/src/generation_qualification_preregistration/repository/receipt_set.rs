//! Durable inert receipt-set publication with fresh canonical readback.

use std::time::Instant;

use rewrite_model::CandidateGenerationReceiptSetV1;
use rewrite_model_store::{
    CandidateGenerationReceiptSetV1Input, CandidateGenerationReceiptSetV1ReadInput,
    CandidateGenerationReceiptSetV1TransactionError, WriteDisposition,
};
use rewrite_types::CancellationToken;

use super::{GenerationQualificationPreregistrationRepository, map_store_error};
use crate::generation_qualification_preregistration::{
    GenerationQualificationPreparationError, check_gate,
};

impl GenerationQualificationPreregistrationRepository {
    pub(crate) fn persist_receipt_set(
        &mut self,
        record: &CandidateGenerationReceiptSetV1,
        deadline: Instant,
        cancellation: &CancellationToken,
    ) -> Result<WriteDisposition, GenerationQualificationPreparationError> {
        let disposition = self
            .store
            .transact_candidate_generation_receipt_set_v1(
                CandidateGenerationReceiptSetV1Input { record },
                || check_gate(deadline, cancellation),
            )
            .map_err(|error| match error {
                CandidateGenerationReceiptSetV1TransactionError::Store(error) => {
                    map_store_error(&error)
                }
                CandidateGenerationReceiptSetV1TransactionError::Gate(error) => error,
            })?;
        check_gate(deadline, cancellation)?;
        let stored = self
            .store
            .candidate_generation_receipt_set_v1(CandidateGenerationReceiptSetV1ReadInput {
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
