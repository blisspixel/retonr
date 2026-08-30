use rewrite_model::{
    ArtifactSetManifest, EmbeddedModelComponent, EmbeddedModelComponentPurpose,
    ModelPackageManifest, ModelPackageMember, ModelPackageMemberRole, ModelWeightLayout,
    PackageSource, PackageSourceKind, PackageTransformation,
};
use rewrite_ollama_package::{
    CONFIG_MEDIA_TYPE, LICENSE_MEDIA_TYPE, MANIFEST_MEDIA_TYPE, MODEL_MEDIA_TYPE,
    PARAMS_MEDIA_TYPE, ReconstructionLimits, TEMPLATE_MEDIA_TYPE, ollama_logical_binding_digest,
    parse_manifest_v2,
};
use rewrite_runtime_attestor::MANAGED_OLLAMA_MODEL_ROOT;
use rewrite_runtime_isolation::MANAGED_RUNTIME_INPUT_ROOT_V1;
use rewrite_types::{CancellationToken, Digest};
use serde_json::json;
use tempfile::TempDir;

use crate::{ArtifactRepository, MANAGED_OLLAMA_INPUT_SCHEMA_VERSION, OllamaModelReference};

use super::{ManagedOllamaInputError, PackageAttestationService, attest_model, import_set, path};

struct Fixture {
    _directory: TempDir,
    repository: ArtifactRepository,
    set: ArtifactSetManifest,
    package: ModelPackageManifest,
    reference: OllamaModelReference,
}

#[test]
fn exact_retained_package_maps_to_one_redacted_ollama_input_plan() {
    let fixture = fixture(false, false);
    let lease = attest_model(&fixture.repository, &fixture.package);
    let source = lease
        .clone_members_for_isolation(&CancellationToken::new())
        .expect("clone retained package");
    let plan = PackageAttestationService::prepare_managed_ollama_inputs(
        source,
        &fixture.package,
        &fixture.reference,
        &ReconstructionLimits::default(),
        &CancellationToken::new(),
    )
    .expect("prepare exact Ollama input tree");

    let model = fixture
        .package
        .members()
        .iter()
        .find(|member| member.roles() == [ModelPackageMemberRole::ModelWeights])
        .expect("model member");
    let expected_target = format!(
        "{MANAGED_RUNTIME_INPUT_ROOT_V1}/blobs/sha256-{}",
        model.artifact_id().digest().as_str()
    );
    assert_eq!(MANAGED_RUNTIME_INPUT_ROOT_V1, MANAGED_OLLAMA_MODEL_ROOT);
    assert_eq!(
        plan.evidence().schema_version(),
        MANAGED_OLLAMA_INPUT_SCHEMA_VERSION
    );
    assert!(plan.evidence().installation_generation() > 0);
    assert_eq!(
        plan.evidence().artifact_set_id(),
        fixture.package.artifact_set_id()
    );
    assert_eq!(
        plan.evidence().model_package_manifest_id(),
        &fixture.package.model_package_manifest_id()
    );
    assert_eq!(plan.evidence().member_count(), 6);
    assert_eq!(
        plan.evidence().total_bytes(),
        fixture
            .package
            .members()
            .iter()
            .map(ModelPackageMember::byte_size)
            .sum::<u64>()
    );
    assert_eq!(plan.evidence().input_layout_digest().as_str().len(), 64);
    assert_eq!(plan.model_target().target_path(), expected_target);
    assert_eq!(plan.model_target().artifact_id(), model.artifact_id());
    assert_eq!(plan.evidence().model_artifact_id(), model.artifact_id());
    assert_eq!(
        plan.retained_model_weight().artifact_id(),
        model.artifact_id()
    );
    assert_eq!(plan.retained_model_weight().byte_size(), model.byte_size());
    assert_eq!(
        plan.model_target().target_digest(),
        plan.evidence().model_target_digest()
    );
    let debug = format!("{plan:?}");
    assert!(!debug.contains("registry.ollama.ai"));
    assert!(!debug.contains("release-1"));
    assert!(!debug.contains("blobs/"));
    assert!(!debug.contains("manifests/"));
}

#[test]
fn explicit_tag_is_bound_without_inference() {
    let fixture = fixture(false, false);
    let lease = attest_model(&fixture.repository, &fixture.package);
    let other_reference = OllamaModelReference::new(
        fixture.reference.registry(),
        fixture.reference.namespace(),
        fixture.reference.model(),
        "release-2",
    )
    .expect("explicit second tag");
    let first = prepare(&lease, &fixture.package, &fixture.reference);
    let second = prepare(&lease, &fixture.package, &other_reference);
    assert_ne!(
        first.evidence().reference_digest(),
        second.evidence().reference_digest()
    );
    assert_ne!(
        first.evidence().mapping_digest(),
        second.evidence().mapping_digest()
    );
}

#[test]
fn identical_cas_descriptors_share_one_target_blob() {
    let fixture = fixture(true, false);
    let lease = attest_model(&fixture.repository, &fixture.package);
    let plan = prepare(&lease, &fixture.package, &fixture.reference);
    assert_eq!(lease.evidence().member_count(), 6);
    assert_eq!(plan.evidence().member_count(), 5);
    assert!(plan.evidence().total_bytes() < lease.evidence().byte_size());
}

#[test]
fn descriptor_drift_and_reference_drift_fail_closed() {
    let drifted = fixture(false, true);
    let lease = attest_model(&drifted.repository, &drifted.package);
    let error = PackageAttestationService::prepare_managed_ollama_inputs(
        lease
            .clone_members_for_isolation(&CancellationToken::new())
            .expect("clone retained package"),
        &drifted.package,
        &drifted.reference,
        &ReconstructionLimits::default(),
        &CancellationToken::new(),
    )
    .expect_err("descriptor mismatch must fail");
    assert!(matches!(
        error,
        ManagedOllamaInputError::RelationshipMismatch
    ));

    let valid = fixture(false, false);
    let valid_lease = attest_model(&valid.repository, &valid.package);
    let wrong_reference = OllamaModelReference::new(
        "registry.ollama.ai",
        "library",
        "another-model",
        "release-1",
    )
    .expect("valid but unrelated reference");
    let error = PackageAttestationService::prepare_managed_ollama_inputs(
        valid_lease
            .clone_members_for_isolation(&CancellationToken::new())
            .expect("clone retained package"),
        &valid.package,
        &wrong_reference,
        &ReconstructionLimits::default(),
        &CancellationToken::new(),
    )
    .expect_err("reference mismatch must fail");
    assert!(matches!(
        error,
        ManagedOllamaInputError::RelationshipMismatch
    ));
}

#[test]
fn source_metadata_and_member_roles_must_match_the_exact_profile() {
    let fixture = fixture(false, false);
    let wrong_source = PackageSource::new(
        PackageSourceKind::LocalArchive,
        fixture.package.source().locator(),
        "sha256:wrong-revision",
        Digest::sha256(b"wrong provenance"),
    )
    .expect("alternate valid source metadata");
    let source_drifted =
        rebuild_package(&fixture, wrong_source, fixture.package.members().to_vec());
    let source_lease = attest_model(&fixture.repository, &source_drifted);
    let source_error = PackageAttestationService::prepare_managed_ollama_inputs(
        source_lease
            .clone_members_for_isolation(&CancellationToken::new())
            .expect("clone source-drifted package"),
        &source_drifted,
        &fixture.reference,
        &ReconstructionLimits::default(),
        &CancellationToken::new(),
    )
    .expect_err("source revision and provenance drift must fail");
    assert!(matches!(
        source_error,
        ManagedOllamaInputError::RelationshipMismatch
    ));

    let mut members = fixture.package.members().to_vec();
    let config = &members[0];
    members[0] = ModelPackageMember::new(
        config.artifact_id().clone(),
        config.byte_size(),
        config.relative_path().clone(),
        vec![ModelPackageMemberRole::LicenseText],
    );
    let role_drifted = rebuild_package(&fixture, fixture.package.source().clone(), members);
    let role_lease = attest_model(&fixture.repository, &role_drifted);
    let role_error = PackageAttestationService::prepare_managed_ollama_inputs(
        role_lease
            .clone_members_for_isolation(&CancellationToken::new())
            .expect("clone role-drifted package"),
        &role_drifted,
        &fixture.reference,
        &ReconstructionLimits::default(),
        &CancellationToken::new(),
    )
    .expect_err("member role drift must fail");
    assert!(matches!(
        role_error,
        ManagedOllamaInputError::RelationshipMismatch
    ));
}

#[test]
fn substituted_logical_binding_evidence_fails_closed() {
    let fixture = fixture(false, false);
    let substituted = ModelPackageManifest::new(
        &fixture.set,
        fixture.package.format_contract_id(),
        fixture.package.format_contract_schema_version(),
        fixture.package.source().clone(),
        PackageTransformation::Untransformed {
            evidence_digest: Digest::sha256(b"substituted logical binding"),
        },
        fixture.package.members().to_vec(),
        fixture.package.weight_layout().clone(),
        fixture.package.embedded_components().to_vec(),
    )
    .expect("substituted evidence remains structurally valid");
    let lease = attest_model(&fixture.repository, &substituted);
    let error = PackageAttestationService::prepare_managed_ollama_inputs(
        lease
            .clone_members_for_isolation(&CancellationToken::new())
            .expect("clone retained package"),
        &substituted,
        &fixture.reference,
        &ReconstructionLimits::default(),
        &CancellationToken::new(),
    )
    .expect_err("logical-binding substitution must fail");
    assert!(matches!(
        error,
        ManagedOllamaInputError::RelationshipMismatch
    ));
}

#[test]
fn cancellation_fails_before_returning_a_tree() {
    let fixture = fixture(false, false);
    let lease = attest_model(&fixture.repository, &fixture.package);
    let source = lease
        .clone_members_for_isolation(&CancellationToken::new())
        .expect("clone retained package");
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let error = PackageAttestationService::prepare_managed_ollama_inputs(
        source,
        &fixture.package,
        &fixture.reference,
        &ReconstructionLimits::default(),
        &cancellation,
    )
    .expect_err("cancelled preparation must fail");
    assert!(matches!(error, ManagedOllamaInputError::Cancelled));
}

fn prepare<'lease>(
    lease: &'lease super::ModelPackageLease,
    package: &ModelPackageManifest,
    reference: &OllamaModelReference,
) -> super::ManagedOllamaInputPlan<'lease> {
    PackageAttestationService::prepare_managed_ollama_inputs(
        lease
            .clone_members_for_isolation(&CancellationToken::new())
            .expect("clone retained package"),
        package,
        reference,
        &ReconstructionLimits::default(),
        &CancellationToken::new(),
    )
    .expect("prepare managed inputs")
}

fn fixture(shared_text_blob: bool, descriptor_drift: bool) -> Fixture {
    let directory = tempfile::tempdir().expect("temporary directory");
    let repository = ArtifactRepository::new(directory.path().join("data")).expect("repository");
    let config = br#"{"model":"fixture"}"#.to_vec();
    let parameters = br#"{"temperature":0}"#.to_vec();
    let model = b"fixture-gguf-model".to_vec();
    let template = b"{{ .Prompt }}".to_vec();
    let license = if shared_text_blob {
        template.clone()
    } else {
        b"model-license".to_vec()
    };
    let described_config = if descriptor_drift {
        b"different-config".as_slice()
    } else {
        config.as_slice()
    };
    let manifest = serde_json::to_vec(&json!({
        "schemaVersion": 2,
        "mediaType": MANIFEST_MEDIA_TYPE,
        "config": descriptor(CONFIG_MEDIA_TYPE, described_config),
        "layers": [
            descriptor(MODEL_MEDIA_TYPE, &model),
            descriptor(TEMPLATE_MEDIA_TYPE, &template),
            descriptor(LICENSE_MEDIA_TYPE, &license),
            descriptor(PARAMS_MEDIA_TYPE, &parameters),
        ]
    }))
    .expect("manifest JSON");
    let files = [
        ("config/ollama-config.json", config.as_slice()),
        ("config/parameters.json", parameters.as_slice()),
        ("legal/license.txt", license.as_slice()),
        ("model/model.gguf", model.as_slice()),
        ("prompts/template.go.tmpl", template.as_slice()),
        ("provenance/ollama-manifest-v2.json", manifest.as_slice()),
    ];
    let set = import_set(&repository, directory.path(), "ollama-input", &files);
    let reference =
        OllamaModelReference::new("registry.ollama.ai", "library", "qwen3", "release-1")
            .expect("reference");
    let manifest_digest = Digest::sha256(&manifest);
    let manifest_plan = parse_manifest_v2(&manifest, &ReconstructionLimits::default())
        .expect("fixture manifest plan");
    let logical_binding =
        ollama_logical_binding_digest(&manifest_plan).expect("fixture logical binding");
    let source = PackageSource::new(
        PackageSourceKind::LocalArchive,
        "registry.ollama.ai/library/qwen3",
        format!("sha256:{}", manifest_digest.as_str()),
        manifest_digest,
    )
    .expect("package source");
    let package = ModelPackageManifest::new(
        &set,
        "ollama-manifest-v2",
        1,
        source,
        PackageTransformation::Untransformed {
            evidence_digest: logical_binding,
        },
        model_members(&set),
        ModelWeightLayout::Single {
            member: path("model/model.gguf"),
        },
        embedded_components(),
    )
    .expect("model package");
    Fixture {
        _directory: directory,
        repository,
        set,
        package,
        reference,
    }
}

fn model_members(set: &ArtifactSetManifest) -> Vec<ModelPackageMember> {
    let roles = [
        ModelPackageMemberRole::AuxiliaryData,
        ModelPackageMemberRole::GenerationConfiguration,
        ModelPackageMemberRole::LicenseText,
        ModelPackageMemberRole::ModelWeights,
        ModelPackageMemberRole::PromptTemplate,
        ModelPackageMemberRole::ProvenanceRecord,
    ];
    set.members()
        .iter()
        .zip(roles)
        .map(|(member, role)| {
            ModelPackageMember::new(
                member.artifact_id().clone(),
                member.byte_size(),
                member.relative_path().clone(),
                vec![role],
            )
        })
        .collect()
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

fn rebuild_package(
    fixture: &Fixture,
    source: PackageSource,
    members: Vec<ModelPackageMember>,
) -> ModelPackageManifest {
    ModelPackageManifest::new(
        &fixture.set,
        fixture.package.format_contract_id(),
        fixture.package.format_contract_schema_version(),
        source,
        fixture.package.transformation().clone(),
        members,
        fixture.package.weight_layout().clone(),
        fixture.package.embedded_components().to_vec(),
    )
    .expect("mutated model package remains structurally valid")
}

fn descriptor(media_type: &str, bytes: &[u8]) -> serde_json::Value {
    json!({
        "mediaType": media_type,
        "digest": format!("sha256:{}", Digest::sha256(bytes).as_str()),
        "size": bytes.len(),
    })
}
