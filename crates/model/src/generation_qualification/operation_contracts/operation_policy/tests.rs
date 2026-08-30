use serde_json::Value;

use super::test_support::{self, ATTEMPT_COUNT, CANDIDATE_BYTES, OUTPUT_ENVELOPE_BYTES};
use super::*;
use crate::generation_qualification::CandidateOutputCeilingsV1;

mod limits;

#[test]
fn policy_round_trips_and_exposes_exact_relationships() {
    let fixture = test_support::fixture();
    let policy = &fixture.policy;
    assert_eq!(policy.schema_version(), 1);
    assert_eq!(
        policy.generation_qualification_plan_id(),
        fixture.plan.qualification_plan_id()
    );
    assert_eq!(
        policy.suite_manifest_id(),
        fixture.suite.suite_manifest_id()
    );
    assert_eq!(
        policy.target_generation_system_id(),
        fixture.systems[0].generation_system_id()
    );
    assert_eq!(
        policy.baseline_generation_system_id(),
        fixture.systems[1].generation_system_id()
    );
    assert_eq!(
        policy.limits(),
        test_support::operation_limits(ATTEMPT_COUNT)
    );
    assert_eq!(
        policy.decision_rule(),
        GenerationQualificationDecisionRuleV1::AllRequiredEvidencePasses
    );
    assert_eq!(
        policy.platform_assessment_policy_id(),
        &fixture.policy_input.platform_assessment_policy_id
    );
    assert_eq!(
        policy.required_license_permission(),
        GenerationQualificationLicensePermissionV1::LocalGeneration
    );
    assert_eq!(
        policy.license_assessment_policy_id(),
        &fixture.policy_input.license_assessment_policy_id
    );
    assert_eq!(
        policy.attempt_ledger_policy_digest(),
        &fixture.policy_input.attempt_ledger_policy_digest
    );
    assert_eq!(
        policy.repeatability_policy_digest(),
        &fixture.policy_input.repeatability_policy_digest
    );
    assert_eq!(
        policy.resource_policy_digest(),
        &fixture.policy_input.resource_policy_digest
    );
    assert_eq!(
        policy.human_adjudication_policy_digest(),
        &fixture.policy_input.human_adjudication_policy_digest
    );

    let bytes = serde_json::to_vec(policy).expect("policy JSON");
    assert_eq!(
        GenerationQualificationOperationPolicyV1::from_json_bytes(
            &bytes,
            fixture.relations(),
            &fixture.policy_input,
        )
        .expect("policy decode"),
        *policy
    );
    policy
        .validate_against(fixture.relations(), &fixture.policy_input)
        .expect("policy revalidation");
    assert_eq!(
        policy.operation_policy_id().digest().as_str(),
        "7d06f0158d1919d55f37e4041c06a2849187547da9176daf359943eaabbb2728"
    );
}

#[test]
fn decoder_is_strict_bounded_canonical_and_schema_first() {
    let fixture = test_support::fixture();
    assert_eq!(
        GenerationQualificationOperationPolicyV1::from_json_bytes(
            &vec![b' '; MAX_GENERATION_QUALIFICATION_OPERATION_POLICY_JSON_BYTES + 1],
            fixture.relations(),
            &fixture.policy_input,
        ),
        Err(GenerationQualificationOperationContractError::EncodedRecordTooLarge)
    );
    assert_eq!(
        GenerationQualificationOperationPolicyV1::from_json_bytes(
            b"{",
            fixture.relations(),
            &fixture.policy_input,
        ),
        Err(GenerationQualificationOperationContractError::InvalidEncoding)
    );
    let mut unknown = serde_json::to_value(&fixture.policy).expect("policy value");
    unknown["authority"] = Value::Bool(true);
    assert_eq!(
        decode_value(&unknown, &fixture),
        Err(GenerationQualificationOperationContractError::InvalidEncoding)
    );
    let bytes = serde_json::to_vec(&fixture.policy).expect("policy bytes");
    let text = String::from_utf8(bytes.clone()).expect("UTF-8 policy JSON");
    let duplicate = text.replacen('{', "{\"schema_version\":1,", 1);
    assert_eq!(
        GenerationQualificationOperationPolicyV1::from_json_bytes(
            duplicate.as_bytes(),
            fixture.relations(),
            &fixture.policy_input,
        ),
        Err(GenerationQualificationOperationContractError::InvalidEncoding)
    );
    let mut whitespace = vec![b' '];
    whitespace.extend(bytes);
    assert_eq!(
        GenerationQualificationOperationPolicyV1::from_json_bytes(
            &whitespace,
            fixture.relations(),
            &fixture.policy_input,
        ),
        Err(GenerationQualificationOperationContractError::NonCanonicalEncoding)
    );
    let mut future = serde_json::to_value(&fixture.policy).expect("policy value");
    future["schema_version"] = Value::from(2);
    future["generation_qualification_plan_id"] =
        Value::String(test_support::digest("foreign").as_str().to_owned());
    assert_eq!(
        decode_value(&future, &fixture),
        Err(GenerationQualificationOperationContractError::UnsupportedSchema)
    );
}

#[test]
fn decoder_rejects_every_substituted_identity_and_policy_digest() {
    let fixture = test_support::fixture();
    for field in [
        "generation_qualification_plan_id",
        "suite_manifest_id",
        "target_generation_system_id",
        "baseline_generation_system_id",
    ] {
        let mut value = serde_json::to_value(&fixture.policy).expect("policy value");
        value[field] = Value::String(test_support::digest(field).as_str().to_owned());
        assert_eq!(
            decode_value(&value, &fixture),
            Err(GenerationQualificationOperationContractError::ScopeMismatch),
            "field {field}"
        );
    }
    for field in [
        "platform_assessment_policy_id",
        "license_assessment_policy_id",
        "attempt_ledger_policy_digest",
        "repeatability_policy_digest",
        "resource_policy_digest",
        "human_adjudication_policy_digest",
    ] {
        let mut value = serde_json::to_value(&fixture.policy).expect("policy value");
        value[field] = Value::String(test_support::digest(field).as_str().to_owned());
        assert_eq!(
            decode_value(&value, &fixture),
            Err(GenerationQualificationOperationContractError::RelationshipMismatch),
            "field {field}"
        );
    }
    let mut value = serde_json::to_value(&fixture.policy).expect("policy value");
    value["limits"]["maximum_elapsed_milliseconds"] = Value::from(59_999);
    assert_eq!(
        decode_value(&value, &fixture),
        Err(GenerationQualificationOperationContractError::RelationshipMismatch)
    );
}

#[test]
fn relationships_reject_cross_system_state_and_incomplete_cross_product() {
    let fixture = test_support::fixture();
    let mut relations = fixture.relations();
    let foreign_state =
        crate::generation_qualification::generation_system::test_support::runtime_state(
            &fixture.system_fixture.runtime_build,
            "foreign",
        );
    relations.target_system.relations.effective_runtime_state = &foreign_state;
    assert_eq!(
        GenerationQualificationOperationPolicyV1::new(relations, fixture.policy_input.clone()),
        Err(GenerationQualificationOperationContractError::RelationshipMismatch)
    );

    let mut system_input = fixture.system_fixture.input();
    system_input.language_digest = test_support::digest("foreign language");
    let foreign_language_system =
        GenerationSystemRecordV1::new(fixture.system_fixture.relations(), system_input)
            .expect("foreign-language system");
    relations = fixture.relations();
    relations.baseline_system.generation_system = &foreign_language_system;
    assert_eq!(
        GenerationQualificationOperationPolicyV1::new(relations, fixture.policy_input.clone()),
        Err(GenerationQualificationOperationContractError::ScopeMismatch)
    );

    relations = fixture.relations();
    relations.baseline_system.generation_system = &fixture.systems[0];
    assert_eq!(
        GenerationQualificationOperationPolicyV1::new(relations, fixture.policy_input.clone()),
        Err(GenerationQualificationOperationContractError::ScopeMismatch)
    );

    relations = fixture.relations();
    relations.planned_attempts = &fixture.attempts[..fixture.attempts.len() - 1];
    assert_eq!(
        GenerationQualificationOperationPolicyV1::new(relations, fixture.policy_input.clone()),
        Err(GenerationQualificationOperationContractError::InvalidCount)
    );

    let mut duplicated = fixture.attempts.clone();
    duplicated[7] = duplicated[6].clone();
    relations = fixture.relations();
    relations.planned_attempts = &duplicated;
    assert_eq!(
        GenerationQualificationOperationPolicyV1::new(relations, fixture.policy_input.clone()),
        Err(GenerationQualificationOperationContractError::RelationshipMismatch)
    );

    duplicated = fixture.attempts.clone();
    duplicated[7] = PlannedCandidateAttemptV1::new(
        crate::generation_qualification::PlannedCandidateAttemptV1Relations {
            suite: &fixture.suite,
            case: &fixture.cases[1],
            cluster: &fixture.cluster,
            repetition: &fixture.repetitions[1],
            generation_system: &fixture.systems[0],
        },
        crate::generation_qualification::PlannedCandidateAttemptV1Input {
            attempt_ordinal: 7,
            declared_seed: 777,
            grounded_request_digest: test_support::digest("duplicate grounded request"),
            generation_request_binding_id:
                crate::generation_qualification::GenerationRequestBindingId::from_derived_digest(
                    test_support::digest("duplicate request binding"),
                ),
            candidate_output_contract_digest: test_support::digest("single candidate contract"),
            output_ceilings: CandidateOutputCeilingsV1::new(1, CANDIDATE_BYTES, CANDIDATE_BYTES)
                .expect("candidate ceilings"),
        },
    )
    .expect("duplicate combination attempt");
    let duplicate_plan = GenerationQualificationPlanV1::new(
        &fixture.suite,
        &fixture.repetitions,
        &fixture.systems,
        &duplicated,
        crate::generation_qualification::GenerationQualificationPlanV1Input {
            limits: fixture.plan.limits(),
            selection_policy_digest: fixture.plan.selection_policy_digest().clone(),
            failure_policy_digest: fixture.plan.failure_policy_digest().clone(),
        },
    )
    .expect("structurally valid duplicate-combination plan");
    relations = fixture.relations();
    relations.plan = &duplicate_plan;
    relations.planned_attempts = &duplicated;
    assert_eq!(
        GenerationQualificationOperationPolicyV1::new(relations, fixture.policy_input.clone()),
        Err(GenerationQualificationOperationContractError::DuplicateEntry)
    );
}

#[test]
fn policy_rejects_plan_failure_policy_and_common_limit_mismatches() {
    let fixture = test_support::fixture();
    let mut changed = fixture.policy_input.clone();
    changed.resource_policy_digest = test_support::digest("changed resource policy");
    assert_eq!(
        GenerationQualificationOperationPolicyV1::new(fixture.relations(), changed),
        Err(GenerationQualificationOperationContractError::InvalidDecisionRule)
    );

    let mut changed = fixture.policy_input.clone();
    changed.limits = GenerationQualificationOperationLimitsV1::new(
        1_024,
        4_096,
        8_193,
        1_024,
        OUTPUT_ENVELOPE_BYTES,
        1,
        CANDIDATE_BYTES,
        CANDIDATE_BYTES,
        ATTEMPT_COUNT,
        1,
        60_000,
    )
    .expect("hard-valid context");
    assert_eq!(
        GenerationQualificationOperationPolicyV1::new(fixture.relations(), changed),
        Err(GenerationQualificationOperationContractError::InvalidLimits)
    );

    let mut changed = fixture.policy_input.clone();
    changed.limits = GenerationQualificationOperationLimitsV1::new(
        1_024,
        4_096,
        4_096,
        1_024,
        3_356,
        1,
        512,
        512,
        ATTEMPT_COUNT,
        1,
        60_000,
    )
    .expect("narrow candidate limits");
    assert_eq!(
        GenerationQualificationOperationPolicyV1::new(fixture.relations(), changed),
        Err(GenerationQualificationOperationContractError::InvalidLimits)
    );

    let mut changed = fixture.policy_input.clone();
    changed.limits = test_support::operation_limits(ATTEMPT_COUNT - 1);
    assert_eq!(
        GenerationQualificationOperationPolicyV1::new(fixture.relations(), changed),
        Err(GenerationQualificationOperationContractError::InvalidLimits)
    );
}

#[test]
fn identities_are_domain_separated_and_debug_is_redacted() {
    let fixture = test_support::fixture();
    assert_eq!(
        GENERATION_QUALIFICATION_OPERATION_POLICY_ID_DOMAIN,
        b"retonr:generation-qualification-operation-policy:v1\0"
    );
    assert_eq!(
        GENERATION_QUALIFICATION_PLAN_FAILURE_POLICY_DOMAIN,
        b"retonr:generation-qualification-plan-failure-policy:v1\0"
    );
    let failure = generation_qualification_plan_failure_policy_digest(
        fixture.policy.decision_rule(),
        fixture.policy.platform_assessment_policy_id(),
        fixture.policy.required_license_permission(),
        fixture.policy.license_assessment_policy_id(),
        fixture.policy.attempt_ledger_policy_digest(),
        fixture.policy.repeatability_policy_digest(),
        fixture.policy.resource_policy_digest(),
        fixture.policy.human_adjudication_policy_digest(),
    );
    assert_eq!(failure, *fixture.plan.failure_policy_digest());
    assert_eq!(failure, fixture.policy.plan_failure_policy_digest());
    assert_eq!(
        failure.as_str(),
        "bf27fd5ae04ac0eed043f2bb18a85032cfec26dec00990838c907ee36938e9b0"
    );
    assert_ne!(failure, *fixture.policy.operation_policy_id().digest());

    let debug = format!("{:?}", fixture.policy);
    assert!(debug.contains(fixture.policy.operation_policy_id().digest().as_str()));
    for hidden in [
        fixture.policy.attempt_ledger_policy_digest(),
        fixture.policy.repeatability_policy_digest(),
        fixture.policy.resource_policy_digest(),
        fixture.policy.human_adjudication_policy_digest(),
    ] {
        assert!(!debug.contains(hidden.as_str()));
    }
}

fn decode_value(
    value: &Value,
    fixture: &test_support::Fixture,
) -> Result<GenerationQualificationOperationPolicyV1, GenerationQualificationOperationContractError>
{
    GenerationQualificationOperationPolicyV1::from_json_bytes(
        &serde_json::to_vec(value).expect("policy JSON"),
        fixture.relations(),
        &fixture.policy_input,
    )
}
