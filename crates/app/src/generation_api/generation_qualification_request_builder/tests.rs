use std::path::PathBuf;

use rewrite_inference::{ReasoningPolicy, candidate_output_contract};
use rewrite_model::{
    GenerationQualificationOperationPolicyV1Relations,
    GenerationQualificationOperationSystemRelationsV1, GenerationSystemRecordV1Relations,
};
use rewrite_types::CancellationToken;

use super::*;

#[path = "tests/adversarial.rs"]
mod adversarial;
#[path = "tests/source.rs"]
mod source;
pub(super) use super::fixture_support as support;
use support::{Fixture, SOURCE, common_limits, operation_input};

#[test]
fn preplanning_derives_only_exact_content_free_attempt_facts() {
    let mut fixture = Fixture::new(SOURCE, vec!["Acme".to_owned()]);
    let lease = fixture.source.acquire();
    let preplanned = fixture
        .builder
        .derive_preplanning(&lease, 7, common_limits(), &CancellationToken::new())
        .expect("preplanning authority");
    preplanned
        .revalidate(&CancellationToken::new())
        .expect("fresh preplanning derivation");
    assert_eq!(preplanned.declared_seed(), 7);
    assert_eq!(
        preplanned.facts().candidate_output_contract_digest(),
        &candidate_output_contract().schema_digest
    );
    assert_eq!(preplanned.facts().output_ceilings().candidate_count(), 1);
    let debug = format!("{preplanned:?} {:?}", preplanned.facts());
    assert!(!debug.contains("Retain Acme"));
    assert!(!debug.contains("{{PROTECTED"));
}

#[test]
fn exact_post_plan_build_reproduces_every_request_and_projection_fact() {
    let mut fixture = Fixture::new(SOURCE, vec!["Acme".to_owned()]);
    let plan = fixture.plan();
    let lease = fixture.source.acquire();
    let built = fixture
        .builder
        .build(
            GenerationQualificationRequestBuildInput {
                source: &lease,
                operation_policy: &plan.operation,
                qualification_plan: &plan.plan,
                suite: &fixture.suite,
                cluster: &fixture.cluster,
                repetition: &fixture.repetition,
                planned_attempt: &plan.target_attempt,
            },
            &CancellationToken::new(),
        )
        .expect("exact planned request");
    built
        .revalidate(&CancellationToken::new())
        .expect("built request revalidates");

    let request = &built.generation_request;
    assert_eq!(request.candidate_count, 1);
    assert_eq!(request.source_byte_count, SOURCE.len() as u64);
    assert_eq!(request.sampling.temperature.to_bits(), 0x0000_0000);
    assert_eq!(request.sampling.top_p.to_bits(), 0x3f80_0000);
    assert_eq!(request.sampling.seed, Some(7));
    assert_eq!(request.reasoning, ReasoningPolicy::Disabled);
    assert_eq!(request.output, candidate_output_contract());
    assert_eq!(
        request.generation_request_binding_id(),
        plan.target_attempt.generation_request_binding_id().clone()
    );
    assert_eq!(
        built.grounded_request_digest,
        plan.target_attempt.grounded_request_digest().clone()
    );
    assert_eq!(
        built.structured_request.structured_request_binding_id(),
        built
            .projection_input()
            .expect("projection input")
            .structured_completion_request_binding_id
    );
    assert_eq!(
        built
            .projection_input()
            .expect("projection input")
            .complete_input_byte_count,
        request.input.len() as u64
    );
    let debug = format!("{built:?}");
    assert!(!debug.contains("Retain Acme"));
    assert!(!debug.contains("{{PROTECTED"));
}

#[test]
fn component_and_request_identities_have_frozen_vectors() {
    let mut fixture = Fixture::new(SOURCE, vec!["Acme".to_owned()]);
    let bindings = fixture.builder.bindings();
    assert_eq!(
        bindings.planner_digest().as_str(),
        "2d253190e768d7d55fc5af52a1407d9212ff0a19db22caae774ef9bfbce3a0d5"
    );
    assert_eq!(
        bindings.prompt_digest().as_str(),
        "068ef615a58d2170e5591a007b7b0dfa73b9a0b0187a8cc208c8debe7aa70adc"
    );
    assert_eq!(
        bindings.request_policy_digest().as_str(),
        "2ccb1856f60bc94ddb1ce894a75c6d4541bfbbab31043a30f4c4bbcf976416d4"
    );
    assert_eq!(
        bindings.strategy_digest().as_str(),
        "5f5110140498a7b97806d3aedfa9750dd3aae8cd26b75497c573d5b3e5dd303a"
    );

    let lease = fixture.source.acquire();
    let preplanned = fixture
        .builder
        .derive_preplanning(&lease, 7, common_limits(), &CancellationToken::new())
        .expect("preplanned request");
    assert_eq!(
        preplanned.facts().grounded_request_digest().as_str(),
        "7339c852575d56f0cd7fe72e18f16eb4e2cc92723d3a09825f484fcc3c9bbf8d"
    );
    assert_eq!(
        preplanned
            .facts()
            .generation_request_binding_id()
            .digest()
            .as_str(),
        "3e3dbca8e3df822291a389f52ce20e335ecdf66a45ff1854fe0dafcfdf37be3c"
    );
}

pub(in crate::generation_api) fn with_projection_compiler_input<T>(
    use_input: impl for<'records, 'store> FnOnce(
        crate::GenerationQualificationRequestProjectionCompilerV1Input<'records, 'store>,
        PathBuf,
    ) -> T,
) -> T {
    let mut fixture = Fixture::new(SOURCE, vec!["Acme".to_owned()]);
    let plan = fixture.plan();
    let Fixture {
        runtime,
        model_set,
        model_package,
        characterized,
        builder,
        baseline_builder,
        cluster,
        deterministic_contract: _,
        case: _,
        suite,
        repetition,
        source,
    } = fixture;
    let support::PlanFixture {
        target_attempt,
        baseline_attempt,
        plan: qualification_plan,
        operation,
        selection_policy: _,
    } = plan;
    let target_system = builder.generation_system().clone();
    let baseline_system = baseline_builder.generation_system().clone();
    let attempts = vec![target_attempt, baseline_attempt];
    let expected_input = operation_input(common_limits());
    let system_relations = GenerationSystemRecordV1Relations {
        runtime_package_manifest: &runtime.runtime.runtime_manifest,
        runtime_build: &runtime.runtime.runtime_build,
        effective_runtime_state: &runtime.runtime.runtime_state,
        model_artifact_set: &model_set,
        model_package_manifest: &model_package,
        effective_package_evidence_v2: characterized.evidence(),
    };
    let relations = GenerationQualificationOperationPolicyV1Relations {
        suite: &suite,
        plan: &qualification_plan,
        repetitions: std::slice::from_ref(&repetition),
        planned_attempts: &attempts,
        target_system: GenerationQualificationOperationSystemRelationsV1 {
            generation_system: &target_system,
            relations: system_relations,
        },
        baseline_system: GenerationQualificationOperationSystemRelationsV1 {
            generation_system: &baseline_system,
            relations: system_relations,
        },
    };
    let database = source.database.clone();
    let lease = source.acquire();
    let operation_deadline = crate::GenerationQualificationOperationDeadlineV1::start(
        expected_input.limits,
        &CancellationToken::new(),
    )
    .expect("operation deadline");
    use_input(
        crate::GenerationQualificationRequestProjectionCompilerV1Input {
            operation_deadline,
            operation_policy: operation,
            operation_policy_relations: relations,
            operation_policy_input: expected_input,
            case_authorities: vec![
                crate::GenerationQualificationRequestProjectionCaseAuthoritiesV1 {
                    source: lease,
                    cluster,
                    target_builder: builder,
                    baseline_builder,
                },
            ],
        },
        database,
    )
}
