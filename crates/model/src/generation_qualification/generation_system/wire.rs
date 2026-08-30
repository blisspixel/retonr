use serde::Deserialize;

use rewrite_types::Digest;

use super::{GenerationSystemRecordV1Input, GenerationSystemRecordV1Relations};
use crate::generation_qualification::{
    FrozenExternalComponentSetId, GenerationQualificationContractError, ManagedGenerationPathId,
    RuntimeAdmissionJoinId,
};
use crate::{
    ArtifactId, ArtifactSetId, EffectivePackageEvidenceV2Id, EffectiveRuntimeStateId,
    ModelPackageManifestId, RuntimeBuildId, RuntimePackageManifestId,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GenerationSystemWire {
    schema_version: u32,
    runtime_admission_join_id: RuntimeAdmissionJoinId,
    managed_generation_path_id: ManagedGenerationPathId,
    frozen_external_component_set_id: FrozenExternalComponentSetId,
    runtime_package_manifest_id: RuntimePackageManifestId,
    runtime_build_id: RuntimeBuildId,
    effective_runtime_state_id: EffectiveRuntimeStateId,
    model_artifact_set_id: ArtifactSetId,
    model_package_manifest_id: ModelPackageManifestId,
    model_artifact_id: ArtifactId,
    effective_package_evidence_v2_id: EffectivePackageEvidenceV2Id,
    static_model_binding_digest: Digest,
    strategy_digest: Digest,
    planner_digest: Digest,
    validator_digest: Digest,
    adapter_digest: Digest,
    prompt_digest: Digest,
    output_schema_digest: Digest,
    request_policy_digest: Digest,
    language_digest: Digest,
    mode_digest: Digest,
    format_digest: Digest,
    operating_system_digest: Digest,
    architecture_digest: Digest,
    execution_class_digest: Digest,
    hardware_envelope_digest: Digest,
}

impl GenerationSystemWire {
    pub(super) const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub(super) fn into_input(self) -> GenerationSystemRecordV1Input {
        GenerationSystemRecordV1Input {
            runtime_admission_join_id: self.runtime_admission_join_id,
            managed_generation_path_id: self.managed_generation_path_id,
            frozen_external_component_set_id: self.frozen_external_component_set_id,
            model_artifact_id: self.model_artifact_id,
            static_model_binding_digest: self.static_model_binding_digest,
            strategy_digest: self.strategy_digest,
            planner_digest: self.planner_digest,
            validator_digest: self.validator_digest,
            adapter_digest: self.adapter_digest,
            prompt_digest: self.prompt_digest,
            output_schema_digest: self.output_schema_digest,
            request_policy_digest: self.request_policy_digest,
            language_digest: self.language_digest,
            mode_digest: self.mode_digest,
            format_digest: self.format_digest,
            operating_system_digest: self.operating_system_digest,
            architecture_digest: self.architecture_digest,
            execution_class_digest: self.execution_class_digest,
            hardware_envelope_digest: self.hardware_envelope_digest,
        }
    }

    pub(super) fn relationship_ids_equal(&self, expected: &ExpectedRelationshipIds) -> bool {
        self.runtime_package_manifest_id == expected.runtime_package
            && self.runtime_build_id == expected.runtime_build
            && self.effective_runtime_state_id == expected.effective_runtime_state
            && self.model_artifact_set_id == expected.model_artifact_set
            && self.model_package_manifest_id == expected.model_package_manifest
            && self.effective_package_evidence_v2_id == expected.effective_package_evidence_v2
    }

    pub(super) fn relationship_error(
        &self,
        expected: &ExpectedRelationshipIds,
    ) -> GenerationQualificationContractError {
        if self.runtime_package_manifest_id != expected.runtime_package {
            GenerationQualificationContractError::RuntimePackageMismatch
        } else if self.runtime_build_id != expected.runtime_build
            || self.effective_runtime_state_id != expected.effective_runtime_state
        {
            GenerationQualificationContractError::RuntimeBuildMismatch
        } else if self.model_artifact_set_id != expected.model_artifact_set
            || self.model_package_manifest_id != expected.model_package_manifest
        {
            GenerationQualificationContractError::ModelPackageMismatch
        } else {
            GenerationQualificationContractError::EffectivePackageMismatch
        }
    }
}

pub(super) struct ExpectedRelationshipIds {
    pub(super) runtime_package: RuntimePackageManifestId,
    pub(super) runtime_build: RuntimeBuildId,
    pub(super) effective_runtime_state: EffectiveRuntimeStateId,
    pub(super) model_artifact_set: ArtifactSetId,
    pub(super) model_package_manifest: ModelPackageManifestId,
    pub(super) effective_package_evidence_v2: EffectivePackageEvidenceV2Id,
}

impl From<GenerationSystemRecordV1Relations<'_>> for ExpectedRelationshipIds {
    fn from(relations: GenerationSystemRecordV1Relations<'_>) -> Self {
        Self {
            runtime_package: relations
                .runtime_package_manifest
                .runtime_package_manifest_id(),
            runtime_build: relations.runtime_build.runtime_build_id(),
            effective_runtime_state: relations
                .effective_runtime_state
                .effective_runtime_state_id(),
            model_artifact_set: relations.model_artifact_set.artifact_set_id(),
            model_package_manifest: relations.model_package_manifest.model_package_manifest_id(),
            effective_package_evidence_v2: relations
                .effective_package_evidence_v2
                .effective_package_evidence_v2_id(),
        }
    }
}
