use rewrite_types::Digest;

use super::*;
use crate::{
    ArtifactSetMember, ArtifactSetRelativePath, ComputeBackend, EffectivePackageEvidenceMode,
    EffectivePackageEvidenceRoleV2, EffectivePackageEvidenceV2Input,
    EffectivePackageMemberEvidenceV2, EffectivePackageMemberPurpose, EffectivePackageMemberUseV2,
    EffectiveRuntimeStateInput, ExecutionPlacement, ModelPackageMember, ModelPackageMemberRole,
    ModelWeightLayout, PackageSource, PackageSourceKind, PackageTransformation,
    PackageTransformationDisposition, RuntimeAbi, RuntimeArchitecture, RuntimeBuildMode,
    RuntimeOperatingSystem, RuntimePackageLoadPolicy, RuntimePackageMember,
    RuntimePackageMemberRole, RuntimeTarget,
};

pub(crate) struct SystemFixture {
    pub runtime_package: RuntimePackageManifest,
    pub runtime_build: RuntimeBuildIdentity,
    pub runtime_state: EffectiveRuntimeState,
    pub model_set: ArtifactSetManifest,
    pub model_package: ModelPackageManifest,
    pub model_artifact_id: ArtifactId,
    pub effective_package: EffectivePackageEvidenceV2,
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

    pub(crate) fn input(&self) -> GenerationSystemRecordV1Input {
        GenerationSystemRecordV1Input {
            runtime_admission_join_id: RuntimeAdmissionJoinId::from_derived_digest(digest(
                "runtime admission",
            )),
            managed_generation_path_id: ManagedGenerationPathId::from_derived_digest(digest(
                "managed path",
            )),
            frozen_external_component_set_id: FrozenExternalComponentSetId::from_derived_digest(
                digest("frozen set"),
            ),
            model_artifact_id: self.model_artifact_id.clone(),
            static_model_binding_digest: digest("static model binding"),
            strategy_digest: digest("strategy"),
            planner_digest: digest("planner"),
            validator_digest: digest("validator"),
            adapter_digest: digest("adapter"),
            prompt_digest: digest("prompt"),
            output_schema_digest: digest("output schema"),
            request_policy_digest: digest("request policy"),
            language_digest: digest("language"),
            mode_digest: digest("mode"),
            format_digest: digest("format"),
            operating_system_digest: digest("operating system"),
            architecture_digest: digest("architecture"),
            execution_class_digest: digest("execution class"),
            hardware_envelope_digest: digest("hardware envelope"),
        }
    }

    pub(crate) fn record(&self) -> GenerationSystemRecordV1 {
        GenerationSystemRecordV1::new(self.relations(), self.input()).expect("generation system")
    }
}

pub(crate) fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}

fn path(value: &str) -> ArtifactSetRelativePath {
    ArtifactSetRelativePath::new(value).expect("fixture path")
}

fn artifact(label: &str) -> ArtifactId {
    ArtifactId::from_digest(digest(label))
}

fn source(label: &str) -> PackageSource {
    PackageSource::new(
        PackageSourceKind::LocalArchive,
        format!("{label}-archive"),
        "sha256-fixture",
        digest(&format!("{label} provenance")),
    )
    .expect("package source")
}

fn runtime_package(label: &str) -> RuntimePackageManifest {
    let set = ArtifactSetManifest::new(vec![
        ArtifactSetMember::new(
            artifact(&format!("{label} runtime")),
            10,
            path("bin/runtime"),
        ),
        ArtifactSetMember::new(
            artifact(&format!("{label} license")),
            11,
            path("legal/license.txt"),
        ),
        ArtifactSetMember::new(
            artifact(&format!("{label} provenance")),
            12,
            path("legal/provenance.txt"),
        ),
    ])
    .expect("runtime set");
    RuntimePackageManifest::new(
        &set,
        "ollama",
        "0.32.15",
        Some(label.to_owned()),
        RuntimeTarget::new(
            RuntimeOperatingSystem::Linux,
            RuntimeArchitecture::X86_64,
            RuntimeAbi::LinuxGnuLibc,
        )
        .expect("target"),
        source(label),
        PackageTransformation::Untransformed {
            evidence_digest: digest(&format!("{label} untransformed")),
        },
        vec![
            RuntimePackageMember::new(
                artifact(&format!("{label} runtime")),
                10,
                path("bin/runtime"),
                vec![RuntimePackageMemberRole::Entrypoint],
                RuntimePackageLoadPolicy::RequiredAtReady,
            ),
            RuntimePackageMember::new(
                artifact(&format!("{label} license")),
                11,
                path("legal/license.txt"),
                vec![RuntimePackageMemberRole::LicenseText],
                RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
            ),
            RuntimePackageMember::new(
                artifact(&format!("{label} provenance")),
                12,
                path("legal/provenance.txt"),
                vec![RuntimePackageMemberRole::ProvenanceRecord],
                RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
            ),
        ],
    )
    .expect("runtime package")
}

pub(crate) fn runtime_state(build: &RuntimeBuildIdentity, suffix: &str) -> EffectiveRuntimeState {
    EffectiveRuntimeState::new(
        build,
        EffectiveRuntimeStateInput {
            provider_snapshot_contract: "ollama-snapshot".to_owned(),
            provider_snapshot_schema_version: 1,
            provider_snapshot_digest: digest(&format!("snapshot {suffix}")),
            launch_policy_digest: digest("launch policy"),
            loaded_components_digest: digest("loaded components"),
            effective_configuration_digest: digest("effective configuration"),
            platform_digest: digest("platform"),
            execution_class_digest: digest("runtime execution class"),
            isolation_policy_digest: digest("isolation"),
            effective_context_tokens: 8_192,
            compute_backend: ComputeBackend::NativeCpu,
            placement: ExecutionPlacement::CpuOnly,
        },
    )
    .expect("runtime state")
}

fn model_records(shared_content: bool) -> (ArtifactSetManifest, ModelPackageManifest, ArtifactId) {
    let model = artifact("model bytes");
    let license = if shared_content {
        model.clone()
    } else {
        artifact("model license")
    };
    let set = ArtifactSetManifest::new(vec![
        ArtifactSetMember::new(license.clone(), 10, path("legal/license.txt")),
        ArtifactSetMember::new(
            artifact("model provenance"),
            11,
            path("legal/provenance.txt"),
        ),
        ArtifactSetMember::new(model.clone(), 12, path("model/model.gguf")),
    ])
    .expect("model set");
    let package = ModelPackageManifest::new(
        &set,
        "gguf",
        1,
        source("model"),
        PackageTransformation::Untransformed {
            evidence_digest: digest("model untransformed"),
        },
        vec![
            ModelPackageMember::new(
                license,
                10,
                path("legal/license.txt"),
                vec![ModelPackageMemberRole::LicenseText],
            ),
            ModelPackageMember::new(
                artifact("model provenance"),
                11,
                path("legal/provenance.txt"),
                vec![ModelPackageMemberRole::ProvenanceRecord],
            ),
            ModelPackageMember::new(
                model.clone(),
                12,
                path("model/model.gguf"),
                vec![ModelPackageMemberRole::ModelWeights],
            ),
        ],
        ModelWeightLayout::Single {
            member: path("model/model.gguf"),
        },
        vec![
            crate::EmbeddedModelComponent::new(
                path("model/model.gguf"),
                crate::EmbeddedModelComponentPurpose::ModelConfiguration,
                "gguf-metadata",
                1,
                "general.architecture",
                digest("model config"),
            )
            .expect("model config"),
            crate::EmbeddedModelComponent::new(
                path("model/model.gguf"),
                crate::EmbeddedModelComponentPurpose::GenerationConfiguration,
                "gguf-metadata",
                1,
                "generation.defaults",
                digest("generation config"),
            )
            .expect("generation config"),
            crate::EmbeddedModelComponent::new(
                path("model/model.gguf"),
                crate::EmbeddedModelComponentPurpose::Tokenizer,
                "gguf-metadata",
                1,
                "tokenizer.ggml",
                digest("tokenizer"),
            )
            .expect("tokenizer"),
            crate::EmbeddedModelComponent::new(
                path("model/model.gguf"),
                crate::EmbeddedModelComponentPurpose::PromptTemplate,
                "gguf-metadata",
                1,
                "tokenizer.chat_template",
                digest("template"),
            )
            .expect("template"),
        ],
    )
    .expect("model package");
    (set, package, model)
}

pub(crate) fn effective_package(
    set: &ArtifactSetManifest,
    build: &RuntimeBuildIdentity,
    state: &EffectiveRuntimeState,
    exclude_model: bool,
) -> EffectivePackageEvidenceV2 {
    let model_artifact_id = set
        .members()
        .iter()
        .find(|member| member.relative_path().as_str() == "model/model.gguf")
        .expect("model member")
        .artifact_id()
        .clone();
    let member_evidence = set
        .members()
        .iter()
        .map(|member| {
            let member_use = match member.relative_path().as_str() {
                "legal/license.txt" if member.artifact_id() == &model_artifact_id => {
                    EffectivePackageMemberUseV2::EffectiveAndEvidence {
                        purposes: effective_weight_purposes(),
                        roles: vec![EffectivePackageEvidenceRoleV2::LicenseText],
                    }
                }
                "legal/license.txt" => EffectivePackageMemberUseV2::EvidenceOnly {
                    roles: vec![EffectivePackageEvidenceRoleV2::LicenseText],
                },
                "legal/provenance.txt" => EffectivePackageMemberUseV2::EvidenceOnly {
                    roles: vec![EffectivePackageEvidenceRoleV2::ProvenanceRecord],
                },
                "model/model.gguf" if exclude_model => EffectivePackageMemberUseV2::Excluded {
                    declared_purposes: vec![EffectivePackageMemberPurpose::ModelWeights],
                },
                "model/model.gguf" => EffectivePackageMemberUseV2::Effective {
                    purposes: effective_weight_purposes(),
                },
                _ => panic!("unexpected model member"),
            };
            EffectivePackageMemberEvidenceV2::new(
                member.relative_path().clone(),
                member.artifact_id().clone(),
                member.byte_size(),
                member_use,
            )
            .expect("member evidence")
        })
        .collect();
    EffectivePackageEvidenceV2::new(
        set,
        build,
        state,
        EffectivePackageEvidenceV2Input {
            evidence_mode: EffectivePackageEvidenceMode::ManagedImmutablePackage,
            evidence_contract_id: "managed-ollama-package".to_owned(),
            evidence_contract_schema_version: 2,
            member_evidence,
            artifact_set_completeness_evidence_digest: digest("set completeness"),
            acquisition_evidence_digest: digest("acquisition"),
            license_review_evidence_digest: digest("license review"),
            transformation: PackageTransformationDisposition::Untransformed {
                evidence_digest: digest("logical binding"),
            },
            runtime_load_closure_evidence_digest: digest("runtime closure"),
            exclusion_isolation_evidence_digest: digest("exclusion isolation"),
        },
    )
    .expect("effective package")
}

fn effective_weight_purposes() -> Vec<EffectivePackageMemberPurpose> {
    vec![
        EffectivePackageMemberPurpose::ModelWeights,
        EffectivePackageMemberPurpose::ModelConfiguration,
        EffectivePackageMemberPurpose::TokenizerModel,
        EffectivePackageMemberPurpose::PromptTemplate,
    ]
}

pub(crate) fn fixture(shared_content: bool) -> SystemFixture {
    let runtime_package = runtime_package("runtime-a");
    let runtime_build = RuntimeBuildIdentity::new_from_package_manifest(
        RuntimeBuildMode::ManagedProcess,
        &runtime_package,
    )
    .expect("runtime build");
    let runtime_state = runtime_state(&runtime_build, "a");
    let (model_set, model_package, model_artifact_id) = model_records(shared_content);
    let effective_package = effective_package(&model_set, &runtime_build, &runtime_state, false);
    SystemFixture {
        runtime_package,
        runtime_build,
        runtime_state,
        model_set,
        model_package,
        model_artifact_id,
        effective_package,
    }
}

pub(crate) fn other_runtime_package() -> RuntimePackageManifest {
    runtime_package("runtime-b")
}
