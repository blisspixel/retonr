use rewrite_model::{CandidateGenerationAttemptRecordV1, CandidateGenerationReceiptV1};

use super::OfflineBatch;

impl OfflineBatch {
    pub(crate) const fn portable_receipt(&self) -> &CandidateGenerationReceiptV1 {
        &self.receipt
    }

    pub(crate) const fn portable_attempt_record(&self) -> &CandidateGenerationAttemptRecordV1 {
        &self.attempt_record
    }
}
