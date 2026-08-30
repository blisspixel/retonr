#[path = "tests/support.rs"]
pub(crate) mod support;

use rewrite_types::CancellationToken;

use super::*;
use support::{Fixture, QualificationFixture, compilation_input, launch};

#[test]
fn operation_request_scope_requires_every_exact_relationship_group() {
    let exact = CandidateAttemptOperationScopeFacts {
        relationships: [true; 4],
    };
    assert!(exact.is_exact());
    for changed in (0..4).map(|index| {
        let mut relationships = [true; 4];
        relationships[index] = false;
        CandidateAttemptOperationScopeFacts { relationships }
    }) {
        assert!(!changed.is_exact());
    }
}

#[test]
fn exact_live_closure_compiles_one_consuming_precursor() {
    let mut fixture = Fixture::new();
    let launch = launch(&fixture.model_lease, "v1");
    let qualification = QualificationFixture::new(&fixture, &launch);
    let expected_runtime_package_id = fixture
        .runtime
        .runtime_manifest
        .runtime_package_manifest_id();
    let capability = CandidateAttemptPrecursorCompiler::compile(
        compilation_input(&mut fixture.runtime, &qualification, launch),
        &CancellationToken::new(),
    )
    .expect("compile exact precursor");

    assert_eq!(
        capability.precursor().runtime_installation_generation(),
        capability
            .runtime_package()
            .installation_key()
            .installation_generation()
    );
    assert_eq!(
        capability.precursor().model_installation_generation(),
        capability.launch_plan().model_installation_generation()
    );
    assert_eq!(
        capability.precursor().structured_request_binding_id(),
        &capability
            .structured_request()
            .structured_request_binding_id()
    );
    assert_eq!(
        capability.generation_request(),
        &qualification.generation_request
    );
    assert_eq!(
        capability.characterized_package().evidence(),
        qualification.characterized_package.evidence()
    );
    let debug = format!("{capability:?}");
    assert!(debug.contains("precursor_id"));
    assert!(!debug.contains(&qualification.generation_request.input));

    let expected_precursor = capability.precursor().clone();
    let handoff = capability.into_runner_handoff();
    assert_eq!(handoff.precursor(), &expected_precursor);
    assert_eq!(
        handoff.precursor.structured_request_binding_id(),
        &handoff.structured_request.structured_request_binding_id()
    );
    assert_eq!(handoff.generation_request, qualification.generation_request);
    assert_eq!(
        handoff.characterized_package.evidence(),
        qualification.characterized_package.evidence()
    );
    assert_eq!(
        handoff
            .runtime_package
            .evidence()
            .runtime_package_manifest_id(),
        &expected_runtime_package_id
    );
    assert_eq!(
        handoff.launch_plan.foundation_id(),
        fixture.model_lease.foundation_id()
    );
}

#[test]
fn request_and_ceiling_substitutions_fail_closed() {
    for mutate in 0_u8..7 {
        let mut fixture = Fixture::new();
        let launch = launch(&fixture.model_lease, "v1");
        let mut qualification = QualificationFixture::new(&fixture, &launch);
        match mutate {
            0 => qualification.generation_request.candidate_count = 2,
            1 => qualification.generation_request.sampling.seed = Some(8),
            2 => qualification.generation_request.source_byte_count += 1,
            3 => qualification.generation_request.candidate_byte_limit += 1,
            4 => qualification.generation_request.input.push('!'),
            5 => {
                qualification.generation_request.output =
                    rewrite_inference::claim_output_contract();
            }
            6 => {
                qualification.output_ceilings =
                    rewrite_model::CandidateOutputCeilingsV1::new(1, 512, 512)
                        .expect("other ceilings");
            }
            _ => unreachable!(),
        }
        let result = CandidateAttemptPrecursorCompiler::compile(
            compilation_input(&mut fixture.runtime, &qualification, launch),
            &CancellationToken::new(),
        );
        assert!(result.is_err(), "request substitution {mutate}");
    }
}

#[test]
fn portable_and_policy_substitutions_fail_but_exact_clones_are_accepted() {
    support::portable_and_policy_substitution_cases();
}

#[test]
fn multi_candidate_and_cancelled_requests_fail_at_initial_boundaries() {
    let mut multi = Fixture::new();
    let launch_plan = launch(&multi.model_lease, "v1");
    let mut qualification = QualificationFixture::new(&multi, &launch_plan);
    qualification.generation_request.candidate_count = 2;
    assert!(matches!(
        CandidateAttemptPrecursorCompiler::compile(
            compilation_input(&mut multi.runtime, &qualification, launch_plan),
            &CancellationToken::new(),
        ),
        Err(CandidateAttemptPrecursorCompilationError::RequestMapping(_))
    ));

    let mut cancelled = Fixture::new();
    let launch_plan = launch(&cancelled.model_lease, "v1");
    let qualification = QualificationFixture::new(&cancelled, &launch_plan);
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert!(matches!(
        CandidateAttemptPrecursorCompiler::compile(
            compilation_input(&mut cancelled.runtime, &qualification, launch_plan),
            &cancellation,
        ),
        Err(CandidateAttemptPrecursorCompilationError::RuntimeRevalidation(_))
    ));
}

#[test]
fn stale_runtime_and_model_bytes_fail_during_initial_revalidation() {
    let mut runtime = Fixture::new();
    let launch_plan = launch(&runtime.model_lease, "v1");
    let qualification = QualificationFixture::new(&runtime, &launch_plan);
    runtime.add_runtime_member();
    assert!(matches!(
        CandidateAttemptPrecursorCompiler::compile(
            compilation_input(&mut runtime.runtime, &qualification, launch_plan),
            &CancellationToken::new(),
        ),
        Err(CandidateAttemptPrecursorCompilationError::RuntimeRevalidation(_))
    ));

    let mut model = Fixture::new();
    let launch_plan = launch(&model.model_lease, "v1");
    let qualification = QualificationFixture::new(&model, &launch_plan);
    model.add_model_member();
    assert!(matches!(
        CandidateAttemptPrecursorCompiler::compile(
            compilation_input(&mut model.runtime, &qualification, launch_plan),
            &CancellationToken::new(),
        ),
        Err(CandidateAttemptPrecursorCompilationError::ModelRevalidation(_))
    ));
}
