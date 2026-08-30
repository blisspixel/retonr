#[path = "tests/adversarial.rs"]
mod adversarial;
#[path = "tests/support.rs"]
pub(crate) mod support;

use std::fs;

use rewrite_types::CancellationToken;

use super::*;
use support::{JudgeFixture, compilation_input};

#[test]
fn exact_live_closure_compiles_and_consumes_one_supported_runner_handoff_without_launching() {
    let mut fixture = JudgeFixture::new();
    let expected_runtime_generation = fixture
        .live
        .runtime
        .runtime_package
        .installation_key()
        .installation_generation();
    let expected_plan = fixture.judge_plan.clone();
    let expected_schedule = fixture.schedule.clone();
    let expected_requests = fixture.requests.clone();
    let expected_system = fixture.judge_system.clone();
    let expected_foundation = fixture.live.model_lease.foundation_id().clone();
    let capability = ManagedJudgePrecursorCompiler::compile(
        compilation_input(&mut fixture),
        &CancellationToken::new(),
    )
    .expect("managed judge precursor");

    assert_eq!(capability.judge_plan(), &expected_plan);
    assert_eq!(capability.judge_schedule(), &expected_schedule);
    assert_eq!(capability.request_aggregate(), &expected_requests);
    assert_eq!(capability.judge_system(), &expected_system);
    assert_eq!(
        capability.runtime_installation_generation(),
        expected_runtime_generation
    );
    assert_ne!(capability.model_installation_generation(), 0);
    let debug = format!("{capability:?}");
    assert!(debug.contains("judge_plan_id"));
    assert!(!debug.contains("fixture:latest"));

    let mut handoff = capability
        .into_runner_handoff(&CancellationToken::new())
        .expect("runner handoff");
    handoff
        .revalidate(&CancellationToken::new())
        .expect("handoff revalidation");
    assert_eq!(handoff.judge_plan(), &expected_plan);
    assert_eq!(handoff.judge_schedule(), &expected_schedule);
    assert_eq!(handoff.request_aggregate(), &expected_requests);
    assert_eq!(handoff.judge_system(), &expected_system);
    assert_eq!(
        handoff.model_package().foundation_id(),
        &expected_foundation
    );
    let debug = format!("{handoff:?}");
    assert!(debug.contains("request_aggregate_id"));
    assert!(!debug.contains("fixture:latest"));

    let parts = handoff.into_parts();
    assert!(parts.1.binds_exact_model_package(parts.2));
    assert_eq!(
        parts.0.installation_key().installation_generation(),
        parts.16
    );
    assert_eq!(parts.12, expected_plan);
    assert_eq!(parts.13, expected_schedule);
    assert_eq!(parts.14, expected_requests);
    assert_eq!(parts.15, expected_system);
    assert_ne!(parts.17, 0);
}

#[test]
fn cancellation_and_terminal_runtime_and_model_drift_release_no_handoff() {
    let mut cancelled = JudgeFixture::new();
    let token = CancellationToken::new();
    token.cancel();
    assert!(matches!(
        ManagedJudgePrecursorCompiler::compile(compilation_input(&mut cancelled), &token),
        Err(ManagedJudgePrecursorCompilationError::Cancelled)
    ));

    let mut runtime = JudgeFixture::new();
    let runtime_path = runtime.live.runtime_drift_path();
    let capability = ManagedJudgePrecursorCompiler::compile(
        compilation_input(&mut runtime),
        &CancellationToken::new(),
    )
    .expect("runtime precursor");
    fs::write(runtime_path, b"runtime drift").expect("runtime drift");
    assert!(matches!(
        capability.into_runner_handoff(&CancellationToken::new()),
        Err(ManagedJudgePrecursorCompilationError::RuntimeRevalidation(
            _
        ))
    ));

    let mut model = JudgeFixture::new();
    let model_path = model.live.model_drift_path();
    let capability = ManagedJudgePrecursorCompiler::compile(
        compilation_input(&mut model),
        &CancellationToken::new(),
    )
    .expect("model precursor");
    fs::write(model_path, b"model drift").expect("model drift");
    assert!(matches!(
        capability.into_runner_handoff(&CancellationToken::new()),
        Err(ManagedJudgePrecursorCompilationError::ModelRevalidation(_))
    ));
}

#[test]
fn handoff_revalidation_observes_cancellation_without_launching() {
    let mut fixture = JudgeFixture::new();
    let capability = ManagedJudgePrecursorCompiler::compile(
        compilation_input(&mut fixture),
        &CancellationToken::new(),
    )
    .expect("precursor");
    let mut handoff = capability
        .into_runner_handoff(&CancellationToken::new())
        .expect("handoff");
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(matches!(
        handoff.revalidate(&cancelled),
        Err(ManagedJudgePrecursorCompilationError::Cancelled)
    ));
}
