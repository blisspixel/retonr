//! Portable projections used by offline receipt-set publication tests.

use rewrite_model::{CandidateGenerationAttemptRecordV1, CandidateGenerationReceiptV1};
use rewrite_types::CancellationToken;

use super::Scenario;

impl Scenario {
    pub(crate) fn into_offline_set(self) -> crate::VerifiedCandidateBatchSet {
        let core = super::super::super::VerifiedCandidateBatchSetCore::verify(
            self.input,
            self.batches,
            &CancellationToken::new(),
        )
        .expect("offline complete set fixture");
        super::super::offline_authority(core)
    }

    pub(crate) fn portable_attempts(
        &self,
    ) -> (
        Vec<CandidateGenerationAttemptRecordV1>,
        Vec<CandidateGenerationReceiptV1>,
    ) {
        (
            self.batches
                .iter()
                .map(|batch| batch.attempt_record.clone())
                .collect(),
            self.batches
                .iter()
                .map(|batch| batch.receipt.clone())
                .collect(),
        )
    }
}
