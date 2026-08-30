use rewrite_model::{
    ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath,
    EffectivePackageEvidenceRoleV2, EffectivePackageMemberPurpose, EffectivePackageMemberUseV2,
    EmbeddedModelComponent, EmbeddedModelComponentPurpose, ModelPackageManifest,
    ModelPackageMember, ModelPackageMemberRole, ModelWeightLayout, PackageSource,
    PackageSourceKind, PackageTransformation, RuntimeAbi, RuntimeArchitecture,
    RuntimeOperatingSystem, RuntimePackageLoadPolicy, RuntimePackageManifest, RuntimePackageMember,
    RuntimePackageMemberRole, RuntimeTarget,
};
use rewrite_runtime_attestor::ExpectedExternalNativeComponent;
use rewrite_types::Digest;

use super::{
    derive_member_evidence, isolation_exclusion_digest_from_portable_facts, purpose_for_role,
    relationship_checks_pass, runtime_closure_digest_from_portable_facts, transformation,
};

#[path = "tests/release.rs"]
mod release;

fn path(value: &str) -> ArtifactSetRelativePath {
    ArtifactSetRelativePath::new(value).expect("valid fixture path")
}

fn artifact(bytes: &[u8]) -> ArtifactId {
    ArtifactId::from_digest(Digest::sha256(bytes))
}

fn member(path_value: &str, bytes: &[u8]) -> ArtifactSetMember {
    ArtifactSetMember::new(
        artifact(bytes),
        u64::try_from(bytes.len()).expect("fixture byte length"),
        path(path_value),
    )
}

fn runtime_fixture(license_bytes: &[u8]) -> RuntimePackageManifest {
    let specifications = [
        (
            "bin/ollama",
            b"entrypoint".as_slice(),
            vec![RuntimePackageMemberRole::Entrypoint],
            RuntimePackageLoadPolicy::RequiredAtReady,
        ),
        (
            "legal/license.txt",
            license_bytes,
            vec![RuntimePackageMemberRole::LicenseText],
            RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
        ),
        (
            "lib/ollama/worker",
            b"worker".as_slice(),
            vec![RuntimePackageMemberRole::WorkerExecutable],
            RuntimePackageLoadPolicy::BackendConditional,
        ),
        (
            "provenance/source.json",
            b"provenance".as_slice(),
            vec![RuntimePackageMemberRole::ProvenanceRecord],
            RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
        ),
    ];
    let artifact_set = ArtifactSetManifest::new(
        specifications
            .iter()
            .map(|(path_value, bytes, ..)| member(path_value, bytes))
            .collect(),
    )
    .expect("runtime artifact set");
    let runtime_members = specifications
        .iter()
        .zip(artifact_set.members())
        .map(|((_, _, roles, policy), content)| {
            RuntimePackageMember::new(
                content.artifact_id().clone(),
                content.byte_size(),
                content.relative_path().clone(),
                roles.clone(),
                *policy,
            )
        })
        .collect();
    RuntimePackageManifest::new(
        &artifact_set,
        "ollama",
        "0.32.15",
        Some("reviewed-revision".to_owned()),
        RuntimeTarget::new(
            RuntimeOperatingSystem::Linux,
            RuntimeArchitecture::X86_64,
            RuntimeAbi::LinuxGnuLibc,
        )
        .expect("runtime target"),
        PackageSource::new(
            PackageSourceKind::RepositoryRevision,
            "https://example.invalid/ollama",
            "reviewed-revision",
            Digest::sha256(b"runtime provenance"),
        )
        .expect("runtime source"),
        PackageTransformation::Untransformed {
            evidence_digest: Digest::sha256(b"runtime logical binding"),
        },
        runtime_members,
    )
    .expect("runtime package")
}

#[derive(Clone, Copy)]
enum SharedLicenseBytes {
    None,
    Template,
    Model,
}

fn fixture(shared_license: SharedLicenseBytes) -> (ArtifactSetManifest, ModelPackageManifest) {
    let template = b"template".as_slice();
    let model = b"weights and embedded metadata".as_slice();
    let license = match shared_license {
        SharedLicenseBytes::None => b"license".as_slice(),
        SharedLicenseBytes::Template => template,
        SharedLicenseBytes::Model => model,
    };
    let set = ArtifactSetManifest::new(vec![
        member("config/ollama-config.json", b"config"),
        member("config/parameters.json", b"parameters"),
        member("legal/license.txt", license),
        member("model/model.gguf", model),
        member("prompts/template.go.tmpl", template),
        member("provenance/ollama-manifest-v2.json", b"manifest"),
    ])
    .expect("canonical model set");
    let roles = [
        ModelPackageMemberRole::AuxiliaryData,
        ModelPackageMemberRole::GenerationConfiguration,
        ModelPackageMemberRole::LicenseText,
        ModelPackageMemberRole::ModelWeights,
        ModelPackageMemberRole::PromptTemplate,
        ModelPackageMemberRole::ProvenanceRecord,
    ];
    let semantic = set
        .members()
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
        .collect();
    let embedded = [
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
            format!("selector.{index}"),
            Digest::sha256(format!("embedded {index}").as_bytes()),
        )
        .expect("embedded fixture component")
    })
    .collect();
    let source = PackageSource::new(
        PackageSourceKind::LocalArchive,
        "registry.ollama.ai/library/fixture",
        "sha256:fixture",
        Digest::sha256(b"manifest"),
    )
    .expect("fixture source");
    let package = ModelPackageManifest::new(
        &set,
        "ollama-manifest-v2",
        1,
        source,
        PackageTransformation::Untransformed {
            evidence_digest: Digest::sha256(b"logical binding"),
        },
        semantic,
        ModelWeightLayout::Single {
            member: path("model/model.gguf"),
        },
        embedded,
    )
    .expect("fixture package");
    (set, package)
}

#[test]
fn exact_ollama_members_derive_runtime_and_evidence_uses() {
    let (_set, package) = fixture(SharedLicenseBytes::None);
    let members = derive_member_evidence(&package, &path("provenance/ollama-manifest-v2.json"))
        .expect("derive exact model member evidence");
    assert_eq!(members.len(), 6);
    assert!(matches!(
        members[0].member_use(),
        EffectivePackageMemberUseV2::Effective { purposes }
            if purposes == &[EffectivePackageMemberPurpose::AuxiliaryData]
    ));
    assert!(matches!(
        members[1].member_use(),
        EffectivePackageMemberUseV2::Effective { purposes }
            if purposes == &[EffectivePackageMemberPurpose::GenerationConfiguration]
    ));
    assert!(matches!(
        members[2].member_use(),
        EffectivePackageMemberUseV2::EvidenceOnly { roles }
            if roles == &[EffectivePackageEvidenceRoleV2::LicenseText]
    ));
    assert!(matches!(
        members[3].member_use(),
        EffectivePackageMemberUseV2::Effective { purposes }
            if purposes == &[
                EffectivePackageMemberPurpose::ModelWeights,
                EffectivePackageMemberPurpose::ModelConfiguration,
                EffectivePackageMemberPurpose::GenerationConfiguration,
                EffectivePackageMemberPurpose::TokenizerModel,
                EffectivePackageMemberPurpose::PromptTemplate,
            ]
    ));
    assert!(matches!(
        members[4].member_use(),
        EffectivePackageMemberUseV2::Effective { purposes }
            if purposes == &[EffectivePackageMemberPurpose::PromptTemplate]
    ));
    assert!(matches!(
        members[5].member_use(),
        EffectivePackageMemberUseV2::EffectiveAndEvidence { purposes, roles }
            if purposes == &[EffectivePackageMemberPurpose::AuxiliaryData]
                && roles == &[EffectivePackageEvidenceRoleV2::ProvenanceRecord]
    ));
}

#[test]
fn shared_license_bytes_receive_the_complete_effective_purpose_union() {
    let (set, package) = fixture(SharedLicenseBytes::Template);
    assert_eq!(
        set.members()[2].artifact_id(),
        set.members()[4].artifact_id()
    );
    let members = derive_member_evidence(&package, &path("provenance/ollama-manifest-v2.json"))
        .expect("derive shared-CAS member evidence");
    assert!(matches!(
        members[2].member_use(),
        EffectivePackageMemberUseV2::EffectiveAndEvidence { purposes, roles }
            if purposes == &[EffectivePackageMemberPurpose::PromptTemplate]
                && roles == &[EffectivePackageEvidenceRoleV2::LicenseText]
    ));
    assert!(matches!(
        members[4].member_use(),
        EffectivePackageMemberUseV2::Effective { purposes }
            if purposes == &[EffectivePackageMemberPurpose::PromptTemplate]
    ));
}

#[test]
fn shared_license_and_model_bytes_receive_the_full_embedded_purpose_union() {
    let (set, package) = fixture(SharedLicenseBytes::Model);
    assert_eq!(
        set.members()[2].artifact_id(),
        set.members()[3].artifact_id()
    );
    let members = derive_member_evidence(&package, &path("provenance/ollama-manifest-v2.json"))
        .expect("derive shared model and license evidence");
    let complete_union = [
        EffectivePackageMemberPurpose::ModelWeights,
        EffectivePackageMemberPurpose::ModelConfiguration,
        EffectivePackageMemberPurpose::GenerationConfiguration,
        EffectivePackageMemberPurpose::TokenizerModel,
        EffectivePackageMemberPurpose::PromptTemplate,
    ];
    assert!(matches!(
        members[2].member_use(),
        EffectivePackageMemberUseV2::EffectiveAndEvidence { purposes, roles }
            if purposes.as_slice() == complete_union
                && roles == &[EffectivePackageEvidenceRoleV2::LicenseText]
    ));
    assert!(matches!(
        members[3].member_use(),
        EffectivePackageMemberUseV2::Effective { purposes }
            if purposes.as_slice() == complete_union
    ));
}

#[test]
fn every_output_affecting_model_role_has_one_exact_mapping() {
    let mappings = [
        (
            ModelPackageMemberRole::ModelWeights,
            EffectivePackageMemberPurpose::ModelWeights,
        ),
        (
            ModelPackageMemberRole::ModelWeightShard,
            EffectivePackageMemberPurpose::ModelWeights,
        ),
        (
            ModelPackageMemberRole::ModelShardIndex,
            EffectivePackageMemberPurpose::ModelShardIndex,
        ),
        (
            ModelPackageMemberRole::ModelConfiguration,
            EffectivePackageMemberPurpose::ModelConfiguration,
        ),
        (
            ModelPackageMemberRole::GenerationConfiguration,
            EffectivePackageMemberPurpose::GenerationConfiguration,
        ),
        (
            ModelPackageMemberRole::TokenizerModel,
            EffectivePackageMemberPurpose::TokenizerModel,
        ),
        (
            ModelPackageMemberRole::TokenizerVocabulary,
            EffectivePackageMemberPurpose::TokenizerVocabulary,
        ),
        (
            ModelPackageMemberRole::TokenizerMerges,
            EffectivePackageMemberPurpose::TokenizerMerges,
        ),
        (
            ModelPackageMemberRole::TokenizerConfiguration,
            EffectivePackageMemberPurpose::TokenizerConfiguration,
        ),
        (
            ModelPackageMemberRole::PromptTemplate,
            EffectivePackageMemberPurpose::PromptTemplate,
        ),
        (
            ModelPackageMemberRole::SystemPrompt,
            EffectivePackageMemberPurpose::SystemPrompt,
        ),
        (
            ModelPackageMemberRole::GrammarOrSchema,
            EffectivePackageMemberPurpose::GrammarOrSchema,
        ),
        (
            ModelPackageMemberRole::Adapter,
            EffectivePackageMemberPurpose::Adapter,
        ),
        (
            ModelPackageMemberRole::Projector,
            EffectivePackageMemberPurpose::Projector,
        ),
        (
            ModelPackageMemberRole::DraftModel,
            EffectivePackageMemberPurpose::DraftModel,
        ),
        (
            ModelPackageMemberRole::CustomModelCode,
            EffectivePackageMemberPurpose::CustomModelCode,
        ),
        (
            ModelPackageMemberRole::CustomGenerationCode,
            EffectivePackageMemberPurpose::CustomGenerationCode,
        ),
        (
            ModelPackageMemberRole::AuxiliaryData,
            EffectivePackageMemberPurpose::AuxiliaryData,
        ),
    ];
    for (role, expected) in mappings {
        assert_eq!(purpose_for_role(role), expected);
    }
}

#[test]
fn transformation_disposition_is_derived_without_losing_any_field() {
    let untransformed = PackageTransformation::Untransformed {
        evidence_digest: Digest::sha256(b"comparison"),
    };
    assert!(matches!(
        transformation(&untransformed),
        rewrite_model::PackageTransformationDisposition::Untransformed { evidence_digest }
            if evidence_digest == Digest::sha256(b"comparison")
    ));
    let source = ArtifactSetManifest::new(vec![member("source.bin", b"source")])
        .expect("source set")
        .artifact_set_id();
    let transformed = PackageTransformation::Transformed {
        source_artifact_set_id: source.clone(),
        tool_evidence_digest: Digest::sha256(b"tool"),
        parameters_digest: Digest::sha256(b"parameters"),
        log_digest: Digest::sha256(b"log"),
    };
    assert!(matches!(
        transformation(&transformed),
        rewrite_model::PackageTransformationDisposition::Transformed {
            source_artifact_set_id,
            process_evidence_digest,
            parameters_digest,
            log_digest,
        } if source_artifact_set_id == source
            && process_evidence_digest == Digest::sha256(b"tool")
            && parameters_digest == Digest::sha256(b"parameters")
            && log_digest == Digest::sha256(b"log")
    ));
}

#[test]
fn portable_runtime_closure_is_stable_and_sensitive_to_every_frozen_fact() {
    let runtime_package = Digest::sha256(b"runtime package");
    let worker = artifact(b"worker");
    let expected = vec![ExpectedExternalNativeComponent::new(
        artifact(b"libc"),
        123,
        rewrite_model::NativeMappingClass::ExecutableMapped,
    )];
    let actual = Digest::sha256(b"portable actual server and worker closure");
    let baseline =
        runtime_closure_digest_from_portable_facts(&runtime_package, &worker, &expected, &actual);
    assert_eq!(
        baseline,
        runtime_closure_digest_from_portable_facts(&runtime_package, &worker, &expected, &actual,)
    );
    assert_ne!(
        baseline,
        runtime_closure_digest_from_portable_facts(
            &Digest::sha256(b"substituted runtime package"),
            &worker,
            &expected,
            &actual,
        )
    );
    assert_ne!(
        baseline,
        runtime_closure_digest_from_portable_facts(
            &runtime_package,
            &artifact(b"substituted worker"),
            &expected,
            &actual,
        )
    );
    assert_ne!(
        baseline,
        runtime_closure_digest_from_portable_facts(
            &runtime_package,
            &worker,
            &[ExpectedExternalNativeComponent::new(
                artifact(b"different libc"),
                123,
                rewrite_model::NativeMappingClass::ExecutableMapped,
            )],
            &actual,
        )
    );
    assert_ne!(
        baseline,
        runtime_closure_digest_from_portable_facts(
            &runtime_package,
            &worker,
            &expected,
            &Digest::sha256(b"different actual closure"),
        )
    );
}

#[test]
fn isolation_exclusion_policy_is_stable_and_covers_every_excluded_member() {
    let runtime = runtime_fixture(b"license");
    let policy = Digest::sha256(b"portable isolation policy");
    let launch = Digest::sha256(b"closed launch specification");
    let baseline = isolation_exclusion_digest_from_portable_facts(&runtime, &policy, &launch);
    assert_eq!(
        baseline,
        isolation_exclusion_digest_from_portable_facts(&runtime, &policy, &launch)
    );
    assert_ne!(
        baseline,
        isolation_exclusion_digest_from_portable_facts(
            &runtime,
            &Digest::sha256(b"different policy"),
            &launch,
        )
    );
    assert_ne!(
        baseline,
        isolation_exclusion_digest_from_portable_facts(
            &runtime,
            &policy,
            &Digest::sha256(b"different launch"),
        )
    );
    assert_ne!(
        baseline,
        isolation_exclusion_digest_from_portable_facts(
            &runtime_fixture(b"substituted license"),
            &policy,
            &launch,
        )
    );
}

#[test]
fn every_required_relationship_rejects_independent_substitution() {
    let mut checks = [true; 16];
    assert!(relationship_checks_pass(&checks));
    for index in 0..checks.len() {
        checks[index] = false;
        assert!(!relationship_checks_pass(&checks));
        checks[index] = true;
    }
}
