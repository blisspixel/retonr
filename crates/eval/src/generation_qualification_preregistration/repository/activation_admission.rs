//! Read-only candidate admission snapshot for one qualification plan.

use rewrite_model::{GenerationQualificationPlanId, PlannedCandidateAttemptId};
use rewrite_model_store::{
    CandidateGenerationAttemptAdmissionV1, CandidateGenerationEvidenceStorageRootId, StoreResult,
};

use super::GenerationQualificationPreregistrationRepository;

impl GenerationQualificationPreregistrationRepository {
    pub(crate) fn read_candidate_activation_admission(
        &self,
        plan_id: &GenerationQualificationPlanId,
        planned_attempt_ids: &[PlannedCandidateAttemptId],
        storage_root_id: &CandidateGenerationEvidenceStorageRootId,
    ) -> StoreResult<Vec<CandidateGenerationAttemptAdmissionV1>> {
        self.store.candidate_generation_attempt_admission_v1(
            plan_id,
            planned_attempt_ids,
            storage_root_id,
        )
    }
}
