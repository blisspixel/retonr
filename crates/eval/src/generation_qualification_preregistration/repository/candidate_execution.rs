use std::time::Instant;

use rewrite_model::CandidateGenerationAttemptPrecursorV1;
use rewrite_model_store::{
    CandidateGenerationAttemptPrecursorCheckpointV1Input,
    CandidateGenerationAttemptPrecursorCheckpointV1TransactionError,
    CandidateGenerationExecutionV1Input, CandidateGenerationExecutionV1Readback,
    CandidateGenerationExecutionV1TransactionError, CandidateGenerationExecutionV1WriteDisposition,
    GenerationQualificationPreregistrationReadInput, WriteDisposition,
};
use rewrite_types::CancellationToken;

use super::{GenerationQualificationPreregistrationRepository, map_store_error};
use crate::generation_qualification_preregistration::{
    GenerationQualificationPreparationError, check_gate,
};

impl GenerationQualificationPreregistrationRepository {
    pub(crate) fn checkpoint_candidate_attempt(
        &mut self,
        preregistration: GenerationQualificationPreregistrationReadInput<'_>,
        precursor: &CandidateGenerationAttemptPrecursorV1,
        deadline: Instant,
        cancellation: &CancellationToken,
    ) -> Result<WriteDisposition, GenerationQualificationPreparationError> {
        let ((), disposition) = self
            .store
            .transact_candidate_generation_attempt_precursor_checkpoint_v1(
                CandidateGenerationAttemptPrecursorCheckpointV1Input {
                    precursor,
                    preregistration,
                },
                || check_gate(deadline, cancellation),
                |readback| {
                    if readback.precursor() == precursor {
                        Ok(())
                    } else {
                        Err(GenerationQualificationPreparationError::ReadbackMismatch)
                    }
                },
            )
            .map_err(map_checkpoint_error)?;
        Ok(disposition.precursor)
    }

    pub(crate) fn persist_candidate_execution<T>(
        &mut self,
        input: &CandidateGenerationExecutionV1Input<'_>,
        validate: impl for<'readback> FnOnce(
            CandidateGenerationExecutionV1Readback<'readback>,
        )
            -> Result<T, GenerationQualificationPreparationError>,
    ) -> Result<
        (T, CandidateGenerationExecutionV1WriteDisposition),
        GenerationQualificationPreparationError,
    > {
        self.store
            .transact_candidate_generation_execution_v1(*input, || Ok(()), validate)
            .map_err(map_execution_error)
    }
}

fn map_checkpoint_error(
    error: CandidateGenerationAttemptPrecursorCheckpointV1TransactionError<
        GenerationQualificationPreparationError,
    >,
) -> GenerationQualificationPreparationError {
    match error {
        CandidateGenerationAttemptPrecursorCheckpointV1TransactionError::Store(error) => {
            map_store_error(&error)
        }
        CandidateGenerationAttemptPrecursorCheckpointV1TransactionError::Gate(error)
        | CandidateGenerationAttemptPrecursorCheckpointV1TransactionError::Validation(error) => {
            error
        }
    }
}

fn map_execution_error(
    error: CandidateGenerationExecutionV1TransactionError<GenerationQualificationPreparationError>,
) -> GenerationQualificationPreparationError {
    match error {
        CandidateGenerationExecutionV1TransactionError::Store(error) => map_store_error(&error),
        CandidateGenerationExecutionV1TransactionError::Gate(error)
        | CandidateGenerationExecutionV1TransactionError::Validation(error) => error,
    }
}
