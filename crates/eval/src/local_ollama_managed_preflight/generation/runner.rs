use rewrite_app::{
    ManagedOllamaIsolationLease, PackageAttestationService, RuntimePackageLease,
    VerifiedAdmittedRuntime, VerifiedManagedGenerationPath, VerifiedManagedOllamaLaunchPlan,
};
use rewrite_inference::StructuredCompletionRequest;
use rewrite_model::RuntimePackageManifest;
use rewrite_ollama::{OllamaCloudDisableVersionStatus, OllamaModelBinding};
use rewrite_runtime_attestor::{
    ManagedGenerationWorkerLimits, VerifiedFrozenExternalNativeComponentSet,
};
use rewrite_runtime_isolation::PreparedIsolation;
use rewrite_types::CancellationToken;

use crate::{
    LocalOllamaBoundPreflightPlan, LocalOllamaManagedPreflightError,
    LocalOllamaManagedPreflightLimits, LocalOllamaModelBindingEvidence,
};

use super::super::validation::{exact_helper_member, validate_static_inputs};
use super::live_lifecycle::GenerationQualificationLiveLifecycle;
use super::managed_attempt;
use super::outcome::{PendingLocalOllamaManagedGenerationOutcome, finalize_retained_bracket};
use super::validation::{
    validate_generation_authority, validate_generation_binding, validate_managed_input_binding,
};
use super::{
    LocalOllamaManagedGenerationError, LocalOllamaManagedGenerationOutcome,
    ManagedGenerationObservationMode, run_live_generation,
};

/// Runs one structured completion inside one retained managed Linux operation.
///
/// Launch requires an app-owned all-pass admitted-runtime capability, a separate
/// exact package-and-worker generation-path capability, and an independently
/// verified frozen external native-component set. Raw component slices and
/// compile-time runtime allowlists are not accepted at this boundary.
///
/// Static package and model relationships, read-only preflight, native process and
/// load evidence, every direct-connection response, generation, and two equal
/// runtime-reported residency observations are joined before cleanup. The returned
/// result is inert. It constructs and cross-binds the app-observed effective runtime
/// state under this contract, but does not formally prove placement, driver absence,
/// handler execution, model use, resident-page identity, page immutability, semantic
/// correctness, or qualification.
///
/// # Errors
///
/// Returns [`LocalOllamaManagedGenerationError`] for every invalid binding, drift,
/// observation, transport, residency, package, isolation, or cleanup failure.
#[expect(
    clippy::too_many_arguments,
    reason = "each independently frozen trust-boundary input remains explicit"
)]
pub async fn run_local_ollama_managed_generation(
    package: &RuntimePackageManifest,
    package_lease: &mut RuntimePackageLease,
    launch_authorization: VerifiedManagedOllamaLaunchPlan<'_>,
    admitted_runtime: &VerifiedAdmittedRuntime,
    generation_path: &VerifiedManagedGenerationPath,
    frozen_external_components: &VerifiedFrozenExternalNativeComponentSet,
    isolation: &PreparedIsolation,
    plan: &LocalOllamaBoundPreflightPlan,
    limits: LocalOllamaManagedPreflightLimits,
    worker_limits: ManagedGenerationWorkerLimits,
    static_model: &LocalOllamaModelBindingEvidence,
    model: &OllamaModelBinding,
    request: StructuredCompletionRequest,
    cancellation: &CancellationToken,
) -> Result<LocalOllamaManagedGenerationOutcome, LocalOllamaManagedGenerationError> {
    validate_generation_authority(
        package,
        plan,
        admitted_runtime,
        generation_path,
        frozen_external_components,
    )?;
    validate_static_inputs(
        package,
        package_lease,
        plan,
        frozen_external_components.expected_components(),
        limits,
    )?;
    validate_generation_binding(package, plan, static_model, model, &request)?;
    validate_managed_input_binding(&launch_authorization, static_model, model, plan)?;
    worker_limits.validate()?;
    package_lease
        .revalidate(cancellation)
        .map_err(LocalOllamaManagedPreflightError::Package)?;
    let preparation = isolation.preparation_evidence();
    if !preparation.all_canaries_passed() {
        return Err(LocalOllamaManagedPreflightError::InvalidHelperBinding.into());
    }
    let helper = exact_helper_member(
        package,
        preparation.helper_digest(),
        preparation.helper_bytes(),
    )?;
    let lifecycle = GenerationQualificationLiveLifecycle::new();
    let mut live_reservation = lifecycle
        .reserve()
        .map_err(|_error| LocalOllamaManagedGenerationError::InvalidGenerationAuthority)?;
    let isolation_lease = PackageAttestationService::launch_managed_ollama_v0_32_15(
        package,
        package_lease,
        admitted_runtime,
        isolation,
        launch_authorization,
        cancellation,
    )?;
    live_reservation.mark_authority_acquired();

    let outcome = run_retained_generation(
        package,
        package_lease,
        isolation,
        plan,
        helper,
        admitted_runtime,
        generation_path,
        frozen_external_components,
        admitted_runtime.cloud_disable_version_status(),
        limits,
        worker_limits,
        static_model,
        model,
        request,
        isolation_lease,
        cancellation,
    )
    .await;
    drop(live_reservation);
    outcome
}

#[expect(
    clippy::too_many_arguments,
    reason = "the retained bracket keeps every authority input explicit"
)]
async fn run_retained_generation(
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
    isolation_lease: ManagedOllamaIsolationLease<'_>,
    cancellation: &CancellationToken,
) -> Result<LocalOllamaManagedGenerationOutcome, LocalOllamaManagedGenerationError> {
    let progress = managed_attempt::ManagedCandidateAttemptProgress::new();
    let operation = run_live_generation(
        package,
        package_lease,
        isolation,
        plan,
        helper,
        admitted_runtime,
        generation_path,
        frozen_external_components,
        cloud_status,
        limits,
        worker_limits,
        static_model,
        model,
        request,
        &isolation_lease,
        ManagedGenerationObservationMode::Compatibility,
        &progress,
        super::deadline::CandidateOperationDeadline::compatibility(),
        cancellation,
    )
    .await;
    let cleanup = isolation_lease.close(&CancellationToken::new());
    let runtime_revalidation = package_lease.revalidate(&CancellationToken::new());
    finalize_retained_bracket(operation, cleanup, runtime_revalidation)
        .map(PendingLocalOllamaManagedGenerationOutcome::after_cleanup)
}
