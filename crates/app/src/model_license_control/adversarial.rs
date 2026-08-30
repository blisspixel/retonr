use rewrite_types::{CancellationToken, Digest};
use serde_json::{Value, json};

use super::fixture::Fixture;
use super::{ModelLicenseControlVerifier, compile, reviewer_json_for_test, value};
use crate::{
    MAX_MODEL_LICENSE_CONTROL_JSON_BYTES, MAX_MODEL_LICENSE_REVIEW_JSON_BYTES,
    ModelLicenseControlCompiler, ModelLicenseControlError, ModelLicensePermission,
    ProductionModelLicenseApprovalPolicy,
};

#[test]
fn reviewer_json_is_strict_canonical_bounded_and_has_no_digest_injection() {
    let fixture = Fixture::new(false);
    let lease = fixture.verify();
    let reviewer = reviewer_json_for_test(&lease, ModelLicensePermission::LocalGeneration);
    let mut duplicate = reviewer[..reviewer.len() - 1].to_vec();
    duplicate.extend_from_slice(b",\"decision\":\"approved\"}");
    let mut trailing = reviewer.clone();
    trailing.extend_from_slice(b" false");
    let mut noncanonical = vec![b' '];
    noncanonical.extend_from_slice(&reviewer);
    let mut unknown = value(&reviewer);
    unknown.as_object_mut().expect("review object").insert(
        "review_evidence_digest".to_owned(),
        json!(Digest::sha256(b"injected")),
    );
    for invalid in [
        b"{".to_vec(),
        duplicate,
        trailing,
        noncanonical,
        serde_json::to_vec(&unknown).expect("unknown-field review"),
    ] {
        assert!(matches!(
            ModelLicenseControlCompiler::compile(
                &lease,
                ModelLicensePermission::LocalGeneration,
                &invalid,
                &CancellationToken::new(),
            ),
            Err(ModelLicenseControlError::InvalidEncoding)
        ));
    }
    let oversized = vec![b' '; MAX_MODEL_LICENSE_REVIEW_JSON_BYTES + 1];
    assert!(matches!(
        ModelLicenseControlCompiler::compile(
            &lease,
            ModelLicensePermission::LocalGeneration,
            &oversized,
            &CancellationToken::new(),
        ),
        Err(ModelLicenseControlError::LimitExceeded)
    ));
}

#[test]
fn reviewer_must_match_every_foundation_source_and_license_fact() {
    let fixture = Fixture::new(false);
    let lease = fixture.verify();
    let reviewer = reviewer_json_for_test(&lease, ModelLicensePermission::LocalGeneration);
    let mutations: [fn(&mut Value); 14] = [
        |review| review["procedure_id"] = json!("substituted-procedure"),
        |review| review["procedure_version"] = json!(2),
        |review| review["schema_version"] = json!(2),
        |review| review["foundation"]["foundation_id"] = json!(Digest::sha256(b"foundation")),
        |review| review["foundation"]["artifact_set_id"] = json!(Digest::sha256(b"set")),
        |review| {
            review["foundation"]["model_package_manifest_id"] = json!(Digest::sha256(b"package"));
        },
        |review| review["foundation"]["package_source_id"] = json!(Digest::sha256(b"source")),
        |review| review["foundation"]["descriptor_mapping_digest"] = json!(Digest::sha256(b"map")),
        |review| {
            review["foundation"]["logical_binding_digest"] = json!(Digest::sha256(b"logical"));
        },
        |review| review["foundation"]["provenance_manifest"]["byte_size"] = json!(1),
        |review| review["source"]["locator"] = json!("registry.ollama.ai/other/model"),
        |review| review["source"]["revision"] = json!("sha256:changed"),
        |review| review["license_members"][0]["byte_size"] = json!(1),
        |review| review["license_members"][0]["relative_path"] = json!("legal/other.txt"),
    ];
    for mutate in mutations {
        let mut changed = value(&reviewer);
        mutate(&mut changed);
        let changed = serde_json::to_vec(&changed).expect("changed review");
        assert!(matches!(
            ModelLicenseControlCompiler::compile(
                &lease,
                ModelLicensePermission::LocalGeneration,
                &changed,
                &CancellationToken::new(),
            ),
            Err(ModelLicenseControlError::ReviewRequired)
        ));
    }
    let mut duplicated = value(&reviewer);
    let licenses = duplicated["license_members"]
        .as_array_mut()
        .expect("license members");
    let mut second = licenses[0].clone();
    second["relative_path"] = json!("legal/second-license.txt");
    licenses.push(second);
    assert!(matches!(
        ModelLicenseControlCompiler::compile(
            &lease,
            ModelLicensePermission::LocalGeneration,
            &serde_json::to_vec(&duplicated).expect("duplicated license review"),
            &CancellationToken::new(),
        ),
        Err(ModelLicenseControlError::ReviewRequired)
    ));
    let mut denied = value(&reviewer);
    denied["decision"] = json!("denied");
    assert!(matches!(
        ModelLicenseControlCompiler::compile(
            &lease,
            ModelLicensePermission::LocalGeneration,
            &serde_json::to_vec(&denied).expect("denied review"),
            &CancellationToken::new(),
        ),
        Err(ModelLicenseControlError::InvalidEncoding)
    ));
}

#[test]
fn canonical_control_rejects_top_level_and_nested_tampering() {
    let fixture = Fixture::new(false);
    let lease = fixture.verify();
    let reviewer = reviewer_json_for_test(&lease, ModelLicensePermission::LocalGeneration);
    let compiled = compile(&lease, ModelLicensePermission::LocalGeneration, &reviewer);
    let mutations: [fn(&mut Value); 7] = [
        |control| control["procedure_id"] = json!("substituted-control-procedure"),
        |control| control["procedure_version"] = json!(2),
        |control| control["schema_version"] = json!(2),
        |control| control["status"] = json!("pending"),
        |control| control["authority"] = json!("live"),
        |control| control["evidence"]["review_evidence_digest"] = json!(Digest::sha256(b"caller")),
        |control| {
            control["review"]["source"]["provenance_digest"] = json!(Digest::sha256(b"stale"));
        },
    ];
    for mutate in mutations {
        let mut changed = value(compiled.canonical_bytes());
        mutate(&mut changed);
        let bytes = serde_json::to_vec(&changed).expect("changed control");
        assert!(
            ModelLicenseControlVerifier::verify(
                &bytes,
                &lease,
                ModelLicensePermission::LocalGeneration,
                &CancellationToken::new(),
            )
            .is_err()
        );
    }
    let mut unknown = value(compiled.canonical_bytes());
    unknown
        .as_object_mut()
        .expect("control object")
        .insert("unknown".to_owned(), Value::Null);
    let mut trailing = compiled.canonical_bytes().to_vec();
    trailing.push(b' ');
    let mut noncanonical = vec![b' '];
    noncanonical.extend_from_slice(compiled.canonical_bytes());
    let mut duplicate = compiled.canonical_bytes()[..compiled.canonical_bytes().len() - 1].to_vec();
    duplicate.extend_from_slice(b",\"authority\":\"none\"}");
    for invalid in [
        b"{".to_vec(),
        serde_json::to_vec(&unknown).expect("unknown-field control"),
        trailing,
        noncanonical,
        duplicate,
    ] {
        assert!(matches!(
            ModelLicenseControlVerifier::verify(
                &invalid,
                &lease,
                ModelLicensePermission::LocalGeneration,
                &CancellationToken::new(),
            ),
            Err(ModelLicenseControlError::InvalidEncoding)
        ));
    }
    let oversized = vec![b' '; MAX_MODEL_LICENSE_CONTROL_JSON_BYTES + 1];
    assert!(matches!(
        ModelLicenseControlVerifier::verify(
            &oversized,
            &lease,
            ModelLicensePermission::LocalGeneration,
            &CancellationToken::new(),
        ),
        Err(ModelLicenseControlError::LimitExceeded)
    ));
}

#[test]
fn production_policy_denial_does_not_mask_untrusted_control_failures() {
    let fixture = Fixture::new(false);
    let lease = fixture.verify();
    let reviewer = reviewer_json_for_test(&lease, ModelLicensePermission::LocalGeneration);
    let compiled = compile(&lease, ModelLicensePermission::LocalGeneration, &reviewer);
    let production_policy = ProductionModelLicenseApprovalPolicy::new();

    assert!(matches!(
        crate::ModelLicenseControlVerifier::verify(
            b"{",
            &lease,
            ModelLicensePermission::LocalGeneration,
            &production_policy,
            &CancellationToken::new(),
        ),
        Err(ModelLicenseControlError::InvalidEncoding)
    ));

    let mut foreign = value(compiled.canonical_bytes());
    foreign["review"]["foundation"]["foundation_id"] = json!(Digest::sha256(b"foreign-foundation"));
    assert!(matches!(
        crate::ModelLicenseControlVerifier::verify(
            &serde_json::to_vec(&foreign).expect("foreign canonical control"),
            &lease,
            ModelLicensePermission::LocalGeneration,
            &production_policy,
            &CancellationToken::new(),
        ),
        Err(ModelLicenseControlError::ReviewRequired)
    ));

    let mut altered = value(compiled.canonical_bytes());
    altered["evidence"]["review_evidence_digest"] = json!(Digest::sha256(b"altered-evidence"));
    assert!(matches!(
        crate::ModelLicenseControlVerifier::verify(
            &serde_json::to_vec(&altered).expect("altered canonical control"),
            &lease,
            ModelLicensePermission::LocalGeneration,
            &production_policy,
            &CancellationToken::new(),
        ),
        Err(ModelLicenseControlError::InvalidBinding)
    ));

    fixture.add_unexpected_member();
    assert!(matches!(
        crate::ModelLicenseControlVerifier::verify(
            compiled.canonical_bytes(),
            &lease,
            ModelLicensePermission::LocalGeneration,
            &production_policy,
            &CancellationToken::new(),
        ),
        Err(ModelLicenseControlError::Lease(_))
    ));
}

#[test]
fn invalid_untrusted_json_fails_before_retained_lease_revalidation() {
    let fixture = Fixture::new(false);
    let lease = fixture.verify();
    let review = reviewer_json_for_test(&lease, ModelLicensePermission::LocalGeneration);
    let compiled = compile(&lease, ModelLicensePermission::LocalGeneration, &review);
    fixture.add_unexpected_member();
    assert!(matches!(
        ModelLicenseControlCompiler::compile(
            &lease,
            ModelLicensePermission::LocalGeneration,
            b"{",
            &CancellationToken::new(),
        ),
        Err(ModelLicenseControlError::InvalidEncoding)
    ));
    assert!(matches!(
        ModelLicenseControlVerifier::verify(
            &vec![b' '; MAX_MODEL_LICENSE_CONTROL_JSON_BYTES + 1],
            &lease,
            ModelLicensePermission::LocalGeneration,
            &CancellationToken::new(),
        ),
        Err(ModelLicenseControlError::LimitExceeded)
    ));
    assert!(matches!(
        ModelLicenseControlCompiler::compile(
            &lease,
            ModelLicensePermission::LocalGeneration,
            &review,
            &CancellationToken::new(),
        ),
        Err(ModelLicenseControlError::Lease(_))
    ));
    assert!(matches!(
        ModelLicenseControlVerifier::verify(
            compiled.canonical_bytes(),
            &lease,
            ModelLicensePermission::LocalGeneration,
            &CancellationToken::new(),
        ),
        Err(ModelLicenseControlError::Lease(_))
    ));
}
