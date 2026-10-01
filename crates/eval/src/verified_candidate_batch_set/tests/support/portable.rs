use rewrite_model::{
    ArtifactSetRelativePath, CandidateGenerationAttemptRecordV1, CandidateGenerationReceiptV1,
};

use super::OfflineBatch;

pub(super) fn path(value: &str) -> ArtifactSetRelativePath {
    ArtifactSetRelativePath::new(value).expect("fixture path")
}

impl OfflineBatch {
    pub(crate) const fn portable_receipt(&self) -> &CandidateGenerationReceiptV1 {
        &self.receipt
    }

    pub(crate) const fn portable_attempt_record(&self) -> &CandidateGenerationAttemptRecordV1 {
        &self.attempt_record
    }
}
