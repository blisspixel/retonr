use serde_json::{Value, json};

use super::*;
use crate::generation_qualification::operation_contracts::operation_policy::test_support::{
    self as policy_support, Fixture as PolicyFixture,
};
use crate::generation_qualification::{
    GenerationQualificationRequestProjectionEntryV1Input,
    GenerationQualificationRequestProjectionV1Relations, StructuredCompletionRequestBindingId,
};

struct Fixture {
    policy: PolicyFixture,
    projection: GenerationQualificationRequestProjectionV1,
    foundation_id: ModelPackageFoundationId,
    control_id: ModelLicenseControlId,
}

impl Fixture {
    fn relations(&self) -> GenerationQualificationLicenseEvidenceV1Relations<'_> {
        GenerationQualificationLicenseEvidenceV1Relations {
            operation_policy: &self.policy.policy,
            request_projection: &self.projection,
            target_generation_system: &self.policy.systems[0],
            target_generation_system_relations: self.policy.system_fixture.relations(),
            model_package_foundation_id: &self.foundation_id,
            model_license_control_id: &self.control_id,
        }
    }

    fn approved(&self) -> GenerationQualificationLicenseEvidenceV1 {
        GenerationQualificationLicenseEvidenceV1::new(self.relations(), approved_input())
            .expect("approved license evidence")
    }
}

const fn approved_input() -> GenerationQualificationLicenseEvidenceV1Input {
    GenerationQualificationLicenseEvidenceV1Input {
        decision: GenerationQualificationLicenseDecisionV1::LocalUseOnly,
        reason: GenerationQualificationLicenseReasonV1::ApprovedLocalGeneration,
    }
}

fn fixture() -> Fixture {
    let policy = policy_support::fixture();
    let projection_inputs = policy
        .attempts
        .iter()
        .enumerate()
        .map(
            |(index, attempt)| GenerationQualificationRequestProjectionEntryV1Input {
                structured_completion_request_binding_id:
                    StructuredCompletionRequestBindingId::from_derived_digest(
                        policy_support::digest(&format!("structured request {index}")),
                    ),
                complete_input_byte_count: attempt.source_byte_count() + 32,
                context_token_limit: policy.policy.limits().maximum_context_tokens(),
                output_token_limit: policy.policy.limits().maximum_output_tokens(),
            },
        )
        .collect::<Vec<_>>();
    let projection = GenerationQualificationRequestProjectionV1::new(
        GenerationQualificationRequestProjectionV1Relations {
            operation_policy: &policy.policy,
            qualification_plan: &policy.plan,
            suite: &policy.suite,
            planned_attempts: &policy.attempts,
        },
        &projection_inputs,
    )
    .expect("request projection");
    Fixture {
        policy,
        projection,
        foundation_id: ModelPackageFoundationId::from_derived_digest(policy_support::digest(
            "model package foundation",
        )),
        control_id: ModelLicenseControlId::from_derived_digest(policy_support::digest(
            "model license control",
        )),
    }
}

fn compact(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).expect("JSON encodes")
}

#[test]
fn approved_record_derives_every_package_policy_and_permission_fact() {
    let fixture = fixture();
    let value = fixture.approved();
    let target = &fixture.policy.systems[0];
    assert_eq!(value.schema_version(), 1);
    assert_eq!(
        value.operation_policy_id(),
        fixture.policy.policy.operation_policy_id()
    );
    assert_eq!(
        value.request_projection_id(),
        fixture.projection.request_projection_id()
    );
    assert_eq!(
        value.target_generation_system_id(),
        target.generation_system_id()
    );
    assert_eq!(
        value.model_artifact_set_id(),
        target.model_artifact_set_id()
    );
    assert_eq!(
        value.model_package_manifest_id(),
        target.model_package_manifest_id()
    );
    assert_eq!(value.model_artifact_id(), target.model_artifact_id());
    assert_eq!(value.model_package_foundation_id(), &fixture.foundation_id);
    assert_eq!(value.model_license_control_id(), &fixture.control_id);
    assert_eq!(
        value.license_assessment_policy_id(),
        fixture.policy.policy.license_assessment_policy_id()
    );
    assert_eq!(
        value.permission(),
        GenerationQualificationLicensePermissionV1::LocalGeneration
    );
    assert_eq!(
        value.decision(),
        GenerationQualificationLicenseDecisionV1::LocalUseOnly
    );
    assert_eq!(
        value.reason(),
        GenerationQualificationLicenseReasonV1::ApprovedLocalGeneration
    );
    value
        .validate_against(fixture.relations(), approved_input())
        .expect("fresh typed closure remains valid");

    let bytes = serde_json::to_vec(&value).expect("license evidence encodes");
    assert_eq!(
        GenerationQualificationLicenseEvidenceV1::from_json_bytes(
            &bytes,
            fixture.relations(),
            approved_input(),
        )
        .expect("license evidence decodes"),
        value
    );
    let encoded = String::from_utf8(bytes).expect("UTF-8 JSON");
    let ordered_fields = [
        "schema_version",
        "operation_policy_id",
        "request_projection_id",
        "target_generation_system_id",
        "model_artifact_set_id",
        "model_package_manifest_id",
        "model_artifact_id",
        "model_package_foundation_id",
        "model_license_control_id",
        "license_assessment_policy_id",
        "permission",
        "decision",
        "reason",
    ];
    let mut previous = 0;
    for field in ordered_fields {
        let position = encoded.find(field).expect("field is encoded");
        assert!(position >= previous, "field order changed at {field}");
        previous = position;
    }
    for forbidden in [
        "assessment_digest",
        "installation_generation",
        "redistribution",
    ] {
        assert!(!encoded.contains(forbidden));
    }
}

#[test]
fn decision_reason_matrix_is_closed_and_wire_cannot_self_promote() {
    let fixture = fixture();
    let approved = fixture.approved();
    let rejected_input = GenerationQualificationLicenseEvidenceV1Input {
        decision: GenerationQualificationLicenseDecisionV1::Rejected,
        reason: GenerationQualificationLicenseReasonV1::ApprovalPolicyDenied,
    };
    let rejected =
        GenerationQualificationLicenseEvidenceV1::new(fixture.relations(), rejected_input)
            .expect("policy-denied evidence");
    assert_ne!(
        approved.license_evidence_id(),
        rejected.license_evidence_id()
    );
    assert_eq!(
        GenerationQualificationLicenseEvidenceV1::from_json_bytes(
            &serde_json::to_vec(&rejected).expect("rejection encodes"),
            fixture.relations(),
            rejected_input,
        )
        .expect("rejection decodes"),
        rejected
    );

    for invalid in [
        GenerationQualificationLicenseEvidenceV1Input {
            decision: GenerationQualificationLicenseDecisionV1::LocalUseOnly,
            reason: GenerationQualificationLicenseReasonV1::ApprovalPolicyDenied,
        },
        GenerationQualificationLicenseEvidenceV1Input {
            decision: GenerationQualificationLicenseDecisionV1::Rejected,
            reason: GenerationQualificationLicenseReasonV1::ApprovedLocalGeneration,
        },
    ] {
        assert_eq!(
            GenerationQualificationLicenseEvidenceV1::new(fixture.relations(), invalid)
                .expect_err("invalid decision and reason pair fails"),
            GenerationQualificationOperationContractError::InvalidLicenseClosure
        );
    }

    let mut promoted = serde_json::to_value(&rejected).expect("record value");
    promoted["decision"] = json!("local_use_only");
    promoted["reason"] = json!("approved_local_generation");
    assert_eq!(
        GenerationQualificationLicenseEvidenceV1::from_json_bytes(
            &compact(&promoted),
            fixture.relations(),
            rejected_input,
        )
        .expect_err("wire cannot promote an independently rejected assessment"),
        GenerationQualificationOperationContractError::RelationshipMismatch
    );
}

#[test]
fn every_relationship_field_is_rederived_during_decode() {
    let fixture = fixture();
    let value = fixture.approved();
    let original = serde_json::to_value(&value).expect("record value");
    let digest_fields = [
        "operation_policy_id",
        "request_projection_id",
        "target_generation_system_id",
        "model_artifact_set_id",
        "model_package_manifest_id",
        "model_artifact_id",
        "model_package_foundation_id",
        "model_license_control_id",
        "license_assessment_policy_id",
    ];
    for field in digest_fields {
        let mut changed = original.clone();
        changed[field] = json!(policy_support::digest(&format!("foreign {field}")));
        assert_eq!(
            GenerationQualificationLicenseEvidenceV1::from_json_bytes(
                &compact(&changed),
                fixture.relations(),
                approved_input(),
            )
            .expect_err("substituted relationship must fail"),
            GenerationQualificationOperationContractError::RelationshipMismatch,
            "field {field}"
        );
    }

    let mut permission = original;
    permission["permission"] = json!("redistribution");
    assert_eq!(
        GenerationQualificationLicenseEvidenceV1::from_json_bytes(
            &compact(&permission),
            fixture.relations(),
            approved_input(),
        )
        .expect_err("unrecognized permission fails"),
        GenerationQualificationOperationContractError::InvalidEncoding
    );
}

#[test]
fn decoder_is_bounded_strict_and_canonical() {
    let fixture = fixture();
    let value = fixture.approved();
    let bytes = serde_json::to_vec(&value).expect("record encodes");

    let oversized = vec![b' '; MAX_GENERATION_QUALIFICATION_LICENSE_EVIDENCE_JSON_BYTES + 1];
    assert_eq!(
        GenerationQualificationLicenseEvidenceV1::from_json_bytes(
            &oversized,
            fixture.relations(),
            approved_input(),
        )
        .expect_err("oversized input fails before decoding"),
        GenerationQualificationOperationContractError::EncodedRecordTooLarge
    );

    let mut unknown = serde_json::to_value(&value).expect("record value");
    unknown["assessment_digest"] = json!(policy_support::digest("forbidden"));
    assert_eq!(
        GenerationQualificationLicenseEvidenceV1::from_json_bytes(
            &compact(&unknown),
            fixture.relations(),
            approved_input(),
        )
        .expect_err("unknown field fails"),
        GenerationQualificationOperationContractError::InvalidEncoding
    );

    let whitespace = [b" ".as_slice(), bytes.as_slice()].concat();
    assert_eq!(
        GenerationQualificationLicenseEvidenceV1::from_json_bytes(
            &whitespace,
            fixture.relations(),
            approved_input(),
        )
        .expect_err("noncanonical whitespace fails"),
        GenerationQualificationOperationContractError::NonCanonicalEncoding
    );

    let text = String::from_utf8(bytes).expect("UTF-8 JSON");
    let duplicate = text.replacen('{', "{\"schema_version\":1,", 1);
    assert_eq!(
        GenerationQualificationLicenseEvidenceV1::from_json_bytes(
            duplicate.as_bytes(),
            fixture.relations(),
            approved_input(),
        )
        .expect_err("duplicate field fails"),
        GenerationQualificationOperationContractError::InvalidEncoding
    );

    let mut schema = serde_json::to_value(&value).expect("record value");
    schema["schema_version"] = json!(2);
    assert_eq!(
        GenerationQualificationLicenseEvidenceV1::from_json_bytes(
            &compact(&schema),
            fixture.relations(),
            approved_input(),
        )
        .expect_err("foreign schema fails"),
        GenerationQualificationOperationContractError::UnsupportedSchema
    );
}

#[test]
fn scope_transitive_target_and_typed_trust_ids_are_revalidated() {
    let fixture = fixture();
    let value = fixture.approved();
    let baseline_relations = GenerationQualificationLicenseEvidenceV1Relations {
        operation_policy: &fixture.policy.policy,
        request_projection: &fixture.projection,
        target_generation_system: &fixture.policy.systems[1],
        target_generation_system_relations: fixture.policy.system_fixture.relations(),
        model_package_foundation_id: &fixture.foundation_id,
        model_license_control_id: &fixture.control_id,
    };
    assert_eq!(
        value
            .validate_against(baseline_relations, approved_input())
            .expect_err("baseline cannot replace target"),
        GenerationQualificationOperationContractError::ScopeMismatch
    );

    let foreign_runtime =
        crate::generation_qualification::generation_system::test_support::other_runtime_package();
    let mut foreign_system_relations = fixture.policy.system_fixture.relations();
    foreign_system_relations.runtime_package_manifest = &foreign_runtime;
    let foreign_closure = GenerationQualificationLicenseEvidenceV1Relations {
        operation_policy: &fixture.policy.policy,
        request_projection: &fixture.projection,
        target_generation_system: &fixture.policy.systems[0],
        target_generation_system_relations: foreign_system_relations,
        model_package_foundation_id: &fixture.foundation_id,
        model_license_control_id: &fixture.control_id,
    };
    assert_eq!(
        value
            .validate_against(foreign_closure, approved_input())
            .expect_err("foreign runtime closure fails"),
        GenerationQualificationOperationContractError::RelationshipMismatch
    );

    let foreign_foundation =
        ModelPackageFoundationId::from_derived_digest(policy_support::digest("foreign foundation"));
    let foreign_control =
        ModelLicenseControlId::from_derived_digest(policy_support::digest("foreign control"));
    for relations in [
        GenerationQualificationLicenseEvidenceV1Relations {
            model_package_foundation_id: &foreign_foundation,
            ..fixture.relations()
        },
        GenerationQualificationLicenseEvidenceV1Relations {
            model_license_control_id: &foreign_control,
            ..fixture.relations()
        },
    ] {
        assert_eq!(
            value
                .validate_against(relations, approved_input())
                .expect_err("foreign trust identity fails"),
            GenerationQualificationOperationContractError::RelationshipMismatch
        );
    }
}

#[test]
fn identity_domain_and_debug_surface_are_stable_and_redacted() {
    let fixture = fixture();
    let value = fixture.approved();
    assert_eq!(
        GENERATION_QUALIFICATION_LICENSE_EVIDENCE_ID_DOMAIN,
        b"retonr:generation-qualification-license-evidence:v1\0"
    );
    assert_eq!(
        value.license_evidence_id().digest().as_str(),
        "e2e44a4159c9b77734b7192880b50040afe3f65682d8e51eba52b71bea749940"
    );
    let debug = format!("{value:?}");
    assert!(debug.contains("GenerationQualificationLicenseEvidenceV1"));
    assert!(debug.contains("LocalUseOnly"));
    for hidden in [
        value.model_artifact_id().digest().as_str(),
        value.model_package_foundation_id().digest().as_str(),
        value.model_license_control_id().digest().as_str(),
    ] {
        assert!(!debug.contains(hidden));
    }
}
