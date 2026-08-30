use serde::{Deserialize, Serialize};

use rewrite_types::Digest;

use super::{CandidateGenerationReceiptEvidenceClassV1, CandidateGenerationUsageObservationV1};
use crate::generation_qualification::{
    CandidateEvidenceId, CandidateGenerationAttemptPrecursorId, CandidateGenerationCleanupId,
    CandidateGenerationEvidenceBundleId, CandidateGenerationEvidenceBundleReadbackId,
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

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CandidateWire {
    candidate_evidence_id: CandidateEvidenceId,
    ordinal: u8,
    byte_count: u64,
    artifact_id: ArtifactId,
    relative_path: String,
    candidate_digest: Digest,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the versioned wire mirrors explicit receipt claims"
)]
pub(super) struct ReceiptWire {
    schema_version: u32,
    qualification_plan_id: GenerationQualificationPlanId,
    suite_manifest_id: GenerationSuiteManifestId,
    case_id: GenerationCaseId,
    cluster_id: GenerationClusterId,
    repetition_id: GenerationRepetitionId,
    planned_attempt_id: PlannedCandidateAttemptId,
    precursor_id: CandidateGenerationAttemptPrecursorId,
    generation_system_id: GenerationSystemId,
    source_digest: Digest,
    case_contract_digest: Digest,
    grounded_request_digest: Digest,
    generation_request_binding_id: GenerationRequestBindingId,
    structured_request_binding_id: StructuredCompletionRequestBindingId,
    managed_evidence_id: ManagedOllamaCandidateGenerationEvidenceV2Id,
    bracket_observation_v1_id: ManagedOllamaGenerationBracketObservationV1Id,
    response_id: OllamaRetainedSessionResponseId,
    runtime_admission_join_id: RuntimeAdmissionJoinId,
    managed_generation_path_id: ManagedGenerationPathId,
    frozen_external_component_set_id: FrozenExternalComponentSetId,
    runtime_package_manifest_id: RuntimePackageManifestId,
    runtime_build_id: RuntimeBuildId,
    effective_runtime_state_id: EffectiveRuntimeStateId,
    effective_runtime_state_join_id: ManagedOllamaEffectiveRuntimeStateJoinId,
    model_artifact_set_id: ArtifactSetId,
    model_package_manifest_id: ModelPackageManifestId,
    model_artifact_id: ArtifactId,
    effective_package_evidence_v2_id: EffectivePackageEvidenceV2Id,
    runtime_installation_generation: u64,
    model_installation_generation: u64,
    static_model_binding_digest: Digest,
    candidate_output_contract_digest: Digest,
    cleanup_id: CandidateGenerationCleanupId,
    bundle_id: CandidateGenerationEvidenceBundleId,
    readback_id: CandidateGenerationEvidenceBundleReadbackId,
    candidate_entries: Vec<CandidateWire>,
    usage_observation: CandidateGenerationUsageObservationV1,
    evidence_class: CandidateGenerationReceiptEvidenceClassV1,
    model_loaded_proven: bool,
    model_used_proven: bool,
    application_handler_proven: bool,
    formal_placement_proven: bool,
    qualified: bool,
}

impl ReceiptWire {
    pub(super) const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub(super) const fn usage_observation(&self) -> CandidateGenerationUsageObservationV1 {
        self.usage_observation
    }
}
