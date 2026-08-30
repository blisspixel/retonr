use rewrite_types::Digest;

use super::super::{
    EffectivePackageEvidenceRoleV2, EffectivePackageEvidenceV2, EffectivePackageEvidenceV2Error,
    EffectivePackageEvidenceV2Input, EffectivePackageMemberEvidenceV2, EffectivePackageMemberUseV2,
};
use crate::{
    ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath, ComputeBackend,
    EffectivePackageEvidenceMode, EffectivePackageMemberPurpose, EffectiveRuntimeState,
    EffectiveRuntimeStateInput, ExecutionPlacement, PackageTransformationDisposition, RuntimeAbi,
    RuntimeArchitecture, RuntimeBuildIdentity, RuntimeBuildIdentityInput, RuntimeBuildMode,
    RuntimeOperatingSystem, RuntimeTarget,
};

pub(super) fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}

fn path(value: &str) -> ArtifactSetRelativePath {
    ArtifactSetRelativePath::new(value).expect("valid path")
}

pub(super) fn artifact_member(relative_path: &str, bytes: &[u8]) -> ArtifactSetMember {
    ArtifactSetMember::new(
        ArtifactId::from_digest(Digest::sha256(bytes)),
        bytes.len() as u64,
        path(relative_path),
    )
}

pub(super) fn artifact_set() -> ArtifactSetManifest {
    ArtifactSetManifest::new(vec![
        artifact_member("config/ollama-config.json", b"ollama config"),
        artifact_member("config/parameters.json", b"ollama parameters"),
        artifact_member("legal/license.txt", b"ollama license"),
        artifact_member("model/model.gguf", b"ollama model"),
        artifact_member("prompts/template.go.tmpl", b"ollama template"),
        artifact_member(
            "provenance/ollama-manifest-v2.json",
            b"ollama manifest provenance",
        ),
    ])
    .expect("six-member Ollama artifact set")
}

pub(super) fn runtime_build(mode: RuntimeBuildMode) -> RuntimeBuildIdentity {
    RuntimeBuildIdentity::new(RuntimeBuildIdentityInput {
        mode,
        runtime_family: "ollama".to_owned(),
        reported_version: "0.16.2".to_owned(),
        build_revision: Some("0123456789abcdef".to_owned()),
        target: RuntimeTarget::new(
            RuntimeOperatingSystem::Linux,
            RuntimeArchitecture::X86_64,
            RuntimeAbi::LinuxGnuLibc,
        )
        .expect("runtime target"),
        package_manifest_digest: digest("runtime package"),
        entrypoint_digest: digest("runtime entrypoint"),
        packaged_dependencies_digest: digest("runtime dependencies"),
        build_configuration_digest: digest("runtime build configuration"),
    })
    .expect("runtime build")
}

pub(super) fn runtime_state(build: &RuntimeBuildIdentity) -> EffectiveRuntimeState {
    EffectiveRuntimeState::new(
        build,
        EffectiveRuntimeStateInput {
            provider_snapshot_contract: "ollama-snapshot".to_owned(),
            provider_snapshot_schema_version: 2,
            provider_snapshot_digest: digest("provider snapshot"),
            launch_policy_digest: digest("launch policy"),
            loaded_components_digest: digest("loaded components"),
            effective_configuration_digest: digest("effective configuration"),
            platform_digest: digest("platform"),
            execution_class_digest: digest("execution class"),
            isolation_policy_digest: digest("isolation policy"),
            effective_context_tokens: 32_768,
            compute_backend: ComputeBackend::NativeCpu,
            placement: ExecutionPlacement::CpuOnly,
        },
    )
    .expect("runtime state")
}

fn member_use(relative_path: &str) -> EffectivePackageMemberUseV2 {
    match relative_path {
        "config/ollama-config.json" => EffectivePackageMemberUseV2::Effective {
            purposes: vec![EffectivePackageMemberPurpose::AuxiliaryData],
        },
        "config/parameters.json" => EffectivePackageMemberUseV2::Effective {
            purposes: vec![EffectivePackageMemberPurpose::GenerationConfiguration],
        },
        "legal/license.txt" => EffectivePackageMemberUseV2::EvidenceOnly {
            roles: vec![EffectivePackageEvidenceRoleV2::LicenseText],
        },
        "model/model.gguf" => EffectivePackageMemberUseV2::Effective {
            purposes: vec![
                EffectivePackageMemberPurpose::ModelWeights,
                EffectivePackageMemberPurpose::ModelConfiguration,
                EffectivePackageMemberPurpose::TokenizerModel,
                EffectivePackageMemberPurpose::PromptTemplate,
            ],
        },
        "prompts/template.go.tmpl" => EffectivePackageMemberUseV2::Effective {
            purposes: vec![EffectivePackageMemberPurpose::PromptTemplate],
        },
        "provenance/ollama-manifest-v2.json" => EffectivePackageMemberUseV2::EffectiveAndEvidence {
            purposes: vec![EffectivePackageMemberPurpose::AuxiliaryData],
            roles: vec![EffectivePackageEvidenceRoleV2::ProvenanceRecord],
        },
        _ => panic!("unexpected fixture member"),
    }
}

pub(super) fn member_evidence(
    member: &ArtifactSetMember,
    member_use: EffectivePackageMemberUseV2,
) -> EffectivePackageMemberEvidenceV2 {
    EffectivePackageMemberEvidenceV2::new(
        member.relative_path().clone(),
        member.artifact_id().clone(),
        member.byte_size(),
        member_use,
    )
    .expect("valid member evidence")
}

pub(super) fn evidence_input(
    artifact_set: &ArtifactSetManifest,
    mode: EffectivePackageEvidenceMode,
) -> EffectivePackageEvidenceV2Input {
    EffectivePackageEvidenceV2Input {
        evidence_mode: mode,
        evidence_contract_id: "managed-ollama-package-attestor".to_owned(),
        evidence_contract_schema_version: 2,
        member_evidence: artifact_set
            .members()
            .iter()
            .map(|member| member_evidence(member, member_use(member.relative_path().as_str())))
            .collect(),
        artifact_set_completeness_evidence_digest: digest("completeness"),
        acquisition_evidence_digest: digest("acquisition"),
        license_review_evidence_digest: digest("license review"),
        transformation: PackageTransformationDisposition::Untransformed {
            evidence_digest: digest("untransformed"),
        },
        runtime_load_closure_evidence_digest: digest("load closure"),
        exclusion_isolation_evidence_digest: digest("exclusion and isolation"),
    }
}

pub(super) fn evidence_fixture() -> (
    ArtifactSetManifest,
    RuntimeBuildIdentity,
    EffectiveRuntimeState,
    EffectivePackageEvidenceV2,
) {
    let set = artifact_set();
    let build = runtime_build(RuntimeBuildMode::ManagedProcess);
    let state = runtime_state(&build);
    let evidence = EffectivePackageEvidenceV2::new(
        &set,
        &build,
        &state,
        evidence_input(&set, EffectivePackageEvidenceMode::ManagedImmutablePackage),
    )
    .expect("version 2 effective-package evidence");
    (set, build, state, evidence)
}

pub(super) fn decode_value(
    value: &serde_json::Value,
    artifact_set: &ArtifactSetManifest,
    build: &RuntimeBuildIdentity,
    state: &EffectiveRuntimeState,
) -> Result<EffectivePackageEvidenceV2, EffectivePackageEvidenceV2Error> {
    EffectivePackageEvidenceV2::from_json_bytes(
        &serde_json::to_vec(value).expect("encode value"),
        artifact_set,
        build,
        state,
    )
}
