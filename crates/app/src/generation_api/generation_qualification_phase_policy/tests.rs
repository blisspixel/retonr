use super::verification::{
    human_policy_digest_for_test, human_policy_json_for_test, resource_policy_digest_for_test,
    resource_policy_json_for_test, verify_human_structure, verify_resource_structure,
};
use super::*;

mod adversarial;
#[cfg(feature = "test-support")]
mod operation_binding;

pub(super) fn resource_limits() -> GenerationQualificationResourcePolicyLimitsV1 {
    GenerationQualificationResourcePolicyLimitsV1 {
        maximum_attempt_elapsed_nanoseconds: 30_000_000_000,
        maximum_first_response_nanoseconds: 5_000_000_000,
        maximum_cleanup_nanoseconds: 2_000_000_000,
        maximum_worker_high_water_resident_bytes: 17_179_869_184,
        maximum_installed_footprint_bytes: 34_359_738_368,
    }
}

#[test]
fn exact_resource_vector_is_stable_and_production_is_source_denied() {
    let json = resource_policy_json_for_test(resource_limits());
    assert_eq!(
        json,
        concat!(
            "{\"authority\":\"none\",",
            "\"decision_rule\":\"all_complete_target_observations_within_declared_limits\",",
            "\"procedure_id\":\"retonr:generation-qualification-resource-policy:procedure\",",
            "\"procedure_version\":1,",
            "\"measurement_profile\":\"managed_local_generation_v1\",",
            "\"maximum_attempt_elapsed_nanoseconds\":30000000000,",
            "\"maximum_first_response_nanoseconds\":5000000000,",
            "\"maximum_cleanup_nanoseconds\":2000000000,",
            "\"maximum_worker_high_water_resident_bytes\":17179869184,",
            "\"maximum_installed_footprint_bytes\":34359738368,",
            "\"required_provider_observations\":[",
            "\"prompt_token_count\",\"generated_token_count\",",
            "\"total_duration_nanoseconds\",\"load_duration_nanoseconds\",",
            "\"prompt_evaluation_duration_nanoseconds\",",
            "\"evaluation_duration_nanoseconds\"],",
            "\"schema_version\":1}"
        )
        .as_bytes()
    );
    let digest = resource_policy_digest_for_test(&json);
    assert_eq!(
        digest.as_str(),
        "01ed4005f8c5001b4b5513d1e174e040e7cd8a41e43002c583aff02f77c9d310"
    );

    let production = ProductionGenerationQualificationResourcePolicySource::new();
    assert_eq!(production.approved_policy_count(), 0);
    let denied = verify_resource_structure(&json, &production).expect("structural resource policy");
    assert_eq!(
        denied.source_disposition(),
        GenerationQualificationPhasePolicySourceDisposition::Denied
    );
    assert_eq!(denied.policy_digest(), &digest);
    assert_eq!(denied.limits(), resource_limits());

    let approved_source =
        ProductionGenerationQualificationResourcePolicySource::exact_test_source(digest);
    let approved =
        verify_resource_structure(&json, &approved_source).expect("test-approved resource policy");
    assert_eq!(approved_source.approved_policy_count(), 1);
    assert_eq!(
        approved.source_disposition(),
        GenerationQualificationPhasePolicySourceDisposition::Approved
    );
}

#[test]
fn exact_human_vector_is_stable_and_production_is_source_denied() {
    let json = human_policy_json_for_test(7_627_468_237_172_680_543);
    assert_eq!(
        json,
        concat!(
            "{\"authority\":\"none\",",
            "\"decision_rule\":",
            "\"two_independent_blinded_reviews_then_role_separated_adjudication\",",
            "\"procedure_id\":",
            "\"retonr:generation-qualification-human-adjudication-policy:procedure\",",
            "\"procedure_version\":1,",
            "\"presentation_rule\":\"deterministic_blinded_candidate_pair_v1\",",
            "\"presentation_seed\":7627468237172680543,",
            "\"eligible_case_rule\":\"all_cases_in_all_passed_repetitions\",",
            "\"primary_reviewer_count\":2,",
            "\"require_distinct_primary_reviewers\":true,",
            "\"require_role_separated_adjudicator\":true,",
            "\"adjudication_trigger\":\"disagreement_tie_or_abstention\",",
            "\"allowed_outcomes\":[\"acceptable\",\"unacceptable\",\"abstain\"],",
            "\"schema_version\":1}"
        )
        .as_bytes()
    );
    let digest = human_policy_digest_for_test(&json);
    assert_eq!(
        digest.as_str(),
        "79d434e04a9e1753a31b8b60db540d33f22c7491d38ca63a8e9ece6ec6368821"
    );

    let production = ProductionGenerationQualificationHumanAdjudicationPolicySource::new();
    assert_eq!(production.approved_policy_count(), 0);
    let denied = verify_human_structure(&json, &production).expect("structural human policy");
    assert_eq!(
        denied.source_disposition(),
        GenerationQualificationPhasePolicySourceDisposition::Denied
    );
    assert_eq!(denied.policy_digest(), &digest);
    assert_eq!(denied.presentation_seed(), 7_627_468_237_172_680_543);
    assert_eq!(denied.primary_reviewer_count(), 2);

    let approved_source =
        ProductionGenerationQualificationHumanAdjudicationPolicySource::exact_test_source(digest);
    let approved =
        verify_human_structure(&json, &approved_source).expect("test-approved human policy");
    assert_eq!(approved_source.approved_policy_count(), 1);
    assert_eq!(
        approved.source_disposition(),
        GenerationQualificationPhasePolicySourceDisposition::Approved
    );
}

#[test]
fn resource_and_human_digests_are_domain_separated() {
    let bytes = b"same canonical bytes";
    assert_ne!(
        resource_policy_digest_for_test(bytes),
        human_policy_digest_for_test(bytes)
    );
}

#[test]
fn debug_and_errors_are_content_redacted() {
    let resource_json = resource_policy_json_for_test(resource_limits());
    let resource_digest = resource_policy_digest_for_test(&resource_json);
    let resource = verify_resource_structure(
        &resource_json,
        &ProductionGenerationQualificationResourcePolicySource::new(),
    )
    .expect("resource authority");
    let human_json = human_policy_json_for_test(91);
    let human_digest = human_policy_digest_for_test(&human_json);
    let human = verify_human_structure(
        &human_json,
        &ProductionGenerationQualificationHumanAdjudicationPolicySource::new(),
    )
    .expect("human authority");

    for debug in [
        format!("{resource:?}"),
        format!("{human:?}"),
        format!(
            "{:?}",
            ProductionGenerationQualificationResourcePolicySource::exact_test_source(
                resource_digest.clone(),
            )
        ),
        format!(
            "{:?}",
            ProductionGenerationQualificationHumanAdjudicationPolicySource::exact_test_source(
                human_digest.clone(),
            )
        ),
    ] {
        assert!(!debug.contains(resource_digest.as_str()));
        assert!(!debug.contains(human_digest.as_str()));
        assert!(!debug.contains("7627468237172680543"));
    }
    for error in [
        GenerationQualificationPhasePolicyError::InvalidEncoding,
        GenerationQualificationPhasePolicyError::NonCanonicalEncoding,
        GenerationQualificationPhasePolicyError::LimitExceeded,
        GenerationQualificationPhasePolicyError::InvalidResourceBinding,
        GenerationQualificationPhasePolicyError::InvalidHumanAdjudicationBinding,
        GenerationQualificationPhasePolicyError::OperationPolicyMismatch,
    ] {
        assert!(!error.to_string().contains(resource_digest.as_str()));
        assert!(!format!("{error:?}").contains(human_digest.as_str()));
    }
}

#[test]
fn positive_roots_are_cfg_test_only_and_test_support_is_not_an_approval_path() {
    let source = include_str!("../generation_qualification_phase_policy.rs");
    assert!(!source.contains("feature = \"test-support\""));
    assert!(source.contains("#[cfg(test)]\n    pub(crate) fn exact_test_source"));
    assert_eq!(PRODUCTION_RESOURCE_POLICY_APPROVALS, &[] as &[&str]);
    assert_eq!(
        PRODUCTION_HUMAN_ADJUDICATION_POLICY_APPROVALS,
        &[] as &[&str]
    );
}
