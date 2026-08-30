use std::{cell::RefCell, rc::Rc, time::Instant};

use rewrite_app::effective_runtime_state_observation::{
    CompletedManagedJudgeObservationSequence, ManagedJudgeObservationError,
    ManagedJudgeObservationSequence,
};
use rewrite_app::{
    MANAGED_OLLAMA_V0_32_15_ENDPOINT, ManagedOllamaIsolationLease,
    derive_managed_judge_preflight_observer_binding_digest, managed_ollama_v0_32_15_launch_spec,
};
use rewrite_inference::OperationContext;
use rewrite_model::{
    CandidateJudgeObservationV1, EffectiveRuntimeState, OllamaRetainedSessionResponseId,
    RuntimeBuildIdentity, RuntimePackageManifest,
};
use rewrite_ollama::{
    OllamaEndpoint, OllamaLimits, OllamaModelBinding, OllamaResponseObservation,
    OllamaResponseObservationPhase, OllamaRetainedStreamSessionConfig,
};
use rewrite_runtime_attestor::{
    AttachedProcessLease, ListenerEndpoint, ManagedGenerationWorkerError,
    ManagedGenerationWorkerLimits, ManagedGenerationWorkerNativeLoadRequest,
    ManagedGenerationWorkerObservationRequest, ManagedGenerationWorkerProfile,
    NativeLoadObservationRequest, NativeManagedLinuxProcessObserver, RetainedNativePackageMember,
    VerifiedFrozenExternalNativeComponentSet,
};
use rewrite_runtime_isolation::PreparedIsolation;
use rewrite_types::{CancellationToken, Digest};

use crate::candidate_judge_preparation::CandidateJudgeRunnerHandoff;
use crate::local_ollama_managed_preflight::generation::validation::exact_retained_worker;
use crate::local_ollama_managed_preflight::report::{build_report, report_evidence_digests};
use crate::local_ollama_managed_preflight::validation::{
    exact_helper_member, managed_expectation, validate_cloud_launch, validate_final_isolation,
    validate_isolation_binding, validate_process_binding,
};
use crate::local_ollama_managed_preflight::{
    LocalOllamaManagedPreflightLimits, bind_successful_managed_preflight,
};
use crate::{
    LocalOllamaBoundPreflightPlan, LocalOllamaManagedPreflightOutcome,
    local_ollama_preflight::local_ollama_preflight_report,
};
use rewrite_app::{RuntimePackageLease, VerifiedAdmittedRuntime, VerifiedManagedGenerationPath};

use super::{
    deadline::{
        OperationGateFailure, cap_native_load_limits, cap_worker_limits, ensure_operation_active,
        remaining_before_deadline,
    },
    error::ManagedJudgeScheduleRunnerPrimaryFailure,
};

mod attempt;

pub(super) struct LiveScheduleInput<'a, 'store, 'model> {
    pub(super) eval: &'a CandidateJudgeRunnerHandoff<'store>,
    pub(super) runtime_manifest: &'a RuntimePackageManifest,
    pub(super) runtime_package: &'a mut RuntimePackageLease,
    pub(super) runtime_build: &'a RuntimeBuildIdentity,
    pub(super) admitted_runtime: &'a VerifiedAdmittedRuntime,
    pub(super) generation_path: &'a VerifiedManagedGenerationPath,
    pub(super) frozen_components: &'a VerifiedFrozenExternalNativeComponentSet,
    pub(super) prepared_isolation: &'a PreparedIsolation,
    pub(super) expected_runtime_state: &'a EffectiveRuntimeState,
    pub(super) preflight_plan: &'a LocalOllamaBoundPreflightPlan,
    pub(super) preflight_limits: LocalOllamaManagedPreflightLimits,
    pub(super) worker_limits: ManagedGenerationWorkerLimits,
    pub(super) model: &'a OllamaModelBinding,
    pub(super) managed_ollama: &'a ManagedOllamaIsolationLease<'model>,
    pub(super) deadline: Instant,
}

pub(super) struct LiveScheduleResult {
    pub(super) completed_sequence: CompletedManagedJudgeObservationSequence,
    pub(super) response_ids: Vec<OllamaRetainedSessionResponseId>,
    pub(super) observations: Vec<CandidateJudgeObservationV1>,
    pub(super) managed_preflight: LocalOllamaManagedPreflightOutcome,
    pub(super) actual_preflight_limits: LocalOllamaManagedPreflightLimits,
    pub(super) retained_session_preflight_digest: Digest,
    pub(super) preflight_observer_binding_digest: Digest,
}

#[expect(
    clippy::too_many_lines,
    reason = "the single retained-session protocol keeps its phase order visible"
)]
pub(super) async fn run_live_schedule(
    mut input: LiveScheduleInput<'_, '_, '_>,
    cancellation: &CancellationToken,
) -> Result<LiveScheduleResult, ManagedJudgeScheduleRunnerPrimaryFailure> {
    ensure_live_operation(cancellation, input.deadline)?;
    let endpoint = OllamaEndpoint::parse(&input.preflight_plan.preflight.endpoint)
        .map_err(|_| invalid_preflight())?;
    ensure_live_operation(cancellation, input.deadline)?;
    if endpoint.socket_addr() != MANAGED_OLLAMA_V0_32_15_ENDPOINT {
        return Err(invalid_preflight());
    }
    ensure_live_operation(cancellation, input.deadline)?;
    let launch = managed_ollama_v0_32_15_launch_spec();
    if &launch.redacted_digest() != input.managed_ollama.plain_launch_spec_digest() {
        return Err(ManagedJudgeScheduleRunnerPrimaryFailure::ManagedBuildRelationship);
    }
    ensure_live_operation(cancellation, input.deadline)?;
    let preparation = input.prepared_isolation.preparation_evidence();
    let helper = exact_helper_member(
        input.runtime_manifest,
        preparation.helper_digest(),
        preparation.helper_bytes(),
    )?;
    ensure_live_operation(cancellation, input.deadline)?;
    let initial_isolation = input.managed_ollama.initial_evidence();
    validate_isolation_binding(&initial_isolation, input.runtime_manifest)?;
    ensure_live_operation(cancellation, input.deadline)?;
    let channel = input
        .managed_ollama
        .connect_loopback_until(cancellation, input.deadline)
        .map_err(|error| ManagedJudgeScheduleRunnerPrimaryFailure::Generation(error.into()))?;
    ensure_live_operation(cancellation, input.deadline)?;
    let (stream, diagnostics, startup_output) = channel.into_parts();
    validate_cloud_launch(&launch, &startup_output)?;
    ensure_live_operation(cancellation, input.deadline)?;

    let expectation = managed_expectation(&initial_isolation)?;
    ensure_live_operation(cancellation, input.deadline)?;
    let listener = ListenerEndpoint::new(endpoint.socket_addr()).map_err(|error| {
        ManagedJudgeScheduleRunnerPrimaryFailure::ManagedPreflight(
            crate::LocalOllamaManagedPreflightError::Witness(error),
        )
    })?;
    ensure_live_operation(cancellation, input.deadline)?;
    let mut process_limits = input.preflight_limits.process;
    process_limits.maximum_elapsed = process_limits
        .maximum_elapsed
        .min(remaining_before_deadline(input.deadline).map_err(deadline_failure)?);
    ensure_live_operation(cancellation, input.deadline)?;
    let process = NativeManagedLinuxProcessObserver
        .attach_until(
            listener,
            diagnostics.into_file(),
            expectation,
            process_limits,
            cancellation,
            input.deadline,
        )
        .map_err(|error| {
            ManagedJudgeScheduleRunnerPrimaryFailure::ManagedPreflight(
                crate::LocalOllamaManagedPreflightError::Witness(error),
            )
        })?;
    ensure_live_operation(cancellation, input.deadline)?;
    validate_process_binding(process.initial_evidence(), input.runtime_manifest)?;
    ensure_live_operation(cancellation, input.deadline)?;

    let package_id = input.runtime_manifest.runtime_package_manifest_id();
    let retained_members = input
        .runtime_package
        .clone_members_for_native_observation(cancellation)
        .map_err(crate::LocalOllamaManagedPreflightError::Package)?;
    ensure_live_operation(cancellation, input.deadline)?;
    let retained_worker = exact_retained_worker(&retained_members, input.generation_path)?;
    ensure_live_operation(cancellation, input.deadline)?;
    let sequence = ManagedJudgeObservationSequence::new_until(
        process,
        input.eval.judge_schedule(),
        cancellation,
        input.deadline,
    )?;
    ensure_live_operation(cancellation, input.deadline)?;
    let sequence = Rc::new(RefCell::new(sequence));
    let callback_sequence = Rc::clone(&sequence);
    let callback_managed_ollama = input.managed_ollama;
    let callback_runtime = input.runtime_manifest;
    let callback_frozen = input.frozen_components;
    let callback_worker_limits = input.worker_limits;
    let callback_deadline = input.deadline;
    let callback_cancellation = cancellation;
    let callback_package_id = &package_id;
    let callback_members = &retained_members;
    let callback_worker = retained_worker;
    let callback = move |observation: OllamaResponseObservation| {
        observe_schedule_response(
            &callback_sequence,
            observation,
            callback_runtime,
            callback_package_id,
            callback_worker,
            callback_members,
            callback_frozen,
            callback_managed_ollama,
            callback_worker_limits,
            callback_deadline,
            callback_cancellation,
        )
    };
    let session_bytes = usize::try_from(input.preflight_plan.maximum_session_body_bytes)
        .map_err(|_| invalid_preflight())?;
    let config = OllamaRetainedStreamSessionConfig::new(
        endpoint,
        vec![input.model.clone()],
        OllamaLimits::default(),
        session_bytes,
    )
    .map_err(|error| {
        ManagedJudgeScheduleRunnerPrimaryFailure::Generation(
            crate::LocalOllamaManagedGenerationError::Session(error),
        )
    })?;
    ensure_live_operation(cancellation, input.deadline)?;
    let mut session = config
        .open(
            stream,
            OperationContext::new(cancellation, Some(input.deadline)),
            callback,
        )
        .await
        .map_err(|error| map_session_at_operation(error, cancellation, input.deadline))?;
    ensure_live_operation(cancellation, input.deadline)?;
    let api_preflight = session
        .preflight(OperationContext::new(cancellation, Some(input.deadline)))
        .await
        .map_err(|error| map_session_at_operation(error, cancellation, input.deadline))?;
    ensure_live_operation(cancellation, input.deadline)?;
    let preflight = local_ollama_preflight_report(&input.preflight_plan.preflight, api_preflight)
        .map_err(crate::LocalOllamaManagedPreflightError::Preflight)?;
    ensure_live_operation(cancellation, input.deadline)?;
    let native_limits = cap_native_load_limits(
        input.preflight_limits.native_load,
        remaining_before_deadline(input.deadline).map_err(deadline_failure)?,
    );
    let native_request = NativeLoadObservationRequest {
        package: input.runtime_manifest,
        expected_package_id: &package_id,
        retained_package_members: &retained_members,
        expected_external_components: input.frozen_components.expected_components(),
        limits: native_limits,
    };
    let actual_preflight_limits = LocalOllamaManagedPreflightLimits {
        process: process_limits,
        native_load: native_limits,
    };
    ensure_live_operation(cancellation, input.deadline)?;
    let preflight_observation = sequence
        .try_borrow_mut()
        .map_err(|_| ManagedJudgeScheduleRunnerPrimaryFailure::SequenceAuthority)?
        .complete_preflight_observation_until(&native_request, cancellation, input.deadline)?;
    ensure_live_operation(cancellation, input.deadline)?;
    let preflight_observer_binding_digest = derive_managed_judge_preflight_observer_binding_digest(
        input.eval.judge_schedule().candidate_judge_schedule_id(),
        &preflight_observation,
    );
    ensure_live_operation(cancellation, input.deadline)?;
    let final_preflight_isolation = input
        .managed_ollama
        .reobserve_until(cancellation, input.deadline)
        .map_err(|error| ManagedJudgeScheduleRunnerPrimaryFailure::Generation(error.into()))?;
    ensure_live_operation(cancellation, input.deadline)?;
    validate_final_isolation(&initial_isolation, &final_preflight_isolation)?;
    ensure_live_operation(cancellation, input.deadline)?;
    input
        .runtime_package
        .revalidate(cancellation)
        .map_err(crate::LocalOllamaManagedPreflightError::Package)?;
    ensure_live_operation(cancellation, input.deadline)?;
    let report_digests = report_evidence_digests(
        input.runtime_package,
        input.prepared_isolation,
        &launch,
        &initial_isolation,
        &final_preflight_isolation,
        &startup_output,
    )?;
    ensure_live_operation(cancellation, input.deadline)?;
    let managed_preflight = build_report(
        input.runtime_manifest,
        input.preflight_plan,
        helper,
        input.frozen_components.expected_components(),
        actual_preflight_limits,
        report_digests,
        preflight_observation.initial_process().clone(),
        preflight_observation.post_preflight_process().clone(),
        preflight_observation.final_process().clone(),
        preflight_observation.connection_witness().clone(),
        preflight_observation.connection_observations().to_vec(),
        input.admitted_runtime.cloud_disable_version_status(),
        preflight,
        preflight_observation.native_load().clone(),
    )?;
    ensure_live_operation(cancellation, input.deadline)?;
    let managed_build = bind_successful_managed_preflight(
        input.runtime_manifest,
        input.preflight_plan,
        &managed_preflight,
    )?;
    ensure_live_operation(cancellation, input.deadline)?;
    if managed_build.runtime_build() != input.runtime_build {
        return Err(ManagedJudgeScheduleRunnerPrimaryFailure::ManagedBuildRelationship);
    }
    ensure_live_operation(cancellation, input.deadline)?;
    sequence
        .try_borrow_mut()
        .map_err(|_| ManagedJudgeScheduleRunnerPrimaryFailure::SequenceAuthority)?
        .seal_preflight_until(preflight_observation, cancellation, input.deadline)?;
    ensure_live_operation(cancellation, input.deadline)?;

    let mut response_ids = Vec::with_capacity(input.eval.judge_schedule().entries().len());
    let mut observations = Vec::with_capacity(input.eval.judge_schedule().entries().len());
    let mut retained_session_preflight_digest = None;
    for schedule_cursor in 0..input.eval.judge_schedule().entries().len() {
        ensure_live_operation(cancellation, input.deadline)?;
        let attempt = attempt::run_attempt(
            &mut input,
            &initial_isolation,
            &native_request,
            &sequence,
            &mut session,
            schedule_cursor,
            cancellation,
        )
        .await?;
        ensure_live_operation(cancellation, input.deadline)?;
        match &retained_session_preflight_digest {
            Some(expected) if expected != &attempt.preflight_digest => {
                return Err(ManagedJudgeScheduleRunnerPrimaryFailure::PreflightObserverBinding);
            }
            None => retained_session_preflight_digest = Some(attempt.preflight_digest.clone()),
            Some(_) => {}
        }
        response_ids.push(attempt.response_id);
        observations.push(attempt.observation);
    }
    drop(session);
    ensure_live_operation(cancellation, input.deadline)?;
    let sequence = Rc::try_unwrap(sequence)
        .map_err(|_| ManagedJudgeScheduleRunnerPrimaryFailure::SequenceAuthority)?
        .into_inner();
    ensure_live_operation(cancellation, input.deadline)?;
    let completed_sequence = sequence.finish_until(cancellation, input.deadline)?;
    ensure_live_operation(cancellation, input.deadline)?;
    let retained_session_preflight_digest = retained_session_preflight_digest
        .ok_or(ManagedJudgeScheduleRunnerPrimaryFailure::SequenceAuthority)?;
    ensure_live_operation(cancellation, input.deadline)?;
    Ok(LiveScheduleResult {
        completed_sequence,
        response_ids,
        observations,
        managed_preflight: LocalOllamaManagedPreflightOutcome::new(
            managed_preflight,
            managed_build,
        ),
        actual_preflight_limits,
        retained_session_preflight_digest,
        preflight_observer_binding_digest,
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "the observer callback retains every exact worker and package authority"
)]
fn observe_schedule_response(
    sequence: &Rc<RefCell<ManagedJudgeObservationSequence>>,
    observation: OllamaResponseObservation,
    runtime: &RuntimePackageManifest,
    package_id: &rewrite_model::RuntimePackageManifestId,
    retained_worker: &RetainedNativePackageMember,
    retained_members: &[RetainedNativePackageMember],
    frozen: &VerifiedFrozenExternalNativeComponentSet,
    managed_ollama: &ManagedOllamaIsolationLease<'_>,
    limits: ManagedGenerationWorkerLimits,
    deadline: Instant,
    cancellation: &CancellationToken,
) -> Result<(), ManagedJudgeObservationError> {
    let phase = observation.phase();
    let mut sequence = sequence
        .try_borrow_mut()
        .map_err(|_| ManagedJudgeObservationError::InvalidResponseSequence)?;
    sequence.observe_response_until(observation, cancellation, deadline)?;
    let OllamaResponseObservationPhase::AfterResponse { ordinal } = phase else {
        return Ok(());
    };
    let Some(cursor) = worker_cursor(ordinal) else {
        return Ok(());
    };
    let schedule_cursor =
        u32::try_from(cursor).map_err(|_| ManagedJudgeObservationError::InvalidCount)?;
    let remaining = remaining_before_deadline(deadline).map_err(|_| {
        ManagedJudgeObservationError::Worker(ManagedGenerationWorkerError::DeadlineExceeded)
    })?;
    let limits = cap_worker_limits(limits, remaining);
    sequence.begin_attempt_worker_observation_until(
        schedule_cursor,
        &ManagedGenerationWorkerObservationRequest {
            package: runtime,
            expected_package_id: package_id,
            retained_worker,
            retained_model_weight: managed_ollama.retained_model_weight(),
            profile: ManagedGenerationWorkerProfile::OllamaV0_32_15Cpu,
            limits,
        },
        &ManagedGenerationWorkerNativeLoadRequest {
            package: runtime,
            expected_package_id: package_id,
            retained_package_code: retained_members,
            expected_external_components: frozen.expected_components(),
        },
        cancellation,
        deadline,
    )
}

pub(super) fn ensure_live_operation(
    cancellation: &CancellationToken,
    deadline: Instant,
) -> Result<(), ManagedJudgeScheduleRunnerPrimaryFailure> {
    ensure_operation_active(cancellation, deadline).map_err(|failure| match failure {
        OperationGateFailure::Cancelled => ManagedJudgeScheduleRunnerPrimaryFailure::Cancelled,
        OperationGateFailure::DeadlineExceeded => {
            ManagedJudgeScheduleRunnerPrimaryFailure::DeadlineExceeded
        }
    })
}

fn deadline_failure(
    _error: super::deadline::JoinedRunDeadlineExceeded,
) -> ManagedJudgeScheduleRunnerPrimaryFailure {
    ManagedJudgeScheduleRunnerPrimaryFailure::DeadlineExceeded
}

pub(super) fn map_session_at_operation(
    error: rewrite_ollama::OllamaObservedSessionError<ManagedJudgeObservationError>,
    cancellation: &CancellationToken,
    deadline: Instant,
) -> ManagedJudgeScheduleRunnerPrimaryFailure {
    match ensure_operation_active(cancellation, deadline) {
        Err(OperationGateFailure::Cancelled) => ManagedJudgeScheduleRunnerPrimaryFailure::Cancelled,
        Err(OperationGateFailure::DeadlineExceeded) => {
            ManagedJudgeScheduleRunnerPrimaryFailure::DeadlineExceeded
        }
        Ok(()) => ManagedJudgeScheduleRunnerPrimaryFailure::Session(error),
    }
}

fn worker_cursor(ordinal: usize) -> Option<usize> {
    const PREFLIGHT: usize = 7;
    const ATTEMPT: usize = 9;
    const WORKER: usize = 4;
    let after_preflight = ordinal.checked_sub(PREFLIGHT)?;
    (after_preflight % ATTEMPT == WORKER).then_some(after_preflight / ATTEMPT)
}

fn invalid_preflight() -> ManagedJudgeScheduleRunnerPrimaryFailure {
    crate::LocalOllamaManagedPreflightError::InvalidInput.into()
}

#[cfg(test)]
mod tests {
    use super::worker_cursor;

    #[test]
    fn worker_cursor_accepts_only_exact_attempt_offset_four() {
        for ordinal in 0..=40 {
            let expected = match ordinal {
                11 => Some(0),
                20 => Some(1),
                29 => Some(2),
                38 => Some(3),
                _ => None,
            };
            assert_eq!(worker_cursor(ordinal), expected);
        }
    }
}
