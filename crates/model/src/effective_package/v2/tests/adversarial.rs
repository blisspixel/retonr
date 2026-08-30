use super::super::{
    EffectivePackageEvidenceRoleV2, EffectivePackageEvidenceV2, EffectivePackageEvidenceV2Error,
    EffectivePackageMemberEvidenceV2, EffectivePackageMemberUseV2,
    MAX_EFFECTIVE_PACKAGE_EVIDENCE_V2_CANONICAL_BYTES,
    MAX_EFFECTIVE_PACKAGE_EVIDENCE_V2_JSON_BYTES, MAX_EFFECTIVE_PACKAGE_MEMBER_V2_PURPOSES,
};
use super::support::{
    artifact_member, artifact_set, decode_value, digest, evidence_fixture, evidence_input,
    member_evidence, runtime_build, runtime_state,
};
use crate::{
    ArtifactId, ArtifactSetManifest, ArtifactSetRelativePath, EffectivePackageEvidenceMode,
    EffectivePackageMemberPurpose, EffectiveRuntimeState, RuntimeBuildMode,
};

#[test]
fn rejects_empty_duplicated_unordered_and_unbounded_member_uses() {
    let member = artifact_set().members()[0].clone();
    let invalid_uses = [
        EffectivePackageMemberUseV2::Effective {
            purposes: Vec::new(),
        },
        EffectivePackageMemberUseV2::Effective {
            purposes: vec![
                EffectivePackageMemberPurpose::ModelWeights,
                EffectivePackageMemberPurpose::ModelWeights,
            ],
        },
        EffectivePackageMemberUseV2::Excluded {
            declared_purposes: vec![
                EffectivePackageMemberPurpose::ModelConfiguration,
                EffectivePackageMemberPurpose::ModelWeights,
            ],
        },
        EffectivePackageMemberUseV2::Effective {
            purposes: vec![
                EffectivePackageMemberPurpose::ModelWeights;
                MAX_EFFECTIVE_PACKAGE_MEMBER_V2_PURPOSES + 1
            ],
        },
        EffectivePackageMemberUseV2::EvidenceOnly { roles: Vec::new() },
        EffectivePackageMemberUseV2::EvidenceOnly {
            roles: vec![
                EffectivePackageEvidenceRoleV2::LicenseText,
                EffectivePackageEvidenceRoleV2::LicenseText,
            ],
        },
        EffectivePackageMemberUseV2::EvidenceOnly {
            roles: vec![
                EffectivePackageEvidenceRoleV2::ProvenanceRecord,
                EffectivePackageEvidenceRoleV2::LicenseText,
            ],
        },
        EffectivePackageMemberUseV2::EffectiveAndEvidence {
            purposes: Vec::new(),
            roles: vec![EffectivePackageEvidenceRoleV2::ProvenanceRecord],
        },
        EffectivePackageMemberUseV2::EffectiveAndEvidence {
            purposes: vec![EffectivePackageMemberPurpose::AuxiliaryData],
            roles: Vec::new(),
        },
        EffectivePackageMemberUseV2::Excluded {
            declared_purposes: Vec::new(),
        },
    ];
    for member_use in invalid_uses {
        assert_eq!(
            EffectivePackageMemberEvidenceV2::new(
                member.relative_path().clone(),
                member.artifact_id().clone(),
                member.byte_size(),
                member_use,
            ),
            Err(EffectivePackageEvidenceV2Error::InvalidMemberUse)
        );
    }
}

#[test]
fn rejects_missing_mismatched_and_noncanonical_member_coverage() {
    let set = artifact_set();
    let build = runtime_build(RuntimeBuildMode::ManagedProcess);
    let state = runtime_state(&build);

    let mut missing = evidence_input(&set, EffectivePackageEvidenceMode::ManagedImmutablePackage);
    missing.member_evidence.pop();
    assert_eq!(
        EffectivePackageEvidenceV2::new(&set, &build, &state, missing),
        Err(EffectivePackageEvidenceV2Error::MemberCoverageMismatch)
    );

    let mut reversed = evidence_input(&set, EffectivePackageEvidenceMode::ManagedImmutablePackage);
    reversed.member_evidence.reverse();
    assert_eq!(
        EffectivePackageEvidenceV2::new(&set, &build, &state, reversed),
        Err(EffectivePackageEvidenceV2Error::NoncanonicalMemberOrder)
    );

    let mut duplicate = evidence_input(&set, EffectivePackageEvidenceMode::ManagedImmutablePackage);
    duplicate.member_evidence[1] = duplicate.member_evidence[0].clone();
    assert_eq!(
        EffectivePackageEvidenceV2::new(&set, &build, &state, duplicate),
        Err(EffectivePackageEvidenceV2Error::DuplicateMemberPath)
    );
}

#[test]
fn rejects_inconsistent_effective_uses_for_shared_artifacts() {
    let shared = b"one shared artifact";
    let set = ArtifactSetManifest::new(vec![
        artifact_member("legal/license.txt", shared),
        artifact_member("prompts/template.go.tmpl", shared),
    ])
    .expect("shared artifact set");
    let build = runtime_build(RuntimeBuildMode::ManagedProcess);
    let state = runtime_state(&build);
    let invalid_second_uses = [
        EffectivePackageMemberUseV2::Excluded {
            declared_purposes: vec![EffectivePackageMemberPurpose::PromptTemplate],
        },
        EffectivePackageMemberUseV2::Effective {
            purposes: vec![EffectivePackageMemberPurpose::AuxiliaryData],
        },
        EffectivePackageMemberUseV2::EffectiveAndEvidence {
            purposes: vec![EffectivePackageMemberPurpose::AuxiliaryData],
            roles: vec![EffectivePackageEvidenceRoleV2::ProvenanceRecord],
        },
    ];
    for second_use in invalid_second_uses {
        let mut input = evidence_input(&set, EffectivePackageEvidenceMode::ManagedImmutablePackage);
        input.member_evidence = vec![
            member_evidence(
                &set.members()[0],
                EffectivePackageMemberUseV2::Effective {
                    purposes: vec![EffectivePackageMemberPurpose::PromptTemplate],
                },
            ),
            member_evidence(&set.members()[1], second_use),
        ];
        assert_eq!(
            EffectivePackageEvidenceV2::new(&set, &build, &state, input),
            Err(EffectivePackageEvidenceV2Error::InconsistentSharedArtifactUse)
        );
    }
}

#[test]
fn shared_artifact_purpose_union_must_appear_at_every_effective_path() {
    let shared = b"one shared artifact";
    let set = ArtifactSetManifest::new(vec![
        artifact_member("legal/license.txt", shared),
        artifact_member("prompts/template.go.tmpl", shared),
    ])
    .expect("shared artifact set");
    let build = runtime_build(RuntimeBuildMode::ManagedProcess);
    let state = runtime_state(&build);
    let union = vec![
        EffectivePackageMemberPurpose::ModelConfiguration,
        EffectivePackageMemberPurpose::PromptTemplate,
    ];
    let mut input = evidence_input(&set, EffectivePackageEvidenceMode::ManagedImmutablePackage);
    input.member_evidence = vec![
        member_evidence(
            &set.members()[0],
            EffectivePackageMemberUseV2::Effective {
                purposes: union.clone(),
            },
        ),
        member_evidence(
            &set.members()[1],
            EffectivePackageMemberUseV2::EffectiveAndEvidence {
                purposes: union,
                roles: vec![EffectivePackageEvidenceRoleV2::ProvenanceRecord],
            },
        ),
    ];
    EffectivePackageEvidenceV2::new(&set, &build, &state, input)
        .expect("every shared effective path carries the complete purpose union");
}

#[test]
fn rejects_member_path_artifact_and_size_drift() {
    let (set, build, state, evidence) = evidence_fixture();
    let encoded = serde_json::to_value(&evidence).expect("evidence JSON");

    let mut changed_path = encoded.clone();
    changed_path["member_evidence"][0]["relative_path"] = serde_json::json!("config/other.json");
    assert_eq!(
        decode_value(&changed_path, &set, &build, &state),
        Err(EffectivePackageEvidenceV2Error::MemberCoverageMismatch)
    );

    let mut changed_artifact = encoded.clone();
    changed_artifact["member_evidence"][0]["artifact_id"] =
        serde_json::json!(digest("different artifact").as_str());
    assert_eq!(
        decode_value(&changed_artifact, &set, &build, &state),
        Err(EffectivePackageEvidenceV2Error::MemberCoverageMismatch)
    );

    let mut changed_size = encoded;
    changed_size["member_evidence"][0]["byte_size"] = serde_json::json!(999);
    assert_eq!(
        decode_value(&changed_size, &set, &build, &state),
        Err(EffectivePackageEvidenceV2Error::MemberCoverageMismatch)
    );
}

#[test]
fn strict_decode_rejects_unknown_malformed_and_future_data() {
    let (set, build, state, evidence) = evidence_fixture();
    let encoded = serde_json::to_value(&evidence).expect("evidence JSON");

    let mut unknown = encoded.clone();
    unknown["unknown"] = serde_json::json!(true);
    assert_eq!(
        decode_value(&unknown, &set, &build, &state),
        Err(EffectivePackageEvidenceV2Error::InvalidEncoding)
    );
    let mut nested = encoded.clone();
    nested["member_evidence"][0]["unknown"] = serde_json::json!(true);
    assert_eq!(
        decode_value(&nested, &set, &build, &state),
        Err(EffectivePackageEvidenceV2Error::InvalidEncoding)
    );
    let mut member_use = encoded.clone();
    member_use["member_evidence"][0]["member_use"]["unknown"] = serde_json::json!(true);
    assert_eq!(
        decode_value(&member_use, &set, &build, &state),
        Err(EffectivePackageEvidenceV2Error::InvalidEncoding)
    );
    let mut future = encoded.clone();
    future["schema_version"] = serde_json::json!(3);
    assert_eq!(
        decode_value(&future, &set, &build, &state),
        Err(EffectivePackageEvidenceV2Error::UnsupportedSchema(3))
    );
    let mut invalid_path = encoded;
    invalid_path["member_evidence"][0]["relative_path"] = serde_json::json!("../escape");
    assert_eq!(
        decode_value(&invalid_path, &set, &build, &state),
        Err(EffectivePackageEvidenceV2Error::InvalidMemberPath)
    );
}

#[test]
fn encoded_limit_precedes_json_decoding() {
    let (set, build, state, _) = evidence_fixture();
    assert_eq!(
        EffectivePackageEvidenceV2::from_json_bytes(
            &vec![b' '; MAX_EFFECTIVE_PACKAGE_EVIDENCE_V2_JSON_BYTES],
            &set,
            &build,
            &state,
        ),
        Err(EffectivePackageEvidenceV2Error::InvalidEncoding)
    );
    assert_eq!(
        EffectivePackageEvidenceV2::from_json_bytes(
            &vec![b' '; MAX_EFFECTIVE_PACKAGE_EVIDENCE_V2_JSON_BYTES + 1],
            &set,
            &build,
            &state,
        ),
        Err(EffectivePackageEvidenceV2Error::EncodedEvidenceTooLarge)
    );
}

#[test]
fn validates_metadata_and_evidence_modes() {
    let set = artifact_set();
    let build = runtime_build(RuntimeBuildMode::ManagedProcess);
    let state = runtime_state(&build);
    for invalid in ["", "Uppercase", "has space", "a/b", &"a".repeat(65)] {
        let mut input = evidence_input(&set, EffectivePackageEvidenceMode::ManagedImmutablePackage);
        input.evidence_contract_id = invalid.to_owned();
        assert_eq!(
            EffectivePackageEvidenceV2::new(&set, &build, &state, input),
            Err(EffectivePackageEvidenceV2Error::InvalidMetadata)
        );
    }
    let mut zero = evidence_input(&set, EffectivePackageEvidenceMode::ManagedImmutablePackage);
    zero.evidence_contract_schema_version = 0;
    assert_eq!(
        EffectivePackageEvidenceV2::new(&set, &build, &state, zero),
        Err(EffectivePackageEvidenceV2Error::InvalidMetadata)
    );

    let attached = runtime_build(RuntimeBuildMode::AttachedAttestedProcess);
    let attached_state = runtime_state(&attached);
    EffectivePackageEvidenceV2::new(
        &set,
        &attached,
        &attached_state,
        evidence_input(&set, EffectivePackageEvidenceMode::AttachedAttestedPackage),
    )
    .expect("attached process mode");
    let container = runtime_build(RuntimeBuildMode::AttachedAttestedContainer);
    let container_state = runtime_state(&container);
    EffectivePackageEvidenceV2::new(
        &set,
        &container,
        &container_state,
        evidence_input(&set, EffectivePackageEvidenceMode::AttachedAttestedPackage),
    )
    .expect("attached container mode");
    assert_eq!(
        EffectivePackageEvidenceV2::new(
            &set,
            &build,
            &state,
            evidence_input(&set, EffectivePackageEvidenceMode::AttachedAttestedPackage),
        ),
        Err(EffectivePackageEvidenceV2Error::EvidenceModeMismatch)
    );
}

#[test]
fn rejects_stale_set_build_and_state_references() {
    let (set, build, state, evidence) = evidence_fixture();
    let changed_set = ArtifactSetManifest::new(vec![artifact_member("model/model.gguf", b"other")])
        .expect("changed set");
    assert_eq!(
        evidence.validate_against(&changed_set, &build, &state),
        Err(EffectivePackageEvidenceV2Error::ArtifactSetMismatch)
    );

    let other_build = runtime_build(RuntimeBuildMode::AttachedAttestedProcess);
    assert_eq!(
        evidence.validate_against(&set, &other_build, &state),
        Err(EffectivePackageEvidenceV2Error::RuntimeBuildMismatch)
    );
    let other_state = runtime_state(&other_build);
    assert_eq!(
        EffectivePackageEvidenceV2::new(
            &set,
            &build,
            &other_state,
            evidence_input(&set, EffectivePackageEvidenceMode::ManagedImmutablePackage),
        ),
        Err(EffectivePackageEvidenceV2Error::RuntimeStateBuildMismatch)
    );

    let mut changed: serde_json::Value = serde_json::to_value(&state).expect("runtime state JSON");
    changed["effective_context_tokens"] = serde_json::json!(65_536);
    let changed_state = EffectiveRuntimeState::from_json_bytes(
        &serde_json::to_vec(&changed).expect("changed state JSON"),
    )
    .expect("changed runtime state");
    assert_eq!(
        evidence.validate_against(&set, &build, &changed_state),
        Err(EffectivePackageEvidenceV2Error::RuntimeStateMismatch)
    );
}

#[test]
fn enforces_empty_member_and_aggregate_assignment_bounds() {
    let set = artifact_set();
    let build = runtime_build(RuntimeBuildMode::ManagedProcess);
    let state = runtime_state(&build);
    let mut empty = evidence_input(&set, EffectivePackageEvidenceMode::ManagedImmutablePackage);
    empty.member_evidence.clear();
    assert_eq!(
        EffectivePackageEvidenceV2::new(&set, &build, &state, empty),
        Err(EffectivePackageEvidenceV2Error::EmptyMemberCoverage)
    );

    let large_set = ArtifactSetManifest::new(
        (0..4_096)
            .map(|index| artifact_member(&format!("{index:04}.bin"), b"shared"))
            .collect(),
    )
    .expect("maximum artifact set");
    let large_build = runtime_build(RuntimeBuildMode::ManagedProcess);
    let large_state = runtime_state(&large_build);
    let mut bounded = evidence_input(&set, EffectivePackageEvidenceMode::ManagedImmutablePackage);
    bounded.member_evidence = large_set
        .members()
        .iter()
        .map(|member| {
            member_evidence(
                member,
                EffectivePackageMemberUseV2::Effective {
                    purposes: vec![
                        EffectivePackageMemberPurpose::ModelWeights,
                        EffectivePackageMemberPurpose::AuxiliaryData,
                    ],
                },
            )
        })
        .collect();
    let maximum =
        EffectivePackageEvidenceV2::new(&large_set, &large_build, &large_state, bounded.clone())
            .expect("exact aggregate assignment bound");
    let encoded = serde_json::to_vec(&maximum).expect("maximum evidence JSON");
    assert!(encoded.len() <= MAX_EFFECTIVE_PACKAGE_EVIDENCE_V2_JSON_BYTES);
    assert!(maximum.canonical_bytes().len() <= MAX_EFFECTIVE_PACKAGE_EVIDENCE_V2_CANONICAL_BYTES);
    assert_eq!(
        EffectivePackageEvidenceV2::from_json_bytes(
            &encoded,
            &large_set,
            &large_build,
            &large_state,
        )
        .expect("maximum evidence round trip"),
        maximum
    );
    bounded.member_evidence[0] = member_evidence(
        &large_set.members()[0],
        EffectivePackageMemberUseV2::Effective {
            purposes: vec![
                EffectivePackageMemberPurpose::ModelWeights,
                EffectivePackageMemberPurpose::ModelConfiguration,
                EffectivePackageMemberPurpose::AuxiliaryData,
            ],
        },
    );
    assert_eq!(
        EffectivePackageEvidenceV2::new(&large_set, &large_build, &large_state, bounded),
        Err(EffectivePackageEvidenceV2Error::TooManyMemberAssignments)
    );
}

#[test]
fn too_many_members_is_rejected_before_member_iteration() {
    let set = artifact_set();
    let member = member_evidence(
        &set.members()[0],
        EffectivePackageMemberUseV2::Excluded {
            declared_purposes: vec![EffectivePackageMemberPurpose::AuxiliaryData],
        },
    );
    let members = vec![member; 4_097];
    assert_eq!(
        super::super::validate_member_evidence(&members),
        Err(EffectivePackageEvidenceV2Error::TooManyMembers)
    );
}

#[test]
fn member_accessors_return_exact_artifact_binding() {
    let set = artifact_set();
    let member = &set.members()[0];
    let evidence = member_evidence(
        member,
        EffectivePackageMemberUseV2::Effective {
            purposes: vec![EffectivePackageMemberPurpose::AuxiliaryData],
        },
    );
    assert_eq!(evidence.relative_path(), member.relative_path());
    assert_eq!(evidence.artifact_id(), member.artifact_id());
    assert_eq!(evidence.byte_size(), member.byte_size());
    assert_eq!(
        ArtifactId::from_digest(digest("accessor sanity")).digest(),
        &digest("accessor sanity")
    );
    assert_eq!(
        ArtifactSetRelativePath::new(evidence.relative_path().as_str()).expect("same path"),
        evidence.relative_path().clone()
    );
}
