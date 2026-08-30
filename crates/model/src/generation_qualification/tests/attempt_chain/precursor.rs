use serde_json::Value;

use super::*;

fn precursor_input(
    runtime_installation_generation: u64,
    model_installation_generation: u64,
    request: &str,
) -> CandidateGenerationAttemptPrecursorV1Input {
    CandidateGenerationAttemptPrecursorV1Input {
        runtime_installation_generation,
        model_installation_generation,
        structured_request_binding_id: StructuredCompletionRequestBindingId::from_derived_digest(
            digest(request),
        ),
    }
}

#[test]
fn precursor_round_trips_and_exposes_exact_relationships() {
    let fixture = fixture();
    let precursor = CandidateGenerationAttemptPrecursorV1::new(
        &fixture.plan,
        &fixture.attempts[0],
        &fixture.systems[0],
        precursor_input(7, 11, "wire request"),
    )
    .expect("precursor");
    assert_eq!(precursor.schema_version(), 1);
    assert_eq!(
        precursor.qualification_plan_id(),
        fixture.plan.qualification_plan_id()
    );
    assert_eq!(
        precursor.planned_attempt_id(),
        fixture.attempts[0].planned_attempt_id()
    );
    assert_eq!(precursor.runtime_installation_generation(), 7);
    assert_eq!(precursor.model_installation_generation(), 11);
    assert_eq!(
        precursor.runtime_admission_join_id(),
        fixture.systems[0].runtime_admission_join_id()
    );
    assert_eq!(
        precursor.managed_generation_path_id(),
        fixture.systems[0].managed_generation_path_id()
    );
    assert_eq!(
        precursor.frozen_external_component_set_id(),
        fixture.systems[0].frozen_external_component_set_id()
    );
    assert_eq!(
        precursor.runtime_package_manifest_id(),
        fixture.systems[0].runtime_package_manifest_id()
    );
    assert_eq!(
        precursor.runtime_build_id(),
        fixture.systems[0].runtime_build_id()
    );
    assert_eq!(
        precursor.expected_effective_runtime_state_id(),
        fixture.systems[0].effective_runtime_state_id()
    );
    assert_eq!(
        precursor.model_artifact_set_id(),
        fixture.systems[0].model_artifact_set_id()
    );
    assert_eq!(
        precursor.model_package_manifest_id(),
        fixture.systems[0].model_package_manifest_id()
    );
    assert_eq!(
        precursor.model_artifact_id(),
        fixture.systems[0].model_artifact_id()
    );
    assert_eq!(
        precursor.effective_package_evidence_v2_id(),
        fixture.systems[0].effective_package_evidence_v2_id()
    );
    assert_eq!(
        precursor.static_model_binding_digest(),
        fixture.systems[0].static_model_binding_digest()
    );
    assert_eq!(
        precursor.structured_request_binding_id().digest(),
        &digest("wire request")
    );
    assert_eq!(
        precursor.precursor_id().digest().as_str(),
        "ebc07b27615f7b1bc8b81ce5edd605b2e34446ef3c89718777f925f01c056f0a"
    );

    let encoded = serde_json::to_vec(&precursor).expect("precursor JSON");
    assert_eq!(
        CandidateGenerationAttemptPrecursorV1::from_json_bytes(
            &encoded,
            &fixture.plan,
            &fixture.attempts[0],
            &fixture.systems[0],
        )
        .expect("precursor decode"),
        precursor
    );
    precursor
        .validate_against(&fixture.plan, &fixture.attempts[0], &fixture.systems[0])
        .expect("precursor revalidates");
}

#[test]
fn precursor_rejects_installation_and_system_substitution() {
    let fixture = fixture();
    for (runtime, model) in [(0, 1), (1, 0)] {
        assert_eq!(
            CandidateGenerationAttemptPrecursorV1::new(
                &fixture.plan,
                &fixture.attempts[0],
                &fixture.systems[0],
                precursor_input(runtime, model, "wire request"),
            ),
            Err(GenerationQualificationContractError::InvalidInstallationGeneration)
        );
    }
    assert_eq!(
        CandidateGenerationAttemptPrecursorV1::new(
            &fixture.plan,
            &fixture.attempts[0],
            &fixture.systems[1],
            precursor_input(7, 11, "wire request"),
        ),
        Err(GenerationQualificationContractError::GenerationSystemMismatch)
    );
}

#[test]
fn precursor_decoder_is_bounded_strict_and_debug_redacted() {
    let fixture = fixture();
    let precursor = CandidateGenerationAttemptPrecursorV1::new(
        &fixture.plan,
        &fixture.attempts[0],
        &fixture.systems[0],
        precursor_input(1, 1, "wire"),
    )
    .expect("precursor");
    assert_eq!(
        CandidateGenerationAttemptPrecursorV1::from_json_bytes(
            &vec![b' '; MAX_CANDIDATE_GENERATION_ATTEMPT_PRECURSOR_JSON_BYTES + 1],
            &fixture.plan,
            &fixture.attempts[0],
            &fixture.systems[0],
        ),
        Err(GenerationQualificationContractError::EncodedRecordTooLarge)
    );
    let mut wire = serde_json::to_value(&precursor).expect("precursor value");
    wire["schema_version"] = Value::from(2);
    wire["planned_attempt_id"] = Value::String(digest("wrong attempt").as_str().to_owned());
    assert_eq!(
        CandidateGenerationAttemptPrecursorV1::from_json_bytes(
            &serde_json::to_vec(&wire).expect("future JSON"),
            &fixture.plan,
            &fixture.attempts[0],
            &fixture.systems[0],
        ),
        Err(GenerationQualificationContractError::UnsupportedSchema(2))
    );
    assert_eq!(
        CandidateGenerationAttemptPrecursorV1::from_json_bytes(
            b"{",
            &fixture.plan,
            &fixture.attempts[0],
            &fixture.systems[0],
        ),
        Err(GenerationQualificationContractError::InvalidEncoding)
    );
    let mut unknown = serde_json::to_value(&precursor).expect("precursor value");
    unknown["authority"] = Value::Bool(true);
    assert_eq!(
        CandidateGenerationAttemptPrecursorV1::from_json_bytes(
            &serde_json::to_vec(&unknown).expect("unknown JSON"),
            &fixture.plan,
            &fixture.attempts[0],
            &fixture.systems[0],
        ),
        Err(GenerationQualificationContractError::InvalidEncoding)
    );
    let mut whitespace = vec![b' '];
    whitespace.extend(serde_json::to_vec(&precursor).expect("precursor JSON"));
    assert_eq!(
        CandidateGenerationAttemptPrecursorV1::from_json_bytes(
            &whitespace,
            &fixture.plan,
            &fixture.attempts[0],
            &fixture.systems[0],
        ),
        Err(GenerationQualificationContractError::NonCanonicalEncoding)
    );
    let mut substituted = serde_json::to_value(&precursor).expect("precursor value");
    substituted["structured_request_binding_id"] =
        Value::String(digest("swapped request").as_str().to_owned());
    assert_eq!(
        CandidateGenerationAttemptPrecursorV1::from_json_bytes(
            &serde_json::to_vec(&substituted).expect("substituted JSON"),
            &fixture.plan,
            &fixture.attempts[0],
            &fixture.systems[0],
        ),
        Err(GenerationQualificationContractError::NonCanonicalEncoding)
    );
    let debug = format!("{precursor:?}");
    assert!(debug.contains(precursor.precursor_id().digest().as_str()));
    assert!(!debug.contains(precursor.model_artifact_id().digest().as_str()));
    assert!(!debug.contains(precursor.structured_request_binding_id().digest().as_str()));
}

#[test]
fn attempt_and_precursor_ids_separate_requests_attempts_and_installations() {
    let fixture = fixture();
    let baseline_attempt = PlannedCandidateAttemptV1::new(relations(&fixture), planned_input(0))
        .expect("baseline attempt");
    let mut changed_request = planned_input(0);
    changed_request.generation_request_binding_id =
        GenerationRequestBindingId::from_derived_digest(digest("changed request"));
    let request_attempt = PlannedCandidateAttemptV1::new(relations(&fixture), changed_request)
        .expect("changed request attempt");
    assert_ne!(
        request_attempt.planned_attempt_id(),
        baseline_attempt.planned_attempt_id()
    );

    let baseline = CandidateGenerationAttemptPrecursorV1::new(
        &fixture.plan,
        &fixture.attempts[0],
        &fixture.systems[0],
        precursor_input(1, 1, "wire"),
    )
    .expect("baseline precursor");
    for variant in [
        CandidateGenerationAttemptPrecursorV1::new(
            &fixture.plan,
            &fixture.attempts[0],
            &fixture.systems[0],
            precursor_input(2, 1, "wire"),
        )
        .expect("runtime reinstall"),
        CandidateGenerationAttemptPrecursorV1::new(
            &fixture.plan,
            &fixture.attempts[0],
            &fixture.systems[0],
            precursor_input(1, 2, "wire"),
        )
        .expect("model reinstall"),
        CandidateGenerationAttemptPrecursorV1::new(
            &fixture.plan,
            &fixture.attempts[0],
            &fixture.systems[0],
            precursor_input(1, 1, "other wire"),
        )
        .expect("wire request swap"),
        CandidateGenerationAttemptPrecursorV1::new(
            &fixture.plan,
            &fixture.attempts[1],
            &fixture.systems[1],
            precursor_input(1, 1, "wire"),
        )
        .expect("cross-attempt precursor"),
    ] {
        assert_ne!(variant.precursor_id(), baseline.precursor_id());
    }
}
