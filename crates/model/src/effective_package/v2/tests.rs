use super::codec::{evidence_role_byte, purpose_byte};
use super::{
    EFFECTIVE_PACKAGE_EVIDENCE_V2_SCHEMA_VERSION, EffectivePackageEvidenceRoleV2,
    EffectivePackageEvidenceV2, EffectivePackageMemberUseV2,
};
use crate::{
    ArtifactSetManifest, EffectivePackageEvidenceMode, EffectivePackageMemberPurpose,
    PackageTransformationDisposition, RuntimeBuildMode,
};

#[path = "tests/adversarial.rs"]
mod adversarial;
#[path = "tests/support.rs"]
mod support;

use support::{
    artifact_member, digest, evidence_fixture, evidence_input, member_evidence, runtime_build,
    runtime_state,
};

#[test]
fn freezes_v2_identity_schema_and_public_names() {
    let (set, build, state, evidence) = evidence_fixture();
    assert_eq!(
        evidence
            .effective_package_evidence_v2_id()
            .digest()
            .as_str(),
        "048951fd8fca079e25fe8cc4ef3f644f437509df706a14799af9583ac792793c"
    );
    assert!(
        evidence
            .canonical_bytes()
            .starts_with(b"retonr:effective-package-evidence:v2\0")
    );
    assert_eq!(
        evidence.schema_version(),
        EFFECTIVE_PACKAGE_EVIDENCE_V2_SCHEMA_VERSION
    );
    assert_eq!(
        evidence.evidence_mode(),
        EffectivePackageEvidenceMode::ManagedImmutablePackage
    );
    assert_eq!(evidence.artifact_set_id(), &set.artifact_set_id());
    assert_eq!(evidence.runtime_build_id(), &build.runtime_build_id());
    assert_eq!(
        evidence.effective_runtime_state_id(),
        &state.effective_runtime_state_id()
    );
    assert_eq!(evidence.member_evidence().len(), 6);

    assert_eq!(
        serde_json::to_string(&[
            EffectivePackageEvidenceRoleV2::LicenseText,
            EffectivePackageEvidenceRoleV2::ProvenanceRecord,
            EffectivePackageEvidenceRoleV2::TransformationEvidence,
        ])
        .expect("role names"),
        "[\"license_text\",\"provenance_record\",\"transformation_evidence\"]"
    );
    assert_eq!(
        serde_json::to_value(EffectivePackageMemberUseV2::Excluded {
            declared_purposes: vec![EffectivePackageMemberPurpose::AuxiliaryData],
        })
        .expect("member use name")["kind"],
        "excluded"
    );
}

#[test]
fn canonical_v2_tags_are_append_only() {
    assert_eq!(
        [
            EffectivePackageEvidenceRoleV2::LicenseText,
            EffectivePackageEvidenceRoleV2::ProvenanceRecord,
            EffectivePackageEvidenceRoleV2::TransformationEvidence,
        ]
        .map(evidence_role_byte),
        [0, 1, 2]
    );
    assert_eq!(
        [
            EffectivePackageMemberPurpose::ModelWeights,
            EffectivePackageMemberPurpose::ModelShardIndex,
            EffectivePackageMemberPurpose::ModelConfiguration,
            EffectivePackageMemberPurpose::GenerationConfiguration,
            EffectivePackageMemberPurpose::TokenizerModel,
            EffectivePackageMemberPurpose::TokenizerVocabulary,
            EffectivePackageMemberPurpose::TokenizerMerges,
            EffectivePackageMemberPurpose::TokenizerConfiguration,
            EffectivePackageMemberPurpose::PromptTemplate,
            EffectivePackageMemberPurpose::SystemPrompt,
            EffectivePackageMemberPurpose::GrammarOrSchema,
            EffectivePackageMemberPurpose::Adapter,
            EffectivePackageMemberPurpose::Projector,
            EffectivePackageMemberPurpose::DraftModel,
            EffectivePackageMemberPurpose::CustomModelCode,
            EffectivePackageMemberPurpose::CustomGenerationCode,
            EffectivePackageMemberPurpose::AuxiliaryData,
        ]
        .map(purpose_byte),
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]
    );
}

#[test]
fn exact_six_member_ollama_package_round_trips() {
    let (set, build, state, evidence) = evidence_fixture();
    let encoded = serde_json::to_vec(&evidence).expect("serialize evidence");
    let decoded = EffectivePackageEvidenceV2::from_json_bytes(&encoded, &set, &build, &state)
        .expect("strict version 2 decode");
    assert_eq!(decoded, evidence);
    assert_eq!(serde_json::to_vec(&decoded).expect("reserialize"), encoded);

    let paths = evidence
        .member_evidence()
        .iter()
        .map(|member| member.relative_path().as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        paths,
        [
            "config/ollama-config.json",
            "config/parameters.json",
            "legal/license.txt",
            "model/model.gguf",
            "prompts/template.go.tmpl",
            "provenance/ollama-manifest-v2.json",
        ]
    );
    assert!(matches!(
        evidence.member_evidence()[2].member_use(),
        EffectivePackageMemberUseV2::EvidenceOnly {
            roles
        } if roles == &[EffectivePackageEvidenceRoleV2::LicenseText]
    ));
    assert!(matches!(
        evidence.member_evidence()[5].member_use(),
        EffectivePackageMemberUseV2::EffectiveAndEvidence {
            purposes,
            roles
        } if purposes == &[EffectivePackageMemberPurpose::AuxiliaryData]
            && roles == &[EffectivePackageEvidenceRoleV2::ProvenanceRecord]
    ));
}

fn shared_template_and_license_set() -> ArtifactSetManifest {
    let shared = b"shared license and template bytes";
    ArtifactSetManifest::new(vec![
        artifact_member("legal/license.txt", shared),
        artifact_member("prompts/template.go.tmpl", shared),
    ])
    .expect("deduplicated artifact set")
}

#[test]
fn rejects_evidence_only_path_sharing_output_effective_bytes() {
    let set = shared_template_and_license_set();
    assert_eq!(
        set.members()[0].artifact_id(),
        set.members()[1].artifact_id()
    );
    let build = runtime_build(RuntimeBuildMode::ManagedProcess);
    let state = runtime_state(&build);
    let mut input = evidence_input(&set, EffectivePackageEvidenceMode::ManagedImmutablePackage);
    input.member_evidence = vec![
        member_evidence(
            &set.members()[0],
            EffectivePackageMemberUseV2::EvidenceOnly {
                roles: vec![EffectivePackageEvidenceRoleV2::LicenseText],
            },
        ),
        member_evidence(
            &set.members()[1],
            EffectivePackageMemberUseV2::Effective {
                purposes: vec![EffectivePackageMemberPurpose::PromptTemplate],
            },
        ),
    ];
    assert_eq!(
        EffectivePackageEvidenceV2::new(&set, &build, &state, input),
        Err(super::EffectivePackageEvidenceV2Error::InconsistentSharedArtifactUse)
    );
}

#[test]
fn permits_union_consistent_shared_template_and_license_content() {
    let set = shared_template_and_license_set();
    let build = runtime_build(RuntimeBuildMode::ManagedProcess);
    let state = runtime_state(&build);
    let purposes = vec![EffectivePackageMemberPurpose::PromptTemplate];
    let mut input = evidence_input(&set, EffectivePackageEvidenceMode::ManagedImmutablePackage);
    input.member_evidence = vec![
        member_evidence(
            &set.members()[0],
            EffectivePackageMemberUseV2::EffectiveAndEvidence {
                purposes: purposes.clone(),
                roles: vec![EffectivePackageEvidenceRoleV2::LicenseText],
            },
        ),
        member_evidence(
            &set.members()[1],
            EffectivePackageMemberUseV2::Effective { purposes },
        ),
    ];
    let evidence = EffectivePackageEvidenceV2::new(&set, &build, &state, input)
        .expect("shared output-effective bytes use one purpose union");
    let encoded = serde_json::to_vec(&evidence).expect("serialize shared evidence");
    assert_eq!(
        EffectivePackageEvidenceV2::from_json_bytes(&encoded, &set, &build, &state)
            .expect("decode shared evidence"),
        evidence
    );
}

#[test]
fn every_typed_member_use_changes_identity() {
    let (set, build, state, baseline) = evidence_fixture();
    let baseline_id = baseline.effective_package_evidence_v2_id();
    let variants = [
        EffectivePackageMemberUseV2::Effective {
            purposes: vec![
                EffectivePackageMemberPurpose::GenerationConfiguration,
                EffectivePackageMemberPurpose::AuxiliaryData,
            ],
        },
        EffectivePackageMemberUseV2::EvidenceOnly {
            roles: vec![EffectivePackageEvidenceRoleV2::TransformationEvidence],
        },
        EffectivePackageMemberUseV2::EffectiveAndEvidence {
            purposes: vec![EffectivePackageMemberPurpose::AuxiliaryData],
            roles: vec![EffectivePackageEvidenceRoleV2::ProvenanceRecord],
        },
        EffectivePackageMemberUseV2::Excluded {
            declared_purposes: vec![EffectivePackageMemberPurpose::AuxiliaryData],
        },
    ];
    for member_use in variants {
        let mut input = evidence_input(&set, EffectivePackageEvidenceMode::ManagedImmutablePackage);
        input.member_evidence[0] = member_evidence(&set.members()[0], member_use);
        let variant = EffectivePackageEvidenceV2::new(&set, &build, &state, input)
            .expect("valid member-use variant");
        assert_ne!(variant.effective_package_evidence_v2_id(), baseline_id);
    }
}

#[test]
fn transformation_forms_and_bindings_change_identity() {
    let (set, build, state, baseline) = evidence_fixture();
    let baseline_id = baseline.effective_package_evidence_v2_id();
    for field in 0..4 {
        let mut input = evidence_input(&set, EffectivePackageEvidenceMode::ManagedImmutablePackage);
        input.transformation = PackageTransformationDisposition::Transformed {
            source_artifact_set_id: crate::ArtifactSetId::from_digest(digest(if field == 0 {
                "changed source"
            } else {
                "source"
            })),
            process_evidence_digest: digest(if field == 1 {
                "changed process"
            } else {
                "process"
            }),
            parameters_digest: digest(if field == 2 {
                "changed parameters"
            } else {
                "parameters"
            }),
            log_digest: digest(if field == 3 { "changed log" } else { "log" }),
        };
        let variant = EffectivePackageEvidenceV2::new(&set, &build, &state, input)
            .expect("transformed variant");
        assert_ne!(variant.effective_package_evidence_v2_id(), baseline_id);
    }
}

#[test]
fn top_level_evidence_bindings_change_identity() {
    let (set, build, state, baseline) = evidence_fixture();
    let baseline_id = baseline.effective_package_evidence_v2_id();
    for field in 0..7 {
        let mut input = evidence_input(&set, EffectivePackageEvidenceMode::ManagedImmutablePackage);
        let changed = digest(&format!("changed binding {field}"));
        match field {
            0 => input.evidence_contract_id = "other-package-attestor".to_owned(),
            1 => input.evidence_contract_schema_version += 1,
            2 => input.artifact_set_completeness_evidence_digest = changed,
            3 => input.acquisition_evidence_digest = changed,
            4 => input.license_review_evidence_digest = changed,
            5 => input.runtime_load_closure_evidence_digest = changed,
            _ => input.exclusion_isolation_evidence_digest = changed,
        }
        let variant = EffectivePackageEvidenceV2::new(&set, &build, &state, input)
            .expect("evidence binding variant");
        assert_ne!(variant.effective_package_evidence_v2_id(), baseline_id);
    }
}
