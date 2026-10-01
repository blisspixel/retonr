//! Exact parent reconstruction and inert repeatability result publication.

use super::{GenerationQualificationPreregistrationRepository, map_store_error};
use crate::generation_qualification_preregistration::{
    GenerationQualificationPreparationError, check_gate,
};
use rewrite_model::GenerationRepeatabilityResultRecordV1;
use rewrite_model_store::{
    GenerationAttemptLedgerV1Input, GenerationAttemptLedgerV1TransactionError,
    GenerationRepeatabilityTerminalResultV1Input, GenerationRepeatabilityTerminalResultV1ReadInput,
    GenerationRepeatabilityTerminalResultV1TransactionError, WriteDisposition,
};
use rewrite_types::CancellationToken;
use std::time::Instant;

impl GenerationQualificationPreregistrationRepository {
    pub(crate) fn persist_attempt_ledger(
        &mut self,
        input: GenerationAttemptLedgerV1Input<'_>,
        deadline: Instant,
        cancellation: &CancellationToken,
    ) -> Result<WriteDisposition, GenerationQualificationPreparationError> {
        check_gate(deadline, cancellation)?;
        let disposition = self
            .store
            .transact_generation_attempt_ledger_v1(input, || check_gate(deadline, cancellation))
            .map_err(|error| match error {
                GenerationAttemptLedgerV1TransactionError::Store(error) => map_store_error(&error),
                GenerationAttemptLedgerV1TransactionError::Gate(error) => error,
            })?;
        let stored = self
            .store
            .generation_attempt_ledger_v1(input)
            .map_err(|error| map_store_error(&error))?;
        if stored.as_ref() != Some(input.manifest) {
            return Err(GenerationQualificationPreparationError::ReadbackMismatch);
        }
        check_gate(deadline, cancellation)?;
        Ok(disposition)
    }

    pub(crate) fn persist_repeatability_phase(
        &mut self,
        input: rewrite_model_store::GenerationRepeatabilityPhaseV1Input<'_>,
        deadline: Instant,
        cancellation: &CancellationToken,
    ) -> Result<WriteDisposition, GenerationQualificationPreparationError> {
        use rewrite_model_store::GenerationRepeatabilityPhaseV1TransactionError;
        check_gate(deadline, cancellation)?;
        let disposition = self
            .store
            .transact_generation_repeatability_phase_v1(input, || {
                check_gate(deadline, cancellation)
            })
            .map_err(|error| match error {
                GenerationRepeatabilityPhaseV1TransactionError::Store(error) => {
                    map_store_error(&error)
                }
                GenerationRepeatabilityPhaseV1TransactionError::Gate(error) => error,
            })?;
        let stored = self
            .store
            .generation_repeatability_phase_v1(input)
            .map_err(|error| map_store_error(&error))?;
        if stored.as_ref() != Some(input.manifest) {
            return Err(GenerationQualificationPreparationError::ReadbackMismatch);
        }
        check_gate(deadline, cancellation)?;
        Ok(disposition)
    }

    pub(crate) fn persist_repeatability_result(
        &mut self,
        record: &GenerationRepeatabilityResultRecordV1,
        deadline: Instant,
        cancellation: &CancellationToken,
    ) -> Result<WriteDisposition, GenerationQualificationPreparationError> {
        check_gate(deadline, cancellation)?;
        let disposition = self
            .store
            .transact_generation_repeatability_terminal_result_v1(
                GenerationRepeatabilityTerminalResultV1Input { record },
                || check_gate(deadline, cancellation),
            )
            .map_err(|error| match error {
                GenerationRepeatabilityTerminalResultV1TransactionError::Store(error) => {
                    map_store_error(&error)
                }
                GenerationRepeatabilityTerminalResultV1TransactionError::Gate(error) => error,
            })?;
        let stored = self
            .store
            .generation_repeatability_terminal_result_v1(
                GenerationRepeatabilityTerminalResultV1ReadInput { record },
            )
            .map_err(|error| map_store_error(&error))?;
        if stored.as_ref() != Some(record) {
            return Err(GenerationQualificationPreparationError::ReadbackMismatch);
        }
        check_gate(deadline, cancellation)?;
        Ok(disposition)
    }
}
