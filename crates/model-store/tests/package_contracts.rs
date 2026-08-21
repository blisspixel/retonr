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
use rewrite_model_store::{ArtifactStateStore, StoreError, WriteDisposition};
use rewrite_types::Digest;
use tempfile::tempdir;

#[test]
fn public_package_contract_api_round_trips_and_rejects_missing_dependencies() {
    let fixture = fixture();
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("contracts.db");
    let mut store = ArtifactStateStore::open(&path).expect("open store");
    assert!(matches!(
        store.put_runtime_package_manifest(&fixture.runtime_package),
        Err(StoreError::MissingRecord)
    ));
    assert!(matches!(
        store.put_model_package_manifest(&fixture.model_package),
        Err(StoreError::MissingRecord)
    ));
    assert!(matches!(
        store.put_native_load_observation(&fixture.native_load),
        Err(StoreError::MissingRecord)
    ));

    store
        .put_artifact_set_manifest(&fixture.runtime_set)
        .expect("store runtime byte set");
    store
        .put_artifact_set_manifest(&fixture.model_set)
        .expect("store model byte set");
    assert_eq!(
        store
            .put_runtime_package_manifest(&fixture.runtime_package)
            .expect("store runtime package"),
        WriteDisposition::Inserted
    );
    assert_eq!(
        store
            .put_model_package_manifest(&fixture.model_package)
            .expect("store model package"),
        WriteDisposition::Inserted
    );
    assert_eq!(
        store
            .put_native_load_observation(&fixture.native_load)
            .expect("store native load"),
        WriteDisposition::Inserted
    );
    assert_eq!(
        store
            .put_runtime_package_manifest(&fixture.runtime_package)
            .expect("repeat runtime package"),
        WriteDisposition::AlreadyPresent
    );
    assert_eq!(
        store
            .put_model_package_manifest(&fixture.model_package)
            .expect("repeat model package"),
        WriteDisposition::AlreadyPresent
    );
    assert_eq!(
        store
            .put_native_load_observation(&fixture.native_load)
            .expect("repeat native load"),
        WriteDisposition::AlreadyPresent
    );
    assert_eq!(
        store
            .runtime_package_manifest(&fixture.runtime_package.runtime_package_manifest_id())
            .expect("load runtime package"),
        Some(fixture.runtime_package.clone())
    );
    assert_eq!(
        store
            .model_package_manifest(&fixture.model_package.model_package_manifest_id())
            .expect("load model package"),
        Some(fixture.model_package.clone())
    );
    assert_eq!(
        store
            .native_load_observation(&fixture.native_load.native_load_observation_id())
            .expect("load native observation"),
        Some(fixture.native_load.clone())
    );
}

struct Fixture {
    runtime_set: ArtifactSetManifest,
    runtime_package: RuntimePackageManifest,
    model_set: ArtifactSetManifest,
    model_package: ModelPackageManifest,
    native_load: NativeLoadObservation,
}

fn fixture() -> Fixture {
    let (runtime_set, runtime_package) = runtime_fixture();
    let (model_set, model_package) = model_fixture();
    let native_load = NativeLoadObservation::new(
        &runtime_package,
        NativeLoadObservationInput {
            evidence_class: NativeLoadEvidenceClass::LinuxProcMapFiles,
            visibility_scope: NativeLoadVisibilityScope::FileBackedExecutableMappings,
            process_evidence_digest: Digest::sha256(b"process"),
            observation_contract_id: "linux-proc-map-files".to_owned(),
            observation_contract_schema_version: 1,
            components: vec![NativeLoadedComponent::new(
                artifact("runtime"),
                10,
                NativeLoadOrigin::PackagedMember {
                    relative_path: path("bin/runtime"),
                },
                NativeMappingClass::ExecutableImage,
                Digest::sha256(b"mapping"),
            )],
        },
    )
    .expect("native observation");
    Fixture {
        runtime_set,
        runtime_package,
        model_set,
        model_package,
        native_load,
    }
}

fn runtime_fixture() -> (ArtifactSetManifest, RuntimePackageManifest) {
    let runtime_members = vec![
        RuntimePackageMember::new(
            artifact("runtime"),
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
    ];
    let runtime_set = ArtifactSetManifest::new(
        runtime_members
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
    .expect("runtime artifact set");
    let package = RuntimePackageManifest::new(
        &runtime_set,
        "integration-runtime",
        "1.0.0",
        None,
        RuntimeTarget::new(
            RuntimeOperatingSystem::Linux,
            RuntimeArchitecture::X86_64,
            RuntimeAbi::LinuxGnuLibc,
        )
        .expect("runtime target"),
        source("runtime"),
        transformation("runtime"),
        runtime_members,
    )
    .expect("runtime package");
    (runtime_set, package)
}

fn model_fixture() -> (ArtifactSetManifest, ModelPackageManifest) {
    let model_members = vec![
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
            artifact("model"),
            22,
            path("model/model.gguf"),
            vec![ModelPackageMemberRole::ModelWeights],
        ),
    ];
    let model_set = ArtifactSetManifest::new(
        model_members
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
    .expect("model artifact set");
    let package = ModelPackageManifest::new(
        &model_set,
        "gguf",
        1,
        source("model"),
        transformation("model"),
        model_members,
        ModelWeightLayout::Single {
            member: path("model/model.gguf"),
        },
        embedded_components(),
    )
    .expect("model package");
    (model_set, package)
}

fn embedded_components() -> Vec<EmbeddedModelComponent> {
    [
        EmbeddedModelComponentPurpose::ModelConfiguration,
        EmbeddedModelComponentPurpose::GenerationConfiguration,
        EmbeddedModelComponentPurpose::Tokenizer,
        EmbeddedModelComponentPurpose::PromptTemplate,
    ]
    .into_iter()
    .enumerate()
    .map(|(index, purpose)| {
        EmbeddedModelComponent::new(
            path("model/model.gguf"),
            purpose,
            "gguf-metadata",
            1,
            format!("selector-{index}"),
            Digest::sha256(format!("component-{index}").as_bytes()),
        )
        .expect("embedded component")
    })
    .collect()
}

fn source(label: &str) -> PackageSource {
    PackageSource::new(
        PackageSourceKind::LocalArchive,
        format!("local-{label}"),
        "sha256-fixture",
        Digest::sha256(format!("source-{label}").as_bytes()),
    )
    .expect("package source")
}

fn transformation(label: &str) -> PackageTransformation {
    PackageTransformation::Untransformed {
        evidence_digest: Digest::sha256(format!("comparison-{label}").as_bytes()),
    }
}

fn artifact(label: &str) -> ArtifactId {
    ArtifactId::from_digest(Digest::sha256(label.as_bytes()))
}

fn path(value: &str) -> ArtifactSetRelativePath {
    ArtifactSetRelativePath::new(value).expect("artifact-set path")
}
