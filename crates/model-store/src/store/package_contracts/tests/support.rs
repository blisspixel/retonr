use rewrite_model::{
    ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath,
    EmbeddedModelComponent, EmbeddedModelComponentPurpose, ModelPackageManifest,
    ModelPackageMember, ModelPackageMemberRole, ModelWeightLayout, NativeLoadEvidenceClass,
    NativeLoadObservation, NativeLoadObservationInput, NativeLoadOrigin, NativeLoadVisibilityScope,
    NativeLoadedComponent, NativeMappingClass, PackageSource, PackageSourceKind,
    PackageTransformation, RuntimeAbi, RuntimeArchitecture, RuntimeOperatingSystem,
    RuntimePackageLoadPolicy, RuntimePackageManifest, RuntimePackageMember,
    RuntimePackageMemberRole, RuntimeTarget,
};
use rewrite_types::Digest;

use super::super::super::ArtifactStateStore;

pub(super) struct PackageFixture {
    pub(super) source_set: ArtifactSetManifest,
    pub(super) runtime_set: ArtifactSetManifest,
    pub(super) runtime_package: RuntimePackageManifest,
    pub(super) model_set: ArtifactSetManifest,
    pub(super) model_package: ModelPackageManifest,
    pub(super) native_load: NativeLoadObservation,
}

impl PackageFixture {
    pub(super) fn new() -> Self {
        let source_set = ArtifactSetManifest::new(vec![ArtifactSetMember::new(
            artifact("source-archive"),
            9,
            path("source/archive.bin"),
        )])
        .expect("valid source artifact set");
        let runtime_members = runtime_members();
        let runtime_set = artifact_set_from_runtime(&runtime_members);
        let runtime_package = RuntimePackageManifest::new(
            &runtime_set,
            "fixture-runtime",
            "1.2.3",
            Some("revision-1".to_owned()),
            RuntimeTarget::new(
                RuntimeOperatingSystem::Linux,
                RuntimeArchitecture::X86_64,
                RuntimeAbi::LinuxGnuLibc,
            )
            .expect("valid target"),
            package_source("runtime"),
            untransformed("runtime"),
            runtime_members,
        )
        .expect("valid runtime package");
        let model_members = model_members();
        let model_set = artifact_set_from_model(&model_members);
        let model_package = ModelPackageManifest::new(
            &model_set,
            "gguf",
            1,
            package_source("model"),
            untransformed("model"),
            model_members,
            ModelWeightLayout::Single {
                member: path("model/model.gguf"),
            },
            embedded_components(),
        )
        .expect("valid model package");
        let native_load = NativeLoadObservation::new(
            &runtime_package,
            NativeLoadObservationInput {
                evidence_class: NativeLoadEvidenceClass::LinuxProcMapFiles,
                visibility_scope: NativeLoadVisibilityScope::FileBackedExecutableMappings,
                process_evidence_digest: Digest::sha256(b"process evidence"),
                observation_contract_id: "linux-proc-map-files".to_owned(),
                observation_contract_schema_version: 1,
                components: vec![NativeLoadedComponent::new(
                    artifact("runtime-entrypoint"),
                    10,
                    NativeLoadOrigin::PackagedMember {
                        relative_path: path("bin/runtime"),
                    },
                    NativeMappingClass::ExecutableImage,
                    Digest::sha256(b"entrypoint mapping"),
                )],
            },
        )
        .expect("valid native observation");
        Self {
            source_set,
            runtime_set,
            runtime_package,
            model_set,
            model_package,
            native_load,
        }
    }

    pub(super) fn put_artifact_sets(&self, store: &ArtifactStateStore) {
        store
            .put_artifact_set_manifest(&self.runtime_set)
            .expect("store runtime byte set");
        store
            .put_artifact_set_manifest(&self.model_set)
            .expect("store model byte set");
    }

    pub(super) fn put_source_artifact_set(&self, store: &ArtifactStateStore) {
        store
            .put_artifact_set_manifest(&self.source_set)
            .expect("store source byte set");
    }

    pub(super) fn transformed_runtime_package(
        &self,
    ) -> (ArtifactSetManifest, RuntimePackageManifest) {
        let mut members = runtime_members();
        members.push(RuntimePackageMember::new(
            artifact("runtime-transformation"),
            13,
            path("legal/transformation.txt"),
            vec![RuntimePackageMemberRole::TransformationRecord],
            RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
        ));
        let artifact_set = artifact_set_from_runtime(&members);
        let package = RuntimePackageManifest::new(
            &artifact_set,
            "fixture-runtime",
            "1.2.3",
            Some("revision-1".to_owned()),
            RuntimeTarget::new(
                RuntimeOperatingSystem::Linux,
                RuntimeArchitecture::X86_64,
                RuntimeAbi::LinuxGnuLibc,
            )
            .expect("valid target"),
            package_source("runtime"),
            transformed("runtime", &self.source_set),
            members,
        )
        .expect("valid transformed runtime package");
        (artifact_set, package)
    }

    pub(super) fn transformed_model_package(&self) -> (ArtifactSetManifest, ModelPackageManifest) {
        let mut members = model_members();
        members.insert(
            2,
            ModelPackageMember::new(
                artifact("model-transformation"),
                23,
                path("legal/transformation.txt"),
                vec![ModelPackageMemberRole::TransformationEvidence],
            ),
        );
        let artifact_set = artifact_set_from_model(&members);
        let package = ModelPackageManifest::new(
            &artifact_set,
            "gguf",
            1,
            package_source("model"),
            transformed("model", &self.source_set),
            members,
            ModelWeightLayout::Single {
                member: path("model/model.gguf"),
            },
            embedded_components(),
        )
        .expect("valid transformed model package");
        (artifact_set, package)
    }
}

pub(super) fn path(value: &str) -> ArtifactSetRelativePath {
    ArtifactSetRelativePath::new(value).expect("valid path")
}

pub(super) fn artifact(value: &str) -> ArtifactId {
    ArtifactId::from_digest(Digest::sha256(value.as_bytes()))
}

fn package_source(label: &str) -> PackageSource {
    PackageSource::new(
        PackageSourceKind::LocalArchive,
        format!("local-{label}-archive"),
        "sha256-fixture",
        Digest::sha256(format!("{label} provenance").as_bytes()),
    )
    .expect("valid package source")
}

fn untransformed(label: &str) -> PackageTransformation {
    PackageTransformation::Untransformed {
        evidence_digest: Digest::sha256(format!("{label} comparison").as_bytes()),
    }
}

fn transformed(label: &str, source: &ArtifactSetManifest) -> PackageTransformation {
    PackageTransformation::Transformed {
        source_artifact_set_id: source.artifact_set_id(),
        tool_evidence_digest: Digest::sha256(format!("{label} tool evidence").as_bytes()),
        parameters_digest: Digest::sha256(format!("{label} parameters").as_bytes()),
        log_digest: Digest::sha256(format!("{label} log").as_bytes()),
    }
}

fn runtime_members() -> Vec<RuntimePackageMember> {
    vec![
        RuntimePackageMember::new(
            artifact("runtime-entrypoint"),
            10,
            path("bin/runtime"),
            vec![RuntimePackageMemberRole::Entrypoint],
            RuntimePackageLoadPolicy::RequiredAtReady,
        ),
        RuntimePackageMember::new(
            artifact("runtime-license"),
            11,
            path("legal/license.txt"),
            vec![RuntimePackageMemberRole::LicenseText],
            RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
        ),
        RuntimePackageMember::new(
            artifact("runtime-provenance"),
            12,
            path("legal/provenance.txt"),
            vec![RuntimePackageMemberRole::ProvenanceRecord],
            RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
        ),
    ]
}

fn artifact_set_from_runtime(members: &[RuntimePackageMember]) -> ArtifactSetManifest {
    ArtifactSetManifest::new(
        members
            .iter()
            .map(|member| {
                ArtifactSetMember::new(
                    member.artifact_id().clone(),
                    member.byte_size(),
                    member.relative_path().clone(),
                )
            })
            .collect(),
    )
    .expect("valid runtime artifact set")
}

fn model_members() -> Vec<ModelPackageMember> {
    vec![
        ModelPackageMember::new(
            artifact("model-license"),
            20,
            path("legal/license.txt"),
            vec![ModelPackageMemberRole::LicenseText],
        ),
        ModelPackageMember::new(
            artifact("model-provenance"),
            21,
            path("legal/provenance.txt"),
            vec![ModelPackageMemberRole::ProvenanceRecord],
        ),
        ModelPackageMember::new(
            artifact("model-weights"),
            22,
            path("model/model.gguf"),
            vec![ModelPackageMemberRole::ModelWeights],
        ),
    ]
}

fn artifact_set_from_model(members: &[ModelPackageMember]) -> ArtifactSetManifest {
    ArtifactSetManifest::new(
        members
            .iter()
            .map(|member| {
                ArtifactSetMember::new(
                    member.artifact_id().clone(),
                    member.byte_size(),
                    member.relative_path().clone(),
                )
            })
            .collect(),
    )
    .expect("valid model artifact set")
}

fn embedded_components() -> Vec<EmbeddedModelComponent> {
    [
        (
            EmbeddedModelComponentPurpose::ModelConfiguration,
            "general.architecture",
        ),
        (
            EmbeddedModelComponentPurpose::GenerationConfiguration,
            "generation.defaults",
        ),
        (EmbeddedModelComponentPurpose::Tokenizer, "tokenizer.ggml"),
        (
            EmbeddedModelComponentPurpose::PromptTemplate,
            "tokenizer.chat-template",
        ),
    ]
    .into_iter()
    .map(|(purpose, selector)| {
        EmbeddedModelComponent::new(
            path("model/model.gguf"),
            purpose,
            "gguf-metadata",
            1,
            selector,
            Digest::sha256(selector.as_bytes()),
        )
        .expect("valid embedded component")
    })
    .collect()
}
