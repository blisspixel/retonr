use rewrite_types::Digest;

use super::super::{
    EffectivePackageEvidence, EffectivePackageEvidenceError, EffectivePackageEvidenceInput,
    EffectivePackageEvidenceMode, EffectivePackageMemberEvidence, EffectivePackageMemberPurpose,
    PackageTransformationDisposition,
};
use crate::{
    ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath, ComputeBackend,
    EffectiveRuntimeState, EffectiveRuntimeStateInput, ExecutionPlacement, RuntimeAbi,
    RuntimeArchitecture, RuntimeBuildIdentity, RuntimeBuildIdentityInput, RuntimeBuildMode,
    RuntimeOperatingSystem, RuntimeTarget,
};

pub(super) fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}

pub(super) fn artifact_member(path: &str, bytes: &[u8]) -> ArtifactSetMember {
    ArtifactSetMember::new(
        ArtifactId::from_digest(Digest::sha256(bytes)),
        bytes.len() as u64,
        ArtifactSetRelativePath::new(path).expect("member path"),
    )
}

pub(super) fn artifact_set() -> ArtifactSetManifest {
    ArtifactSetManifest::new(vec![
        artifact_member("config.json", b"{}"),
        artifact_member("model.gguf", b"weights"),
    ])
    .expect("artifact set")
}

fn build_input(mode: RuntimeBuildMode) -> RuntimeBuildIdentityInput {
    RuntimeBuildIdentityInput {
        mode,
        runtime_family: "llama-server".to_owned(),
        reported_version: "b10417".to_owned(),
        build_revision: Some("0123456789abcdef".to_owned()),
        target: RuntimeTarget::new(
            RuntimeOperatingSystem::Windows,
            RuntimeArchitecture::X86_64,
            RuntimeAbi::WindowsMsvc,
        )
        .expect("runtime target"),
        package_manifest_digest: digest("package"),
        entrypoint_digest: digest("entrypoint"),
        packaged_dependencies_digest: digest("dependencies"),
        build_configuration_digest: digest("build configuration"),
    }
}

pub(super) fn runtime_build(mode: RuntimeBuildMode) -> RuntimeBuildIdentity {
    RuntimeBuildIdentity::new(build_input(mode)).expect("runtime build")
}

pub(super) fn runtime_state(build: &RuntimeBuildIdentity) -> EffectiveRuntimeState {
    EffectiveRuntimeState::new(
        build,
        EffectiveRuntimeStateInput {
            provider_snapshot_contract: "llama-server-snapshot".to_owned(),
            provider_snapshot_schema_version: 1,
            provider_snapshot_digest: digest("provider snapshot"),
            launch_policy_digest: digest("launch"),
            loaded_components_digest: digest("loaded components"),
            effective_configuration_digest: digest("effective configuration"),
            platform_digest: digest("platform"),
            execution_class_digest: digest("execution class"),
            isolation_policy_digest: digest("isolation"),
            effective_context_tokens: 32_768,
            compute_backend: ComputeBackend::Cuda,
            placement: ExecutionPlacement::AcceleratorOnly,
        },
    )
    .expect("runtime state")
}

pub(super) fn member_evidence(
    path: &str,
    purposes: Vec<EffectivePackageMemberPurpose>,
) -> EffectivePackageMemberEvidence {
    EffectivePackageMemberEvidence::new(
        ArtifactSetRelativePath::new(path).expect("evidence path"),
        purposes,
    )
    .expect("member evidence")
}

pub(super) fn evidence_input(mode: EffectivePackageEvidenceMode) -> EffectivePackageEvidenceInput {
    EffectivePackageEvidenceInput {
        evidence_mode: mode,
        evidence_contract_id: "managed-package-attestor".to_owned(),
        evidence_contract_schema_version: 1,
        member_evidence: vec![
            member_evidence(
                "config.json",
                vec![EffectivePackageMemberPurpose::ModelConfiguration],
            ),
            member_evidence(
                "model.gguf",
                vec![EffectivePackageMemberPurpose::ModelWeights],
            ),
        ],
        artifact_set_completeness_evidence_digest: digest("completeness"),
        acquisition_evidence_digest: digest("acquisition"),
        license_review_evidence_digest: digest("license review"),
        transformation: PackageTransformationDisposition::Untransformed {
            evidence_digest: digest("untransformed"),
        },
        runtime_load_closure_evidence_digest: digest("load closure"),
        exclusion_isolation_evidence_digest: digest("exclusion isolation"),
    }
}

pub(super) fn evidence_fixture() -> (
    ArtifactSetManifest,
    RuntimeBuildIdentity,
    EffectiveRuntimeState,
    EffectivePackageEvidence,
) {
    let artifact_set = artifact_set();
    let build = runtime_build(RuntimeBuildMode::ManagedProcess);
    let state = runtime_state(&build);
    let evidence = EffectivePackageEvidence::new(
        &artifact_set,
        &build,
        &state,
        evidence_input(EffectivePackageEvidenceMode::ManagedImmutablePackage),
    )
    .expect("effective package evidence");
    (artifact_set, build, state, evidence)
}

pub(super) fn decode_value(
    value: &serde_json::Value,
    artifact_set: &ArtifactSetManifest,
    build: &RuntimeBuildIdentity,
    state: &EffectiveRuntimeState,
) -> Result<EffectivePackageEvidence, EffectivePackageEvidenceError> {
    EffectivePackageEvidence::from_json_bytes(
        &serde_json::to_vec(value).expect("encode value"),
        artifact_set,
        build,
        state,
    )
}

pub(super) const fn all_purposes() -> [EffectivePackageMemberPurpose; 17] {
    [
        EffectivePackageMemberPurpose::ModelWeights,
        EffectivePackageMemberPurpose::ModelShardIndex,
        EffectivePackageMemberPurpose::ModelConfiguration,
        EffectivePackageMemberPurpose::GenerationConfiguration,
        EffectivePackageMemberPurpose::TokenizerModel,
        EffectivePackageMemberPurpose::TokenizerVocabulary,
        EffectivePackageMemberPurpose::TokenizerMerges,
        EffectivePackageMemberPurpose::TokenizerConfiguration,
        EffectivePackageMemberPurpose::PromptTemplate,
        EffectivePackageMemberPurpose::SystemPrompt,
        EffectivePackageMemberPurpose::GrammarOrSchema,
        EffectivePackageMemberPurpose::Adapter,
        EffectivePackageMemberPurpose::Projector,
        EffectivePackageMemberPurpose::DraftModel,
        EffectivePackageMemberPurpose::CustomModelCode,
        EffectivePackageMemberPurpose::CustomGenerationCode,
        EffectivePackageMemberPurpose::AuxiliaryData,
    ]
}
