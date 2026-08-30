use serde::Deserialize;

use super::{
    CandidateGenerationCleanupFailureCategoryV1, CandidateGenerationCleanupRecordV1Input,
    CandidateGenerationPackageRevalidationStatusV1, CandidateGenerationProcessCleanupStatusV1,
};
use crate::generation_qualification::{
    CandidateGenerationAttemptPrecursorId, CandidateGenerationAttemptPrecursorV1,
    ManagedOllamaCandidateGenerationEvidenceV2, ManagedOllamaCandidateGenerationEvidenceV2Id,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CleanupWire {
    schema_version: u32,
    precursor_id: CandidateGenerationAttemptPrecursorId,
    managed_evidence_id: ManagedOllamaCandidateGenerationEvidenceV2Id,
    process_cleanup_status: CandidateGenerationProcessCleanupStatusV1,
    runtime_package_revalidation_status: CandidateGenerationPackageRevalidationStatusV1,
    model_package_revalidation_status: CandidateGenerationPackageRevalidationStatusV1,
    failure_categories: Vec<CandidateGenerationCleanupFailureCategoryV1>,
}

impl CleanupWire {
    pub(super) const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub(super) const fn input(&self) -> CandidateGenerationCleanupRecordV1Input {
        CandidateGenerationCleanupRecordV1Input {
            process_cleanup_status: self.process_cleanup_status,
            runtime_package_revalidation_status: self.runtime_package_revalidation_status,
            model_package_revalidation_status: self.model_package_revalidation_status,
        }
    }

    pub(super) fn failure_categories(&self) -> &[CandidateGenerationCleanupFailureCategoryV1] {
        &self.failure_categories
    }

    pub(super) fn matches(
        &self,
        precursor: &CandidateGenerationAttemptPrecursorV1,
        managed_evidence: &ManagedOllamaCandidateGenerationEvidenceV2,
    ) -> bool {
        self.precursor_id == *precursor.precursor_id()
            && self.managed_evidence_id == *managed_evidence.managed_evidence_v2_id()
    }
}
