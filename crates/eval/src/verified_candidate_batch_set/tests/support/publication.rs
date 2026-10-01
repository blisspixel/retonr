//! Synthetic completed cohorts persisted through the real bounded store contracts.

use std::rc::Rc;

use rewrite_model::{
    CandidateGenerationAttemptPrecursorV1, CandidateGenerationCleanupRecordV1,
    CandidateGenerationEvidenceBundleManifestV1, CandidateGenerationEvidenceBundleReadbackV1,
    ManagedOllamaCandidateGenerationEvidenceV2, ManagedOllamaCandidateGenerationEvidenceV2Input,
};
use rewrite_model_store::{
    ArtifactStateStore, CandidateGenerationAttemptPrecursorCheckpointV1Input,
    CandidateGenerationEvidenceBundleStorageV1, CandidateGenerationEvidenceStorageRootId,
    CandidateGenerationEvidenceStorageV1Limits, CandidateGenerationExecutionV1Input,
    GenerationQualificationPreregistrationReadInput,
};

use super::{OfflineBatch, OfflineBatchFailureControl};

pub(super) struct PortablePublication {
    pub(super) precursor: CandidateGenerationAttemptPrecursorV1,
    pub(super) managed: ManagedOllamaCandidateGenerationEvidenceV2,
    pub(super) cleanup: CandidateGenerationCleanupRecordV1,
    pub(super) bundle: CandidateGenerationEvidenceBundleManifestV1,
    pub(super) readback: CandidateGenerationEvidenceBundleReadbackV1,
}

impl OfflineBatch {
    pub(crate) fn fail_next_revalidation(&self) {
        self.fail_on_call
            .set(Some(self.revalidation_calls.get() + 1));
    }

    pub(crate) fn remove_selected_candidate(&mut self) {
        self.candidates.pop();
    }

    pub(crate) fn failure_control(&self) -> OfflineBatchFailureControl {
        OfflineBatchFailureControl::new(
            Rc::clone(&self.revalidation_calls),
            Rc::clone(&self.fail_on_call),
        )
    }

    /// Creates inert synthetic fixture rows; it does not execute or observe a runtime.
    pub(crate) fn persist_synthetic_completed(
        &self,
        store: &mut ArtifactStateStore,
        preregistration: GenerationQualificationPreregistrationReadInput<'_>,
        storage_root: &CandidateGenerationEvidenceStorageRootId,
    ) {
        let publication = &self.publication;
        store
            .transact_candidate_generation_attempt_precursor_checkpoint_v1(
                CandidateGenerationAttemptPrecursorCheckpointV1Input {
                    precursor: &publication.precursor,
                    preregistration,
                },
                || Ok::<_, ()>(()),
                |_| Ok::<_, ()>(()),
            )
            .expect("synthetic checkpoint through real store validation");
        let storage = CandidateGenerationEvidenceBundleStorageV1::new(
            storage_root.clone(),
            self.receipt.qualification_plan_id().clone(),
            self.receipt.planned_attempt_id().clone(),
            publication.bundle.evidence_bundle_id().clone(),
            CandidateGenerationEvidenceStorageV1Limits::new(8193, 256, 256 * 1024 * 1024)
                .expect("bounded storage limits"),
        )
        .expect("synthetic inert storage reference");
        let managed_input = ManagedOllamaCandidateGenerationEvidenceV2Input {
            bracket_observation_v1_id: self.receipt.bracket_observation_v1_id().clone(),
            effective_runtime_state_join_id: self.receipt.effective_runtime_state_join_id().clone(),
            response_id: self.receipt.response_id().clone(),
        };
        store
            .transact_candidate_generation_execution_v1(
                CandidateGenerationExecutionV1Input::Completed {
                    preregistration,
                    precursor: &publication.precursor,
                    managed_evidence: &publication.managed,
                    managed_evidence_input: &managed_input,
                    cleanup: &publication.cleanup,
                    bundle: &publication.bundle,
                    storage: &storage,
                    readback: &publication.readback,
                    receipt: &self.receipt,
                    attempt: &self.attempt_record,
                },
                || Ok::<_, ()>(()),
                |_| Ok::<_, ()>(()),
            )
            .expect("synthetic completed cohort through real store validation");
    }
}
