use super::*;

pub(crate) fn portable_and_policy_substitution_cases() {
    exact_portable_clone_is_accepted();
    changed_portable_state_is_rejected();
    wrong_policy_authorities_are_rejected();
    record_and_binding_substitutions_are_rejected();
    mismatched_selected_attempt_reaches_model_contract();
    characterized_model_substitution_is_rejected();
}

fn exact_portable_clone_is_accepted() {
    let mut fixture = Fixture::new();
    let launch_plan = launch(&fixture.model_lease, "v1");
    let qualification = QualificationFixture::new(&fixture, &launch_plan);
    let cloned_state = qualification.expected_runtime_state.clone();
    let mut input = compilation_input(&mut fixture.runtime, &qualification, launch_plan);
    input.expected_runtime_state = &cloned_state;
    CandidateAttemptPrecursorCompiler::compile(input, &CancellationToken::new())
        .expect("byte-for-byte equal portable state is valid");
}

fn changed_portable_state_is_rejected() {
    let mut fixture = Fixture::new();
    let launch_plan = launch(&fixture.model_lease, "v1");
    let qualification = QualificationFixture::new(&fixture, &launch_plan);
    let changed = changed_state(&fixture.runtime);
    let mut input = compilation_input(&mut fixture.runtime, &qualification, launch_plan);
    input.expected_runtime_state = &changed;
    assert_relationship_error(&CandidateAttemptPrecursorCompiler::compile(
        input,
        &CancellationToken::new(),
    ));
}

fn wrong_policy_authorities_are_rejected() {
    for (permission, purpose) in [
        (
            GenerationSystemPolicyPermission::ValidateGenerationSystem,
            GenerationSystemPolicyPurpose::ManagedCandidateGeneration,
        ),
        (
            GenerationSystemPolicyPermission::ConstructGenerationSystem,
            GenerationSystemPolicyPurpose::ManagedJudgeGeneration,
        ),
    ] {
        let mut fixture = Fixture::new();
        let launch_plan = launch(&fixture.model_lease, "v1");
        let mut qualification = QualificationFixture::new(&fixture, &launch_plan);
        qualification.generation_policy = verified_policy(
            &bindings(candidate_output_contract().schema_digest),
            permission,
            purpose,
        );
        assert_relationship_error(&CandidateAttemptPrecursorCompiler::compile(
            compilation_input(&mut fixture.runtime, &qualification, launch_plan),
            &CancellationToken::new(),
        ));
    }
}

fn record_and_binding_substitutions_are_rejected() {
    for substitution in 0_u8..3 {
        let mut selected = Fixture::new();
        let selected_launch = launch(&selected.model_lease, "v1");
        let mut qualification = QualificationFixture::new(&selected, &selected_launch);
        let other = Fixture::new_model_variant(true);
        let other_launch = launch(&other.model_lease, "v1");
        let other_qualification = QualificationFixture::new(&other, &other_launch);
        match substitution {
            0 => qualification.static_model = other_qualification.static_model,
            1 => qualification.generation_system = other_qualification.generation_system,
            2 => {
                qualification.generation_policy = verified_policy(
                    &bindings_with_strategy(
                        candidate_output_contract().schema_digest,
                        "other strategy",
                    ),
                    GenerationSystemPolicyPermission::ConstructGenerationSystem,
                    GenerationSystemPolicyPurpose::ManagedCandidateGeneration,
                );
            }
            _ => unreachable!(),
        }
        assert_relationship_error(&CandidateAttemptPrecursorCompiler::compile(
            compilation_input(&mut selected.runtime, &qualification, selected_launch),
            &CancellationToken::new(),
        ));
    }
}

fn mismatched_selected_attempt_reaches_model_contract() {
    let mut fixture = Fixture::new();
    let launch_plan = launch(&fixture.model_lease, "v1");
    let qualification = QualificationFixture::new(&fixture, &launch_plan);
    let other = QualificationFixture::with_grounded_request(
        &fixture,
        &launch_plan,
        "other grounded request",
    );
    let mut input = compilation_input(&mut fixture.runtime, &qualification, launch_plan);
    input.planned_attempt = &other.planned_attempt;
    assert!(matches!(
        CandidateAttemptPrecursorCompiler::compile(input, &CancellationToken::new()),
        Err(CandidateAttemptPrecursorCompilationError::Precursor(_))
    ));
}

fn characterized_model_substitution_is_rejected() {
    let mut selected = Fixture::new();
    let selected_launch = launch(&selected.model_lease, "v1");
    let mut qualification = QualificationFixture::new(&selected, &selected_launch);
    let other = Fixture::new_model_variant(true);
    let other_launch = launch(&other.model_lease, "v1");
    let other_qualification = QualificationFixture::new(&other, &other_launch);
    qualification.characterized_package = other_qualification.characterized_package;
    assert_relationship_error(&CandidateAttemptPrecursorCompiler::compile(
        compilation_input(&mut selected.runtime, &qualification, selected_launch),
        &CancellationToken::new(),
    ));
}

fn assert_relationship_error(
    result: &Result<
        crate::VerifiedManagedCandidateAttemptPrecursor<'_, '_, '_>,
        CandidateAttemptPrecursorCompilationError,
    >,
) {
    assert!(matches!(
        result,
        Err(CandidateAttemptPrecursorCompilationError::RelationshipMismatch)
    ));
}
