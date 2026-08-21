use rewrite_types::Digest;

use super::{
    ArtifactSetManifest, EmbeddedModelComponentPurpose, ModelPackageManifest,
    ModelPackageManifestError, ModelPackageMember, ModelPackageMemberRole, ModelWeightLayout,
    PackageTransformation, artifact, artifact_set_for, base_manifest, embedded, members, path,
    source,
};

fn dual_prompt_members() -> Vec<ModelPackageMember> {
    let mut values = members();
    values.push(ModelPackageMember::new(
        artifact("explicit-template"),
        13,
        path("prompts/template.go.tmpl"),
        vec![ModelPackageMemberRole::PromptTemplate],
    ));
    values
}

fn dual_prompt_manifest() -> (ArtifactSetManifest, ModelPackageManifest) {
    let members = dual_prompt_members();
    let artifact_set = artifact_set_for(&members);
    let package = ModelPackageManifest::new(
        &artifact_set,
        "gguf",
        1,
        source(),
        PackageTransformation::Untransformed {
            evidence_digest: Digest::sha256(b"dual prompt candidates"),
        },
        members,
        ModelWeightLayout::Single {
            member: path("model/model.gguf"),
        },
        embedded(),
    )
    .expect("one explicit and one embedded prompt candidate are valid");
    (artifact_set, package)
}

#[test]
fn dual_prompt_candidates_are_committed_and_have_stable_identity() {
    let (artifact_set, package) = dual_prompt_manifest();
    let prompt_members = package
        .members()
        .iter()
        .filter(|member| {
            member
                .roles()
                .contains(&ModelPackageMemberRole::PromptTemplate)
        })
        .collect::<Vec<_>>();
    let prompt_components = package
        .embedded_components()
        .iter()
        .filter(|component| component.purpose() == EmbeddedModelComponentPurpose::PromptTemplate)
        .collect::<Vec<_>>();
    assert_eq!(prompt_members.len(), 1);
    assert_eq!(
        prompt_members[0].relative_path().as_str(),
        "prompts/template.go.tmpl"
    );
    assert_eq!(prompt_components.len(), 1);
    assert_eq!(prompt_components[0].selector(), "tokenizer.chat_template");

    let encoded = serde_json::to_vec(&package).expect("serialize dual prompt package");
    assert_eq!(
        ModelPackageManifest::from_json_bytes(&encoded, &artifact_set)
            .expect("decode dual prompt package"),
        package
    );
    assert_eq!(
        package.model_package_manifest_id().digest().as_str(),
        "18fc17ba7f3aa6579ad820f51c218f5a148049bb64178db17bd65536e8d8fe35"
    );
}

#[test]
fn prompt_candidates_reject_missing_and_duplicate_sources() {
    let mut without_prompt = embedded();
    without_prompt
        .retain(|component| component.purpose() != EmbeddedModelComponentPurpose::PromptTemplate);
    assert_eq!(
        ModelPackageManifest::new(
            &base_manifest(),
            "gguf",
            1,
            source(),
            PackageTransformation::Untransformed {
                evidence_digest: Digest::sha256(b"missing prompt"),
            },
            members(),
            ModelWeightLayout::Single {
                member: path("model/model.gguf"),
            },
            without_prompt,
        ),
        Err(ModelPackageManifestError::MissingFoundationalComponent)
    );

    let mut duplicate_explicit = dual_prompt_members();
    duplicate_explicit.push(ModelPackageMember::new(
        artifact("second-explicit-template"),
        14,
        path("prompts/template.jinja"),
        vec![ModelPackageMemberRole::PromptTemplate],
    ));
    let duplicate_explicit_set = artifact_set_for(&duplicate_explicit);
    assert_eq!(
        ModelPackageManifest::new(
            &duplicate_explicit_set,
            "gguf",
            1,
            source(),
            PackageTransformation::Untransformed {
                evidence_digest: Digest::sha256(b"duplicate explicit prompt"),
            },
            duplicate_explicit,
            ModelWeightLayout::Single {
                member: path("model/model.gguf"),
            },
            embedded(),
        ),
        Err(ModelPackageManifestError::MissingFoundationalComponent)
    );

    let mut duplicate_embedded = embedded();
    let prompt = duplicate_embedded
        .last()
        .expect("embedded prompt candidate")
        .clone();
    duplicate_embedded.push(prompt);
    assert_eq!(
        ModelPackageManifest::new(
            &base_manifest(),
            "gguf",
            1,
            source(),
            PackageTransformation::Untransformed {
                evidence_digest: Digest::sha256(b"duplicate embedded prompt"),
            },
            members(),
            ModelWeightLayout::Single {
                member: path("model/model.gguf"),
            },
            duplicate_embedded,
        ),
        Err(ModelPackageManifestError::InvalidEmbeddedComponent)
    );
}

#[test]
fn dual_prompt_support_preserves_configuration_and_tokenizer_exclusivity() {
    for extra_role in [
        ModelPackageMemberRole::ModelConfiguration,
        ModelPackageMemberRole::TokenizerModel,
    ] {
        let mut candidate_members = dual_prompt_members();
        candidate_members[3] = ModelPackageMember::new(
            artifact("explicit-template"),
            13,
            path("prompts/template.go.tmpl"),
            vec![extra_role, ModelPackageMemberRole::PromptTemplate],
        );
        let artifact_set = artifact_set_for(&candidate_members);
        assert_eq!(
            ModelPackageManifest::new(
                &artifact_set,
                "gguf",
                1,
                source(),
                PackageTransformation::Untransformed {
                    evidence_digest: Digest::sha256(b"ambiguous foundational component"),
                },
                candidate_members,
                ModelWeightLayout::Single {
                    member: path("model/model.gguf"),
                },
                embedded(),
            ),
            Err(ModelPackageManifestError::MissingFoundationalComponent)
        );
    }
}
