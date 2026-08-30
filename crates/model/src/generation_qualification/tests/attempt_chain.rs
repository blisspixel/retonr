use serde_json::Value;

use super::*;

fn planned_input(ordinal: u32) -> PlannedCandidateAttemptV1Input {
    PlannedCandidateAttemptV1Input {
        attempt_ordinal: ordinal,
        declared_seed: u64::from(ordinal) + 41,
        grounded_request_digest: digest("grounded request"),
        generation_request_binding_id: GenerationRequestBindingId::from_derived_digest(digest(
            "provider-neutral request",
        )),
        candidate_output_contract_digest: digest("candidate contract"),
        output_ceilings: CandidateOutputCeilingsV1::new(2, 1_024, 2_048).expect("candidate policy"),
    }
}

fn relations(fixture: &Fixture) -> PlannedCandidateAttemptV1Relations<'_> {
    PlannedCandidateAttemptV1Relations {
        suite: &fixture.suite,
        case: &fixture.cases[0],
        cluster: &fixture.cluster,
        repetition: &fixture.repetition,
        generation_system: &fixture.systems[0],
    }
}

fn plan_input(limits: GenerationQualificationPlanLimitsV1) -> GenerationQualificationPlanV1Input {
    GenerationQualificationPlanV1Input {
        limits,
        selection_policy_digest: digest("selection"),
        failure_policy_digest: digest("failure"),
    }
}

#[test]
fn planned_attempt_round_trips_and_exposes_exact_derived_facts() {
    let fixture = fixture();
    let attempt = PlannedCandidateAttemptV1::new(relations(&fixture), planned_input(0))
        .expect("planned attempt");
    assert_eq!(attempt.schema_version(), 1);
    assert_eq!(
        attempt.suite_manifest_id(),
        fixture.suite.suite_manifest_id()
    );
    assert_eq!(attempt.case_id(), fixture.cases[0].case_id());
    assert_eq!(attempt.cluster_id(), fixture.cluster.cluster_id());
    assert_eq!(attempt.repetition_id(), fixture.repetition.repetition_id());
    assert_eq!(attempt.attempt_ordinal(), 0);
    assert_eq!(attempt.declared_seed(), 41);
    assert_eq!(
        attempt.generation_system_id(),
        fixture.systems[0].generation_system_id()
    );
    assert_eq!(
        attempt.source_artifact_id(),
        fixture.cases[0].source_artifact_id()
    );
    assert_eq!(attempt.source_digest(), fixture.cases[0].source_digest());
    assert_eq!(
        attempt.source_byte_count(),
        fixture.cases[0].source_byte_count()
    );
    assert_eq!(
        attempt.case_contract_digest(),
        fixture.cases[0].case_contract_digest()
    );
    assert_eq!(
        attempt.grounded_request_digest(),
        &digest("grounded request")
    );
    assert_eq!(
        attempt.generation_request_binding_id().digest(),
        &digest("provider-neutral request")
    );
    assert_eq!(
        attempt.candidate_output_contract_digest(),
        &digest("candidate contract")
    );
    assert_eq!(attempt.output_ceilings().candidate_count(), 2);
    assert_eq!(attempt.output_ceilings().maximum_candidate_bytes(), 1_024);
    assert_eq!(
        attempt
            .output_ceilings()
            .maximum_aggregate_candidate_bytes(),
        2_048
    );
    assert_eq!(attempt.output_ceilings().maximum_envelope_bytes(), 12_584);

    let encoded = serde_json::to_vec(&attempt).expect("attempt JSON");
    assert_eq!(
        PlannedCandidateAttemptV1::from_json_bytes(&encoded, relations(&fixture))
            .expect("attempt decode"),
        attempt
    );
    assert_eq!(
        attempt.planned_attempt_id().digest().as_str(),
        "46921ad462a380f3402e9c65b8cefa38d73bb737d6dd135eac660cf5888b7af2"
    );
    let debug = format!("{attempt:?}");
    assert!(debug.contains(attempt.planned_attempt_id().digest().as_str()));
    assert!(!debug.contains(attempt.source_digest().as_str()));
    assert!(!debug.contains(attempt.grounded_request_digest().as_str()));
    attempt
        .validate_against(relations(&fixture))
        .expect("attempt revalidates");
}

#[test]
fn candidate_policy_derives_envelope_and_rejects_invalid_or_injected_values() {
    let ceilings = CandidateOutputCeilingsV1::new(1, 1_024, 1_024).expect("valid ceilings");
    let mut direct_wire = serde_json::to_value(ceilings).expect("ceilings value");
    direct_wire["maximum_envelope_bytes"] = Value::from(1_u64);
    assert!(serde_json::from_value::<CandidateOutputCeilingsV1>(direct_wire).is_err());

    assert_eq!(
        CandidateOutputCeilingsV1::new(16, 10, 160)
            .expect("maximum count")
            .maximum_envelope_bytes(),
        1_424
    );
    for result in [
        CandidateOutputCeilingsV1::new(0, 1, 1),
        CandidateOutputCeilingsV1::new(17, 1, 1),
        CandidateOutputCeilingsV1::new(1, 0, 1),
        CandidateOutputCeilingsV1::new(1, 1, 0),
        CandidateOutputCeilingsV1::new(1, 1, 2),
        CandidateOutputCeilingsV1::new(16, u64::MAX, u64::MAX),
    ] {
        assert_eq!(
            result,
            Err(GenerationQualificationContractError::InvalidCandidateOutputPolicy)
        );
    }

    let fixture = fixture();
    let attempt = fixture.attempts[0].clone();
    let mut wire = serde_json::to_value(&attempt).expect("attempt value");
    wire["maximum_envelope_bytes"] =
        Value::from(attempt.output_ceilings().maximum_envelope_bytes() + 1);
    assert_eq!(
        PlannedCandidateAttemptV1::from_json_bytes(
            &serde_json::to_vec(&wire).expect("changed envelope"),
            PlannedCandidateAttemptV1Relations {
                suite: &fixture.suite,
                case: &fixture.cases[0],
                cluster: &fixture.cluster,
                repetition: &fixture.repetition,
                generation_system: &fixture.systems[0],
            },
        ),
        Err(GenerationQualificationContractError::InvalidCandidateOutputPolicy)
    );
}

#[test]
fn planned_attempt_rejects_manifest_system_ordinal_and_source_substitution() {
    let fixture = fixture();
    let other_cluster =
        GenerationClusterRecordV1::new("other", digest("other cluster")).expect("other cluster");
    let mut changed = relations(&fixture);
    changed.cluster = &other_cluster;
    assert_eq!(
        PlannedCandidateAttemptV1::new(changed, planned_input(0)),
        Err(GenerationQualificationContractError::ClusterMismatch)
    );

    let other_case = case(&fixture.cluster, "other-case", "other source");
    changed = relations(&fixture);
    changed.case = &other_case;
    assert_eq!(
        PlannedCandidateAttemptV1::new(changed, planned_input(0)),
        Err(GenerationQualificationContractError::PlannedCaseMismatch)
    );

    let changed_repetition =
        GenerationRepetitionRecordV1::new(&fixture.suite, 1, digest("other repetition"))
            .expect("other repetition");
    changed = relations(&fixture);
    changed.repetition = &changed_repetition;
    assert!(PlannedCandidateAttemptV1::new(changed, planned_input(0)).is_ok());

    let system_fixture = super::super::generation_system::test_support::fixture(false);
    let mut system_input = system_fixture.input();
    system_input.language_digest = digest("different language");
    let other_system = GenerationSystemRecordV1::new(system_fixture.relations(), system_input)
        .expect("other system");
    changed = relations(&fixture);
    changed.generation_system = &other_system;
    assert_eq!(
        PlannedCandidateAttemptV1::new(changed, planned_input(0)),
        Err(GenerationQualificationContractError::GenerationSystemMismatch)
    );

    assert_eq!(
        PlannedCandidateAttemptV1::new(
            relations(&fixture),
            planned_input(u32::try_from(MAX_PLANNED_GENERATION_ATTEMPTS).expect("maximum")),
        ),
        Err(GenerationQualificationContractError::AttemptOrdinalMismatch)
    );

    let mut wire = serde_json::to_value(&fixture.attempts[0]).expect("attempt value");
    wire["source_digest"] = Value::String(digest("substituted source").as_str().to_owned());
    assert_eq!(
        PlannedCandidateAttemptV1::from_json_bytes(
            &serde_json::to_vec(&wire).expect("changed source"),
            relations(&fixture),
        ),
        Err(GenerationQualificationContractError::PlannedAttemptMismatch)
    );
}

#[test]
fn planned_attempt_decoder_is_bounded_canonical_and_future_schema_first() {
    let fixture = fixture();
    let attempt = &fixture.attempts[0];
    assert_eq!(
        PlannedCandidateAttemptV1::from_json_bytes(
            &vec![b' '; MAX_PLANNED_CANDIDATE_ATTEMPT_JSON_BYTES + 1],
            relations(&fixture),
        ),
        Err(GenerationQualificationContractError::EncodedRecordTooLarge)
    );
    assert_eq!(
        PlannedCandidateAttemptV1::from_json_bytes(b"{", relations(&fixture)),
        Err(GenerationQualificationContractError::InvalidEncoding)
    );
    let mut unknown = serde_json::to_value(attempt).expect("attempt value");
    unknown["authority"] = Value::Bool(true);
    assert_eq!(
        PlannedCandidateAttemptV1::from_json_bytes(
            &serde_json::to_vec(&unknown).expect("unknown JSON"),
            relations(&fixture),
        ),
        Err(GenerationQualificationContractError::InvalidEncoding)
    );
    let mut whitespace = vec![b' '];
    whitespace.extend(serde_json::to_vec(attempt).expect("attempt JSON"));
    assert_eq!(
        PlannedCandidateAttemptV1::from_json_bytes(&whitespace, relations(&fixture)),
        Err(GenerationQualificationContractError::NonCanonicalEncoding)
    );
    let mut future = serde_json::to_value(attempt).expect("attempt value");
    future["schema_version"] = Value::from(2);
    future["source_digest"] = Value::String(digest("future source").as_str().to_owned());
    assert_eq!(
        PlannedCandidateAttemptV1::from_json_bytes(
            &serde_json::to_vec(&future).expect("future JSON"),
            relations(&fixture),
        ),
        Err(GenerationQualificationContractError::UnsupportedSchema(2))
    );
}

#[test]
fn plan_enforces_repetition_closure_and_contiguous_ordinals() {
    let fixture = fixture();
    assert_eq!(
        GenerationQualificationPlanV1::new(
            &fixture.suite,
            &[],
            &fixture.systems,
            &fixture.attempts,
            plan_input(limits(4)),
        ),
        Err(GenerationQualificationContractError::InvalidCollectionSize)
    );

    let repetition_one =
        GenerationRepetitionRecordV1::new(&fixture.suite, 1, digest("repetition one"))
            .expect("repetition one");
    assert_eq!(
        GenerationQualificationPlanV1::new(
            &fixture.suite,
            &[fixture.repetition.clone(), repetition_one.clone()],
            &fixture.systems,
            &fixture.attempts,
            plan_input(limits(4)),
        ),
        Err(GenerationQualificationContractError::RepetitionMismatch)
    );
    assert_eq!(
        GenerationQualificationPlanV1::new(
            &fixture.suite,
            &[repetition_one.clone(), fixture.repetition.clone()],
            &fixture.systems,
            &fixture.attempts,
            plan_input(limits(4)),
        ),
        Err(GenerationQualificationContractError::RepetitionMismatch)
    );
    let repetition_two =
        GenerationRepetitionRecordV1::new(&fixture.suite, 2, digest("repetition two"))
            .expect("repetition two");
    assert_eq!(
        GenerationQualificationPlanV1::new(
            &fixture.suite,
            &[fixture.repetition.clone(), repetition_two],
            &fixture.systems,
            &fixture.attempts,
            plan_input(limits(4)),
        ),
        Err(GenerationQualificationContractError::RepetitionMismatch)
    );

    let other_case = case(&fixture.cluster, "foreign", "foreign source");
    let other_suite = GenerationSuiteManifestV1::new(digest("foreign suite"), &[other_case])
        .expect("other suite");
    let foreign = GenerationRepetitionRecordV1::new(&other_suite, 0, digest("foreign repetition"))
        .expect("foreign repetition");
    assert_eq!(
        GenerationQualificationPlanV1::new(
            &fixture.suite,
            &[foreign],
            &fixture.systems,
            &fixture.attempts,
            plan_input(limits(4)),
        ),
        Err(GenerationQualificationContractError::RepetitionMismatch)
    );
}

#[test]
fn plan_enforces_source_and_output_boundaries_at_equal_and_one_over() {
    let fixture = fixture();
    let output = fixture.attempts[0].output_ceilings();
    let exact = GenerationQualificationPlanLimitsV1::new(
        2,
        4,
        fixture.cases[0].source_byte_count(),
        16,
        64,
        output.maximum_envelope_bytes(),
    )
    .expect("exact plan bounds");
    GenerationQualificationPlanV1::new(
        &fixture.suite,
        &fixture.repetitions,
        &fixture.systems,
        &fixture.attempts,
        plan_input(exact),
    )
    .expect("equal limits");

    for limits in [
        GenerationQualificationPlanLimitsV1::new(
            2,
            4,
            fixture.cases[0].source_byte_count() - 1,
            16,
            64,
            output.maximum_envelope_bytes(),
        )
        .expect("source one below"),
        GenerationQualificationPlanLimitsV1::new(
            2,
            4,
            fixture.cases[0].source_byte_count(),
            16,
            64,
            output.maximum_envelope_bytes() - 1,
        )
        .expect("bundle one below"),
    ] {
        assert_eq!(
            GenerationQualificationPlanV1::new(
                &fixture.suite,
                &fixture.repetitions,
                &fixture.systems,
                &fixture.attempts,
                plan_input(limits),
            ),
            Err(GenerationQualificationContractError::InvalidLimits)
        );
    }
}

#[path = "attempt_chain/precursor.rs"]
mod precursor;

#[test]
fn attempt_chain_types_publish_machine_readable_schemas() {
    for schema in [
        schemars::schema_for!(GenerationQualificationPlanV1),
        schemars::schema_for!(PlannedCandidateAttemptV1),
        schemars::schema_for!(CandidateGenerationAttemptPrecursorV1),
    ] {
        let value = serde_json::to_value(schema).expect("schema JSON");
        assert_eq!(
            value["$schema"],
            "https://json-schema.org/draft/2020-12/schema"
        );
    }
}
