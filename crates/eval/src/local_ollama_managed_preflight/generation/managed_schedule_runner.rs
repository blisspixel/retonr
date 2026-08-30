//! Single-launch execution of one exact managed candidate-judge schedule.

use std::time::Instant;

use rewrite_app::{
    ManagedJudgeEffectivePackagePlanInput, ManagedJudgeObservationAuthorityCompiler,
    ManagedJudgeObservationAuthorityInput, PackageAttestationService,
    VerifiedManagedJudgeEffectivePackagePlan,
};
use rewrite_model::{CandidateJudgeObservationBatchV1, CandidateJudgeResponseAggregateV1};
use rewrite_types::CancellationToken;

use super::live_lifecycle::GenerationQualificationLiveLifecycle;
use super::runner_configuration::{
    ManagedJudgeRunnerContext, VerifiedManagedJudgeRunnerConfiguration,
};

mod deadline;
mod error;
mod finalization;
mod live;
mod outcome;
mod post_release;

#[cfg(test)]
pub(in crate::local_ollama_managed_preflight::generation) use deadline::remaining_before_deadline_at;
pub(in crate::local_ollama_managed_preflight::generation) use deadline::{
    OperationGateFailure, OperationPrecedenceError, ensure_operation_active, operation_precedence,
};
use deadline::{
    cap_preflight_limits, cap_worker_limits, ensure_before_deadline, remaining_before_deadline,
};
pub(super) use error::ManagedJudgeScheduleRunnerError;
use error::ManagedJudgeScheduleRunnerPrimaryFailure;
use finalization::finalize_failed_live_schedule;
use live::{ensure_live_operation, run_live_schedule};
pub(super) use outcome::{
    ManagedJudgeScheduleAuthorityFailures, ManagedJudgeScheduleExecutionAuthorityError,
    ManagedJudgeScheduleExecutionView, VerifiedManagedJudgeScheduleExecution,
};
use post_release::{ReleaseDisposition, classify_release, finalize_invalid_released_execution};

/// Executes one exact judge schedule through one managed process and one session.
///
/// The result is content-free and triage-only. It does not prove handler
/// execution, model use, semantic correctness, or qualification.
#[expect(
    clippy::too_many_lines,
    reason = "the launch-to-cleanup protocol keeps every mandatory phase boundary visible"
)]
pub(super) async fn run_managed_judge_schedule<'store, 'records, 'model, 'runtime>(
    configuration: VerifiedManagedJudgeRunnerConfiguration<'store, 'records, 'model, 'runtime, '_>,
    deadline: Instant,
    lifecycle: &GenerationQualificationLiveLifecycle,
    cancellation: &CancellationToken,
) -> Result<
    VerifiedManagedJudgeScheduleExecution<'store, 'records, 'model, 'runtime>,
    ManagedJudgeScheduleRunnerError,
> {
    let mut live_reservation = lifecycle
        .reserve()
        .map_err(|_error| ManagedJudgeScheduleRunnerError::LiveReservationRefused)?;
    match ensure_operation_active(cancellation, deadline) {
        Ok(()) => {}
        Err(OperationGateFailure::Cancelled) => {
            return Err(ManagedJudgeScheduleRunnerError::CancelledBeforeLaunch);
        }
        Err(OperationGateFailure::DeadlineExceeded) => {
            return Err(ManagedJudgeScheduleRunnerError::DeadlineExceededBeforeLaunch);
        }
    }
    let context_result = configuration.into_revalidated_runner_context(cancellation);
    let context = match operation_precedence(context_result, cancellation, deadline) {
        Ok(context) => context,
        Err(OperationPrecedenceError::DeadlineExceeded) => {
            return Err(ManagedJudgeScheduleRunnerError::DeadlineExceededBeforeLaunch);
        }
        Err(OperationPrecedenceError::Cancelled) => {
            return Err(ManagedJudgeScheduleRunnerError::CancelledBeforeLaunch);
        }
        Err(OperationPrecedenceError::Underlying(error)) => {
            return Err(ManagedJudgeScheduleRunnerError::Configuration(error));
        }
    };

    let ManagedJudgeRunnerContext {
        eval,
        runtime_package,
        launch_plan,
        model_package,
        characterized_package,
        runtime_manifest,
        runtime_build,
        admitted_runtime,
        generation_path,
        frozen_components,
        prepared_isolation,
        app_static_model,
        expected_runtime_state,
        app_judge_plan,
        app_judge_schedule,
        app_request_aggregate,
        app_judge_system,
        runtime_installation_generation,
        model_installation_generation,
        preflight_plan,
        preflight_limits,
        worker_limits,
        model_evidence: _model_evidence,
        model,
    } = context;

    let launch_result = PackageAttestationService::launch_managed_ollama_v0_32_15_until(
        runtime_manifest,
        runtime_package,
        admitted_runtime,
        prepared_isolation,
        launch_plan,
        cancellation,
        deadline,
    );
    let managed_ollama = match launch_result {
        Ok(managed_ollama) => {
            live_reservation.mark_authority_acquired();
            managed_ollama
        }
        Err(error) => return Err(ManagedJudgeScheduleRunnerError::Launch(error)),
    };
    if let Err(primary) = ensure_live_operation(cancellation, deadline) {
        return Err(finalize_failed_live_schedule(
            primary,
            managed_ollama,
            model_package,
            runtime_package,
            cancellation,
            deadline,
        ));
    }
    let Ok(remaining) = remaining_before_deadline(deadline) else {
        return Err(finalize_failed_live_schedule(
            ManagedJudgeScheduleRunnerPrimaryFailure::DeadlineExceeded,
            managed_ollama,
            model_package,
            runtime_package,
            cancellation,
            deadline,
        ));
    };
    let preflight_limits = cap_preflight_limits(preflight_limits, remaining);
    let worker_limits = cap_worker_limits(worker_limits, remaining);

    let live = match run_live_schedule(
        live::LiveScheduleInput {
            eval: &eval,
            runtime_manifest,
            runtime_package,
            runtime_build,
            admitted_runtime,
            generation_path,
            frozen_components,
            prepared_isolation,
            expected_runtime_state: &expected_runtime_state,
            preflight_plan: &preflight_plan,
            preflight_limits,
            worker_limits,
            model: &model,
            managed_ollama: &managed_ollama,
            deadline,
        },
        cancellation,
    )
    .await
    {
        Ok(value) => value,
        Err(primary) => {
            return Err(finalize_failed_live_schedule(
                primary,
                managed_ollama,
                model_package,
                runtime_package,
                cancellation,
                deadline,
            ));
        }
    };

    if let Err(primary) = ensure_live_operation(cancellation, deadline) {
        return Err(finalize_failed_live_schedule(
            primary,
            managed_ollama,
            model_package,
            runtime_package,
            cancellation,
            deadline,
        ));
    }

    let authority = ManagedJudgeObservationAuthorityCompiler::compile(
        ManagedJudgeObservationAuthorityInput {
            judge_plan: app_judge_plan,
            judge_schedule: app_judge_schedule,
            completed_sequence: live.completed_sequence,
        },
        cancellation,
    )
    .map_err(ManagedJudgeScheduleRunnerPrimaryFailure::ObservationAuthority);
    let authority = match authority {
        Ok(authority) => authority,
        Err(primary) => {
            return Err(finalize_failed_live_schedule(
                primary,
                managed_ollama,
                model_package,
                runtime_package,
                cancellation,
                deadline,
            ));
        }
    };
    if let Err(primary) = ensure_live_operation(cancellation, deadline) {
        return Err(finalize_failed_live_schedule(
            primary,
            managed_ollama,
            model_package,
            runtime_package,
            cancellation,
            deadline,
        ));
    }
    if authority.preflight_observer_binding_digest() != &live.preflight_observer_binding_digest {
        return Err(finalize_failed_live_schedule(
            ManagedJudgeScheduleRunnerPrimaryFailure::PreflightObserverBinding,
            managed_ollama,
            model_package,
            runtime_package,
            cancellation,
            deadline,
        ));
    }
    if let Err(primary) = ensure_live_operation(cancellation, deadline) {
        return Err(finalize_failed_live_schedule(
            primary,
            managed_ollama,
            model_package,
            runtime_package,
            cancellation,
            deadline,
        ));
    }
    if authority.retained_session_preflight_digest() != &live.retained_session_preflight_digest {
        return Err(finalize_failed_live_schedule(
            ManagedJudgeScheduleRunnerPrimaryFailure::RetainedSessionPreflightBinding,
            managed_ollama,
            model_package,
            runtime_package,
            cancellation,
            deadline,
        ));
    }

    let response_aggregate = match CandidateJudgeResponseAggregateV1::new(
        eval.judge_plan(),
        eval.judge_schedule(),
        eval.request_aggregate(),
        live.response_ids,
    ) {
        Ok(value) => value,
        Err(error) => {
            return Err(finalize_failed_live_schedule(
                ManagedJudgeScheduleRunnerPrimaryFailure::PortableContract(error),
                managed_ollama,
                model_package,
                runtime_package,
                cancellation,
                deadline,
            ));
        }
    };
    if let Err(primary) = ensure_live_operation(cancellation, deadline) {
        return Err(finalize_failed_live_schedule(
            primary,
            managed_ollama,
            model_package,
            runtime_package,
            cancellation,
            deadline,
        ));
    }
    let observation_batch = match CandidateJudgeObservationBatchV1::new(
        eval.judge_plan(),
        eval.judge_schedule(),
        eval.request_aggregate(),
        live.observations,
    ) {
        Ok(value) => value,
        Err(error) => {
            return Err(finalize_failed_live_schedule(
                ManagedJudgeScheduleRunnerPrimaryFailure::PortableContract(error),
                managed_ollama,
                model_package,
                runtime_package,
                cancellation,
                deadline,
            ));
        }
    };
    if let Err(primary) = ensure_live_operation(cancellation, deadline) {
        return Err(finalize_failed_live_schedule(
            primary,
            managed_ollama,
            model_package,
            runtime_package,
            cancellation,
            deadline,
        ));
    }
    if authority.judge_plan_id() != eval.judge_plan().candidate_judge_plan_id()
        || authority.judge_schedule_id() != eval.judge_schedule().candidate_judge_schedule_id()
        || authority.attempt_count() != u64::from(eval.judge_schedule().entry_count())
        || app_request_aggregate != *eval.request_aggregate()
        || app_judge_system != *eval.judge_system()
        || response_aggregate.candidate_judge_plan_id()
            != eval.judge_plan().candidate_judge_plan_id()
        || response_aggregate.candidate_judge_schedule_id()
            != eval.judge_schedule().candidate_judge_schedule_id()
        || response_aggregate.candidate_judge_request_aggregate_id()
            != eval.request_aggregate().request_aggregate_id()
        || observation_batch.candidate_judge_plan_id()
            != eval.judge_plan().candidate_judge_plan_id()
        || observation_batch.candidate_judge_schedule_id()
            != eval.judge_schedule().candidate_judge_schedule_id()
        || observation_batch.candidate_judge_request_aggregate_id()
            != eval.request_aggregate().request_aggregate_id()
        || response_aggregate.entry_count() != observation_batch.entry_count()
        || observation_batch
            .observations()
            .iter()
            .zip(response_aggregate.responses())
            .any(|(observation, response)| {
                observation
                    .candidate_judge_response()
                    .candidate_judge_response_id()
                    != response.candidate_judge_response_id()
            })
    {
        return Err(finalize_failed_live_schedule(
            ManagedJudgeScheduleRunnerPrimaryFailure::PortableOutputRelationship,
            managed_ollama,
            model_package,
            runtime_package,
            cancellation,
            deadline,
        ));
    }
    if let Err(primary) = ensure_live_operation(cancellation, deadline) {
        return Err(finalize_failed_live_schedule(
            primary,
            managed_ollama,
            model_package,
            runtime_package,
            cancellation,
            deadline,
        ));
    }

    let effective_plan = VerifiedManagedJudgeEffectivePackagePlan::prepare(
        ManagedJudgeEffectivePackagePlanInput {
            observation_authority: authority,
            request_aggregate: app_request_aggregate,
            judge_system: app_judge_system,
            expected_runtime_state,
            runtime_manifest,
            runtime_package,
            runtime_build,
            admitted_runtime,
            generation_path,
            frozen_components,
            prepared_isolation,
            model_package,
            static_model: app_static_model,
            characterized_package,
            runtime_installation_generation,
            model_installation_generation,
            managed_ollama,
        },
        cancellation,
    )
    .map_err(ManagedJudgeScheduleRunnerError::EffectivePackagePlan)?;
    let cancelled_after_prepare = cancellation.is_cancelled();
    let deadline_exceeded_after_prepare = ensure_before_deadline(deadline).is_err();
    let release = effective_plan.release();
    let cancelled = cancelled_after_prepare || cancellation.is_cancelled();
    let deadline_exceeded =
        deadline_exceeded_after_prepare || ensure_before_deadline(deadline).is_err();
    let released = match classify_release(release, cancelled, deadline_exceeded) {
        ReleaseDisposition::Released(released) => released,
        ReleaseDisposition::Failed {
            source,
            cancelled,
            deadline_exceeded,
        } => {
            return Err(ManagedJudgeScheduleRunnerError::EffectivePackageRelease {
                source: Box::new(source),
                cancelled,
                deadline_exceeded,
            });
        }
        ReleaseDisposition::Invalidated {
            released,
            cancelled,
            deadline_exceeded,
        } => {
            let finalization = finalize_invalid_released_execution(
                &eval,
                &released,
                model_package,
                runtime_package,
            );
            return Err(ManagedJudgeScheduleRunnerError::PostReleaseInvalidated {
                cancelled,
                deadline_exceeded,
                finalization,
            });
        }
    };

    Ok(VerifiedManagedJudgeScheduleExecution::new(
        eval,
        runtime_manifest,
        frozen_components,
        preflight_plan,
        live.actual_preflight_limits,
        model_package,
        runtime_package,
        released,
        response_aggregate,
        observation_batch,
        live.managed_preflight,
    ))
}
