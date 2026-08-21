use rewrite_types::Digest;

use crate::{ArtifactSetManifest, ArtifactSetMember, PackageTransformation};

use super::{
    EmbeddedModelComponent, EmbeddedModelComponentPurpose, ModelPackageManifest,
    ModelPackageManifestError, ModelPackageMember, ModelPackageMemberRole, ModelWeightLayout,
    artifact, artifact_set_for, base_manifest, embedded, members, path, source,
};

pub(super) fn sharded_members() -> Vec<ModelPackageMember> {
    vec![
        super::model_member(
            "configuration",
            20,
            "config/model.json",
            vec![
                ModelPackageMemberRole::ModelConfiguration,
                ModelPackageMemberRole::GenerationConfiguration,
                ModelPackageMemberRole::TokenizerConfiguration,
                ModelPackageMemberRole::SystemPrompt,
                ModelPackageMemberRole::Adapter,
                ModelPackageMemberRole::Projector,
                ModelPackageMemberRole::DraftModel,
                ModelPackageMemberRole::GrammarOrSchema,
            ],
        ),
        super::model_member(
            "license",
            21,
            "legal/license.txt",
            vec![ModelPackageMemberRole::LicenseText],
        ),
        super::model_member(
            "provenance",
            22,
            "legal/provenance.txt",
            vec![ModelPackageMemberRole::ProvenanceRecord],
        ),
        super::model_member(
            "transformation",
            23,
            "legal/transformation.json",
            vec![ModelPackageMemberRole::TransformationEvidence],
        ),
        super::model_member(
            "index",
            24,
            "model/index.json",
            vec![ModelPackageMemberRole::ModelShardIndex],
        ),
        super::model_member(
            "shard-1",
            25,
            "model/shard-1.safetensors",
            vec![ModelPackageMemberRole::ModelWeightShard],
        ),
        super::model_member(
            "shard-2",
            26,
            "model/shard-2.safetensors",
            vec![ModelPackageMemberRole::ModelWeightShard],
        ),
        super::model_member(
            "template",
            27,
            "prompts/chat.jinja",
            vec![ModelPackageMemberRole::PromptTemplate],
        ),
        super::model_member(
            "extensions",
            28,
            "runtime/extensions.py",
            vec![
                ModelPackageMemberRole::CustomModelCode,
                ModelPackageMemberRole::CustomGenerationCode,
                ModelPackageMemberRole::AuxiliaryData,
            ],
        ),
        super::model_member(
            "merges",
            29,
            "tokenizer/merges.txt",
            vec![ModelPackageMemberRole::TokenizerMerges],
        ),
        super::model_member(
            "tokenizer",
            30,
            "tokenizer/model.bin",
            vec![ModelPackageMemberRole::TokenizerModel],
        ),
        super::model_member(
            "vocabulary",
            31,
            "tokenizer/vocab.json",
            vec![ModelPackageMemberRole::TokenizerVocabulary],
        ),
    ]
}

#[test]
fn exact_member_identity_empty_roles_and_role_limit_are_enforced() {
    let base = base_manifest();
    let construct = |members| {
        ModelPackageManifest::new(
            &base,
            "gguf",
            1,
            source(),
            PackageTransformation::Untransformed {
                evidence_digest: Digest::sha256(b"same"),
            },
            members,
            ModelWeightLayout::Single {
                member: path("model/model.gguf"),
            },
            embedded(),
        )
    };
    let mut mismatch = members();
    mismatch[2] = ModelPackageMember::new(
        artifact("wrong"),
        12,
        path("model/model.gguf"),
        vec![ModelPackageMemberRole::ModelWeights],
    );
    assert_eq!(
        construct(mismatch),
        Err(ModelPackageManifestError::MemberCoverageMismatch)
    );
    let mut empty = members();
    empty[2] = ModelPackageMember::new(
        artifact("weights"),
        12,
        path("model/model.gguf"),
        Vec::new(),
    );
    assert_eq!(
        construct(empty),
        Err(ModelPackageManifestError::InvalidMemberRoles)
    );
    let mut excessive = members();
    excessive[2] = ModelPackageMember::new(
        artifact("weights"),
        12,
        path("model/model.gguf"),
        vec![ModelPackageMemberRole::ModelWeights; 9],
    );
    assert_eq!(
        construct(excessive),
        Err(ModelPackageManifestError::InvalidMemberRoles)
    );
}

#[test]
fn embedded_count_container_and_duplicate_purpose_are_enforced() {
    let base = base_manifest();
    let construct = |components| {
        ModelPackageManifest::new(
            &base,
            "gguf",
            1,
            source(),
            PackageTransformation::Untransformed {
                evidence_digest: Digest::sha256(b"same"),
            },
            members(),
            ModelWeightLayout::Single {
                member: path("model/model.gguf"),
            },
            components,
        )
    };
    let component = embedded()[0].clone();
    assert_eq!(
        construct(vec![component; 65]),
        Err(ModelPackageManifestError::InvalidEmbeddedComponent)
    );
    let mut absent_container = embedded();
    absent_container.remove(1);
    absent_container.insert(
        1,
        EmbeddedModelComponent::new(
            path("model/missing.gguf"),
            EmbeddedModelComponentPurpose::GenerationConfiguration,
            "gguf-metadata",
            1,
            "generation",
            Digest::sha256(b"generation"),
        )
        .expect("structurally valid component"),
    );
    assert_eq!(
        construct(absent_container),
        Err(ModelPackageManifestError::InvalidEmbeddedComponent)
    );
    let mut duplicate = embedded();
    duplicate.insert(1, duplicate[0].clone());
    assert_eq!(
        construct(duplicate),
        Err(ModelPackageManifestError::InvalidEmbeddedComponent)
    );
}

#[test]
fn aggregate_role_assignments_are_bounded_before_foundational_interpretation() {
    let roles = vec![
        ModelPackageMemberRole::ModelWeights,
        ModelPackageMemberRole::ModelWeightShard,
        ModelPackageMemberRole::ModelShardIndex,
        ModelPackageMemberRole::ModelConfiguration,
        ModelPackageMemberRole::GenerationConfiguration,
        ModelPackageMemberRole::TokenizerModel,
        ModelPackageMemberRole::TokenizerVocabulary,
        ModelPackageMemberRole::TokenizerMerges,
    ];
    let members = (0..1_025)
        .map(|index| {
            ModelPackageMember::new(
                artifact(&format!("member-{index:04}")),
                1,
                path(&format!("member/{index:04}")),
                roles.clone(),
            )
        })
        .collect::<Vec<_>>();
    let artifact_set = artifact_set_for(&members);
    assert_eq!(
        ModelPackageManifest::new(
            &artifact_set,
            "fixture",
            1,
            source(),
            PackageTransformation::Untransformed {
                evidence_digest: Digest::sha256(b"same"),
            },
            members,
            ModelWeightLayout::Single {
                member: path("member/0000"),
            },
            Vec::new(),
        ),
        Err(ModelPackageManifestError::TooManyRoleAssignments)
    );
}

#[test]
fn sharded_decoder_rejects_invalid_shard_and_index_paths() {
    let members = vec![
        ModelPackageMember::new(
            artifact("license"),
            1,
            path("legal/license"),
            vec![ModelPackageMemberRole::LicenseText],
        ),
        ModelPackageMember::new(
            artifact("provenance"),
            1,
            path("legal/provenance"),
            vec![ModelPackageMemberRole::ProvenanceRecord],
        ),
        ModelPackageMember::new(
            artifact("weights"),
            1,
            path("model/weights"),
            vec![ModelPackageMemberRole::ModelWeights],
        ),
    ];
    let artifact_set = artifact_set_for(&members);
    let package = ModelPackageManifest::new(
        &artifact_set,
        "fixture",
        1,
        source(),
        PackageTransformation::Untransformed {
            evidence_digest: Digest::sha256(b"same"),
        },
        members,
        ModelWeightLayout::Single {
            member: path("model/weights"),
        },
        embedded(),
    );
    assert_eq!(
        package,
        Err(ModelPackageManifestError::InvalidEmbeddedComponent)
    );

    let valid = super::manifest();
    let mut value = serde_json::to_value(valid).expect("value");
    value["weight_layout"] = serde_json::json!({
        "kind": "sharded",
        "shards": ["../shard"],
        "index": "model/index"
    });
    assert_eq!(
        ModelPackageManifest::from_json_bytes(
            &serde_json::to_vec(&value).expect("encode"),
            &base_manifest()
        ),
        Err(ModelPackageManifestError::InvalidMemberPath)
    );
}

#[test]
fn artifact_set_helper_preserves_exact_declared_order() {
    let declared = members();
    let set: ArtifactSetManifest = artifact_set_for(&declared);
    let rebuilt = ArtifactSetManifest::new(
        declared
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
    .expect("rebuilt set");
    assert_eq!(set, rebuilt);
}
