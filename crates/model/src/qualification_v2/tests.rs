use super::codec::{claim_extraction_role_byte, license_byte, status_byte};
use super::{
    MAX_QUALIFICATION_V2_CANONICAL_BYTES, MAX_QUALIFICATION_V2_JSON_BYTES,
    QUALIFICATION_V2_SCHEMA_VERSION, QualificationRecordV2, QualificationRecordV2Error,
};
use crate::{
    ArtifactRole, LicenseDecision, QUALIFICATION_SCHEMA_VERSION, QualificationRecordError,
    QualificationStatus, RuntimeBuildMode,
};

#[path = "tests/support.rs"]
mod support;

use support::{
    artifact_set, decode_value, digest, effective_package, fixture, qualification_input,
    runtime_build, runtime_state, v1_qualification,
};

#[test]
fn freezes_qualification_v2_identity_and_public_contract() {
    let (artifact_set, build, state, package, record) = fixture();
    assert_eq!(
        record.qualification_v2_id().digest().as_str(),
        "bf627c4139b9120dfdc23a86ee296575fe2a38c4be622a03fc94b8fd45a27f79"
    );
    assert_eq!(record.schema_version(), QUALIFICATION_V2_SCHEMA_VERSION);
    assert_eq!(record.role(), ArtifactRole::ClaimExtraction);
    assert_eq!(record.artifact_set_id(), &artifact_set.artifact_set_id());
    assert_eq!(
        record.effective_package_evidence_id(),
        &package.effective_package_evidence_id()
    );
    assert_eq!(record.runtime_build_id(), &build.runtime_build_id());
    assert_eq!(
        record.effective_runtime_state_id(),
        &state.effective_runtime_state_id()
    );
    assert_eq!(record.source_byte_limit(), 16_777_216);
    assert_eq!(record.context_token_limit(), 32_768);
    assert_eq!(record.prompt_template_digest(), &digest("prompt template"));
    assert_eq!(
        record.claim_output_contract_digest(),
        &digest("claim output contract")
    );
    assert_eq!(
        record.claim_operation_contract_digest(),
        &digest("claim operation contract")
    );
    assert_eq!(record.request_policy_digest(), &digest("request policy"));
    assert_eq!(
        record.threshold_policy_digest(),
        &digest("threshold policy")
    );
    assert_eq!(record.language_policy_digest(), &digest("language policy"));
    assert_eq!(
        record.hardware_envelope_digest(),
        &digest("hardware envelope")
    );
    assert_eq!(
        record.qualification_suite_digest(),
        &digest("qualification suite")
    );
    assert_eq!(
        record.qualification_result_evidence_digest(),
        &digest("qualification result")
    );
    assert_eq!(record.license_decision(), LicenseDecision::LocalUseOnly);
    assert_eq!(record.status(), QualificationStatus::Qualified);
    assert!(record.canonical_bytes().len() <= MAX_QUALIFICATION_V2_CANONICAL_BYTES);
}

#[test]
fn freezes_v2_enum_tags_and_json_names() {
    assert_eq!(claim_extraction_role_byte(ArtifactRole::ClaimExtraction), 6);
    assert_eq!(
        [
            LicenseDecision::LocalUseOnly,
            LicenseDecision::RedistributionApproved,
            LicenseDecision::Rejected,
        ]
        .map(license_byte),
        [0, 1, 2]
    );
    assert_eq!(
        [
            QualificationStatus::Qualified,
            QualificationStatus::Rejected
        ]
        .map(status_byte),
        [0, 1]
    );
    let (_, _, _, _, record) = fixture();
    let value = serde_json::to_value(record).expect("qualification v2 JSON");
    assert_eq!(value["role"], "claim_extraction");
    assert_eq!(value["license_decision"], "local_use_only");
    assert_eq!(value["status"], "qualified");
}

#[test]
fn strict_decode_revalidates_encoding_policy_and_references() {
    let (artifact_set, build, state, package, record) = fixture();
    let encoded = serde_json::to_vec(&record).expect("qualification v2 JSON");
    assert_eq!(
        QualificationRecordV2::from_json_bytes(&encoded, &artifact_set, &build, &state, &package,)
            .expect("validated decode"),
        record
    );

    let mut unknown: serde_json::Value = serde_json::from_slice(&encoded).expect("JSON value");
    unknown["unknown"] = serde_json::json!(true);
    assert_eq!(
        decode_value(&unknown, &artifact_set, &build, &state, &package),
        Err(QualificationRecordV2Error::InvalidEncoding)
    );
    let mut future: serde_json::Value = serde_json::from_slice(&encoded).expect("JSON value");
    future["schema_version"] = serde_json::json!(3);
    assert_eq!(
        decode_value(&future, &artifact_set, &build, &state, &package),
        Err(QualificationRecordV2Error::UnsupportedSchema(3))
    );
    let mut wrong_role: serde_json::Value = serde_json::from_slice(&encoded).expect("JSON value");
    wrong_role["role"] = serde_json::json!("generation");
    assert_eq!(
        decode_value(&wrong_role, &artifact_set, &build, &state, &package),
        Err(QualificationRecordV2Error::UnsupportedRole)
    );

    for (field, expected) in [
        (
            "artifact_set_id",
            QualificationRecordV2Error::ArtifactSetMismatch,
        ),
        (
            "effective_package_evidence_id",
            QualificationRecordV2Error::PackageEvidenceMismatch,
        ),
        (
            "runtime_build_id",
            QualificationRecordV2Error::RuntimeBuildMismatch,
        ),
        (
            "effective_runtime_state_id",
            QualificationRecordV2Error::RuntimeStateMismatch,
        ),
    ] {
        let mut tampered: serde_json::Value = serde_json::from_slice(&encoded).expect("JSON value");
        tampered[field] = serde_json::json!(digest(&format!("tampered {field}")));
        assert_eq!(
            decode_value(&tampered, &artifact_set, &build, &state, &package),
            Err(expected)
        );
    }
}

#[test]
fn encoded_limit_precedes_json_decoding() {
    let (artifact_set, build, state, package, _) = fixture();
    assert_eq!(
        QualificationRecordV2::from_json_bytes(
            &vec![b' '; MAX_QUALIFICATION_V2_JSON_BYTES],
            &artifact_set,
            &build,
            &state,
            &package,
        ),
        Err(QualificationRecordV2Error::InvalidEncoding)
    );
    assert_eq!(
        QualificationRecordV2::from_json_bytes(
            &vec![b' '; MAX_QUALIFICATION_V2_JSON_BYTES + 1],
            &artifact_set,
            &build,
            &state,
            &package,
        ),
        Err(QualificationRecordV2Error::EncodedRecordTooLarge)
    );
}

#[test]
fn validates_resource_license_and_status_policy() {
    let artifact_set = artifact_set();
    let build = runtime_build(RuntimeBuildMode::ManagedProcess, "b10417");
    let state = runtime_state(&build, 32_768);
    let package = effective_package(&artifact_set, &build, &state, "acquisition");

    let mut zero_source = qualification_input();
    zero_source.source_byte_limit = 0;
    assert_eq!(
        QualificationRecordV2::new(&artifact_set, &build, &state, &package, zero_source),
        Err(QualificationRecordV2Error::InvalidPolicy)
    );
    let mut zero_context = qualification_input();
    zero_context.context_token_limit = 0;
    assert_eq!(
        QualificationRecordV2::new(&artifact_set, &build, &state, &package, zero_context),
        Err(QualificationRecordV2Error::InvalidPolicy)
    );
    let mut rejected_license = qualification_input();
    rejected_license.license_decision = LicenseDecision::Rejected;
    assert_eq!(
        QualificationRecordV2::new(&artifact_set, &build, &state, &package, rejected_license,),
        Err(QualificationRecordV2Error::InvalidPolicy)
    );

    let mut rejected_run = qualification_input();
    rejected_run.status = QualificationStatus::Rejected;
    rejected_run.license_decision = LicenseDecision::Rejected;
    QualificationRecordV2::new(&artifact_set, &build, &state, &package, rejected_run)
        .expect("rejected result remains valid evidence");
}

#[test]
fn every_policy_and_result_field_changes_the_identity() {
    let (artifact_set, build, state, package, baseline) = fixture();
    let baseline_id = baseline.qualification_v2_id();
    let mut variants = Vec::new();
    let mut input = qualification_input();
    input.source_byte_limit += 1;
    variants.push(input);
    let mut input = qualification_input();
    input.context_token_limit += 1;
    variants.push(input);
    for field in 0..9 {
        let mut input = qualification_input();
        let changed = digest(&format!("changed qualification field {field}"));
        match field {
            0 => input.prompt_template_digest = changed,
            1 => input.claim_output_contract_digest = changed,
            2 => input.claim_operation_contract_digest = changed,
            3 => input.request_policy_digest = changed,
            4 => input.threshold_policy_digest = changed,
            5 => input.language_policy_digest = changed,
            6 => input.hardware_envelope_digest = changed,
            7 => input.qualification_suite_digest = changed,
            _ => input.qualification_result_evidence_digest = changed,
        }
        variants.push(input);
    }
    let mut input = qualification_input();
    input.license_decision = LicenseDecision::RedistributionApproved;
    variants.push(input);
    let mut input = qualification_input();
    input.status = QualificationStatus::Rejected;
    variants.push(input);

    for input in variants {
        let variant = QualificationRecordV2::new(&artifact_set, &build, &state, &package, input)
            .expect("identity variant");
        assert_ne!(variant.qualification_v2_id(), baseline_id);
    }
}

#[test]
fn every_exact_subject_identity_changes_the_qualification_identity() {
    let (artifact_set, build, state, _package, baseline) = fixture();
    let baseline_id = baseline.qualification_v2_id();

    let changed_set = crate::ArtifactSetManifest::new(vec![
        support::artifact_member("config.json", b"{\"changed\":true}"),
        support::artifact_member("model.gguf", b"weights"),
    ])
    .expect("changed artifact set");
    let changed_package = effective_package(&changed_set, &build, &state, "acquisition");
    let changed = QualificationRecordV2::new(
        &changed_set,
        &build,
        &state,
        &changed_package,
        qualification_input(),
    )
    .expect("changed artifact-set subject");
    assert_ne!(changed.qualification_v2_id(), baseline_id);

    let changed_package = effective_package(&artifact_set, &build, &state, "new acquisition");
    let changed = QualificationRecordV2::new(
        &artifact_set,
        &build,
        &state,
        &changed_package,
        qualification_input(),
    )
    .expect("changed package-evidence subject");
    assert_ne!(changed.qualification_v2_id(), baseline_id);

    let changed_build = runtime_build(RuntimeBuildMode::ManagedProcess, "b10418");
    let changed_state = runtime_state(&changed_build, 32_768);
    let changed_package =
        effective_package(&artifact_set, &changed_build, &changed_state, "acquisition");
    let changed = QualificationRecordV2::new(
        &artifact_set,
        &changed_build,
        &changed_state,
        &changed_package,
        qualification_input(),
    )
    .expect("changed build subject");
    assert_ne!(changed.qualification_v2_id(), baseline_id);

    let changed_state = runtime_state(&build, 65_536);
    let changed_package = effective_package(&artifact_set, &build, &changed_state, "acquisition");
    let changed = QualificationRecordV2::new(
        &artifact_set,
        &build,
        &changed_state,
        &changed_package,
        qualification_input(),
    )
    .expect("changed runtime-state subject");
    assert_ne!(changed.qualification_v2_id(), baseline_id);
}

#[test]
fn stale_or_cross_product_references_fail_closed() {
    let (artifact_set, build, state, package, record) = fixture();
    let changed_set =
        crate::ArtifactSetManifest::new(vec![support::artifact_member("model.gguf", b"changed")])
            .expect("changed artifact set");
    assert_eq!(
        record.validate_against(&changed_set, &build, &state, &package),
        Err(QualificationRecordV2Error::ArtifactSetMismatch)
    );

    let changed_package = effective_package(&artifact_set, &build, &state, "changed acquisition");
    assert_eq!(
        record.validate_against(&artifact_set, &build, &state, &changed_package),
        Err(QualificationRecordV2Error::PackageEvidenceMismatch)
    );

    let changed_build = runtime_build(RuntimeBuildMode::ManagedProcess, "b10418");
    assert_eq!(
        record.validate_against(&artifact_set, &changed_build, &state, &package),
        Err(QualificationRecordV2Error::RuntimeBuildMismatch)
    );

    let changed_state = runtime_state(&build, 65_536);
    assert_eq!(
        record.validate_against(&artifact_set, &build, &changed_state, &package),
        Err(QualificationRecordV2Error::RuntimeStateMismatch)
    );

    let other_set =
        crate::ArtifactSetManifest::new(vec![support::artifact_member("model.gguf", b"other")])
            .expect("other set");
    let other_package = effective_package(&other_set, &build, &state, "other acquisition");
    assert_eq!(
        QualificationRecordV2::new(
            &artifact_set,
            &build,
            &state,
            &other_package,
            qualification_input(),
        ),
        Err(QualificationRecordV2Error::InvalidPackageEvidence(
            crate::EffectivePackageEvidenceError::ArtifactSetMismatch
        ))
    );
}

#[test]
fn qualification_v1_identity_and_authority_remain_unchanged() {
    let v1 = v1_qualification();
    assert_eq!(v1.schema_version, QUALIFICATION_SCHEMA_VERSION);
    assert_eq!(
        v1.qualification_id()
            .expect("valid v1 qualification")
            .digest()
            .as_str(),
        "aa156c8224aa6a6dacc7bd3351b3ebd67fab2c345ce340c1bc7294d49193d4dd"
    );
    let mut extraction = v1;
    extraction.supported_roles = vec![ArtifactRole::ClaimExtraction];
    assert_eq!(
        extraction.validate(),
        Err(QualificationRecordError::UnsupportedRole)
    );
}
