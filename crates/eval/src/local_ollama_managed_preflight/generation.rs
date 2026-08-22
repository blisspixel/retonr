use std::{cell::RefCell, rc::Rc};

use rewrite_app::RuntimePackageLease;
use rewrite_inference::{
    InferenceError, OperationContext, StructuredCompletionRequest, StructuredCompletionResponse,
};
use rewrite_model::RuntimePackageManifest;
use rewrite_ollama::{
    OllamaCloudDisableFeaturePolicy, OllamaCloudDisableVersionStatus, OllamaEndpoint, OllamaLimits,
    OllamaModelBinding, OllamaResidentSessionExecutionReceipt, OllamaRetainedStreamSessionConfig,
    OllamaVersion,
};
use rewrite_runtime_attestor::{
    AttachedProcessLease, ExpectedExternalNativeComponent, ListenerEndpoint,
    NativeManagedLinuxProcessObserver,
};
use rewrite_runtime_isolation::{
    IsolationError, LaunchSpec, PreparedIsolation, RetainedIsolationLease,
};
use rewrite_types::CancellationToken;
use thiserror::Error;

use crate::{
    LocalOllamaBoundPreflightError, LocalOllamaBoundPreflightPlan, LocalOllamaModelBindingEvidence,
    local_ollama_bound_preflight::ConnectionObservationSequence,
    local_ollama_preflight::local_ollama_preflight_report,
};

use super::{
    LocalOllamaManagedBuildBinding, LocalOllamaManagedPreflightError,
    LocalOllamaManagedPreflightLimits, LocalOllamaManagedPreflightReport,
    bind_successful_managed_preflight,
    report::{build_report, report_evidence_digests},
    validation::{
        exact_helper_member, managed_expectation, validate_cloud_launch, validate_final_isolation,
        validate_isolation_binding, validate_process_binding, validate_static_inputs,
    },
};

mod evidence;
mod validation;

use evidence::{GenerationEvidenceInput, build_generation_evidence};
pub use evidence::{
    LOCAL_OLLAMA_MANAGED_GENERATION_EVIDENCE_SCHEMA_VERSION, LocalOllamaManagedGenerationEvidence,
};
use validation::{
    ManagedSessionObserver, map_session_error, observe_native_load, reobserve_process,
    validate_generation_admission, validate_generation_binding,
};

const GENERATION_RESPONSE_COUNT: usize = 9;

/// Failure from one retained managed-generation operation.
#[derive(Debug, Error)]
pub enum LocalOllamaManagedGenerationError {
    /// Managed package, launch, preflight, observation, or report validation failed.
    #[error("managed Ollama generation prerequisite failed: {0}")]
    Managed(#[from] LocalOllamaManagedPreflightError),
    /// The exact runtime package has not passed the production cloud-disable review.
    #[error("managed Ollama runtime package is not admitted for generation")]
    RuntimeNotAdmitted,
    /// The retained Ollama session failed closed before completing its exact sequence.
    #[error("managed Ollama retained generation session failed: {0}")]
    Session(#[source] InferenceError),
    /// The managed process tree could not be terminated and reaped.
    #[error("managed Ollama generation cleanup failed: {0}")]
    Cleanup(#[source] IsolationError),
    /// The primary operation and independent cleanup both failed.
    #[error("managed Ollama generation cleanup failed with {cleanup} after {operation}")]
    CleanupAfterFailure {
        /// Original operation failure retained without weakening cleanup reporting.
        #[source]
        operation: Box<LocalOllamaManagedGenerationError>,
        /// Independent termination and reap failure.
        cleanup: IsolationError,
    },
}

/// One content response plus redacted evidence from its closed managed bracket.
#[derive(Debug)]
pub struct LocalOllamaManagedGenerationOutcome {
    response: StructuredCompletionResponse,
    residency_receipt: OllamaResidentSessionExecutionReceipt,
    managed_preflight: LocalOllamaManagedPreflightReport,
    managed_build: LocalOllamaManagedBuildBinding,
    evidence: LocalOllamaManagedGenerationEvidence,
}

impl LocalOllamaManagedGenerationOutcome {
    /// Returns the bounded untrusted structured response.
    #[must_use]
    pub const fn response(&self) -> &StructuredCompletionResponse {
        &self.response
    }

    /// Returns the content-free runtime-reported residency receipt.
    #[must_use]
    pub const fn residency_receipt(&self) -> &OllamaResidentSessionExecutionReceipt {
        &self.residency_receipt
    }

    /// Returns the completed inert managed preflight report.
    #[must_use]
    pub const fn managed_preflight(&self) -> &LocalOllamaManagedPreflightReport {
        &self.managed_preflight
    }

    /// Returns the package-declared runtime-build binding.
    #[must_use]
    pub const fn managed_build(&self) -> &LocalOllamaManagedBuildBinding {
        &self.managed_build
    }

    /// Returns the redacted retained-generation evidence.
    #[must_use]
    pub const fn evidence(&self) -> &LocalOllamaManagedGenerationEvidence {
        &self.evidence
    }

    /// Splits the result into its content response and content-free evidence.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        StructuredCompletionResponse,
        OllamaResidentSessionExecutionReceipt,
        LocalOllamaManagedPreflightReport,
        LocalOllamaManagedBuildBinding,
        LocalOllamaManagedGenerationEvidence,
    ) {
        (
            self.response,
            self.residency_receipt,
            self.managed_preflight,
            self.managed_build,
            self.evidence,
        )
    }
}

/// Runs one structured completion inside one retained managed Linux operation.
///
/// Static package and model relationships, read-only preflight, native process and
/// load evidence, every direct-connection response, generation, and two equal
/// runtime-reported residency observations are joined before cleanup. The returned
/// result is inert. It does not prove model weight use, handler execution, a complete
/// effective runtime identity, semantic correctness, or qualification.
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
    isolation: &PreparedIsolation,
    launch: &LaunchSpec,
    plan: &LocalOllamaBoundPreflightPlan,
    external_components: &[ExpectedExternalNativeComponent],
    limits: LocalOllamaManagedPreflightLimits,
    static_model: &LocalOllamaModelBindingEvidence,
    model: &OllamaModelBinding,
    request: StructuredCompletionRequest,
    cancellation: &CancellationToken,
) -> Result<LocalOllamaManagedGenerationOutcome, LocalOllamaManagedGenerationError> {
    validate_static_inputs(package, package_lease, plan, external_components, limits)?;
    validate_generation_binding(package, plan, static_model, model, &request)?;
    validate_generation_admission(package, plan)?;
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
    let executable = package_lease
        .clone_entrypoint_for_launch(cancellation)
        .map_err(LocalOllamaManagedPreflightError::Package)?;
    let isolation_lease = isolation
        .launch_retained(launch, executable, cancellation)
        .map_err(LocalOllamaManagedPreflightError::Isolation)?;

    run_retained_generation(
        package,
        package_lease,
        isolation,
        launch,
        plan,
        helper,
        external_components,
        limits,
        static_model,
        model,
        request,
        isolation_lease,
        cancellation,
    )
    .await
}

#[expect(
    clippy::too_many_arguments,
    reason = "the retained bracket keeps every authority input explicit"
)]
async fn run_retained_generation(
    package: &RuntimePackageManifest,
    package_lease: &mut RuntimePackageLease,
    isolation: &PreparedIsolation,
    launch: &LaunchSpec,
    plan: &LocalOllamaBoundPreflightPlan,
    helper: &rewrite_model::RuntimePackageMember,
    external_components: &[ExpectedExternalNativeComponent],
    limits: LocalOllamaManagedPreflightLimits,
    static_model: &LocalOllamaModelBindingEvidence,
    model: &OllamaModelBinding,
    request: StructuredCompletionRequest,
    mut isolation_lease: RetainedIsolationLease,
    cancellation: &CancellationToken,
) -> Result<LocalOllamaManagedGenerationOutcome, LocalOllamaManagedGenerationError> {
    let operation = run_live_generation(
        package,
        package_lease,
        isolation,
        launch,
        plan,
        helper,
        external_components,
        limits,
        static_model,
        model,
        request,
        &mut isolation_lease,
        cancellation,
    )
    .await;
    let cleanup = isolation_lease.close(&CancellationToken::new());
    match (operation, cleanup) {
        (Err(operation), Err(cleanup)) => {
            Err(LocalOllamaManagedGenerationError::CleanupAfterFailure {
                operation: Box::new(operation),
                cleanup,
            })
        }
        (Ok(_outcome), Err(cleanup)) => Err(LocalOllamaManagedGenerationError::Cleanup(cleanup)),
        (operation, Ok(())) => operation,
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
    launch: &LaunchSpec,
    plan: &LocalOllamaBoundPreflightPlan,
    helper: &rewrite_model::RuntimePackageMember,
    external_components: &[ExpectedExternalNativeComponent],
    limits: LocalOllamaManagedPreflightLimits,
    static_model: &LocalOllamaModelBindingEvidence,
    model: &OllamaModelBinding,
    request: StructuredCompletionRequest,
    isolation_lease: &mut RetainedIsolationLease,
    cancellation: &CancellationToken,
) -> Result<LocalOllamaManagedGenerationOutcome, LocalOllamaManagedGenerationError> {
    let endpoint = OllamaEndpoint::parse(&plan.preflight.endpoint)
        .map_err(|_error| LocalOllamaManagedPreflightError::InvalidInput)?;
    let initial_isolation = isolation_lease.initial_evidence();
    validate_isolation_binding(&initial_isolation, package)?;
    let channel = isolation_lease
        .connect_loopback(endpoint.socket_addr(), cancellation)
        .map_err(LocalOllamaManagedPreflightError::Isolation)?;
    let (stream, diagnostics, startup_output) = channel.into_parts();
    validate_cloud_launch(launch, &startup_output)?;

    let expectation = managed_expectation(&initial_isolation)?;
    let listener = ListenerEndpoint::new(endpoint.socket_addr())
        .map_err(LocalOllamaManagedPreflightError::Witness)?;
    let process = NativeManagedLinuxProcessObserver
        .attach(
            listener,
            diagnostics.into_file(),
            expectation,
            limits.process,
            cancellation,
        )
        .map_err(LocalOllamaManagedPreflightError::Witness)?;
    let initial_process = process.initial_evidence().clone();
    validate_process_binding(&initial_process, package)?;

    let preflight_responses = plan.preflight.models.len().saturating_add(6);
    let total_responses = preflight_responses.saturating_add(GENERATION_RESPONSE_COUNT);
    let session_bytes = usize::try_from(plan.maximum_session_body_bytes)
        .map_err(|_error| LocalOllamaManagedPreflightError::InvalidInput)?;
    let config = OllamaRetainedStreamSessionConfig::new(
        endpoint,
        vec![model.clone()],
        OllamaLimits::default(),
        session_bytes,
    )
    .map_err(LocalOllamaManagedGenerationError::Session)?;
    let observer = Rc::new(RefCell::new(ManagedSessionObserver {
        process,
        connections: ConnectionObservationSequence::new(total_responses),
    }));
    let callback_observer = Rc::clone(&observer);
    let mut session = config
        .open(
            stream,
            OperationContext::new(cancellation, None),
            move |observation| {
                let mut state = callback_observer
                    .try_borrow_mut()
                    .map_err(|_error| LocalOllamaBoundPreflightError::InvalidObservationSequence)?;
                let ManagedSessionObserver {
                    process,
                    connections,
                } = &mut *state;
                connections.observe(process, cancellation, observation)
            },
        )
        .await
        .map_err(map_session_error)?;
    let api_preflight = session
        .preflight(OperationContext::new(cancellation, None))
        .await
        .map_err(map_session_error)?;
    let post_preflight_process = reobserve_process(&observer, package, cancellation)?;
    let preflight_connections = {
        let state = observer
            .try_borrow()
            .map_err(|_error| LocalOllamaManagedPreflightError::InvalidEvidenceBinding)?;
        state
            .connections
            .validate_progress(preflight_responses)
            .map_err(LocalOllamaManagedPreflightError::BoundObservation)?;
        state.connections.evidence().to_vec()
    };
    let preflight = local_ollama_preflight_report(&plan.preflight, api_preflight)
        .map_err(LocalOllamaManagedPreflightError::Preflight)?;

    let runtime_version = plan
        .preflight
        .expected_runtime_version
        .parse::<OllamaVersion>()
        .map_err(|_error| LocalOllamaManagedPreflightError::InvalidInput)?;
    let package_id = package.runtime_package_manifest_id();
    let cloud_status = OllamaCloudDisableFeaturePolicy::assess(runtime_version, &package_id);
    if cloud_status == OllamaCloudDisableVersionStatus::FeatureUnavailable {
        return Err(LocalOllamaManagedPreflightError::CloudDisableFeatureUnavailable.into());
    }
    if OllamaCloudDisableFeaturePolicy::reviewed_runtime_count() == 0
        && cloud_status != OllamaCloudDisableVersionStatus::Unreviewed
    {
        return Err(LocalOllamaManagedPreflightError::InvalidEvidenceBinding.into());
    }

    let retained_members = package_lease
        .clone_members_for_native_observation(cancellation)
        .map_err(LocalOllamaManagedPreflightError::Package)?;
    let preflight_native_load = observe_native_load(
        &observer,
        package,
        &package_id,
        &retained_members,
        external_components,
        limits,
        cancellation,
    )?;
    let preflight_final_process = reobserve_process(&observer, package, cancellation)?;
    let preflight_final_isolation = isolation_lease
        .reobserve(cancellation)
        .map_err(LocalOllamaManagedPreflightError::Isolation)?;
    validate_final_isolation(&initial_isolation, &preflight_final_isolation)?;
    package_lease
        .revalidate(cancellation)
        .map_err(LocalOllamaManagedPreflightError::Package)?;

    let connection_witness = preflight_connections
        .last()
        .cloned()
        .ok_or(LocalOllamaManagedPreflightError::InvalidEvidenceBinding)?;
    let report_digests = report_evidence_digests(
        package_lease,
        isolation,
        launch,
        &initial_isolation,
        &preflight_final_isolation,
        &startup_output,
    )?;
    let managed_preflight = build_report(
        package,
        plan,
        helper,
        external_components,
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
    )?;
    let managed_build = bind_successful_managed_preflight(package, plan, &managed_preflight)?;

    let retained_request = request.clone();
    let (response, residency_receipt) = session
        .complete_structured_with_residency(request, OperationContext::new(cancellation, None))
        .await
        .map_err(map_session_error)?;
    let post_generation_process = reobserve_process(&observer, package, cancellation)?;
    let post_generation_native_load = observe_native_load(
        &observer,
        package,
        &package_id,
        &retained_members,
        external_components,
        limits,
        cancellation,
    )?;
    let final_process = reobserve_process(&observer, package, cancellation)?;
    if final_process != post_generation_process {
        return Err(LocalOllamaManagedPreflightError::InvalidEvidenceBinding.into());
    }
    let final_isolation = isolation_lease
        .reobserve(cancellation)
        .map_err(LocalOllamaManagedPreflightError::Isolation)?;
    validate_final_isolation(&initial_isolation, &final_isolation)?;
    package_lease
        .revalidate(cancellation)
        .map_err(LocalOllamaManagedPreflightError::Package)?;
    let connection_observations = {
        let state = observer
            .try_borrow()
            .map_err(|_error| LocalOllamaManagedPreflightError::InvalidEvidenceBinding)?;
        state
            .connections
            .validate_complete()
            .map_err(LocalOllamaManagedPreflightError::BoundObservation)?;
        state.connections.evidence().to_vec()
    };
    let evidence = build_generation_evidence(&GenerationEvidenceInput {
        managed_report: &managed_preflight,
        build_binding: &managed_build,
        static_model,
        model,
        request: &retained_request,
        receipt: &residency_receipt,
        post_generation_process: &final_process,
        post_generation_native_load: &post_generation_native_load,
        final_isolation: &final_isolation,
        connection_observations: &connection_observations,
    })?;
    drop(session);
    drop(observer);

    Ok(LocalOllamaManagedGenerationOutcome {
        response,
        residency_receipt,
        managed_preflight,
        managed_build,
        evidence,
    })
}
