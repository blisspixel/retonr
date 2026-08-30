use rewrite_app::{
    ManagedOllamaIsolationLease, PackageAttestationService, RuntimePackageLease,
    VerifiedManagedOllamaLaunchPlan,
};
use rewrite_inference::StructuredCompletionRequest;
use rewrite_model::{CandidateGenerationAttemptFailurePhaseV1, RuntimePackageMember};
use rewrite_ollama::OllamaCloudDisableVersionStatus;
use rewrite_types::CancellationToken;

use super::{ManagedCandidateAttemptProgress, ManagedCandidateAttemptRunInput};
use crate::local_ollama_managed_preflight::generation::{
    LocalOllamaManagedGenerationError, deadline::CandidateOperationDeadline,
    live_lifecycle::GenerationQualificationLiveReservation,
};
use crate::local_ollama_managed_preflight::validation::{
    exact_helper_member, validate_static_inputs,
};
use crate::local_ollama_managed_preflight::{
    LocalOllamaManagedPreflightError,
    generation::validation::{
        validate_generation_authority, validate_generation_binding, validate_managed_input_binding,
    },
};

pub(super) struct PreparedCandidateLaunch<'input, 'model> {
    pub(super) helper: &'input RuntimePackageMember,
    pub(super) cloud_status: OllamaCloudDisableVersionStatus,
    pub(super) isolation_lease: ManagedOllamaIsolationLease<'model>,
}

pub(super) struct CandidateLaunchOperation<'operation, 'lifecycle> {
    pub(super) progress: &'operation ManagedCandidateAttemptProgress,
    pub(super) operation_deadline: CandidateOperationDeadline,
    pub(super) live_reservation: &'operation mut GenerationQualificationLiveReservation<'lifecycle>,
    pub(super) cancellation: &'operation CancellationToken,
}

fn launch_candidate<'model>(
    input: &ManagedCandidateAttemptRunInput<'_>,
    runtime_package: &mut RuntimePackageLease,
    launch_plan: VerifiedManagedOllamaLaunchPlan<'model>,
    operation_deadline: CandidateOperationDeadline,
    cancellation: &CancellationToken,
) -> Result<ManagedOllamaIsolationLease<'model>, LocalOllamaManagedGenerationError> {
    match operation_deadline.instant() {
        Some(deadline) => PackageAttestationService::launch_managed_ollama_v0_32_15_until(
            input.runtime_manifest,
            runtime_package,
            input.admitted_runtime,
            input.isolation,
            launch_plan,
            cancellation,
            deadline,
        ),
        None => PackageAttestationService::launch_managed_ollama_v0_32_15(
            input.runtime_manifest,
            runtime_package,
            input.admitted_runtime,
            input.isolation,
            launch_plan,
            cancellation,
        ),
    }
    .map_err(LocalOllamaManagedGenerationError::from)
}

fn validate_candidate_launch_inputs<'input>(
    input: &ManagedCandidateAttemptRunInput<'input>,
    runtime_package: &mut RuntimePackageLease,
    launch_plan: &VerifiedManagedOllamaLaunchPlan<'_>,
    structured_request: &StructuredCompletionRequest,
    operation_deadline: CandidateOperationDeadline,
    cancellation: &CancellationToken,
) -> Result<
    (
        &'input RuntimePackageMember,
        OllamaCloudDisableVersionStatus,
    ),
    LocalOllamaManagedGenerationError,
> {
    macro_rules! step {
        ($result:expr) => {{
            operation_deadline.ensure_active(cancellation)?;
            let result = $result;
            operation_deadline.precedence(result, cancellation)?
        }};
    }

    step!(validate_generation_authority(
        input.runtime_manifest,
        input.preflight_plan,
        input.admitted_runtime,
        input.generation_path,
        input.frozen_components,
    ));
    step!(
        validate_static_inputs(
            input.runtime_manifest,
            runtime_package,
            input.preflight_plan,
            input.frozen_components.expected_components(),
            input.limits,
        )
        .map_err(LocalOllamaManagedGenerationError::from)
    );
    step!(
        validate_generation_binding(
            input.runtime_manifest,
            input.preflight_plan,
            input.static_model,
            input.model,
            structured_request,
        )
        .map_err(LocalOllamaManagedGenerationError::from)
    );
    step!(
        validate_managed_input_binding(
            launch_plan,
            input.static_model,
            input.model,
            input.preflight_plan,
        )
        .map_err(LocalOllamaManagedGenerationError::from)
    );
    step!(
        input
            .worker_limits
            .validate()
            .map_err(LocalOllamaManagedGenerationError::from)
    );
    step!(
        runtime_package
            .revalidate(cancellation)
            .map_err(LocalOllamaManagedPreflightError::Package)
            .map_err(LocalOllamaManagedGenerationError::from)
    );
    let preparation = step!(Ok::<_, LocalOllamaManagedGenerationError>(
        input.isolation.preparation_evidence()
    ));
    step!(
        preparation
            .all_canaries_passed()
            .then_some(())
            .ok_or(LocalOllamaManagedPreflightError::InvalidHelperBinding)
            .map_err(LocalOllamaManagedGenerationError::from)
    );
    let helper = step!(
        exact_helper_member(
            input.runtime_manifest,
            preparation.helper_digest(),
            preparation.helper_bytes(),
        )
        .map_err(LocalOllamaManagedGenerationError::from)
    );
    let cloud_status = input.admitted_runtime.cloud_disable_version_status();
    Ok((helper, cloud_status))
}

pub(super) fn validate_and_launch_candidate<'input, 'model>(
    input: &ManagedCandidateAttemptRunInput<'input>,
    runtime_package: &mut RuntimePackageLease,
    launch_plan: VerifiedManagedOllamaLaunchPlan<'model>,
    structured_request: &StructuredCompletionRequest,
    operation: CandidateLaunchOperation<'_, '_>,
) -> Result<PreparedCandidateLaunch<'input, 'model>, LocalOllamaManagedGenerationError> {
    let CandidateLaunchOperation {
        progress,
        operation_deadline,
        live_reservation,
        cancellation,
    } = operation;
    let (helper, cloud_status) = validate_candidate_launch_inputs(
        input,
        runtime_package,
        &launch_plan,
        structured_request,
        operation_deadline,
        cancellation,
    )?;
    operation_deadline.ensure_active(cancellation)?;
    progress.set_phase(CandidateGenerationAttemptFailurePhaseV1::Launch);
    let launch_result = launch_candidate(
        input,
        runtime_package,
        launch_plan,
        operation_deadline,
        cancellation,
    );
    let isolation_lease = match launch_result {
        Ok(isolation_lease) => {
            live_reservation.mark_authority_acquired();
            isolation_lease
        }
        Err(error) => operation_deadline.precedence(Err(error), cancellation)?,
    };
    let isolation_lease = operation_deadline.precedence(Ok(isolation_lease), cancellation)?;
    progress.set_phase(CandidateGenerationAttemptFailurePhaseV1::PreTrafficRevalidation);
    operation_deadline.ensure_active(cancellation)?;
    Ok(PreparedCandidateLaunch {
        helper,
        cloud_status,
        isolation_lease,
    })
}
