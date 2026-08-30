use rewrite_model::{
    ArtifactId, ArtifactSetManifest, EffectivePackageEvidenceV2, EffectiveRuntimeState,
    FrozenExternalComponentSetId, GenerationSystemRecordV1Input, GenerationSystemRecordV1Relations,
    ManagedGenerationPathId, ModelPackageManifest, RuntimeAdmissionJoinId, RuntimeBuildIdentity,
    RuntimePackageManifest,
};

use super::digest;

pub(crate) struct SystemFixture {
    pub(crate) runtime_set: ArtifactSetManifest,
    pub(crate) runtime_package: RuntimePackageManifest,
    pub(crate) runtime_build: RuntimeBuildIdentity,
    pub(crate) runtime_state: EffectiveRuntimeState,
    pub(crate) model_set: ArtifactSetManifest,
    pub(crate) model_package: ModelPackageManifest,
    pub(crate) model_artifact_id: ArtifactId,
    pub(crate) effective_package: EffectivePackageEvidenceV2,
}

impl SystemFixture {
    pub(crate) fn relations(&self) -> GenerationSystemRecordV1Relations<'_> {
        GenerationSystemRecordV1Relations {
            runtime_package_manifest: &self.runtime_package,
            runtime_build: &self.runtime_build,
            effective_runtime_state: &self.runtime_state,
            model_artifact_set: &self.model_set,
            model_package_manifest: &self.model_package,
            effective_package_evidence_v2: &self.effective_package,
        }
    }

    pub(crate) fn input(&self, strategy: &str) -> GenerationSystemRecordV1Input {
        GenerationSystemRecordV1Input {
            runtime_admission_join_id: RuntimeAdmissionJoinId::from_derived_digest(digest(
                "admission",
            )),
            managed_generation_path_id: ManagedGenerationPathId::from_derived_digest(digest(
                "path",
            )),
            frozen_external_component_set_id: FrozenExternalComponentSetId::from_derived_digest(
                digest("frozen"),
            ),
            model_artifact_id: self.model_artifact_id.clone(),
            static_model_binding_digest: digest("static model"),
            strategy_digest: digest(strategy),
            planner_digest: digest("planner"),
            validator_digest: digest("validator"),
            adapter_digest: digest("adapter"),
            prompt_digest: digest("prompt"),
            output_schema_digest: digest("output schema"),
            request_policy_digest: digest("request policy"),
            language_digest: digest("language"),
            mode_digest: digest("mode"),
            format_digest: digest("format"),
            operating_system_digest: digest("os"),
            architecture_digest: digest("architecture"),
            execution_class_digest: digest("execution class"),
            hardware_envelope_digest: digest("hardware envelope"),
        }
    }
}
