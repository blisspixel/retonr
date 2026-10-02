//! Atomic inert negative closeout and independent fresh snapshot readback.

use super::{GenerationQualificationPreregistrationRepository, map_store_error};
use crate::generation_qualification_preregistration::{
    GenerationQualificationPreparationError, check_gate,
};
use rewrite_model_store::{
    GenerationQualificationTerminalEvidenceV1TransactionError, GenerationResourceRejectionV1Input,
    WriteDisposition,
};
use rewrite_types::CancellationToken;
use std::time::Instant;

impl GenerationQualificationPreregistrationRepository {
    pub(crate) fn persist_resource_rejection(
        &mut self,
        input: GenerationResourceRejectionV1Input<'_>,
        deadline: Instant,
        cancellation: &CancellationToken,
    ) -> Result<WriteDisposition, GenerationQualificationPreparationError> {
        let disposition = self
            .store
            .transact_generation_resource_rejection_v1(input, || check_gate(deadline, cancellation))
            .map_err(|error| match error {
                GenerationQualificationTerminalEvidenceV1TransactionError::Store(error) => {
                    map_store_error(&error)
                }
                GenerationQualificationTerminalEvidenceV1TransactionError::Gate(error) => error,
            })?;
        if self
            .store
            .generation_resource_rejection_v1(input)
            .map_err(|error| map_store_error(&error))?
            .as_ref()
            != Some(input.record)
        {
            return Err(GenerationQualificationPreparationError::ReadbackMismatch);
        }
        check_gate(deadline, cancellation)?;
        Ok(disposition)
    }
}
