use super::codec::{evidence_mode_byte, purpose_byte};
use super::{
    EFFECTIVE_PACKAGE_EVIDENCE_SCHEMA_VERSION, EffectivePackageEvidence,
    EffectivePackageEvidenceError, EffectivePackageEvidenceMode, EffectivePackageMemberEvidence,
    EffectivePackageMemberPurpose, MAX_EFFECTIVE_PACKAGE_CANONICAL_BYTES,
    MAX_EFFECTIVE_PACKAGE_EVIDENCE_JSON_BYTES, MAX_EFFECTIVE_PACKAGE_MEMBER_PURPOSES,
    PackageTransformationDisposition,
};
use crate::{
    ArtifactSetManifest, ArtifactSetRelativePath, ComputeBackend, EffectiveRuntimeState,
    EffectiveRuntimeStateInput, ExecutionPlacement, RuntimeBuildMode,
};

#[path = "tests/support.rs"]
mod support;

use support::{
    all_purposes, artifact_member, artifact_set, decode_value, digest, evidence_fixture,
    evidence_input, member_evidence, runtime_build, runtime_state,
};

#[test]
fn freezes_effective_package_identity_and_public_names() {
    let (artifact_set, build, state, evidence) = evidence_fixture();
    assert_eq!(
        evidence.effective_package_evidence_id().digest().as_str(),
        "8b094fc150f535a8152e9e004507a4a5e73cc87f72c7db68816a485fe0545b79"
    );
    assert_eq!(
        evidence.schema_version(),
        EFFECTIVE_PACKAGE_EVIDENCE_SCHEMA_VERSION
    );
    assert_eq!(
        evidence.evidence_mode(),
        EffectivePackageEvidenceMode::ManagedImmutablePackage
    );
    assert_eq!(evidence.artifact_set_id(), &artifact_set.artifact_set_id());
    assert_eq!(evidence.runtime_build_id(), &build.runtime_build_id());
    assert_eq!(
        evidence.effective_runtime_state_id(),
        &state.effective_runtime_state_id()
    );
    assert_eq!(evidence.member_evidence().len(), 2);

    assert_eq!(
        serde_json::to_string(&[
            EffectivePackageEvidenceMode::ManagedImmutablePackage,
            EffectivePackageEvidenceMode::AttachedAttestedPackage,
        ])
        .expect("mode names"),
        "[\"managed_immutable_package\",\"attached_attested_package\"]"
    );
    assert_eq!(
        serde_json::to_string(&all_purposes()).expect("purpose names"),
        concat!(
            "[\"model_weights\",\"model_shard_index\",\"model_configuration\",",
            "\"generation_configuration\",\"tokenizer_model\",\"tokenizer_vocabulary\",",
            "\"tokenizer_merges\",\"tokenizer_configuration\",\"prompt_template\",",
            "\"system_prompt\",\"grammar_or_schema\",\"adapter\",\"projector\",",
            "\"draft_model\",\"custom_model_code\",\"custom_generation_code\",",
            "\"auxiliary_data\"]"
        )
    );
}

#[test]
fn canonical_enum_tags_are_append_only() {
    assert_eq!(
        [
            EffectivePackageEvidenceMode::ManagedImmutablePackage,
            EffectivePackageEvidenceMode::AttachedAttestedPackage,
        ]
        .map(evidence_mode_byte),
        [0, 1]
    );
    assert_eq!(
        all_purposes().map(purpose_byte),
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]
    );
}

#[test]
fn strict_decode_revalidates_encoding_and_references() {
    let (artifact_set, build, state, evidence) = evidence_fixture();
    let encoded = serde_json::to_vec(&evidence).expect("serialize evidence");
    assert_eq!(
        EffectivePackageEvidence::from_json_bytes(&encoded, &artifact_set, &build, &state)
            .expect("validated decode"),
        evidence
    );

    let mut unknown: serde_json::Value = serde_json::from_slice(&encoded).expect("JSON value");
    unknown["unknown"] = serde_json::json!(true);
    assert_eq!(
        decode_value(&unknown, &artifact_set, &build, &state),
        Err(EffectivePackageEvidenceError::InvalidEncoding)
    );
    let mut future: serde_json::Value = serde_json::from_slice(&encoded).expect("JSON value");
    future["schema_version"] = serde_json::json!(2);
    assert_eq!(
        decode_value(&future, &artifact_set, &build, &state),
        Err(EffectivePackageEvidenceError::UnsupportedSchema(2))
    );
    let mut nested: serde_json::Value = serde_json::from_slice(&encoded).expect("JSON value");
    nested["member_evidence"][0]["unknown"] = serde_json::json!(true);
    assert_eq!(
        decode_value(&nested, &artifact_set, &build, &state),
        Err(EffectivePackageEvidenceError::InvalidEncoding)
    );
    let mut invalid_path: serde_json::Value = serde_json::from_slice(&encoded).expect("JSON value");
    invalid_path["member_evidence"][0]["relative_path"] = serde_json::json!("../secret");
    assert_eq!(
        decode_value(&invalid_path, &artifact_set, &build, &state),
        Err(EffectivePackageEvidenceError::InvalidMemberPath)
    );
}

#[test]
fn encoded_limit_is_checked_before_json_decoding() {
    let (artifact_set, build, state, _) = evidence_fixture();
    assert_eq!(
        EffectivePackageEvidence::from_json_bytes(
            &vec![b' '; MAX_EFFECTIVE_PACKAGE_EVIDENCE_JSON_BYTES],
            &artifact_set,
            &build,
            &state,
        ),
        Err(EffectivePackageEvidenceError::InvalidEncoding)
    );
    assert_eq!(
        EffectivePackageEvidence::from_json_bytes(
            &vec![b' '; MAX_EFFECTIVE_PACKAGE_EVIDENCE_JSON_BYTES + 1],
            &artifact_set,
            &build,
            &state,
        ),
        Err(EffectivePackageEvidenceError::EncodedEvidenceTooLarge)
    );
}

#[test]
fn requires_exact_member_coverage_and_canonical_order() {
    let artifact_set = artifact_set();
    let build = runtime_build(RuntimeBuildMode::ManagedProcess);
    let state = runtime_state(&build);

    let mut missing = evidence_input(EffectivePackageEvidenceMode::ManagedImmutablePackage);
    missing.member_evidence.pop();
    assert_eq!(
        EffectivePackageEvidence::new(&artifact_set, &build, &state, missing),
        Err(EffectivePackageEvidenceError::MemberCoverageMismatch)
    );

    let mut wrong = evidence_input(EffectivePackageEvidenceMode::ManagedImmutablePackage);
    wrong.member_evidence[0] = member_evidence(
        "different.json",
        vec![EffectivePackageMemberPurpose::ModelConfiguration],
    );
    assert_eq!(
        EffectivePackageEvidence::new(&artifact_set, &build, &state, wrong),
        Err(EffectivePackageEvidenceError::MemberCoverageMismatch)
    );

    let mut reversed = evidence_input(EffectivePackageEvidenceMode::ManagedImmutablePackage);
    reversed.member_evidence.reverse();
    assert_eq!(
        EffectivePackageEvidence::new(&artifact_set, &build, &state, reversed),
        Err(EffectivePackageEvidenceError::NoncanonicalMemberOrder)
    );
}

#[test]
fn validates_member_purpose_sets() {
    let path = ArtifactSetRelativePath::new("model.gguf").expect("path");
    for purposes in [
        Vec::new(),
        vec![
            EffectivePackageMemberPurpose::ModelWeights,
            EffectivePackageMemberPurpose::ModelWeights,
        ],
        vec![
            EffectivePackageMemberPurpose::ModelConfiguration,
            EffectivePackageMemberPurpose::ModelWeights,
        ],
        all_purposes()[..=MAX_EFFECTIVE_PACKAGE_MEMBER_PURPOSES].to_vec(),
    ] {
        assert_eq!(
            EffectivePackageMemberEvidence::new(path.clone(), purposes),
            Err(EffectivePackageEvidenceError::InvalidMemberPurposes)
        );
    }
    EffectivePackageMemberEvidence::new(
        path,
        all_purposes()[..MAX_EFFECTIVE_PACKAGE_MEMBER_PURPOSES].to_vec(),
    )
    .expect("exact per-member purpose bound");
}

#[test]
fn enforces_managed_and_attached_evidence_modes() {
    let artifact_set = artifact_set();
    for build_mode in [
        RuntimeBuildMode::AttachedAttestedProcess,
        RuntimeBuildMode::AttachedAttestedContainer,
    ] {
        let build = runtime_build(build_mode);
        let state = runtime_state(&build);
        EffectivePackageEvidence::new(
            &artifact_set,
            &build,
            &state,
            evidence_input(EffectivePackageEvidenceMode::AttachedAttestedPackage),
        )
        .expect("attached attested evidence");
        assert_eq!(
            EffectivePackageEvidence::new(
                &artifact_set,
                &build,
                &state,
                evidence_input(EffectivePackageEvidenceMode::ManagedImmutablePackage),
            ),
            Err(EffectivePackageEvidenceError::EvidenceModeMismatch)
        );
    }
    let build = runtime_build(RuntimeBuildMode::ManagedProcess);
    let state = runtime_state(&build);
    assert_eq!(
        EffectivePackageEvidence::new(
            &artifact_set,
            &build,
            &state,
            evidence_input(EffectivePackageEvidenceMode::AttachedAttestedPackage),
        ),
        Err(EffectivePackageEvidenceError::EvidenceModeMismatch)
    );
}

#[test]
fn rejects_build_state_mismatch_and_stale_references() {
    let (artifact_set, build, state, evidence) = evidence_fixture();
    let other_build = runtime_build(RuntimeBuildMode::AttachedAttestedProcess);
    let other_state = runtime_state(&other_build);
    assert_eq!(
        EffectivePackageEvidence::new(
            &artifact_set,
            &build,
            &other_state,
            evidence_input(EffectivePackageEvidenceMode::ManagedImmutablePackage),
        ),
        Err(EffectivePackageEvidenceError::RuntimeStateBuildMismatch)
    );
    assert_eq!(
        evidence.validate_against(&artifact_set, &other_build, &state),
        Err(EffectivePackageEvidenceError::RuntimeBuildMismatch)
    );

    let changed_set = ArtifactSetManifest::new(vec![artifact_member("model.gguf", b"changed")])
        .expect("changed set");
    assert_eq!(
        evidence.validate_against(&changed_set, &build, &state),
        Err(EffectivePackageEvidenceError::ArtifactSetMismatch)
    );

    let mut changed_state_input = EffectiveRuntimeStateInput {
        provider_snapshot_contract: "llama-server-snapshot".to_owned(),
        provider_snapshot_schema_version: 1,
        provider_snapshot_digest: digest("provider snapshot"),
        launch_policy_digest: digest("launch"),
        loaded_components_digest: digest("loaded components"),
        effective_configuration_digest: digest("effective configuration"),
        platform_digest: digest("platform"),
        execution_class_digest: digest("execution class"),
        isolation_policy_digest: digest("isolation"),
        effective_context_tokens: 32_768,
        compute_backend: ComputeBackend::Cuda,
        placement: ExecutionPlacement::AcceleratorOnly,
    };
    changed_state_input.effective_context_tokens += 1;
    let changed_state =
        EffectiveRuntimeState::new(&build, changed_state_input).expect("changed state");
    assert_eq!(
        evidence.validate_against(&artifact_set, &build, &changed_state),
        Err(EffectivePackageEvidenceError::RuntimeStateMismatch)
    );
}

#[test]
fn every_evidence_field_participates_in_identity() {
    let (artifact_set, build, state, baseline) = evidence_fixture();
    let baseline_id = baseline.effective_package_evidence_id();
    let mut variants = Vec::new();

    let mut input = evidence_input(EffectivePackageEvidenceMode::ManagedImmutablePackage);
    input.evidence_contract_id = "other-attestor".to_owned();
    variants.push(input);
    let mut input = evidence_input(EffectivePackageEvidenceMode::ManagedImmutablePackage);
    input.evidence_contract_schema_version = 2;
    variants.push(input);
    let mut input = evidence_input(EffectivePackageEvidenceMode::ManagedImmutablePackage);
    input.member_evidence[0] = member_evidence(
        "config.json",
        vec![
            EffectivePackageMemberPurpose::ModelConfiguration,
            EffectivePackageMemberPurpose::GenerationConfiguration,
        ],
    );
    variants.push(input);
    for field in 0..5 {
        let mut input = evidence_input(EffectivePackageEvidenceMode::ManagedImmutablePackage);
        let changed = digest(&format!("changed evidence {field}"));
        match field {
            0 => input.artifact_set_completeness_evidence_digest = changed,
            1 => input.acquisition_evidence_digest = changed,
            2 => input.license_review_evidence_digest = changed,
            3 => input.runtime_load_closure_evidence_digest = changed,
            _ => input.exclusion_isolation_evidence_digest = changed,
        }
        variants.push(input);
    }
    let mut input = evidence_input(EffectivePackageEvidenceMode::ManagedImmutablePackage);
    input.transformation = PackageTransformationDisposition::Untransformed {
        evidence_digest: digest("changed transformation"),
    };
    variants.push(input);
    let transformed = PackageTransformationDisposition::Transformed {
        source_artifact_set_id: artifact_set.artifact_set_id(),
        process_evidence_digest: digest("process"),
        parameters_digest: digest("parameters"),
        log_digest: digest("log"),
    };
    let mut input = evidence_input(EffectivePackageEvidenceMode::ManagedImmutablePackage);
    input.transformation = transformed;
    variants.push(input);

    for input in variants {
        let variant = EffectivePackageEvidence::new(&artifact_set, &build, &state, input)
            .expect("identity variant");
        assert_ne!(variant.effective_package_evidence_id(), baseline_id);
    }

    let changed_set = ArtifactSetManifest::new(vec![
        artifact_member("config.json", b"{\"changed\":true}"),
        artifact_member("model.gguf", b"weights"),
    ])
    .expect("changed artifact set");
    let changed_set_evidence = EffectivePackageEvidence::new(
        &changed_set,
        &build,
        &state,
        evidence_input(EffectivePackageEvidenceMode::ManagedImmutablePackage),
    )
    .expect("changed set evidence");
    assert_ne!(
        changed_set_evidence.effective_package_evidence_id(),
        baseline_id
    );

    let mut changed_state_input = serde_json::to_value(&state).expect("state value");
    changed_state_input["effective_context_tokens"] = serde_json::json!(65_536);
    let changed_state = EffectiveRuntimeState::from_json_bytes(
        &serde_json::to_vec(&changed_state_input).expect("state JSON"),
    )
    .expect("changed state");
    let changed_state_evidence = EffectivePackageEvidence::new(
        &artifact_set,
        &build,
        &changed_state,
        evidence_input(EffectivePackageEvidenceMode::ManagedImmutablePackage),
    )
    .expect("changed state evidence");
    assert_ne!(
        changed_state_evidence.effective_package_evidence_id(),
        baseline_id
    );
}

#[test]
fn every_transformation_binding_participates_in_identity() {
    let (artifact_set, build, state, baseline) = evidence_fixture();
    let baseline_id = baseline.effective_package_evidence_id();
    for field in 0..4 {
        let mut input = evidence_input(EffectivePackageEvidenceMode::ManagedImmutablePackage);
        input.transformation = PackageTransformationDisposition::Transformed {
            source_artifact_set_id: crate::ArtifactSetId::from_digest(digest(if field == 0 {
                "changed source set"
            } else {
                "source set"
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
        let variant = EffectivePackageEvidence::new(&artifact_set, &build, &state, input)
            .expect("transformation variant");
        assert_ne!(variant.effective_package_evidence_id(), baseline_id);
    }

    assert_eq!(
        serde_json::to_value(PackageTransformationDisposition::Untransformed {
            evidence_digest: digest("evidence")
        })
        .expect("untransformed JSON")["kind"],
        "untransformed"
    );
    assert_eq!(
        serde_json::to_value(PackageTransformationDisposition::Transformed {
            source_artifact_set_id: artifact_set.artifact_set_id(),
            process_evidence_digest: digest("process"),
            parameters_digest: digest("parameters"),
            log_digest: digest("log"),
        })
        .expect("transformed JSON")["kind"],
        "transformed"
    );
}

#[test]
fn validates_metadata_bounds_and_nested_unknown_fields() {
    let (artifact_set, build, state, evidence) = evidence_fixture();
    for invalid in ["", "Uppercase", "has space", "a/b", &"a".repeat(65)] {
        let mut input = evidence_input(EffectivePackageEvidenceMode::ManagedImmutablePackage);
        input.evidence_contract_id = invalid.to_owned();
        assert_eq!(
            EffectivePackageEvidence::new(&artifact_set, &build, &state, input),
            Err(EffectivePackageEvidenceError::InvalidMetadata)
        );
    }
    let mut zero = evidence_input(EffectivePackageEvidenceMode::ManagedImmutablePackage);
    zero.evidence_contract_schema_version = 0;
    assert_eq!(
        EffectivePackageEvidence::new(&artifact_set, &build, &state, zero),
        Err(EffectivePackageEvidenceError::InvalidMetadata)
    );

    let mut encoded = serde_json::to_value(&evidence).expect("JSON value");
    encoded["transformation"]["unknown"] = serde_json::json!(true);
    assert_eq!(
        decode_value(&encoded, &artifact_set, &build, &state),
        Err(EffectivePackageEvidenceError::InvalidEncoding)
    );
}

#[test]
fn maximum_member_and_purpose_budget_is_bounded() {
    let artifact_members = (0..4_096)
        .map(|index| artifact_member(&format!("{index:04}.bin"), b"x"))
        .collect();
    let artifact_set = ArtifactSetManifest::new(artifact_members).expect("maximum artifact set");
    let purpose_pair = vec![
        EffectivePackageMemberPurpose::ModelWeights,
        EffectivePackageMemberPurpose::AuxiliaryData,
    ];
    let member_evidence = artifact_set
        .members()
        .iter()
        .map(|member| {
            EffectivePackageMemberEvidence::new(
                member.relative_path().clone(),
                purpose_pair.clone(),
            )
            .expect("bounded member evidence")
        })
        .collect();
    let build = runtime_build(RuntimeBuildMode::ManagedProcess);
    let state = runtime_state(&build);
    let mut input = evidence_input(EffectivePackageEvidenceMode::ManagedImmutablePackage);
    input.member_evidence = member_evidence;
    let evidence = EffectivePackageEvidence::new(&artifact_set, &build, &state, input)
        .expect("exact aggregate purpose bound");
    assert!(evidence.canonical_bytes().len() <= MAX_EFFECTIVE_PACKAGE_CANONICAL_BYTES);
    let encoded = serde_json::to_vec(&evidence).expect("maximum evidence JSON");
    assert!(encoded.len() <= MAX_EFFECTIVE_PACKAGE_EVIDENCE_JSON_BYTES);
    assert_eq!(
        EffectivePackageEvidence::from_json_bytes(&encoded, &artifact_set, &build, &state)
            .expect("maximum evidence round trip"),
        evidence
    );

    let mut over = evidence_input(EffectivePackageEvidenceMode::ManagedImmutablePackage);
    over.member_evidence = evidence.member_evidence().to_vec();
    over.member_evidence[0] = EffectivePackageMemberEvidence::new(
        over.member_evidence[0].relative_path().clone(),
        vec![
            EffectivePackageMemberPurpose::ModelWeights,
            EffectivePackageMemberPurpose::ModelConfiguration,
            EffectivePackageMemberPurpose::AuxiliaryData,
        ],
    )
    .expect("three purposes");
    assert_eq!(
        EffectivePackageEvidence::new(&artifact_set, &build, &state, over),
        Err(EffectivePackageEvidenceError::TooManyPurposeAssignments)
    );
}
