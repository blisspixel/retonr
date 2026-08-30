use serde_json::{Value, json};

use super::*;
use crate::generation_qualification::operation_contracts::operation_policy::test_support::{
    self as policy_support, Fixture as PolicyFixture,
};
use crate::generation_qualification::{
    GenerationQualificationRequestProjectionEntryV1Input,
    GenerationQualificationRequestProjectionV1Relations, StructuredCompletionRequestBindingId,
};
use crate::{RuntimeAbi, RuntimeArchitecture, RuntimeOperatingSystem, RuntimeTarget};

#[path = "tests/runtime_target.rs"]
mod runtime_target;

struct Fixture {
    policy: PolicyFixture,
    projection: GenerationQualificationRequestProjectionV1,
}

impl Fixture {
    fn relations(&self) -> GenerationQualificationPlatformEvidenceV1Relations<'_> {
        GenerationQualificationPlatformEvidenceV1Relations {
            operation_policy: &self.policy.policy,
            request_projection: &self.projection,
            target_generation_system: &self.policy.systems[0],
            target_generation_system_relations: self.policy.system_fixture.relations(),
        }
    }

    fn supported(&self) -> GenerationQualificationPlatformEvidenceV1 {
        GenerationQualificationPlatformEvidenceV1::new(self.relations(), supported_input())
            .expect("supported platform evidence")
    }
}

const fn supported_input() -> GenerationQualificationPlatformEvidenceV1Input {
    GenerationQualificationPlatformEvidenceV1Input {
        status: GenerationQualificationPlatformStatusV1::Supported,
        reason: GenerationQualificationPlatformReasonV1::ReviewedManagedLinuxNativeCpu,
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
    Fixture { policy, projection }
}

fn compact(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).expect("JSON encodes")
}

#[test]
fn supported_record_derives_every_system_and_policy_fact() {
    let fixture = fixture();
    let value = fixture.supported();
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
        value.runtime_target(),
        fixture.policy.system_fixture.runtime_package.target()
    );
    assert_eq!(
        value.operating_system_digest(),
        target.operating_system_digest()
    );
    assert_eq!(value.architecture_digest(), target.architecture_digest());
    assert_eq!(
        value.execution_class_digest(),
        target.execution_class_digest()
    );
    assert_eq!(
        value.hardware_envelope_digest(),
        target.hardware_envelope_digest()
    );
    assert_eq!(
        value.runtime_admission_join_id(),
        target.runtime_admission_join_id()
    );
    assert_eq!(
        value.managed_generation_path_id(),
        target.managed_generation_path_id()
    );
    assert_eq!(
        value.frozen_external_component_set_id(),
        target.frozen_external_component_set_id()
    );
    assert_eq!(
        value.platform_assessment_policy_id(),
        fixture.policy.policy.platform_assessment_policy_id()
    );
    assert_eq!(
        value.status(),
        GenerationQualificationPlatformStatusV1::Supported
    );
    assert_eq!(
        value.reason(),
        GenerationQualificationPlatformReasonV1::ReviewedManagedLinuxNativeCpu
    );
    value
        .validate_against(fixture.relations(), supported_input())
        .expect("fresh typed closure remains valid");

    let bytes = serde_json::to_vec(&value).expect("platform evidence encodes");
    assert_eq!(
        GenerationQualificationPlatformEvidenceV1::from_json_bytes(
            &bytes,
            fixture.relations(),
            supported_input(),
        )
        .expect("platform evidence decodes"),
        value
    );
    let encoded = String::from_utf8(bytes).expect("UTF-8 JSON");
    let ordered_fields = [
        "schema_version",
        "operation_policy_id",
        "request_projection_id",
        "target_generation_system_id",
        "runtime_target",
        "operating_system_digest",
        "architecture_digest",
        "execution_class_digest",
        "hardware_envelope_digest",
        "runtime_admission_join_id",
        "managed_generation_path_id",
        "frozen_external_component_set_id",
        "platform_assessment_policy_id",
        "status",
        "reason",
    ];
    let mut previous = 0;
    for field in ordered_fields {
        let position = encoded.find(field).expect("field is encoded");
        assert!(position >= previous, "field order changed at {field}");
        previous = position;
    }
    assert!(!encoded.contains("assessment_digest"));
}

#[test]
fn status_reason_matrix_is_closed_and_identity_sensitive() {
    let fixture = fixture();
    let supported = fixture.supported();
    let compatible_rejection_reasons = [
        GenerationQualificationPlatformReasonV1::UnsupportedExecutionClass,
        GenerationQualificationPlatformReasonV1::UnsupportedHardwareEnvelope,
        GenerationQualificationPlatformReasonV1::AssessmentPolicyDenied,
    ];
    for reason in compatible_rejection_reasons {
        let rejected = GenerationQualificationPlatformEvidenceV1::new(
            fixture.relations(),
            GenerationQualificationPlatformEvidenceV1Input {
                status: GenerationQualificationPlatformStatusV1::Rejected,
                reason,
            },
        )
        .expect("closed rejection reason");
        assert_ne!(
            rejected.platform_evidence_id(),
            supported.platform_evidence_id()
        );
        assert_eq!(rejected.reason(), reason);
        assert_eq!(
            GenerationQualificationPlatformEvidenceV1::from_json_bytes(
                &serde_json::to_vec(&rejected).expect("encode rejection"),
                fixture.relations(),
                GenerationQualificationPlatformEvidenceV1Input {
                    status: GenerationQualificationPlatformStatusV1::Rejected,
                    reason,
                },
            )
            .expect("decode rejection"),
            rejected
        );

        assert_eq!(
            GenerationQualificationPlatformEvidenceV1::new(
                fixture.relations(),
                GenerationQualificationPlatformEvidenceV1Input {
                    status: GenerationQualificationPlatformStatusV1::Supported,
                    reason,
                },
            )
            .expect_err("supported status cannot use a rejection reason"),
            GenerationQualificationOperationContractError::InvalidPlatformClosure
        );
    }
    assert_eq!(
        GenerationQualificationPlatformEvidenceV1::new(
            fixture.relations(),
            GenerationQualificationPlatformEvidenceV1Input {
                status: GenerationQualificationPlatformStatusV1::Rejected,
                reason: GenerationQualificationPlatformReasonV1::ReviewedManagedLinuxNativeCpu,
            },
        )
        .expect_err("rejection cannot use the reviewed supported reason"),
        GenerationQualificationOperationContractError::InvalidPlatformClosure
    );

    for reason in [
        GenerationQualificationPlatformReasonV1::UnsupportedOperatingSystem,
        GenerationQualificationPlatformReasonV1::UnsupportedArchitecture,
        GenerationQualificationPlatformReasonV1::UnsupportedAbi,
    ] {
        assert_eq!(
            GenerationQualificationPlatformEvidenceV1::new(
                fixture.relations(),
                GenerationQualificationPlatformEvidenceV1Input {
                    status: GenerationQualificationPlatformStatusV1::Rejected,
                    reason,
                },
            )
            .expect_err("reason must match the exact runtime target"),
            GenerationQualificationOperationContractError::InvalidPlatformClosure
        );
    }

    let mut promoted = serde_json::to_value(
        GenerationQualificationPlatformEvidenceV1::new(
            fixture.relations(),
            GenerationQualificationPlatformEvidenceV1Input {
                status: GenerationQualificationPlatformStatusV1::Rejected,
                reason: GenerationQualificationPlatformReasonV1::AssessmentPolicyDenied,
            },
        )
        .expect("rejected record"),
    )
    .expect("record value");
    promoted["status"] = json!("supported");
    promoted["reason"] = json!("reviewed_managed_linux_native_cpu");
    assert_eq!(
        GenerationQualificationPlatformEvidenceV1::from_json_bytes(
            &compact(&promoted),
            fixture.relations(),
            GenerationQualificationPlatformEvidenceV1Input {
                status: GenerationQualificationPlatformStatusV1::Rejected,
                reason: GenerationQualificationPlatformReasonV1::AssessmentPolicyDenied,
            },
        )
        .expect_err("wire cannot promote an independently rejected assessment"),
        GenerationQualificationOperationContractError::RelationshipMismatch
    );
}

#[test]
fn every_relationship_field_is_rederived_during_decode() {
    let fixture = fixture();
    let value = fixture.supported();
    let original = serde_json::to_value(&value).expect("record value");
    let digest_fields = [
        "operation_policy_id",
        "request_projection_id",
        "target_generation_system_id",
        "operating_system_digest",
        "architecture_digest",
        "execution_class_digest",
        "hardware_envelope_digest",
        "runtime_admission_join_id",
        "managed_generation_path_id",
        "frozen_external_component_set_id",
        "platform_assessment_policy_id",
    ];
    for field in digest_fields {
        let mut changed = original.clone();
        changed[field] = json!(policy_support::digest(&format!("foreign {field}")));
        assert_eq!(
            GenerationQualificationPlatformEvidenceV1::from_json_bytes(
                &compact(&changed),
                fixture.relations(),
                supported_input(),
            )
            .expect_err("substituted relationship must fail"),
            GenerationQualificationOperationContractError::RelationshipMismatch,
            "field {field}"
        );
    }

    let mut target = original;
    target["runtime_target"] = json!({
        "operating_system": "windows",
        "architecture": "aarch64",
        "abi": "windows_msvc"
    });
    assert_eq!(
        GenerationQualificationPlatformEvidenceV1::from_json_bytes(
            &compact(&target),
            fixture.relations(),
            supported_input(),
        )
        .expect_err("substituted runtime target must fail"),
        GenerationQualificationOperationContractError::RelationshipMismatch
    );
}

#[test]
fn decoder_is_bounded_strict_and_canonical() {
    let fixture = fixture();
    let value = fixture.supported();
    let bytes = serde_json::to_vec(&value).expect("record encodes");

    let oversized = vec![b' '; MAX_GENERATION_QUALIFICATION_PLATFORM_EVIDENCE_JSON_BYTES + 1];
    assert_eq!(
        GenerationQualificationPlatformEvidenceV1::from_json_bytes(
            &oversized,
            fixture.relations(),
            supported_input(),
        )
        .expect_err("oversized input fails before decoding"),
        GenerationQualificationOperationContractError::EncodedRecordTooLarge
    );

    let mut unknown = serde_json::to_value(&value).expect("record value");
    unknown["assessment_digest"] = json!(policy_support::digest("forbidden"));
    assert_eq!(
        GenerationQualificationPlatformEvidenceV1::from_json_bytes(
            &compact(&unknown),
            fixture.relations(),
            supported_input(),
        )
        .expect_err("unknown field fails"),
        GenerationQualificationOperationContractError::InvalidEncoding
    );

    let whitespace = [b" ".as_slice(), bytes.as_slice()].concat();
    assert_eq!(
        GenerationQualificationPlatformEvidenceV1::from_json_bytes(
            &whitespace,
            fixture.relations(),
            supported_input(),
        )
        .expect_err("noncanonical whitespace fails"),
        GenerationQualificationOperationContractError::NonCanonicalEncoding
    );

    let text = String::from_utf8(bytes).expect("UTF-8 JSON");
    let duplicate = text.replacen('{', "{\"schema_version\":1,", 1);
    assert_eq!(
        GenerationQualificationPlatformEvidenceV1::from_json_bytes(
            duplicate.as_bytes(),
            fixture.relations(),
            supported_input(),
        )
        .expect_err("duplicate field fails"),
        GenerationQualificationOperationContractError::InvalidEncoding
    );

    let mut schema = serde_json::to_value(&value).expect("record value");
    schema["schema_version"] = json!(2);
    assert_eq!(
        GenerationQualificationPlatformEvidenceV1::from_json_bytes(
            &compact(&schema),
            fixture.relations(),
            supported_input(),
        )
        .expect_err("foreign schema fails"),
        GenerationQualificationOperationContractError::UnsupportedSchema
    );
}

#[test]
fn scope_and_transitive_target_closure_are_revalidated() {
    let gate_fixture = fixture();
    let value = gate_fixture.supported();
    let baseline_relations = GenerationQualificationPlatformEvidenceV1Relations {
        operation_policy: &gate_fixture.policy.policy,
        request_projection: &gate_fixture.projection,
        target_generation_system: &gate_fixture.policy.systems[1],
        target_generation_system_relations: gate_fixture.policy.system_fixture.relations(),
    };
    assert_eq!(
        value
            .validate_against(baseline_relations, supported_input())
            .expect_err("baseline cannot replace target"),
        GenerationQualificationOperationContractError::ScopeMismatch
    );

    let foreign_runtime =
        crate::generation_qualification::generation_system::test_support::other_runtime_package();
    let mut foreign_system_relations = gate_fixture.policy.system_fixture.relations();
    foreign_system_relations.runtime_package_manifest = &foreign_runtime;
    let foreign_closure = GenerationQualificationPlatformEvidenceV1Relations {
        operation_policy: &gate_fixture.policy.policy,
        request_projection: &gate_fixture.projection,
        target_generation_system: &gate_fixture.policy.systems[0],
        target_generation_system_relations: foreign_system_relations,
    };
    assert_eq!(
        value
            .validate_against(foreign_closure, supported_input())
            .expect_err("foreign runtime closure fails"),
        GenerationQualificationOperationContractError::RelationshipMismatch
    );
}

#[test]
fn identity_domain_and_debug_surface_are_stable_and_redacted() {
    let fixture = fixture();
    let value = fixture.supported();
    assert_eq!(
        GENERATION_QUALIFICATION_PLATFORM_EVIDENCE_ID_DOMAIN,
        b"retonr:generation-qualification-platform-evidence:v1\0"
    );
    assert_eq!(
        value.platform_evidence_id().digest().as_str(),
        "71680b83c6a118cc23062ef5a25a04a59b601bc26b4be877a902d38ada8b651f"
    );
    let debug = format!("{value:?}");
    assert!(debug.contains("GenerationQualificationPlatformEvidenceV1"));
    assert!(debug.contains("Supported"));
    for hidden in [
        value.operating_system_digest().as_str(),
        value.architecture_digest().as_str(),
        value.execution_class_digest().as_str(),
        value.hardware_envelope_digest().as_str(),
    ] {
        assert!(!debug.contains(hidden));
    }
}
