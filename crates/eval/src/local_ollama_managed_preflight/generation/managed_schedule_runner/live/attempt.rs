use std::{cell::RefCell, rc::Rc};

use rewrite_app::effective_runtime_state_observation::{
    ManagedJudgeObservationSequence, ManagedOllamaEffectiveRuntimeStateJoin,
    join_managed_ollama_effective_runtime_state_until, observe_linux_platform_framework,
    observe_ollama_cpu_execution, observe_ollama_provider_snapshot,
    observe_ollama_wire_output_configuration,
};
use rewrite_inference::OperationContext;
use rewrite_model::{CandidateJudgeObservationV1, OllamaRetainedSessionResponseId};
use rewrite_ollama::{OllamaResponseObservation, OllamaRetainedStreamSession};
use rewrite_runtime_attestor::NativeLoadObservationRequest;
use rewrite_runtime_isolation::IsolationEvidence;
use rewrite_types::{CancellationToken, Digest};

use crate::local_judge_execution::normalization::{
    CandidateJudgeAttemptNormalizationInput, normalize_candidate_judge_attempt,
};
use crate::local_ollama_managed_preflight::validation::validate_final_isolation;

use super::super::{
    deadline::{cap_native_load_limits, remaining_before_deadline},
    error::ManagedJudgeScheduleRunnerPrimaryFailure,
};
use super::{LiveScheduleInput, ensure_live_operation, map_session_at_operation};

pub(super) struct CompletedAttempt {
    pub(super) response_id: OllamaRetainedSessionResponseId,
    pub(super) observation: CandidateJudgeObservationV1,
    pub(super) preflight_digest: Digest,
}

#[expect(
    clippy::too_many_lines,
    reason = "one attempt keeps its traffic, observation, sealing, and normalization order visible"
)]
pub(super) async fn run_attempt<F>(
    input: &mut LiveScheduleInput<'_, '_, '_>,
    initial_isolation: &IsolationEvidence,
    native_request: &NativeLoadObservationRequest<'_>,
    sequence: &Rc<RefCell<ManagedJudgeObservationSequence>>,
    session: &mut OllamaRetainedStreamSession<F>,
    schedule_cursor: usize,
    cancellation: &CancellationToken,
) -> Result<CompletedAttempt, ManagedJudgeScheduleRunnerPrimaryFailure>
where
    F: FnMut(
        OllamaResponseObservation,
    ) -> Result<
        (),
        rewrite_app::effective_runtime_state_observation::ManagedJudgeObservationError,
    >,
{
    ensure_live_operation(cancellation, input.deadline)?;
    let traffic = input
        .eval
        .prepare_traffic_request_at(schedule_cursor, cancellation)
        .map_err(ManagedJudgeScheduleRunnerPrimaryFailure::Request)?;
    ensure_live_operation(cancellation, input.deadline)?;
    if traffic.schedule_cursor() != schedule_cursor
        || input
            .eval
            .request_aggregate()
            .structured_request_binding_ids()
            .get(schedule_cursor)
            != Some(&traffic.request_binding_id())
    {
        return Err(ManagedJudgeScheduleRunnerPrimaryFailure::RuntimeStateRelationship);
    }
    ensure_live_operation(cancellation, input.deadline)?;
    let (response, receipt) = session
        .complete_structured_with_residency_borrowed(
            traffic.request(),
            OperationContext::new(cancellation, Some(input.deadline)),
        )
        .await
        .map_err(|error| map_session_at_operation(error, cancellation, input.deadline))?;
    ensure_live_operation(cancellation, input.deadline)?;
    input
        .runtime_package
        .revalidate(cancellation)
        .map_err(crate::LocalOllamaManagedPreflightError::Package)?;
    ensure_live_operation(cancellation, input.deadline)?;
    input
        .managed_ollama
        .revalidate_model_package_until(cancellation, input.deadline)
        .map_err(|error| ManagedJudgeScheduleRunnerPrimaryFailure::Generation(error.into()))?;
    ensure_live_operation(cancellation, input.deadline)?;
    let schedule_cursor_u32 = u32::try_from(schedule_cursor)
        .map_err(|_| ManagedJudgeScheduleRunnerPrimaryFailure::RuntimeStateRelationship)?;
    let native_request = NativeLoadObservationRequest {
        package: native_request.package,
        expected_package_id: native_request.expected_package_id,
        retained_package_members: native_request.retained_package_members,
        expected_external_components: native_request.expected_external_components,
        limits: cap_native_load_limits(
            native_request.limits,
            remaining_before_deadline(input.deadline)
                .map_err(|_| ManagedJudgeScheduleRunnerPrimaryFailure::DeadlineExceeded)?,
        ),
    };
    ensure_live_operation(cancellation, input.deadline)?;
    let attempt_observation = sequence
        .try_borrow_mut()
        .map_err(|_| ManagedJudgeScheduleRunnerPrimaryFailure::SequenceAuthority)?
        .complete_attempt_observation_until(
            schedule_cursor_u32,
            &native_request,
            cancellation,
            input.deadline,
        )?;
    ensure_live_operation(cancellation, input.deadline)?;
    let final_isolation = input
        .managed_ollama
        .reobserve_until(cancellation, input.deadline)
        .map_err(|error| ManagedJudgeScheduleRunnerPrimaryFailure::Generation(error.into()))?;
    ensure_live_operation(cancellation, input.deadline)?;
    validate_final_isolation(initial_isolation, &final_isolation)?;
    ensure_live_operation(cancellation, input.deadline)?;
    input
        .runtime_package
        .revalidate(cancellation)
        .map_err(crate::LocalOllamaManagedPreflightError::Package)?;
    ensure_live_operation(cancellation, input.deadline)?;
    let provider = observe_ollama_provider_snapshot(
        input.runtime_build,
        input.managed_ollama.input_evidence(),
        input.model,
        traffic.request(),
        &receipt,
    )?;
    ensure_live_operation(cancellation, input.deadline)?;
    let wire = observe_ollama_wire_output_configuration(
        input.runtime_build,
        input.managed_ollama.input_evidence(),
        input.model,
        traffic.request(),
        &receipt,
    )?;
    ensure_live_operation(cancellation, input.deadline)?;
    let platform = observe_linux_platform_framework(
        input.runtime_build,
        attempt_observation.worker_native_load(),
        cancellation,
    )?;
    ensure_live_operation(cancellation, input.deadline)?;
    let cpu = observe_ollama_cpu_execution(
        input.managed_ollama.input_evidence(),
        traffic.request(),
        &receipt,
        initial_isolation,
        &final_isolation,
        attempt_observation.initial_worker(),
        attempt_observation.final_worker(),
        attempt_observation.worker_native_load(),
        attempt_observation.model_mapping(),
    )?;
    ensure_live_operation(cancellation, input.deadline)?;
    let state = join_managed_ollama_effective_runtime_state_until(
        &ManagedOllamaEffectiveRuntimeStateJoin {
            runtime_build: input.runtime_build,
            admitted_runtime: input.admitted_runtime,
            runtime_package_lease: input.runtime_package,
            generation_path: input.generation_path,
            managed_ollama: input.managed_ollama,
            request: traffic.request(),
            receipt: &receipt,
            initial_isolation,
            final_isolation: &final_isolation,
            initial_worker: attempt_observation.initial_worker(),
            final_worker: attempt_observation.final_worker(),
            worker_native_load: attempt_observation.worker_native_load(),
            server_process: attempt_observation.process(),
            server_native_load: attempt_observation.native_load(),
            model_mapping: attempt_observation.model_mapping(),
            provider_snapshot: &provider,
            wire_configuration: &wire,
            platform: &platform,
            cpu_execution: &cpu,
        },
        cancellation,
        input.deadline,
    )?;
    ensure_live_operation(cancellation, input.deadline)?;
    if state.state() != input.expected_runtime_state {
        return Err(ManagedJudgeScheduleRunnerPrimaryFailure::RuntimeStateRelationship);
    }
    ensure_live_operation(cancellation, input.deadline)?;
    let response_id = receipt.execution().retained_response_id();
    let preflight_digest = receipt.execution().preflight_digest().clone();
    ensure_live_operation(cancellation, input.deadline)?;
    sequence
        .try_borrow_mut()
        .map_err(|_| ManagedJudgeScheduleRunnerPrimaryFailure::SequenceAuthority)?
        .seal_attempt_until(
            attempt_observation,
            traffic.into_structured_request(),
            receipt,
            state,
            cancellation,
            input.deadline,
        )?;
    ensure_live_operation(cancellation, input.deadline)?;
    let observation = input
        .eval
        .with_attempt_at(schedule_cursor, cancellation, |material| {
            normalize_candidate_judge_attempt(&CandidateJudgeAttemptNormalizationInput {
                plan: input.eval.judge_plan(),
                schedule: input.eval.judge_schedule(),
                request_aggregate: input.eval.request_aggregate(),
                schedule_cursor,
                request: material.request(),
                response: &response,
                retained_response_id: &response_id,
                case_key: material.case_key(),
                presentation: material.presentation(),
                admitted_rubric_clause_ids: material.rubric_clause_ids(),
                source: material.source(),
                presented_first: material.presented_first(),
                presented_second: material.presented_second(),
            })
        })
        .map_err(ManagedJudgeScheduleRunnerPrimaryFailure::Normalization)?;
    ensure_live_operation(cancellation, input.deadline)?;
    Ok(CompletedAttempt {
        response_id,
        observation,
        preflight_digest,
    })
}
