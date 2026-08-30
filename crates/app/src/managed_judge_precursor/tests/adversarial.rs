use rewrite_model::{CandidateJudgeRequestAggregateV1, CandidateJudgeScheduleV1};
use rewrite_runtime_isolation::{IsolationPolicy, PreparedIsolation};
use rewrite_types::CancellationToken;

use super::support::{JudgeFixture, compilation_input, digest, pair_set_id};
use crate::candidate_attempt_precursor::tests::support::{
    Fixture, bindings_with_strategy, changed_state, characterized, frozen_with_label, launch,
    system, verified_policy,
};
use crate::{
    GenerationSystemPolicyBindingsV1, GenerationSystemPolicyBindingsV1Input,
    GenerationSystemPolicyPermission, GenerationSystemPolicyPurpose,
    ManagedJudgePrecursorCompilationError, ManagedJudgePrecursorCompiler,
    ManagedJudgePrecursorRelationship, StaticModelInterpretationV1,
};

#[test]
fn permission_purpose_and_every_policy_binding_fail_closed() {
    for (permission, purpose) in [
        (
            GenerationSystemPolicyPermission::ValidateGenerationSystem,
            GenerationSystemPolicyPurpose::ManagedJudgeGeneration,
        ),
        (
            GenerationSystemPolicyPermission::ConstructGenerationSystem,
            GenerationSystemPolicyPurpose::ManagedCandidateGeneration,
        ),
    ] {
        let mut fixture = JudgeFixture::new();
        fixture.policy = verified_policy(&fixture.policy_bindings, permission, purpose);
        assert_relationship(
            ManagedJudgePrecursorCompiler::compile(
                compilation_input(&mut fixture),
                &CancellationToken::new(),
            ),
            ManagedJudgePrecursorRelationship::GenerationPolicy,
        );
    }

    for index in 0..14 {
        let mut fixture = JudgeFixture::new();
        let changed = changed_policy_bindings(&fixture.policy_bindings, index);
        fixture.policy = verified_policy(
            &changed,
            GenerationSystemPolicyPermission::ConstructGenerationSystem,
            GenerationSystemPolicyPurpose::ManagedJudgeGeneration,
        );
        assert_relationship(
            ManagedJudgePrecursorCompiler::compile(
                compilation_input(&mut fixture),
                &CancellationToken::new(),
            ),
            ManagedJudgePrecursorRelationship::GenerationPolicy,
        );
    }
}

#[test]
fn portable_plan_schedule_and_request_substitutions_fail_closed() {
    let mut foreign_system = JudgeFixture::new();
    let launch_plan = launch(&foreign_system.live.model_lease, "latest");
    let other_bindings = bindings_with_strategy(
        foreign_system.judge_system.output_schema_digest().clone(),
        "foreign judge",
    );
    foreign_system.judge_system = system(
        &foreign_system.live,
        &launch_plan,
        &foreign_system.live.runtime.runtime_state,
        &foreign_system.characterized,
        &foreign_system.static_model,
        &other_bindings,
    );
    assert_relationship(
        ManagedJudgePrecursorCompiler::compile(
            compilation_input(&mut foreign_system),
            &CancellationToken::new(),
        ),
        ManagedJudgePrecursorRelationship::JudgePlan,
    );

    let mut foreign_schedule = JudgeFixture::new();
    let other_plan = foreign_schedule.plan_with_presentation_seed(43);
    foreign_schedule.schedule =
        CandidateJudgeScheduleV1::new(&other_plan, &pair_set_id("other pair"))
            .expect("foreign schedule");
    assert_relationship(
        ManagedJudgePrecursorCompiler::compile(
            compilation_input(&mut foreign_schedule),
            &CancellationToken::new(),
        ),
        ManagedJudgePrecursorRelationship::JudgeSchedule,
    );

    let mut foreign_requests = JudgeFixture::new();
    let other_schedule = CandidateJudgeScheduleV1::new(
        &foreign_requests.judge_plan,
        &pair_set_id("other request pair"),
    )
    .expect("other schedule");
    foreign_requests.requests = CandidateJudgeRequestAggregateV1::new(
        &foreign_requests.judge_plan,
        &other_schedule,
        foreign_requests
            .requests
            .structured_request_binding_ids()
            .to_vec(),
    )
    .expect("foreign requests");
    assert_relationship(
        ManagedJudgePrecursorCompiler::compile(
            compilation_input(&mut foreign_requests),
            &CancellationToken::new(),
        ),
        ManagedJudgePrecursorRelationship::RequestAggregate,
    );
}

#[test]
fn request_count_and_uniqueness_are_closed_but_order_provenance_remains_eval_owned() {
    let mut fixture = JudgeFixture::new();
    let mut reversed_ids = fixture.requests.structured_request_binding_ids().to_vec();
    assert!(
        CandidateJudgeRequestAggregateV1::new(
            &fixture.judge_plan,
            &fixture.schedule,
            reversed_ids[..1].to_vec(),
        )
        .is_err()
    );
    assert!(
        CandidateJudgeRequestAggregateV1::new(
            &fixture.judge_plan,
            &fixture.schedule,
            vec![reversed_ids[0].clone(), reversed_ids[0].clone()],
        )
        .is_err()
    );

    let original_id = fixture.requests.request_aggregate_id().clone();
    reversed_ids.reverse();
    fixture.requests =
        CandidateJudgeRequestAggregateV1::new(&fixture.judge_plan, &fixture.schedule, reversed_ids)
            .expect("reordered self-consistent aggregate");
    assert_ne!(fixture.requests.request_aggregate_id(), &original_id);
    let expected = fixture.requests.request_aggregate_id().clone();
    let capability = ManagedJudgePrecursorCompiler::compile(
        compilation_input(&mut fixture),
        &CancellationToken::new(),
    )
    .expect("app compiler validates self-consistency only");
    assert_eq!(
        capability.request_aggregate().request_aggregate_id(),
        &expected
    );
}

#[test]
fn runtime_state_and_opaque_runtime_substitutions_fail_closed() {
    let mut state = JudgeFixture::new();
    let changed_state = changed_state(&state.live.runtime);
    let mut input = compilation_input(&mut state);
    input.expected_runtime_state = &changed_state;
    assert_relationship(
        ManagedJudgePrecursorCompiler::compile(input, &CancellationToken::new()),
        ManagedJudgePrecursorRelationship::Runtime,
    );

    let mut admitted = JudgeFixture::new();
    let foreign_admitted = crate::VerifiedAdmittedRuntime::exact_candidate_precursor_test_fixture(
        &admitted.live.runtime.runtime_manifest,
        serde_json::from_value(serde_json::json!(digest("foreign frozen set")))
            .expect("foreign frozen set identity"),
    );
    let mut input = compilation_input(&mut admitted);
    input.admitted_runtime = &foreign_admitted;
    assert_relationship(
        ManagedJudgePrecursorCompiler::compile(input, &CancellationToken::new()),
        ManagedJudgePrecursorRelationship::Runtime,
    );

    let mut path = JudgeFixture::new();
    let foreign_admitted = crate::VerifiedAdmittedRuntime::exact_candidate_precursor_test_fixture(
        &path.live.runtime.runtime_manifest,
        serde_json::from_value(serde_json::json!(digest("other frozen set")))
            .expect("other frozen set identity"),
    );
    let foreign_path = crate::VerifiedManagedGenerationPath::exact_candidate_precursor_test_fixture(
        &foreign_admitted,
        &path.live.runtime.runtime_manifest,
    );
    let mut input = compilation_input(&mut path);
    input.generation_path = &foreign_path;
    assert_relationship(
        ManagedJudgePrecursorCompiler::compile(input, &CancellationToken::new()),
        ManagedJudgePrecursorRelationship::Runtime,
    );

    let mut frozen = JudgeFixture::new();
    let foreign_frozen = frozen_with_label(
        &frozen.live.runtime.runtime_manifest,
        "foreign managed process",
    );
    let mut input = compilation_input(&mut frozen);
    input.frozen_components = &foreign_frozen;
    assert_relationship(
        ManagedJudgePrecursorCompiler::compile(input, &CancellationToken::new()),
        ManagedJudgePrecursorRelationship::Runtime,
    );
}

#[test]
fn prepared_isolation_policy_substitution_fails_closed() {
    let mut fixture = JudgeFixture::new();
    let foreign_policy = IsolationPolicy::new(
        std::time::Duration::from_secs(4),
        std::time::Duration::from_secs(5),
        32,
        32,
        4_096,
        256,
        64,
    )
    .expect("foreign isolation policy");
    let foreign_isolation = PreparedIsolation::test_support_from_policy(foreign_policy);
    let mut input = compilation_input(&mut fixture);
    input.prepared_isolation = &foreign_isolation;
    assert_relationship(
        ManagedJudgePrecursorCompiler::compile(input, &CancellationToken::new()),
        ManagedJudgePrecursorRelationship::Isolation,
    );
}

#[test]
fn model_static_and_characterization_substitutions_fail_closed() {
    let mut wrong_launch = JudgeFixture::new();
    let other_live = Fixture::new_model_variant(true);
    let mut input = compilation_input(&mut wrong_launch);
    input.launch_plan = launch(&other_live.model_lease, "latest");
    assert_relationship(
        ManagedJudgePrecursorCompiler::compile(input, &CancellationToken::new()),
        ManagedJudgePrecursorRelationship::Model,
    );

    let mut wrong_static = JudgeFixture::new();
    let other_live = Fixture::new_model_variant(true);
    let other_launch = launch(&other_live.model_lease, "latest");
    let other_static =
        StaticModelInterpretationV1::derive(&other_launch).expect("other static model");
    let mut input = compilation_input(&mut wrong_static);
    input.static_model = &other_static;
    assert_relationship(
        ManagedJudgePrecursorCompiler::compile(input, &CancellationToken::new()),
        ManagedJudgePrecursorRelationship::Model,
    );

    let mut wrong_characterized = JudgeFixture::new();
    let changed = changed_state(&wrong_characterized.live.runtime);
    let exact_launch = launch(&wrong_characterized.live.model_lease, "latest");
    let other_characterized = characterized(&wrong_characterized.live, &exact_launch, &changed);
    let mut input = compilation_input(&mut wrong_characterized);
    input.characterized_package = &other_characterized;
    assert_relationship(
        ManagedJudgePrecursorCompiler::compile(input, &CancellationToken::new()),
        ManagedJudgePrecursorRelationship::Model,
    );
}

#[test]
fn initial_runtime_and_model_drift_and_error_debug_are_redacted() {
    let mut runtime = JudgeFixture::new();
    let runtime_path = runtime.live.runtime_drift_path();
    let runtime_input = compilation_input(&mut runtime);
    std::fs::write(runtime_path, b"runtime drift").expect("runtime drift");
    let runtime_error =
        ManagedJudgePrecursorCompiler::compile(runtime_input, &CancellationToken::new())
            .expect_err("runtime drift");
    assert!(matches!(
        runtime_error,
        ManagedJudgePrecursorCompilationError::RuntimeRevalidation(_)
    ));
    assert_eq!(
        format!("{runtime_error:?}"),
        "ManagedJudgePrecursorCompilationError { kind: \"runtime_revalidation\", .. }"
    );

    let mut model = JudgeFixture::new();
    let model_path = model.live.model_drift_path();
    let model_input = compilation_input(&mut model);
    std::fs::write(model_path, b"model drift").expect("model drift");
    let model_error =
        ManagedJudgePrecursorCompiler::compile(model_input, &CancellationToken::new())
            .expect_err("model drift");
    assert!(matches!(
        model_error,
        ManagedJudgePrecursorCompilationError::ModelRevalidation(_)
    ));
    assert!(!format!("{model_error:?}").contains("unexpected-member"));

    for error in [
        ManagedJudgePrecursorCompilationError::Cancelled,
        ManagedJudgePrecursorCompilationError::Relationship(
            ManagedJudgePrecursorRelationship::InstallationGeneration,
        ),
        ManagedJudgePrecursorCompilationError::PortableContract(
            rewrite_model::GenerationQualificationContractError::InvalidEncoding,
        ),
    ] {
        assert!(format!("{error:?}").starts_with("ManagedJudgePrecursorCompilationError"));
    }
}

fn changed_policy_bindings(
    original: &GenerationSystemPolicyBindingsV1,
    index: usize,
) -> GenerationSystemPolicyBindingsV1 {
    let replacement = digest(&format!("changed policy binding {index}"));
    let mut values = [
        original.strategy_digest().clone(),
        original.planner_digest().clone(),
        original.validator_digest().clone(),
        original.adapter_digest().clone(),
        original.prompt_digest().clone(),
        original.output_schema_digest().clone(),
        original.request_policy_digest().clone(),
        original.language_digest().clone(),
        original.mode_digest().clone(),
        original.format_digest().clone(),
        original.operating_system_digest().clone(),
        original.architecture_digest().clone(),
        original.execution_class_digest().clone(),
        original.hardware_envelope_digest().clone(),
    ];
    values[index] = replacement;
    let [
        strategy_digest,
        planner_digest,
        validator_digest,
        adapter_digest,
        prompt_digest,
        output_schema_digest,
        request_policy_digest,
        language_digest,
        mode_digest,
        format_digest,
        operating_system_digest,
        architecture_digest,
        execution_class_digest,
        hardware_envelope_digest,
    ] = values;
    GenerationSystemPolicyBindingsV1::new(GenerationSystemPolicyBindingsV1Input {
        strategy_digest,
        planner_digest,
        validator_digest,
        adapter_digest,
        prompt_digest,
        output_schema_digest,
        request_policy_digest,
        language_digest,
        mode_digest,
        format_digest,
        operating_system_digest,
        architecture_digest,
        execution_class_digest,
        hardware_envelope_digest,
    })
}

fn assert_relationship<T>(
    result: Result<T, ManagedJudgePrecursorCompilationError>,
    expected: ManagedJudgePrecursorRelationship,
) {
    match result {
        Err(ManagedJudgePrecursorCompilationError::Relationship(actual)) => {
            assert_eq!(actual, expected);
        }
        Err(error) => panic!("expected {expected:?}, got {error:?}"),
        Ok(_) => panic!("expected {expected:?}, got success"),
    }
}
