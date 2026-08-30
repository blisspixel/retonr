use super::{
    CandidateArtifactEntryV1, CandidateGenerationAttemptPrecursorId, CandidateGenerationCleanupId,
    CandidateGenerationEvidenceBundleId, CandidateGenerationEvidenceBundleReadbackId,
    CandidateGenerationReceiptEvidenceClassV1, CandidateGenerationReceiptId,
    CandidateGenerationReceiptV1, CandidateGenerationUsageObservationV1, Digest,
    FrozenExternalComponentSetId, GenerationCaseId, GenerationClusterId,
    GenerationQualificationPlanId, GenerationRepetitionId, GenerationRequestBindingId,
    GenerationSuiteManifestId, GenerationSystemId, ManagedGenerationPathId,
    ManagedOllamaCandidateGenerationEvidenceV2Id, ManagedOllamaEffectiveRuntimeStateJoinId,
    ManagedOllamaGenerationBracketObservationV1Id, OllamaRetainedSessionResponseId,
    PlannedCandidateAttemptId, RuntimeAdmissionJoinId, StructuredCompletionRequestBindingId,
};
use crate::{
    ArtifactId, ArtifactSetId, EffectivePackageEvidenceV2Id, EffectiveRuntimeStateId,
    ModelPackageManifestId, RuntimeBuildId, RuntimePackageManifestId,
};

impl CandidateGenerationReceiptV1 {
    /// Returns the schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
    /// Returns the qualification-plan identity.
    #[must_use]
    pub const fn qualification_plan_id(&self) -> &GenerationQualificationPlanId {
        &self.qualification_plan_id
    }
    /// Returns the suite identity.
    #[must_use]
    pub const fn suite_manifest_id(&self) -> &GenerationSuiteManifestId {
        &self.suite_manifest_id
    }
    /// Returns the case identity.
    #[must_use]
    pub const fn case_id(&self) -> &GenerationCaseId {
        &self.case_id
    }
    /// Returns the cluster identity.
    #[must_use]
    pub const fn cluster_id(&self) -> &GenerationClusterId {
        &self.cluster_id
    }
    /// Returns the repetition identity.
    #[must_use]
    pub const fn repetition_id(&self) -> &GenerationRepetitionId {
        &self.repetition_id
    }
    /// Returns the planned-attempt identity.
    #[must_use]
    pub const fn planned_attempt_id(&self) -> &PlannedCandidateAttemptId {
        &self.planned_attempt_id
    }
    /// Returns the execution precursor identity.
    #[must_use]
    pub const fn precursor_id(&self) -> &CandidateGenerationAttemptPrecursorId {
        &self.precursor_id
    }
    /// Returns the generation-system identity.
    #[must_use]
    pub const fn generation_system_id(&self) -> &GenerationSystemId {
        &self.generation_system_id
    }
    /// Returns the source digest.
    #[must_use]
    pub const fn source_digest(&self) -> &Digest {
        &self.source_digest
    }
    /// Returns the immutable case-contract digest.
    #[must_use]
    pub const fn case_contract_digest(&self) -> &Digest {
        &self.case_contract_digest
    }
    /// Returns the grounded-request digest.
    #[must_use]
    pub const fn grounded_request_digest(&self) -> &Digest {
        &self.grounded_request_digest
    }
    /// Returns the provider-neutral request identity.
    #[must_use]
    pub const fn generation_request_binding_id(&self) -> &GenerationRequestBindingId {
        &self.generation_request_binding_id
    }
    /// Returns the structured request identity.
    #[must_use]
    pub const fn structured_request_binding_id(&self) -> &StructuredCompletionRequestBindingId {
        &self.structured_request_binding_id
    }
    /// Returns the managed evidence identity.
    #[must_use]
    pub const fn managed_evidence_id(&self) -> &ManagedOllamaCandidateGenerationEvidenceV2Id {
        &self.managed_evidence_id
    }
    /// Returns the bracket V1 observation identity.
    #[must_use]
    pub const fn bracket_observation_v1_id(
        &self,
    ) -> &ManagedOllamaGenerationBracketObservationV1Id {
        &self.bracket_observation_v1_id
    }
    /// Returns the retained response identity.
    #[must_use]
    pub const fn response_id(&self) -> &OllamaRetainedSessionResponseId {
        &self.response_id
    }
    /// Returns the runtime-admission join identity.
    #[must_use]
    pub const fn runtime_admission_join_id(&self) -> &RuntimeAdmissionJoinId {
        &self.runtime_admission_join_id
    }
    /// Returns the managed generation-path identity.
    #[must_use]
    pub const fn managed_generation_path_id(&self) -> &ManagedGenerationPathId {
        &self.managed_generation_path_id
    }
    /// Returns the frozen external-component set identity.
    #[must_use]
    pub const fn frozen_external_component_set_id(&self) -> &FrozenExternalComponentSetId {
        &self.frozen_external_component_set_id
    }
    /// Returns the runtime-package manifest identity.
    #[must_use]
    pub const fn runtime_package_manifest_id(&self) -> &RuntimePackageManifestId {
        &self.runtime_package_manifest_id
    }
    /// Returns the runtime-build identity.
    #[must_use]
    pub const fn runtime_build_id(&self) -> &RuntimeBuildId {
        &self.runtime_build_id
    }
    /// Returns the observed effective-runtime-state identity.
    #[must_use]
    pub const fn effective_runtime_state_id(&self) -> &EffectiveRuntimeStateId {
        &self.effective_runtime_state_id
    }
    /// Returns the app-owned effective-state live-join identity.
    #[must_use]
    pub const fn effective_runtime_state_join_id(
        &self,
    ) -> &ManagedOllamaEffectiveRuntimeStateJoinId {
        &self.effective_runtime_state_join_id
    }
    /// Returns the model artifact-set identity.
    #[must_use]
    pub const fn model_artifact_set_id(&self) -> &ArtifactSetId {
        &self.model_artifact_set_id
    }
    /// Returns the model-package manifest identity.
    #[must_use]
    pub const fn model_package_manifest_id(&self) -> &ModelPackageManifestId {
        &self.model_package_manifest_id
    }
    /// Returns the selected model artifact identity.
    #[must_use]
    pub const fn model_artifact_id(&self) -> &ArtifactId {
        &self.model_artifact_id
    }
    /// Returns the effective-package evidence V2 identity.
    #[must_use]
    pub const fn effective_package_evidence_v2_id(&self) -> &EffectivePackageEvidenceV2Id {
        &self.effective_package_evidence_v2_id
    }
    /// Returns the runtime installation generation.
    #[must_use]
    pub const fn runtime_installation_generation(&self) -> u64 {
        self.runtime_installation_generation
    }
    /// Returns the model installation generation.
    #[must_use]
    pub const fn model_installation_generation(&self) -> u64 {
        self.model_installation_generation
    }
    /// Returns the static model-binding digest.
    #[must_use]
    pub const fn static_model_binding_digest(&self) -> &Digest {
        &self.static_model_binding_digest
    }
    /// Returns the candidate output-contract digest.
    #[must_use]
    pub const fn candidate_output_contract_digest(&self) -> &Digest {
        &self.candidate_output_contract_digest
    }
    /// Returns the cleanup identity.
    #[must_use]
    pub const fn cleanup_id(&self) -> &CandidateGenerationCleanupId {
        &self.cleanup_id
    }
    /// Returns the evidence-bundle identity.
    #[must_use]
    pub const fn bundle_id(&self) -> &CandidateGenerationEvidenceBundleId {
        &self.bundle_id
    }
    /// Returns the verified readback identity.
    #[must_use]
    pub const fn readback_id(&self) -> &CandidateGenerationEvidenceBundleReadbackId {
        &self.readback_id
    }
    /// Returns candidate artifacts in contiguous ordinal order.
    #[must_use]
    pub fn candidate_entries(&self) -> &[CandidateArtifactEntryV1] {
        &self.candidate_entries
    }
    /// Returns the bounded response usage observation.
    #[must_use]
    pub const fn usage_observation(&self) -> CandidateGenerationUsageObservationV1 {
        self.usage_observation
    }
    /// Returns the closed evidence class.
    #[must_use]
    pub const fn evidence_class(&self) -> CandidateGenerationReceiptEvidenceClassV1 {
        self.evidence_class
    }
    /// Returns whether model loading was proven.
    #[must_use]
    pub const fn model_loaded_proven(&self) -> bool {
        self.model_loaded_proven
    }
    /// Returns whether model use was proven.
    #[must_use]
    pub const fn model_used_proven(&self) -> bool {
        self.model_used_proven
    }
    /// Returns whether application-handler execution was proven.
    #[must_use]
    pub const fn application_handler_proven(&self) -> bool {
        self.application_handler_proven
    }
    /// Returns whether formal placement was proven.
    #[must_use]
    pub const fn formal_placement_proven(&self) -> bool {
        self.formal_placement_proven
    }
    /// Returns the content-derived receipt identity.
    #[must_use]
    pub const fn receipt_id(&self) -> &CandidateGenerationReceiptId {
        &self.id
    }
}
