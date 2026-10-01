use std::{cell::RefCell, rc::Rc};

use rewrite_app::{
    MANAGED_OLLAMA_V0_32_15_ENDPOINT, ManagedOllamaIsolationLease, RuntimePackageLease,
    VerifiedAdmittedRuntime, VerifiedManagedGenerationPath, managed_ollama_v0_32_15_launch_spec,
};
use rewrite_inference::{StructuredCompletionRequest, StructuredCompletionResponse};
use rewrite_model::RuntimePackageManifest;
use rewrite_ollama::{
    OllamaCloudDisableVersionStatus, OllamaEndpoint, OllamaLimits, OllamaModelBinding,
    OllamaResidentResourceObservedCompletion, OllamaResidentSessionExecutionReceipt,
    OllamaRetainedStreamSessionConfig,
};
use rewrite_runtime_attestor::{
    AttachedProcessLease, ListenerEndpoint, ManagedGenerationWorkerLimits,
    NativeManagedLinuxProcessObserver, VerifiedFrozenExternalNativeComponentSet,
};
use rewrite_runtime_isolation::PreparedIsolation;
use rewrite_types::CancellationToken;

use crate::{
    LocalOllamaBoundPreflightError, LocalOllamaBoundPreflightPlan, LocalOllamaModelBindingEvidence,
    local_ollama_bound_preflight::ConnectionObservationSequence,
    local_ollama_preflight::local_ollama_preflight_report,
};

use super::{
    LocalOllamaManagedPreflightError, LocalOllamaManagedPreflightLimits,
    bind_successful_managed_preflight,
    report::{build_report, report_evidence_digests},
    validation::{
        managed_expectation, validate_cloud_launch, validate_final_isolation,
        validate_isolation_binding, validate_process_binding,
    },
};

mod deadline;
mod effective_state_join;
mod evidence;
mod final_observation;
mod live_lifecycle;
mod managed_attempt;
mod managed_candidate_judge;
mod managed_local_judge_receipt;
mod managed_schedule_runner;
mod runner;
mod runner_configuration;
mod validation;
mod verified_candidate_judge_join;
mod verified_repeatability_joins;

use deadline::CandidateOperationDeadline;
pub use evidence::{
    LOCAL_OLLAMA_MANAGED_GENERATION_EVIDENCE_SCHEMA_VERSION, LocalOllamaManagedGenerationEvidence,
    MANAGED_OLLAMA_GENERATION_BRACKET_OBSERVATION_SCHEMA_VERSION,
    ManagedOllamaGenerationBracketObservationV1,
};
use final_observation::{FinalObservationInput, finish_final_observation};
pub(crate) use live_lifecycle::GenerationQualificationLiveLifecycle;
pub(crate) use managed_attempt::ResourceObservedManagedCandidateAttemptClosure;
pub use managed_attempt::{
    FailedManagedCandidateAttempt, ManagedCandidateAttemptCleanupFailures,
    ManagedCandidateAttemptExecutionError, ManagedCandidateAttemptExecutionOutcome,
    ManagedCandidateAttemptFailureRecordError, ManagedCandidateAttemptPrimaryFailure,
    ManagedCandidateAttemptRunInput, VerifiedCompletedManagedCandidateAttempt,
};
pub(crate) use managed_attempt::{
    ResourceObservedManagedCandidateAttemptRunInput,
    run_resource_observed_verified_managed_candidate_attempt_until,
    run_verified_managed_candidate_attempt_until,
};
pub(crate) use managed_candidate_judge::run_verified_managed_candidate_judge_until;
pub use managed_candidate_judge::{
    ManagedCandidateJudgeRunError, ManagedCandidateJudgeRunErrorKind,
};
pub use runner::run_local_ollama_managed_generation;
use validation::{
    ManagedGenerationSessionObservationError, ManagedSessionObserver, exact_retained_worker,
    map_session_error, observe_native_load, observe_response_and_worker, reobserve_process,
};
pub use verified_candidate_judge_join::{
    VerifiedCandidateJudgeJoin, VerifiedCandidateJudgeJoinRevalidationError,
    VerifiedCandidateJudgeJoinRevalidationErrorKind,
};
pub use verified_repeatability_joins::{
    CompletePassedRepeatabilityRelations, GenerationQualificationResourcePhaseAuthorityError,
    GenerationQualificationResourcePhaseCompilationError,
    GenerationQualificationResourcePhaseCompiler,
    GenerationQualificationResourcePhaseDerivationError, VerifiedCompletePassedRepeatabilityJoins,
    VerifiedCompletePassedRepeatabilityJoinsError, VerifiedGenerationQualificationResourcePhase,
    VerifiedPassedRepeatabilityJoins, VerifiedPassedRepeatabilityJoinsError,
    VerifiedPassedRepeatabilityJoinsErrorKind, verify_complete_passed_repeatability_joins,
    verify_passed_repeatability_joins,
};

mod outcome;
#[cfg(test)]
use outcome::finalize_retained_bracket;
pub use outcome::{LocalOllamaManagedGenerationError, LocalOllamaManagedGenerationOutcome};
use outcome::{
    PendingLocalOllamaManagedGenerationOutcome, PendingManagedGenerationCompletion,
    PendingResourceObservedGenerationCompletion,
};

#[derive(Clone, Copy)]
pub(super) enum ManagedGenerationObservationMode {
    Compatibility,
    ResourceObserved,
}

pub(super) enum LiveManagedGenerationCompletion {
    Compatibility {
        response: Box<StructuredCompletionResponse>,
        receipt: Box<OllamaResidentSessionExecutionReceipt>,
    },
    ResourceObserved(Box<OllamaResidentResourceObservedCompletion>),
}

impl LiveManagedGenerationCompletion {
    const fn receipt(&self) -> &OllamaResidentSessionExecutionReceipt {
        match self {
            Self::Compatibility { receipt, .. } => receipt,
            Self::ResourceObserved(completion) => completion.resident_execution_receipt(),
        }
    }
}
#[expect(
    clippy::too_many_arguments,
    reason = "the linear evidence join keeps all frozen capabilities visible"
)]
#[expect(
    clippy::too_many_lines,
    reason = "the security-sensitive operation order is intentionally linear"
)]
async fn run_live_generation(
    package: &RuntimePackageManifest,
    package_lease: &mut RuntimePackageLease,
    isolation: &PreparedIsolation,
    plan: &LocalOllamaBoundPreflightPlan,
    helper: &rewrite_model::RuntimePackageMember,
    admitted_runtime: &VerifiedAdmittedRuntime,
    generation_path: &VerifiedManagedGenerationPath,
    frozen_external_components: &VerifiedFrozenExternalNativeComponentSet,
    cloud_status: OllamaCloudDisableVersionStatus,
    limits: LocalOllamaManagedPreflightLimits,
    worker_limits: ManagedGenerationWorkerLimits,
    static_model: &LocalOllamaModelBindingEvidence,
    model: &OllamaModelBinding,
    request: StructuredCompletionRequest,
    isolation_lease: &ManagedOllamaIsolationLease<'_>,
    observation_mode: ManagedGenerationObservationMode,
    attempt_progress: &managed_attempt::ManagedCandidateAttemptProgress,
    operation_deadline: CandidateOperationDeadline,
    cancellation: &CancellationToken,
) -> Result<PendingLocalOllamaManagedGenerationOutcome, LocalOllamaManagedGenerationError> {
    macro_rules! candidate_step {
        ($result:expr) => {{
            operation_deadline.ensure_active(cancellation)?;
            let result = $result;
            operation_deadline.precedence(result, cancellation)?
        }};
    }
    macro_rules! candidate_value {
        ($value:expr) => {{
            operation_deadline.ensure_active(cancellation)?;
            let value = $value;
            operation_deadline.ensure_active(cancellation)?;
            value
        }};
    }

    let endpoint = candidate_step!(
        OllamaEndpoint::parse(&plan.preflight.endpoint)
            .map_err(|_error| LocalOllamaManagedPreflightError::InvalidInput)
            .map_err(LocalOllamaManagedGenerationError::from)
    );
    candidate_step!(
        (endpoint.socket_addr() == MANAGED_OLLAMA_V0_32_15_ENDPOINT)
            .then_some(())
            .ok_or(LocalOllamaManagedPreflightError::InvalidInput)
            .map_err(LocalOllamaManagedGenerationError::from)
    );
    let launch = candidate_value!(managed_ollama_v0_32_15_launch_spec());
    candidate_step!(
        (&launch.redacted_digest() == isolation_lease.plain_launch_spec_digest())
            .then_some(())
            .ok_or(LocalOllamaManagedGenerationError::InvalidGenerationAuthority)
    );
    let initial_isolation = candidate_value!(isolation_lease.initial_evidence());
    candidate_step!(
        validate_isolation_binding(&initial_isolation, package)
            .map_err(LocalOllamaManagedGenerationError::from)
    );
    let channel_result = match operation_deadline.instant() {
        Some(deadline) => isolation_lease.connect_loopback_until(cancellation, deadline),
        None => isolation_lease.connect_loopback(cancellation),
    }
    .map_err(LocalOllamaManagedGenerationError::from);
    let channel = candidate_step!(channel_result);
    let (stream, diagnostics, startup_output) = channel.into_parts();
    operation_deadline.ensure_active(cancellation)?;
    candidate_step!(
        validate_cloud_launch(&launch, &startup_output)
            .map_err(LocalOllamaManagedGenerationError::from)
    );

    let expectation = candidate_step!(
        managed_expectation(&initial_isolation).map_err(LocalOllamaManagedGenerationError::from)
    );
    let listener = candidate_step!(
        ListenerEndpoint::new(endpoint.socket_addr())
            .map_err(LocalOllamaManagedPreflightError::Witness)
            .map_err(LocalOllamaManagedGenerationError::from)
    );
    operation_deadline.ensure_active(cancellation)?;
    let process_result = match operation_deadline.instant() {
        Some(deadline) => NativeManagedLinuxProcessObserver.attach_until(
            listener,
            diagnostics.into_file(),
            expectation,
            limits.process,
            cancellation,
            deadline,
        ),
        None => NativeManagedLinuxProcessObserver.attach(
            listener,
            diagnostics.into_file(),
            expectation,
            limits.process,
            cancellation,
        ),
    }
    .map_err(LocalOllamaManagedPreflightError::Witness)
    .map_err(LocalOllamaManagedGenerationError::from);
    let process = operation_deadline.precedence(process_result, cancellation)?;
    let initial_process = candidate_value!(process.initial_evidence().clone());
    candidate_step!(
        validate_process_binding(&initial_process, package)
            .map_err(LocalOllamaManagedGenerationError::from)
    );

    let package_id = candidate_value!(package.runtime_package_manifest_id());
    let retained_members = candidate_step!(
        package_lease
            .clone_members_for_native_observation(cancellation)
            .map_err(LocalOllamaManagedPreflightError::Package)
            .map_err(LocalOllamaManagedGenerationError::from)
    );
    let retained_worker =
        candidate_step!(exact_retained_worker(&retained_members, generation_path));

    let preflight_responses = candidate_value!(plan.preflight.models.len().saturating_add(6));
    let total_responses = candidate_value!(preflight_responses.saturating_add(9));
    let worker_response_ordinal = candidate_value!(preflight_responses.saturating_add(4));
    let session_bytes = candidate_step!(
        usize::try_from(plan.maximum_session_body_bytes)
            .map_err(|_error| LocalOllamaManagedPreflightError::InvalidInput)
            .map_err(LocalOllamaManagedGenerationError::from)
    );
    let config = candidate_step!(
        OllamaRetainedStreamSessionConfig::new(
            endpoint,
            vec![model.clone()],
            OllamaLimits::default(),
            session_bytes,
        )
        .map_err(LocalOllamaManagedGenerationError::Session)
    );
    let observer = candidate_value!(Rc::new(RefCell::new(ManagedSessionObserver {
        process,
        connections: ConnectionObservationSequence::new(total_responses),
        worker: None,
    })));
    let callback_observer = Rc::clone(&observer);
    let callback_package_id = &package_id;
    let callback_retained_members = &retained_members;
    let callback_isolation_lease = isolation_lease;
    let callback_attempt_progress = attempt_progress;
    let callback_operation_deadline = operation_deadline;
    operation_deadline.ensure_active(cancellation)?;
    let session_result = config
        .open(
            stream,
            operation_deadline.context(cancellation),
            move |observation| {
                callback_operation_deadline
                    .ensure_active(cancellation)
                    .map_err(ManagedGenerationSessionObservationError::Gate)?;
                callback_attempt_progress.observe_response(
                    observation.phase(),
                    preflight_responses,
                    worker_response_ordinal,
                );
                callback_operation_deadline
                    .ensure_active(cancellation)
                    .map_err(ManagedGenerationSessionObservationError::Gate)?;
                let mut state = callback_observer.try_borrow_mut().map_err(|_error| {
                    ManagedGenerationSessionObservationError::Connection(
                        LocalOllamaBoundPreflightError::InvalidObservationSequence,
                    )
                })?;
                observe_response_and_worker(
                    &mut state,
                    observation,
                    worker_response_ordinal,
                    package,
                    callback_package_id,
                    retained_worker,
                    callback_retained_members,
                    frozen_external_components,
                    callback_isolation_lease,
                    worker_limits,
                    callback_operation_deadline,
                    cancellation,
                )
            },
        )
        .await
        .map_err(map_session_error);
    let mut session = operation_deadline.precedence(session_result, cancellation)?;
    operation_deadline.ensure_active(cancellation)?;
    let api_preflight = session
        .preflight(operation_deadline.context(cancellation))
        .await
        .map_err(map_session_error);
    let api_preflight = operation_deadline.precedence(api_preflight, cancellation)?;
    let post_preflight_process =
        reobserve_process(&observer, package, operation_deadline, cancellation)?;
    let preflight_connections = candidate_step!((|| {
        let state = observer
            .try_borrow()
            .map_err(|_error| LocalOllamaManagedPreflightError::InvalidEvidenceBinding)
            .map_err(LocalOllamaManagedGenerationError::from)?;
        state
            .connections
            .validate_progress(preflight_responses)
            .map_err(LocalOllamaManagedPreflightError::BoundObservation)
            .map_err(LocalOllamaManagedGenerationError::from)?;
        Ok(state.connections.evidence().to_vec())
    })());
    let preflight = candidate_step!(
        local_ollama_preflight_report(&plan.preflight, api_preflight)
            .map_err(LocalOllamaManagedPreflightError::Preflight)
            .map_err(LocalOllamaManagedGenerationError::from)
    );

    let preflight_native_load = observe_native_load(
        &observer,
        package,
        &retained_members,
        frozen_external_components,
        limits,
        operation_deadline,
        cancellation,
    )?;
    let preflight_final_process =
        reobserve_process(&observer, package, operation_deadline, cancellation)?;
    let isolation_result = match operation_deadline.instant() {
        Some(deadline) => isolation_lease.reobserve_until(cancellation, deadline),
        None => isolation_lease.reobserve(cancellation),
    }
    .map_err(LocalOllamaManagedGenerationError::from);
    let preflight_final_isolation = candidate_step!(isolation_result);
    candidate_step!(
        validate_final_isolation(&initial_isolation, &preflight_final_isolation)
            .map_err(LocalOllamaManagedGenerationError::from)
    );
    candidate_step!(
        package_lease
            .revalidate(cancellation)
            .map_err(LocalOllamaManagedPreflightError::Package)
            .map_err(LocalOllamaManagedGenerationError::from)
    );

    let connection_witness = candidate_step!(
        preflight_connections
            .last()
            .cloned()
            .ok_or(LocalOllamaManagedPreflightError::InvalidEvidenceBinding)
            .map_err(LocalOllamaManagedGenerationError::from)
    );
    let report_digests = candidate_step!(
        report_evidence_digests(
            package_lease,
            isolation,
            &launch,
            &initial_isolation,
            &preflight_final_isolation,
            &startup_output,
        )
        .map_err(LocalOllamaManagedGenerationError::from)
    );
    let managed_preflight = candidate_step!(
        build_report(
            package,
            plan,
            helper,
            frozen_external_components.expected_components(),
            limits,
            report_digests,
            initial_process,
            post_preflight_process,
            preflight_final_process,
            connection_witness,
            preflight_connections,
            cloud_status,
            preflight,
            preflight_native_load,
        )
        .map_err(LocalOllamaManagedGenerationError::from)
    );
    let managed_build = candidate_step!(
        bind_successful_managed_preflight(package, plan, &managed_preflight)
            .map_err(LocalOllamaManagedGenerationError::from)
    );

    let retained_request = candidate_value!(request.clone());
    candidate_value!(
        attempt_progress
            .set_phase(rewrite_model::CandidateGenerationAttemptFailurePhaseV1::GenerationTraffic)
    );
    let completion = match observation_mode {
        ManagedGenerationObservationMode::Compatibility => {
            operation_deadline.ensure_active(cancellation)?;
            let completion_result = session
                .complete_structured_with_residency(
                    request,
                    operation_deadline.context(cancellation),
                )
                .await
                .map_err(map_session_error);
            let (response, receipt) =
                operation_deadline.precedence(completion_result, cancellation)?;
            LiveManagedGenerationCompletion::Compatibility {
                response: Box::new(response),
                receipt: Box::new(receipt),
            }
        }
        ManagedGenerationObservationMode::ResourceObserved => {
            operation_deadline.ensure_active(cancellation)?;
            let completion = session
                .complete_structured_with_residency_and_resource_observation_borrowed(
                    &request,
                    operation_deadline.context(cancellation),
                )
                .await
                .map_err(map_session_error);
            let completion = operation_deadline.precedence(completion, cancellation)?;
            LiveManagedGenerationCompletion::ResourceObserved(Box::new(completion))
        }
    };
    operation_deadline.ensure_active(cancellation)?;
    candidate_value!(
        attempt_progress
            .set_phase(rewrite_model::CandidateGenerationAttemptFailurePhaseV1::FinalObservation)
    );
    let pending = finish_final_observation(FinalObservationInput {
        package,
        package_lease,
        admitted_runtime,
        generation_path,
        frozen_components: frozen_external_components,
        limits,
        static_model,
        model,
        isolation_lease,
        observer: &observer,
        initial_isolation: &initial_isolation,
        retained_members: &retained_members,
        retained_request,
        managed_preflight,
        managed_build,
        completion,
        operation_deadline,
        cancellation,
    })?;
    operation_deadline.ensure_active(cancellation)?;
    drop(session);
    operation_deadline.ensure_active(cancellation)?;
    drop(observer);
    operation_deadline.ensure_active(cancellation)?;

    operation_deadline.precedence(Ok(pending), cancellation)
}

#[cfg(test)]
mod tests;

pub(crate) use verified_candidate_judge_join::JudgeSettlementView;

#[cfg(test)]
pub(crate) use verified_candidate_judge_join::synthetic_join;
