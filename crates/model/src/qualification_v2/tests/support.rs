use rewrite_types::Digest;

use super::super::{QualificationRecordV2, QualificationRecordV2Error, QualificationRecordV2Input};
use crate::{
    ArtifactId, ArtifactRole, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath,
    ComputeBackend, EffectivePackageEvidence, EffectivePackageEvidenceInput,
    EffectivePackageEvidenceMode, EffectivePackageMemberEvidence, EffectivePackageMemberPurpose,
    EffectiveRuntimeState, EffectiveRuntimeStateInput, ExecutionPlacement, HardwareTier,
    LicenseDecision, PackageTransformationDisposition, QualificationRecord, QualificationStatus,
    RuntimeAbi, RuntimeArchitecture, RuntimeBuildIdentity, RuntimeBuildIdentityInput,
    RuntimeBuildMode, RuntimeIdentity, RuntimeOperatingSystem, RuntimeTarget,
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

pub(super) fn runtime_build(mode: RuntimeBuildMode, version: &str) -> RuntimeBuildIdentity {
    RuntimeBuildIdentity::new(RuntimeBuildIdentityInput {
        mode,
        runtime_family: "llama-server".to_owned(),
        reported_version: version.to_owned(),
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
    })
    .expect("runtime build")
}

pub(super) fn runtime_state(
    build: &RuntimeBuildIdentity,
    context_tokens: u32,
) -> EffectiveRuntimeState {
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
            effective_context_tokens: context_tokens,
            compute_backend: ComputeBackend::Cuda,
            placement: ExecutionPlacement::AcceleratorOnly,
        },
    )
    .expect("runtime state")
}

pub(super) fn effective_package(
    artifact_set: &ArtifactSetManifest,
    build: &RuntimeBuildIdentity,
    state: &EffectiveRuntimeState,
    acquisition_label: &str,
) -> EffectivePackageEvidence {
    let member_evidence = artifact_set
        .members()
        .iter()
        .map(|member| {
            let purpose = if member.relative_path().as_str() == "model.gguf" {
                EffectivePackageMemberPurpose::ModelWeights
            } else {
                EffectivePackageMemberPurpose::ModelConfiguration
            };
            EffectivePackageMemberEvidence::new(member.relative_path().clone(), vec![purpose])
                .expect("member evidence")
        })
        .collect();
    EffectivePackageEvidence::new(
        artifact_set,
        build,
        state,
        EffectivePackageEvidenceInput {
            evidence_mode: EffectivePackageEvidenceMode::ManagedImmutablePackage,
            evidence_contract_id: "managed-package-attestor".to_owned(),
            evidence_contract_schema_version: 1,
            member_evidence,
            artifact_set_completeness_evidence_digest: digest("completeness"),
            acquisition_evidence_digest: digest(acquisition_label),
            license_review_evidence_digest: digest("license review"),
            transformation: PackageTransformationDisposition::Untransformed {
                evidence_digest: digest("untransformed"),
            },
            runtime_load_closure_evidence_digest: digest("load closure"),
            exclusion_isolation_evidence_digest: digest("exclusion isolation"),
        },
    )
    .expect("effective package evidence")
}

pub(super) fn qualification_input() -> QualificationRecordV2Input {
    QualificationRecordV2Input {
        source_byte_limit: 16_777_216,
        context_token_limit: 32_768,
        prompt_template_digest: digest("prompt template"),
        claim_output_contract_digest: digest("claim output contract"),
        claim_operation_contract_digest: digest("claim operation contract"),
        request_policy_digest: digest("request policy"),
        threshold_policy_digest: digest("threshold policy"),
        language_policy_digest: digest("language policy"),
        hardware_envelope_digest: digest("hardware envelope"),
        qualification_suite_digest: digest("qualification suite"),
        qualification_result_evidence_digest: digest("qualification result"),
        license_decision: LicenseDecision::LocalUseOnly,
        status: QualificationStatus::Qualified,
    }
}

pub(super) fn fixture() -> (
    ArtifactSetManifest,
    RuntimeBuildIdentity,
    EffectiveRuntimeState,
    EffectivePackageEvidence,
    QualificationRecordV2,
) {
    let artifact_set = artifact_set();
    let build = runtime_build(RuntimeBuildMode::ManagedProcess, "b10417");
    let state = runtime_state(&build, 32_768);
    let package = effective_package(&artifact_set, &build, &state, "acquisition");
    let record = QualificationRecordV2::new(
        &artifact_set,
        &build,
        &state,
        &package,
        qualification_input(),
    )
    .expect("qualification v2");
    (artifact_set, build, state, package, record)
}

pub(super) fn decode_value(
    value: &serde_json::Value,
    artifact_set: &ArtifactSetManifest,
    build: &RuntimeBuildIdentity,
    state: &EffectiveRuntimeState,
    package: &EffectivePackageEvidence,
) -> Result<QualificationRecordV2, QualificationRecordV2Error> {
    QualificationRecordV2::from_json_bytes(
        &serde_json::to_vec(value).expect("encode value"),
        artifact_set,
        build,
        state,
        package,
    )
}

pub(super) fn v1_qualification() -> QualificationRecord {
    let artifact_digest = digest("artifact");
    QualificationRecord {
        schema_version: crate::QUALIFICATION_SCHEMA_VERSION,
        artifact_id: ArtifactId::from_digest(artifact_digest.clone()),
        artifact_digest,
        runtime: RuntimeIdentity {
            backend: "fake".to_owned(),
            version: "1.0.0".to_owned(),
            digest: None,
        },
        operating_system: "test".to_owned(),
        hardware_tier: HardwareTier {
            id: "fixture".to_owned(),
            memory_mib: 8_192,
            accelerator: "none".to_owned(),
        },
        supported_roles: vec![ArtifactRole::Generation],
        source_byte_limit: 4_096,
        context_token_limit: 8_192,
        prompt_template_digest: digest("prompt"),
        request_policy_digest: digest("request"),
        threshold_policy_digest: digest("threshold"),
        license_decision: LicenseDecision::LocalUseOnly,
        status: QualificationStatus::Qualified,
    }
}
