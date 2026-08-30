use rewrite_model::{
    ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath, ComputeBackend,
    EffectivePackageEvidenceMode, EffectivePackageEvidenceRoleV2, EffectivePackageEvidenceV2,
    EffectivePackageEvidenceV2Input, EffectivePackageMemberEvidenceV2,
    EffectivePackageMemberPurpose, EffectivePackageMemberUseV2, EffectiveRuntimeState,
    EffectiveRuntimeStateInput, EmbeddedModelComponent, EmbeddedModelComponentPurpose,
    ExecutionPlacement, FrozenExternalComponentSetId, GenerationSystemRecordV1,
    GenerationSystemRecordV1Input, GenerationSystemRecordV1Relations, ManagedGenerationPathId,
    ModelPackageManifest, ModelPackageMember, ModelPackageMemberRole, ModelWeightLayout,
    PackageSource, PackageSourceKind, PackageTransformation, PackageTransformationDisposition,
    RuntimeAbi, RuntimeAdmissionJoinId, RuntimeArchitecture, RuntimeBuildIdentity,
    RuntimeBuildMode, RuntimeOperatingSystem, RuntimePackageLoadPolicy, RuntimePackageManifest,
    RuntimePackageMember, RuntimePackageMemberRole, RuntimeTarget,
};
use rewrite_types::Digest;

pub(super) struct SystemFixture {
    runtime_package: RuntimePackageManifest,
    runtime_build: RuntimeBuildIdentity,
    runtime_state: EffectiveRuntimeState,
    model_set: ArtifactSetManifest,
    model_package: ModelPackageManifest,
    model_artifact_id: ArtifactId,
    pub(super) effective_package: EffectivePackageEvidenceV2,
}

impl SystemFixture {
    pub(super) fn relations(&self) -> GenerationSystemRecordV1Relations<'_> {
        GenerationSystemRecordV1Relations {
            runtime_package_manifest: &self.runtime_package,
            runtime_build: &self.runtime_build,
            effective_runtime_state: &self.runtime_state,
            model_artifact_set: &self.model_set,
            model_package_manifest: &self.model_package,
            effective_package_evidence_v2: &self.effective_package,
        }
    }

    fn input(&self, identity_label: &str) -> GenerationSystemRecordV1Input {
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
            strategy_digest: digest(&format!("strategy {identity_label}")),
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

    pub(super) fn record(&self) -> GenerationSystemRecordV1 {
        self.record_named("default")
    }

    pub(super) fn record_named(&self, identity_label: &str) -> GenerationSystemRecordV1 {
        GenerationSystemRecordV1::new(self.relations(), self.input(identity_label))
            .expect("generation system")
    }

    pub(super) fn record_named_with_contract(
        &self,
        identity_label: &str,
        prompt_digest: Digest,
        output_schema_digest: Digest,
    ) -> GenerationSystemRecordV1 {
        let mut input = self.input(identity_label);
        input.prompt_digest = prompt_digest;
        input.output_schema_digest = output_schema_digest;
        GenerationSystemRecordV1::new(self.relations(), input).expect("judge generation system")
    }
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

fn runtime_package() -> RuntimePackageManifest {
    let set = ArtifactSetManifest::new(vec![
        ArtifactSetMember::new(artifact("runtime"), 10, path("bin/runtime")),
        ArtifactSetMember::new(artifact("runtime license"), 11, path("legal/license.txt")),
        ArtifactSetMember::new(
            artifact("runtime provenance"),
            12,
            path("legal/provenance.txt"),
        ),
    ])
    .expect("runtime set");
    RuntimePackageManifest::new(
        &set,
        "ollama",
        "0.32.15",
        Some("fixture".to_owned()),
        RuntimeTarget::new(
            RuntimeOperatingSystem::Linux,
            RuntimeArchitecture::X86_64,
            RuntimeAbi::LinuxGnuLibc,
        )
        .expect("runtime target"),
        source("runtime"),
        PackageTransformation::Untransformed {
            evidence_digest: digest("runtime untransformed"),
        },
        vec![
            RuntimePackageMember::new(
                artifact("runtime"),
                10,
                path("bin/runtime"),
                vec![RuntimePackageMemberRole::Entrypoint],
                RuntimePackageLoadPolicy::RequiredAtReady,
            ),
            RuntimePackageMember::new(
                artifact("runtime license"),
                11,
                path("legal/license.txt"),
                vec![RuntimePackageMemberRole::LicenseText],
                RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
            ),
            RuntimePackageMember::new(
                artifact("runtime provenance"),
                12,
                path("legal/provenance.txt"),
                vec![RuntimePackageMemberRole::ProvenanceRecord],
                RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
            ),
        ],
    )
    .expect("runtime package")
}

fn runtime_state(build: &RuntimeBuildIdentity) -> EffectiveRuntimeState {
    EffectiveRuntimeState::new(
        build,
        EffectiveRuntimeStateInput {
            provider_snapshot_contract: "ollama-snapshot".to_owned(),
            provider_snapshot_schema_version: 1,
            provider_snapshot_digest: digest("snapshot"),
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

fn model_records(model_label: &str) -> (ArtifactSetManifest, ModelPackageManifest, ArtifactId) {
    let model = artifact(model_label);
    let set = ArtifactSetManifest::new(vec![
        ArtifactSetMember::new(artifact("model license"), 10, path("legal/license.txt")),
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
                artifact("model license"),
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
            embedded_component(
                EmbeddedModelComponentPurpose::ModelConfiguration,
                "general.architecture",
                "model config",
            ),
            embedded_component(
                EmbeddedModelComponentPurpose::GenerationConfiguration,
                "generation.defaults",
                "generation config",
            ),
            embedded_component(
                EmbeddedModelComponentPurpose::Tokenizer,
                "tokenizer.ggml",
                "tokenizer",
            ),
            embedded_component(
                EmbeddedModelComponentPurpose::PromptTemplate,
                "tokenizer.chat_template",
                "template",
            ),
        ],
    )
    .expect("model package");
    (set, package, model)
}

fn embedded_component(
    purpose: EmbeddedModelComponentPurpose,
    semantic_key: &str,
    label: &str,
) -> EmbeddedModelComponent {
    EmbeddedModelComponent::new(
        path("model/model.gguf"),
        purpose,
        "gguf-metadata",
        1,
        semantic_key,
        digest(label),
    )
    .expect("embedded component")
}

fn effective_package(
    set: &ArtifactSetManifest,
    build: &RuntimeBuildIdentity,
    state: &EffectiveRuntimeState,
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
                "legal/license.txt" => EffectivePackageMemberUseV2::EvidenceOnly {
                    roles: vec![EffectivePackageEvidenceRoleV2::LicenseText],
                },
                "legal/provenance.txt" => EffectivePackageMemberUseV2::EvidenceOnly {
                    roles: vec![EffectivePackageEvidenceRoleV2::ProvenanceRecord],
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
    assert!(
        set.members()
            .iter()
            .any(|member| member.artifact_id() == &model_artifact_id)
    );
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

pub(super) fn fixture() -> SystemFixture {
    fixture_with_model("model bytes")
}

pub(super) fn fixture_with_model(model_label: &str) -> SystemFixture {
    let runtime_package = runtime_package();
    let runtime_build = RuntimeBuildIdentity::new_from_package_manifest(
        RuntimeBuildMode::ManagedProcess,
        &runtime_package,
    )
    .expect("runtime build");
    let runtime_state = runtime_state(&runtime_build);
    let (model_set, model_package, model_artifact_id) = model_records(model_label);
    let effective_package = effective_package(&model_set, &runtime_build, &runtime_state);
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

pub(crate) fn judge_system(
    identity_label: &str,
    prompt_digest: Digest,
    output_schema_digest: Digest,
) -> GenerationSystemRecordV1 {
    fixture().record_named_with_contract(identity_label, prompt_digest, output_schema_digest)
}

pub(crate) fn judge_system_with_model(
    identity_label: &str,
    prompt_digest: Digest,
    output_schema_digest: Digest,
    model_label: &str,
) -> GenerationSystemRecordV1 {
    fixture_with_model(model_label).record_named_with_contract(
        identity_label,
        prompt_digest,
        output_schema_digest,
    )
}

pub(super) fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}
