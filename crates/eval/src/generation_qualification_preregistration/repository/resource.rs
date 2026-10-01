//! Atomic resource cohort publication and immediate exact cold readback.

use super::{GenerationQualificationPreregistrationRepository, map_store_error};
use crate::generation_qualification_preregistration::{
    GenerationQualificationPreparationError, check_gate,
};
use rewrite_model_store::{
    GenerationResourcePhaseV1Disposition, GenerationResourcePhaseV1Input,
    GenerationResourcePhaseV1TransactionError,
};
use rewrite_types::CancellationToken;
use std::time::Instant;

impl GenerationQualificationPreregistrationRepository {
    pub(crate) fn persist_resource_phase(
        &mut self,
        input: GenerationResourcePhaseV1Input<'_>,
        deadline: Instant,
        cancellation: &CancellationToken,
    ) -> Result<GenerationResourcePhaseV1Disposition, GenerationQualificationPreparationError> {
        check_gate(deadline, cancellation)?;
        let disposition = self
            .store
            .transact_generation_resource_phase_v1(input, || check_gate(deadline, cancellation))
            .map_err(|error| match error {
                GenerationResourcePhaseV1TransactionError::Store(error) => map_store_error(&error),
                GenerationResourcePhaseV1TransactionError::Gate(error) => error,
            })?;
        let stored = self
            .store
            .generation_resource_phase_v1(input)
            .map_err(|error| map_store_error(&error))?
            .ok_or(GenerationQualificationPreparationError::ReadbackMismatch)?;
        if stored.manifest != *input.manifest || stored.ordered_results != input.ordered_results {
            return Err(GenerationQualificationPreparationError::ReadbackMismatch);
        }
        check_gate(deadline, cancellation)?;
        Ok(disposition)
    }
}
