//! Atomic terminal qualification closeout and independent fresh snapshot readback.

use super::{GenerationQualificationPreregistrationRepository, map_store_error};
use crate::generation_qualification_preregistration::{
    GenerationQualificationPreparationError, check_gate,
};
use rewrite_model::GenerationQualificationRecordV1;
use rewrite_model_store::{
    GenerationQualificationTerminalCloseoutV1Input,
    GenerationQualificationTerminalEvidenceV1TransactionError, WriteDisposition,
};
use rewrite_types::CancellationToken;
use std::time::Instant;

impl GenerationQualificationPreregistrationRepository {
    pub(crate) fn persist_terminal_closeout(
        &mut self,
        input: GenerationQualificationTerminalCloseoutV1Input<'_>,
        deadline: Instant,
        cancellation: &CancellationToken,
    ) -> Result<WriteDisposition, GenerationQualificationPreparationError> {
        let disposition = self
            .store
            .transact_generation_qualification_terminal_closeout_v1(input, || {
                check_gate(deadline, cancellation)
            })
            .map_err(|error| match error {
                GenerationQualificationTerminalEvidenceV1TransactionError::Store(error) => {
                    map_store_error(&error)
                }
                GenerationQualificationTerminalEvidenceV1TransactionError::Gate(error) => error,
            })?;
        if self
            .store
            .generation_qualification_terminal_closeout_v1(input)
            .map_err(|error| map_store_error(&error))?
            .as_ref()
            != Some(input.record)
        {
            return Err(GenerationQualificationPreparationError::ReadbackMismatch);
        }
        check_gate(deadline, cancellation)?;
        Ok(disposition)
    }

    pub(crate) fn read_qualification_record(
        &self,
        record: &GenerationQualificationRecordV1,
    ) -> Result<Option<GenerationQualificationRecordV1>, GenerationQualificationPreparationError>
    {
        self.store
            .generation_qualification_record_v1(
                rewrite_model_store::GenerationQualificationRecordV1ReadInput { record },
            )
            .map_err(|error| map_store_error(&error))
    }
}
