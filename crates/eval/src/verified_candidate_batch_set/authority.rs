//! Private type erasure for retained candidate-batch authorities.

use rewrite_inference::GenerationCandidate;
use rewrite_model::{
    CandidateGenerationAttemptRecordV1, CandidateGenerationReceiptSetV1,
    CandidateGenerationReceiptV1, GenerationResourceAttemptResultRecordV1,
};
use rewrite_types::CancellationToken;

use crate::active_generation_qualification_subject::ActiveGenerationQualificationBinding;

use super::{
    CandidateBatchSetScope, CoreError, VerifiedCandidateBatch, VerifiedCandidateBatchError,
    VerifiedCandidateBatchSet, VerifiedCandidateBatchSetCore, VerifiedCandidateBatchSetError,
};

pub(super) trait RetainedCandidateBatch {
    type Error;

    fn active_binding(&self) -> Option<&ActiveGenerationQualificationBinding> {
        None
    }
    fn receipt(&self) -> &CandidateGenerationReceiptV1;
    fn attempt_record(&self) -> &CandidateGenerationAttemptRecordV1;
    fn resource_result(&self) -> Option<&GenerationResourceAttemptResultRecordV1>;
    fn candidate_count(&self) -> usize;
    fn candidate(
        &self,
        ordinal: u8,
        cancellation: &CancellationToken,
    ) -> Result<Option<&GenerationCandidate>, Self::Error>;
    fn revalidate(&self, cancellation: &CancellationToken) -> Result<(), Self::Error>;
}

#[rustfmt::skip]
impl RetainedCandidateBatch for VerifiedCandidateBatch {
    type Error = VerifiedCandidateBatchError;
    fn active_binding(&self) -> Option<&ActiveGenerationQualificationBinding> { VerifiedCandidateBatch::active_binding(self) }
    fn receipt(&self) -> &CandidateGenerationReceiptV1 { VerifiedCandidateBatch::receipt(self) }
    fn attempt_record(&self) -> &CandidateGenerationAttemptRecordV1 { VerifiedCandidateBatch::attempt_record(self) }
    fn resource_result(&self) -> Option<&GenerationResourceAttemptResultRecordV1> { VerifiedCandidateBatch::resource_result(self) }
    fn candidate_count(&self) -> usize { VerifiedCandidateBatch::candidate_count(self) }
    fn candidate(&self, ordinal: u8, cancellation: &CancellationToken) -> Result<Option<&GenerationCandidate>, Self::Error> { VerifiedCandidateBatch::candidate(self, ordinal, cancellation) }
    fn revalidate(&self, cancellation: &CancellationToken) -> Result<(), Self::Error> { VerifiedCandidateBatch::revalidate(self, cancellation) }
}

pub(super) trait CandidateBatchSetAuthority {
    fn validate_scope(
        &self,
        scope: CandidateBatchSetScope<'_>,
        cancellation: &CancellationToken,
    ) -> Result<(), VerifiedCandidateBatchSetError>;
    fn managed_evidence_inputs(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<
        Vec<(
            rewrite_model::PlannedCandidateAttemptId,
            rewrite_model::ManagedOllamaCandidateGenerationEvidenceV2Input,
        )>,
        VerifiedCandidateBatchSetError,
    >;
    fn active_binding(&self) -> Option<&ActiveGenerationQualificationBinding>;
    fn batch_count(&self) -> usize;
    fn revalidate(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), VerifiedCandidateBatchSetError>;
    fn receipt_set(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<&CandidateGenerationReceiptSetV1, VerifiedCandidateBatchSetError>;
    #[cfg(test)]
    fn attempt_records_for_test(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<Vec<CandidateGenerationAttemptRecordV1>, VerifiedCandidateBatchSetError>;
    fn selected_candidates<'a>(
        &'a self,
        cancellation: &CancellationToken,
    ) -> Result<Vec<&'a GenerationCandidate>, VerifiedCandidateBatchSetError>;
    fn resource_results<'a>(
        &'a self,
        cancellation: &CancellationToken,
    ) -> Result<
        Option<Vec<&'a GenerationResourceAttemptResultRecordV1>>,
        VerifiedCandidateBatchSetError,
    >;
}

struct CandidateBatchSetAuthorityCore<B: RetainedCandidateBatch> {
    core: VerifiedCandidateBatchSetCore<B>,
    map_error: fn(CoreError<B::Error>) -> VerifiedCandidateBatchSetError,
}

impl<B: RetainedCandidateBatch> CandidateBatchSetAuthority for CandidateBatchSetAuthorityCore<B> {
    fn validate_scope(
        &self,
        scope: CandidateBatchSetScope<'_>,
        cancellation: &CancellationToken,
    ) -> Result<(), VerifiedCandidateBatchSetError> {
        self.core.revalidate(cancellation).map_err(self.map_error)?;
        super::scope::validate_core_scope(&self.core, scope)
            .map_err(VerifiedCandidateBatchSetError::Relationship)?;
        self.core.revalidate(cancellation).map_err(self.map_error)
    }
    fn managed_evidence_inputs(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<
        Vec<(
            rewrite_model::PlannedCandidateAttemptId,
            rewrite_model::ManagedOllamaCandidateGenerationEvidenceV2Input,
        )>,
        VerifiedCandidateBatchSetError,
    > {
        self.core.revalidate(cancellation).map_err(self.map_error)?;
        let inputs = self
            .core
            .batches
            .iter()
            .map(|batch| {
                let receipt = batch.receipt();
                (
                    receipt.planned_attempt_id().clone(),
                    rewrite_model::ManagedOllamaCandidateGenerationEvidenceV2Input {
                        bracket_observation_v1_id: receipt.bracket_observation_v1_id().clone(),
                        effective_runtime_state_join_id: receipt
                            .effective_runtime_state_join_id()
                            .clone(),
                        response_id: receipt.response_id().clone(),
                    },
                )
            })
            .collect();
        self.core.revalidate(cancellation).map_err(self.map_error)?;
        Ok(inputs)
    }
    fn active_binding(&self) -> Option<&ActiveGenerationQualificationBinding> {
        self.core.active_binding.as_ref()
    }

    fn batch_count(&self) -> usize {
        self.core.batches.len()
    }

    fn revalidate(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), VerifiedCandidateBatchSetError> {
        self.core.revalidate(cancellation).map_err(self.map_error)
    }

    fn receipt_set(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<&CandidateGenerationReceiptSetV1, VerifiedCandidateBatchSetError> {
        self.core.receipt_set(cancellation).map_err(self.map_error)
    }

    #[cfg(test)]
    fn attempt_records_for_test(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<Vec<CandidateGenerationAttemptRecordV1>, VerifiedCandidateBatchSetError> {
        self.core.revalidate(cancellation).map_err(self.map_error)?;
        let records = self.core.attempt_records.clone();
        self.core.revalidate(cancellation).map_err(self.map_error)?;
        Ok(records)
    }

    fn selected_candidates<'a>(
        &'a self,
        cancellation: &CancellationToken,
    ) -> Result<Vec<&'a GenerationCandidate>, VerifiedCandidateBatchSetError> {
        self.core
            .selected_candidates(cancellation)
            .map_err(self.map_error)
    }

    fn resource_results<'a>(
        &'a self,
        cancellation: &CancellationToken,
    ) -> Result<
        Option<Vec<&'a GenerationResourceAttemptResultRecordV1>>,
        VerifiedCandidateBatchSetError,
    > {
        self.core
            .resource_results(cancellation)
            .map_err(self.map_error)
    }
}

pub(super) fn erase_core<B: RetainedCandidateBatch + 'static>(
    core: VerifiedCandidateBatchSetCore<B>,
    map_error: fn(CoreError<B::Error>) -> VerifiedCandidateBatchSetError,
) -> VerifiedCandidateBatchSet {
    VerifiedCandidateBatchSet {
        authority: Box::new(CandidateBatchSetAuthorityCore { core, map_error }),
    }
}
