use rewrite_types::Digest;

use crate::{
    ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath, PackageSource,
    PackageSourceKind, PackageTransformation,
};

use super::{
    EmbeddedModelComponent, EmbeddedModelComponentPurpose, MAX_MODEL_PACKAGE_MANIFEST_JSON_BYTES,
    ModelPackageManifest, ModelPackageManifestError, ModelPackageMember, ModelPackageMemberRole,
    ModelWeightLayout,
};

#[path = "tests/adversarial.rs"]
mod adversarial;
#[path = "tests/prompt_candidates.rs"]
mod prompt_candidates;

fn path(value: &str) -> ArtifactSetRelativePath {
    ArtifactSetRelativePath::new(value).expect("valid path")
}

fn artifact(value: &str) -> ArtifactId {
    ArtifactId::from_digest(Digest::sha256(value.as_bytes()))
}

fn base_manifest() -> ArtifactSetManifest {
    ArtifactSetManifest::new(vec![
        ArtifactSetMember::new(artifact("license"), 10, path("legal/license.txt")),
        ArtifactSetMember::new(artifact("provenance"), 11, path("legal/provenance.txt")),
        ArtifactSetMember::new(artifact("weights"), 12, path("model/model.gguf")),
    ])
    .expect("valid artifact set")
}

fn source() -> PackageSource {
    PackageSource::new(
        PackageSourceKind::LocalArchive,
        "local-model-archive",
        "sha256-fixture",
        Digest::sha256(b"model provenance"),
    )
    .expect("valid source")
}

fn members() -> Vec<ModelPackageMember> {
    vec![
        ModelPackageMember::new(
            artifact("license"),
            10,
            path("legal/license.txt"),
            vec![ModelPackageMemberRole::LicenseText],
        ),
        ModelPackageMember::new(
            artifact("provenance"),
            11,
            path("legal/provenance.txt"),
            vec![ModelPackageMemberRole::ProvenanceRecord],
        ),
        ModelPackageMember::new(
            artifact("weights"),
            12,
            path("model/model.gguf"),
            vec![ModelPackageMemberRole::ModelWeights],
        ),
    ]
}

fn embedded() -> Vec<EmbeddedModelComponent> {
    [
        (
            EmbeddedModelComponentPurpose::ModelConfiguration,
            "general.architecture",
            b"config".as_slice(),
        ),
        (
            EmbeddedModelComponentPurpose::GenerationConfiguration,
            "generation.defaults",
            b"generation".as_slice(),
        ),
        (
            EmbeddedModelComponentPurpose::Tokenizer,
            "tokenizer.ggml",
            b"tokenizer".as_slice(),
        ),
        (
            EmbeddedModelComponentPurpose::PromptTemplate,
            "tokenizer.chat_template",
            b"template".as_slice(),
        ),
    ]
    .into_iter()
    .map(|(purpose, selector, bytes)| {
        EmbeddedModelComponent::new(
            path("model/model.gguf"),
            purpose,
            "gguf-metadata",
            1,
            selector,
            Digest::sha256(bytes),
        )
        .expect("valid embedded component")
    })
    .collect()
}

fn manifest() -> ModelPackageManifest {
    ModelPackageManifest::new(
        &base_manifest(),
        "gguf",
        1,
        source(),
        PackageTransformation::Untransformed {
            evidence_digest: Digest::sha256(b"untransformed"),
        },
        members(),
        ModelWeightLayout::Single {
            member: path("model/model.gguf"),
        },
        embedded(),
    )
    .expect("valid model package")
}

#[test]
fn model_package_has_stable_identity_and_complete_accessors() {
    let package = manifest();
    assert_eq!(package.schema_version(), 1);
    assert_eq!(package.format_contract_id(), "gguf");
    assert_eq!(package.format_contract_schema_version(), 1);
    assert_eq!(package.source(), &source());
    assert_eq!(package.members().len(), 3);
    assert_eq!(package.embedded_components().len(), 4);
    assert!(matches!(
        package.weight_layout(),
        ModelWeightLayout::Single { member } if member.as_str() == "model/model.gguf"
    ));
    let component = &package.embedded_components()[0];
    assert_eq!(component.container_path().as_str(), "model/model.gguf");
    assert_eq!(
        component.purpose(),
        EmbeddedModelComponentPurpose::ModelConfiguration
    );
    assert_eq!(component.extraction_contract_id(), "gguf-metadata");
    assert_eq!(component.extraction_contract_schema_version(), 1);
    assert_eq!(component.selector(), "general.architecture");
    assert_eq!(component.value_digest(), &Digest::sha256(b"config"));
    assert_eq!(
        package.model_package_manifest_id().digest().as_str(),
        "6deb02259d96e44d0bebafb3a35468b7ff4b4cb998598dc97f2e5a7a94485c6f"
    );
}

#[test]
fn model_package_round_trips_and_rejects_encoding_boundaries() {
    let base = base_manifest();
    let package = manifest();
    let encoded = serde_json::to_vec(&package).expect("serialize");
    assert_eq!(
        ModelPackageManifest::from_json_bytes(&encoded, &base).expect("decode"),
        package
    );
    let mut value = serde_json::to_value(&package).expect("value");
    value["schema_version"] = serde_json::json!(2);
    assert_eq!(
        ModelPackageManifest::from_json_bytes(&serde_json::to_vec(&value).expect("encode"), &base),
        Err(ModelPackageManifestError::UnsupportedSchema(2))
    );
    value["schema_version"] = serde_json::json!(1);
    value["extra"] = serde_json::json!(true);
    assert_eq!(
        ModelPackageManifest::from_json_bytes(&serde_json::to_vec(&value).expect("encode"), &base),
        Err(ModelPackageManifestError::InvalidEncoding)
    );
    assert_eq!(
        ModelPackageManifest::from_json_bytes(
            &vec![b' '; MAX_MODEL_PACKAGE_MANIFEST_JSON_BYTES + 1],
            &base
        ),
        Err(ModelPackageManifestError::EncodedManifestTooLarge)
    );
}

#[test]
fn model_package_rejects_member_and_layout_drift() {
    let base = base_manifest();
    let construct = |members, layout, embedded| {
        ModelPackageManifest::new(
            &base,
            "gguf",
            1,
            source(),
            PackageTransformation::Untransformed {
                evidence_digest: Digest::sha256(b"same"),
            },
            members,
            layout,
            embedded,
        )
    };
    let layout = || ModelWeightLayout::Single {
        member: path("model/model.gguf"),
    };

    let mut invalid = members();
    invalid.pop();
    assert_eq!(
        construct(invalid, layout(), embedded()),
        Err(ModelPackageManifestError::MemberCoverageMismatch)
    );
    let mut invalid = members();
    invalid[2] = ModelPackageMember::new(
        artifact("weights"),
        12,
        path("model/model.gguf"),
        vec![
            ModelPackageMemberRole::ModelWeights,
            ModelPackageMemberRole::LicenseText,
        ],
    );
    assert_eq!(
        construct(invalid, layout(), embedded()),
        Err(ModelPackageManifestError::InvalidMemberRoles)
    );
    assert_eq!(
        construct(
            members(),
            ModelWeightLayout::Sharded {
                shards: vec![path("model/model.gguf")],
                index: path("model/model.gguf"),
            },
            embedded()
        ),
        Err(ModelPackageManifestError::InvalidWeightLayout)
    );
}

#[test]
fn model_package_rejects_incomplete_or_ambiguous_foundational_evidence() {
    let base = base_manifest();
    let construct = |members, transformation, embedded| {
        ModelPackageManifest::new(
            &base,
            "gguf",
            1,
            source(),
            transformation,
            members,
            ModelWeightLayout::Single {
                member: path("model/model.gguf"),
            },
            embedded,
        )
    };
    let untransformed = || PackageTransformation::Untransformed {
        evidence_digest: Digest::sha256(b"same"),
    };

    let mut invalid_embedded = embedded();
    invalid_embedded.swap(0, 1);
    assert_eq!(
        construct(members(), untransformed(), invalid_embedded),
        Err(ModelPackageManifestError::InvalidEmbeddedComponent)
    );
    let mut incomplete = embedded();
    incomplete.pop();
    assert_eq!(
        construct(members(), untransformed(), incomplete),
        Err(ModelPackageManifestError::MissingFoundationalComponent)
    );
    let mut no_license = members();
    no_license[0] = ModelPackageMember::new(
        artifact("license"),
        10,
        path("legal/license.txt"),
        vec![ModelPackageMemberRole::AuxiliaryData],
    );
    assert_eq!(
        construct(no_license, untransformed(), embedded()),
        Err(ModelPackageManifestError::MissingEvidence)
    );
    assert_eq!(
        construct(
            members(),
            PackageTransformation::Transformed {
                source_artifact_set_id: base.artifact_set_id(),
                tool_evidence_digest: Digest::sha256(b"tool"),
                parameters_digest: Digest::sha256(b"params"),
                log_digest: Digest::sha256(b"log"),
            },
            embedded()
        ),
        Err(ModelPackageManifestError::MissingTransformationEvidence)
    );
}

#[test]
fn embedded_component_and_format_contract_validation_is_strict() {
    assert_eq!(
        EmbeddedModelComponent::new(
            path("model/model.gguf"),
            EmbeddedModelComponentPurpose::Tokenizer,
            "Bad Contract",
            0,
            "",
            Digest::sha256(b"value"),
        ),
        Err(ModelPackageManifestError::InvalidEmbeddedComponent)
    );
    assert_eq!(
        ModelPackageManifest::new(
            &base_manifest(),
            "Bad_Format",
            0,
            source(),
            PackageTransformation::Untransformed {
                evidence_digest: Digest::sha256(b"same"),
            },
            members(),
            ModelWeightLayout::Single {
                member: path("model/model.gguf"),
            },
            embedded(),
        ),
        Err(ModelPackageManifestError::InvalidFormatContract)
    );
    assert!(ModelPackageMemberRole::LicenseText.is_evidence_only());
    assert!(!ModelPackageMemberRole::ModelWeights.is_evidence_only());
}

#[test]
fn transformed_sharded_package_round_trips_every_declared_role() {
    let members = adversarial::sharded_members();
    let artifact_set = artifact_set_for(&members);
    let package = ModelPackageManifest::new(
        &artifact_set,
        "safetensors",
        2,
        source(),
        PackageTransformation::Transformed {
            source_artifact_set_id: base_manifest().artifact_set_id(),
            tool_evidence_digest: Digest::sha256(b"tool"),
            parameters_digest: Digest::sha256(b"parameters"),
            log_digest: Digest::sha256(b"log"),
        },
        members,
        ModelWeightLayout::Sharded {
            shards: vec![
                path("model/shard-1.safetensors"),
                path("model/shard-2.safetensors"),
            ],
            index: path("model/index.json"),
        },
        Vec::new(),
    )
    .expect("valid sharded package");
    assert_eq!(package.artifact_set_id(), &artifact_set.artifact_set_id());
    assert!(matches!(
        package.transformation(),
        PackageTransformation::Transformed { .. }
    ));
    let encoded = serde_json::to_vec(&package).expect("serialize");
    assert_eq!(
        ModelPackageManifest::from_json_bytes(&encoded, &artifact_set).expect("decode"),
        package
    );
    assert_eq!(
        package.model_package_manifest_id().digest().as_str(),
        "d9178c50338985b472054070b0228ffbc2ae37c297824f6a6a043191dcabcd9d"
    );
}

#[test]
fn model_decoder_rejects_nested_path_source_and_reference_drift() {
    let base = base_manifest();
    let package = manifest();
    let value = serde_json::to_value(&package).expect("value");
    let mut invalid = value.clone();
    invalid["members"][0]["relative_path"] = serde_json::json!("../license");
    assert_eq!(
        decode_value(&invalid, &base),
        Err(ModelPackageManifestError::InvalidMemberPath)
    );
    let mut invalid = value.clone();
    invalid["embedded_components"][0]["container_path"] = serde_json::json!("../model");
    assert_eq!(
        decode_value(&invalid, &base),
        Err(ModelPackageManifestError::InvalidMemberPath)
    );
    let mut invalid = value.clone();
    invalid["source"]["locator"] = serde_json::json!("https://user@example.invalid/model");
    assert_eq!(
        decode_value(&invalid, &base),
        Err(ModelPackageManifestError::InvalidSource)
    );
    let other = ArtifactSetManifest::new(vec![ArtifactSetMember::new(
        artifact("other"),
        1,
        path("other"),
    )])
    .expect("other set");
    assert_eq!(
        ModelPackageManifest::from_json_bytes(&serde_json::to_vec(&value).expect("encode"), &other),
        Err(ModelPackageManifestError::ArtifactSetMismatch)
    );
    assert_eq!(
        package.validate_against(&other),
        Err(ModelPackageManifestError::ArtifactSetMismatch)
    );
}

fn model_member(
    name: &str,
    byte_size: u64,
    relative_path: &str,
    roles: Vec<ModelPackageMemberRole>,
) -> ModelPackageMember {
    ModelPackageMember::new(artifact(name), byte_size, path(relative_path), roles)
}

fn artifact_set_for(members: &[ModelPackageMember]) -> ArtifactSetManifest {
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
    .expect("valid artifact set")
}

fn decode_value(
    value: &serde_json::Value,
    artifact_set: &ArtifactSetManifest,
) -> Result<ModelPackageManifest, ModelPackageManifestError> {
    ModelPackageManifest::from_json_bytes(&serde_json::to_vec(value).expect("encode"), artifact_set)
}
