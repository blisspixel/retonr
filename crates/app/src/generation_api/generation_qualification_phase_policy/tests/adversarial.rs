use super::*;
use crate::generation_api::generation_qualification_phase_policy::wire::{
    ALLOWED_OUTCOMES, REQUIRED_PROVIDER_OBSERVATIONS, encode_human, encode_resource,
    expected_human, expected_resource,
};

#[test]
fn malformed_unknown_missing_duplicate_trailing_and_reordered_json_is_rejected() {
    let resource = resource_policy_json_for_test(resource_limits());
    let unknown = replace_once(
        &resource,
        b"{\"authority\":\"none\",",
        b"{\"unknown\":1,\"authority\":\"none\",",
    );
    let missing = replace_once(&resource, b"\"procedure_version\":1,", b"");
    let duplicate = replace_once(
        &resource,
        b"{\"authority\":\"none\",",
        b"{\"authority\":\"none\",\"authority\":\"none\",",
    );
    for invalid in [unknown, missing, duplicate, b"{".to_vec()] {
        assert_error(
            verify_resource_structure(
                &invalid,
                &ProductionGenerationQualificationResourcePolicySource::new(),
            ),
            GenerationQualificationPhasePolicyError::InvalidEncoding,
        );
    }

    let mut trailing = resource.clone();
    trailing.push(b' ');
    assert_error(
        verify_resource_structure(
            &trailing,
            &ProductionGenerationQualificationResourcePolicySource::new(),
        ),
        GenerationQualificationPhasePolicyError::NonCanonicalEncoding,
    );
    let reordered = replace_once(
        &resource,
        b"{\"authority\":\"none\",\"decision_rule\":",
        b"{\"decision_rule\":",
    );
    let reordered = replace_once(
        &reordered,
        b"\"all_complete_target_observations_within_declared_limits\",",
        b"\"all_complete_target_observations_within_declared_limits\",\"authority\":\"none\",",
    );
    assert_error(
        verify_resource_structure(
            &reordered,
            &ProductionGenerationQualificationResourcePolicySource::new(),
        ),
        GenerationQualificationPhasePolicyError::NonCanonicalEncoding,
    );
}

#[test]
fn oversized_and_float_numbers_are_rejected_before_authority() {
    let oversized = vec![b' '; MAX_GENERATION_QUALIFICATION_PHASE_POLICY_JSON_BYTES + 1];
    assert_error(
        verify_human_structure(
            &oversized,
            &ProductionGenerationQualificationHumanAdjudicationPolicySource::new(),
        ),
        GenerationQualificationPhasePolicyError::LimitExceeded,
    );
    let human = human_policy_json_for_test(7);
    let float = replace_once(
        &human,
        b"\"presentation_seed\":7,",
        b"\"presentation_seed\":7.0,",
    );
    assert_error(
        verify_human_structure(
            &float,
            &ProductionGenerationQualificationHumanAdjudicationPolicySource::new(),
        ),
        GenerationQualificationPhasePolicyError::InvalidEncoding,
    );
    let resource = resource_policy_json_for_test(resource_limits());
    let float = replace_once(
        &resource,
        b"\"maximum_cleanup_nanoseconds\":2000000000,",
        b"\"maximum_cleanup_nanoseconds\":2000000000.0,",
    );
    assert_error(
        verify_resource_structure(
            &float,
            &ProductionGenerationQualificationResourcePolicySource::new(),
        ),
        GenerationQualificationPhasePolicyError::InvalidEncoding,
    );
}

#[test]
fn every_resource_ceiling_must_be_nonzero() {
    for ordinal in 0..5 {
        let mut limits = resource_limits();
        match ordinal {
            0 => limits.maximum_attempt_elapsed_nanoseconds = 0,
            1 => limits.maximum_first_response_nanoseconds = 0,
            2 => limits.maximum_cleanup_nanoseconds = 0,
            3 => limits.maximum_worker_high_water_resident_bytes = 0,
            4 => limits.maximum_installed_footprint_bytes = 0,
            _ => unreachable!(),
        }
        let bytes = resource_policy_json_for_test(limits);
        assert_error(
            verify_resource_structure(
                &bytes,
                &ProductionGenerationQualificationResourcePolicySource::new(),
            ),
            GenerationQualificationPhasePolicyError::InvalidResourceBinding,
        );
    }
}

#[test]
fn provider_observation_array_is_exact_complete_unique_and_ordered() {
    let mut missing = expected_resource(resource_limits());
    missing.required_provider_observations.pop();
    let mut duplicate = expected_resource(resource_limits());
    duplicate.required_provider_observations[5] = duplicate.required_provider_observations[0];
    let mut reordered = expected_resource(resource_limits());
    reordered.required_provider_observations.swap(0, 1);
    let mut extra = expected_resource(resource_limits());
    extra
        .required_provider_observations
        .push(REQUIRED_PROVIDER_OBSERVATIONS[0]);

    for policy in [missing, duplicate, reordered, extra] {
        let bytes = encode_resource(&policy).expect("bounded adversarial resource policy");
        assert_error(
            verify_resource_structure(
                &bytes,
                &ProductionGenerationQualificationResourcePolicySource::new(),
            ),
            GenerationQualificationPhasePolicyError::InvalidResourceBinding,
        );
    }
}

#[test]
fn human_constants_and_outcomes_are_exact() {
    let mut count = expected_human(7);
    count.primary_reviewer_count = 3;
    let mut distinct = expected_human(7);
    distinct.require_distinct_primary_reviewers = false;
    let mut separated = expected_human(7);
    separated.require_role_separated_adjudicator = false;
    let mut missing = expected_human(7);
    missing.allowed_outcomes.pop();
    let mut duplicate = expected_human(7);
    duplicate.allowed_outcomes[2] = duplicate.allowed_outcomes[0];
    let mut reordered = expected_human(7);
    reordered.allowed_outcomes.swap(0, 1);
    let mut extra = expected_human(7);
    extra.allowed_outcomes.push(ALLOWED_OUTCOMES[0]);

    for policy in [
        count, distinct, separated, missing, duplicate, reordered, extra,
    ] {
        let bytes = encode_human(&policy).expect("bounded adversarial human policy");
        assert_error(
            verify_human_structure(
                &bytes,
                &ProductionGenerationQualificationHumanAdjudicationPolicySource::new(),
            ),
            GenerationQualificationPhasePolicyError::InvalidHumanAdjudicationBinding,
        );
    }
}

#[test]
fn procedure_schema_and_closed_rules_are_exact() {
    let mut resource_procedure = expected_resource(resource_limits());
    resource_procedure.procedure_id.push_str(":substituted");
    let mut resource_version = expected_resource(resource_limits());
    resource_version.procedure_version = 2;
    let mut resource_schema = expected_resource(resource_limits());
    resource_schema.schema_version = 2;
    for policy in [resource_procedure, resource_version, resource_schema] {
        let bytes = encode_resource(&policy).expect("bounded resource substitution");
        assert_error(
            verify_resource_structure(
                &bytes,
                &ProductionGenerationQualificationResourcePolicySource::new(),
            ),
            GenerationQualificationPhasePolicyError::InvalidResourceBinding,
        );
    }

    let mut human_procedure = expected_human(7);
    human_procedure.procedure_id.push_str(":substituted");
    let mut human_version = expected_human(7);
    human_version.procedure_version = 2;
    let mut human_schema = expected_human(7);
    human_schema.schema_version = 2;
    for policy in [human_procedure, human_version, human_schema] {
        let bytes = encode_human(&policy).expect("bounded human substitution");
        assert_error(
            verify_human_structure(
                &bytes,
                &ProductionGenerationQualificationHumanAdjudicationPolicySource::new(),
            ),
            GenerationQualificationPhasePolicyError::InvalidHumanAdjudicationBinding,
        );
    }

    let resource = resource_policy_json_for_test(resource_limits());
    for invalid in [
        replace_once(
            &resource,
            b"all_complete_target_observations_within_declared_limits",
            b"caller_selected_rule",
        ),
        replace_once(
            &resource,
            b"managed_local_generation_v1",
            b"caller_selected_profile",
        ),
    ] {
        assert_error(
            verify_resource_structure(
                &invalid,
                &ProductionGenerationQualificationResourcePolicySource::new(),
            ),
            GenerationQualificationPhasePolicyError::InvalidEncoding,
        );
    }

    let human = human_policy_json_for_test(7);
    for invalid in [
        replace_once(
            &human,
            b"two_independent_blinded_reviews_then_role_separated_adjudication",
            b"caller_selected_rule",
        ),
        replace_once(
            &human,
            b"deterministic_blinded_candidate_pair_v1",
            b"caller_selected_presentation",
        ),
        replace_once(
            &human,
            b"all_cases_in_all_passed_repetitions",
            b"caller_selected_cases",
        ),
        replace_once(
            &human,
            b"disagreement_tie_or_abstention",
            b"caller_selected_trigger",
        ),
    ] {
        assert_error(
            verify_human_structure(
                &invalid,
                &ProductionGenerationQualificationHumanAdjudicationPolicySource::new(),
            ),
            GenerationQualificationPhasePolicyError::InvalidEncoding,
        );
    }
}

fn replace_once(input: &[u8], needle: &[u8], replacement: &[u8]) -> Vec<u8> {
    let start = input
        .windows(needle.len())
        .position(|window| window == needle)
        .expect("test needle");
    let mut output = Vec::with_capacity(input.len() - needle.len() + replacement.len());
    output.extend_from_slice(&input[..start]);
    output.extend_from_slice(replacement);
    output.extend_from_slice(&input[start + needle.len()..]);
    output
}

fn assert_error<T>(
    result: Result<T, GenerationQualificationPhasePolicyError>,
    expected: GenerationQualificationPhasePolicyError,
) {
    assert_eq!(result.err(), Some(expected));
}
