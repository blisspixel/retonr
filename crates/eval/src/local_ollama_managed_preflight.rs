//! Managed Linux isolation, package, process, connection, and native-load preflight join.

use rewrite_app::{PackageAttestationError, RuntimePackageLease};
use rewrite_inference::OperationContext;
use rewrite_model::{
    ArtifactId, ArtifactSetId, ArtifactSetRelativePath, NativeLoadObservation,
    RuntimePackageManifest, RuntimePackageManifestId,
};
use rewrite_ollama::{
    OllamaCloudDisableEvidenceError, OllamaCloudDisableFeaturePolicy,
    OllamaCloudDisableVersionStatus, OllamaEndpoint, OllamaLimits, OllamaObservedPreflightError,
    OllamaSingleConnectionPreflight, OllamaVersion,
};
use rewrite_runtime_attestor::{
    AttachedProcessEvidence, AttachedProcessLease, AttachedProcessWitnessError,
    AttachedProcessWitnessLimits, ExpectedExternalNativeComponent, ListenerEndpoint,
    NativeLoadObservationLimits, NativeLoadObservationRequest, NativeLoadObserverError,
    NativeManagedLinuxProcessObserver, RetainedTcpConnectionEvidence,
};
use rewrite_runtime_isolation::{
    IsolationError, LaunchSpec, PreparedIsolation, RetainedIsolationLease,
};
use rewrite_types::{CancellationToken, Digest};
use serde::Serialize;
use thiserror::Error;

use crate::{
    LocalOllamaBoundPreflightError, LocalOllamaBoundPreflightPlan, LocalOllamaPreflightError,
    LocalOllamaPreflightReport,
    local_ollama_bound_preflight::ConnectionObservationSequence,
    local_ollama_preflight::{local_ollama_preflight_report, local_ollama_preflight_targets},
};

mod build_binding;
mod generation;
mod report;
#[cfg(test)]
mod test_support;
mod validation;

use build_binding::bind_successful_managed_preflight;
pub use build_binding::{
    LOCAL_OLLAMA_MANAGED_BUILD_BINDING_SCHEMA_VERSION,
    LocalOllamaEffectiveStateMissingRelationship, LocalOllamaManagedBuildBinding,
    LocalOllamaManagedBuildEvidenceClass, LocalOllamaManagedPreflightOutcome,
};
pub use generation::{
    LOCAL_OLLAMA_MANAGED_GENERATION_EVIDENCE_SCHEMA_VERSION, LocalOllamaManagedGenerationError,
    LocalOllamaManagedGenerationEvidence, LocalOllamaManagedGenerationOutcome,
    run_local_ollama_managed_generation,
};
use report::{build_report, report_evidence_digests};
use validation::{
    exact_helper_member, managed_expectation, validate_cloud_launch, validate_final_isolation,
    validate_isolation_binding, validate_process_binding, validate_static_inputs,
};

/// Current managed Ollama preflight report contract version.
pub const LOCAL_OLLAMA_MANAGED_PREFLIGHT_REPORT_SCHEMA_VERSION: u32 = 1;

/// Caller-owned ceilings for managed process and native-load observation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LocalOllamaManagedPreflightLimits {
    /// Native process, socket, and executable observation ceilings.
    pub process: AttachedProcessWitnessLimits,
    /// Native loaded-component observation ceilings.
    pub native_load: NativeLoadObservationLimits,
}

/// Exact process evidence strength represented by a managed report.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalOllamaManagedProcessEvidenceLevel {
    /// A retained managed Linux process and namespace-local socket diagnostics were joined.
    ManagedLinuxIsolationSockDiag,
}

/// Versioned, redacted, inert managed Ollama preflight report.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "security claim limitations remain explicit in the versioned report"
)]
pub struct LocalOllamaManagedPreflightReport {
    /// Managed report contract version.
    pub schema_version: u32,
    /// Digest of the canonical bound-preflight plan.
    pub plan_digest: Digest,
    /// Domain-separated digest binding every report evidence input.
    pub binding_digest: Digest,
    /// Exact verified runtime-package identity.
    pub runtime_package_manifest_id: RuntimePackageManifestId,
    /// Exact verified managed artifact-set identity.
    pub artifact_set_id: ArtifactSetId,
    /// Digest of the static package attestation facts consumed by this join.
    pub package_attestation_digest: Digest,
    /// Digest of the prepared internal isolation policy.
    pub isolation_policy_digest: Digest,
    /// Digest of the exact managed launch description.
    pub launch_spec_digest: Digest,
    /// Exact package member bound to the prepared isolation helper.
    pub helper_member_artifact_id: ArtifactId,
    /// Canonical logical path of that package member.
    pub helper_member_relative_path: ArtifactSetRelativePath,
    /// Exact verified helper byte length.
    pub helper_member_bytes: u64,
    /// Launch-time isolation evidence digest.
    pub initial_isolation_evidence_digest: Digest,
    /// Final isolation reobservation digest.
    pub final_isolation_evidence_digest: Digest,
    /// Managed process evidence strength.
    pub process_evidence_level: LocalOllamaManagedProcessEvidenceLevel,
    /// Process evidence established before HTTP observation.
    pub initial_process_witness: AttachedProcessEvidence,
    /// Stable process evidence immediately after the HTTP observation.
    pub post_preflight_process_witness: AttachedProcessEvidence,
    /// Final process evidence after native-load observation.
    pub final_process_witness: AttachedProcessEvidence,
    /// Final exact-connection attribution evidence.
    pub connection_witness: RetainedTcpConnectionEvidence,
    /// Initial and post-response connection observations in order.
    pub connection_observations: Vec<RetainedTcpConnectionEvidence>,
    /// Digest of bounded managed startup standard output.
    pub startup_standard_output_digest: Digest,
    /// Digest of bounded managed startup standard error.
    pub startup_standard_error_digest: Digest,
    /// Retained standard-output byte count.
    pub startup_standard_output_bytes: u64,
    /// Retained standard-error byte count.
    pub startup_standard_error_bytes: u64,
    /// Production exact-version cloud-disable policy disposition.
    pub cloud_disable_version_status: OllamaCloudDisableVersionStatus,
    /// True only when the exact launch environment contained `OLLAMA_NO_CLOUD=1`.
    pub cloud_disable_environment_observed: bool,
    /// True only when the exact startup marker was observed once without conflict.
    pub cloud_disable_startup_marker_observed: bool,
    /// Whether production policy reviewed this exact version and package.
    pub cloud_disable_runtime_reviewed: bool,
    /// True after retained loopback-only namespace canaries and final reobservation.
    pub operating_system_network_isolation_enforced: bool,
    /// Exact read-only Ollama API evidence.
    pub preflight: LocalOllamaPreflightReport,
    /// Exact file-backed native-load evidence for the retained process.
    pub native_load: NativeLoadObservation,
    /// True because the managed transport has no reconnect path.
    pub all_responses_used_retained_transport: bool,
    /// True after exact callback count and ordinal validation.
    pub kernel_attribution_checked_around_every_response: bool,
    /// Always false because socket attribution does not identify application handlers.
    pub application_handler_proven: bool,
    /// Always false because admitted APIs do not prove exclusive socket ownership.
    pub exclusive_socket_owner_proven: bool,
    /// Always false because this preflight neither loads nor exercises model inference.
    pub model_loaded_or_used_proven: bool,
    /// Always false because this inert report does not construct an effective identity.
    pub effective_runtime_identity_proven: bool,
    /// Always false because this report is not qualification authority.
    pub qualified: bool,
}

/// Managed package, isolation, native witness, preflight, or cleanup failure.
#[derive(Debug, Error)]
pub enum LocalOllamaManagedPreflightError {
    /// The bound plan or managed limits are invalid.
    #[error("invalid managed Ollama preflight input")]
    InvalidInput,
    /// The runtime package and retained static evidence do not match exactly.
    #[error("managed Ollama runtime package binding is invalid")]
    InvalidPackageBinding,
    /// The prepared helper does not identify exactly one helper package member.
    #[error("managed Ollama isolation helper binding is invalid")]
    InvalidHelperBinding,
    /// The managed process or isolation evidence does not match the frozen package.
    #[error("managed Ollama runtime evidence binding is invalid")]
    InvalidEvidenceBinding,
    /// Startup output was truncated and cannot support a complete marker observation.
    #[error("managed Ollama startup output was truncated")]
    TruncatedStartupOutput,
    /// The exact managed cloud-disable declaration or marker was invalid.
    #[error("managed Ollama cloud-disable evidence is invalid: {0}")]
    CloudDisable(#[source] OllamaCloudDisableEvidenceError),
    /// The runtime predates the production cloud-disable feature floor.
    #[error("managed Ollama cloud-disable feature is unavailable")]
    CloudDisableFeatureUnavailable,
    /// Static retained package revalidation failed.
    #[error("managed Ollama package revalidation failed: {0}")]
    Package(#[source] PackageAttestationError),
    /// Managed isolation launch, channel, or reobservation failed.
    #[error("managed Ollama isolation failed: {0}")]
    Isolation(#[source] IsolationError),
    /// Managed process or namespace-local connection attribution failed.
    #[error("managed Ollama native witness failed: {0}")]
    Witness(#[source] AttachedProcessWitnessError),
    /// Native loaded-component observation failed.
    #[error("managed Ollama native-load observation failed: {0}")]
    NativeLoad(#[source] NativeLoadObserverError),
    /// The exact response observation sequence was invalid.
    #[error("managed Ollama response observation failed: {0}")]
    BoundObservation(#[source] LocalOllamaBoundPreflightError),
    /// The supplied retained stream preflight failed.
    #[error("managed Ollama API preflight failed: {0}")]
    Preflight(#[source] LocalOllamaPreflightError),
    /// The managed process tree could not be terminated and reaped.
    #[error("managed Ollama cleanup failed: {0}")]
    Cleanup(#[source] IsolationError),
    /// The primary operation and the independent cleanup both failed.
    #[error("managed Ollama cleanup failed with {cleanup} after {operation}")]
    CleanupAfterFailure {
        /// Original operation failure retained without weakening cleanup reporting.
        #[source]
        operation: Box<LocalOllamaManagedPreflightError>,
        /// Independent termination and reap failure.
        cleanup: IsolationError,
    },
    /// Redacted report evidence could not be encoded canonically.
    #[error("managed Ollama report encoding failed")]
    ReportEncoding,
}

/// Runs a bound preflight entirely inside one retained managed Linux isolation lease.
///
/// The result is evidence only. It does not qualify a runtime, prove application-handler
/// execution, prove exclusive socket ownership, prove model use, or construct an effective
/// runtime identity.
///
/// # Errors
///
/// Returns [`LocalOllamaManagedPreflightError`] for any invalid input, package drift,
/// isolation failure, response attribution failure, cloud-disable evidence failure,
/// native-load mismatch, final drift, or cleanup failure.
#[expect(
    clippy::too_many_arguments,
    reason = "each independently frozen trust-boundary input remains explicit"
)]
pub async fn run_local_ollama_managed_preflight(
    package: &RuntimePackageManifest,
    package_lease: &mut RuntimePackageLease,
    isolation: &PreparedIsolation,
    launch: &LaunchSpec,
    plan: &LocalOllamaBoundPreflightPlan,
    external_components: &[ExpectedExternalNativeComponent],
    limits: LocalOllamaManagedPreflightLimits,
    cancellation: &CancellationToken,
) -> Result<LocalOllamaManagedPreflightReport, LocalOllamaManagedPreflightError> {
    run_local_ollama_managed_preflight_with_build_binding(
        package,
        package_lease,
        isolation,
        launch,
        plan,
        external_components,
        limits,
        cancellation,
    )
    .await
    .map(LocalOllamaManagedPreflightOutcome::into_report)
}

/// Runs a bound managed preflight and returns its point-in-time runtime-build binding.
///
/// The returned binding identifies the verified runtime package and the managed process
/// witnessed during preflight. Cleanup completes before this function returns, so the
/// binding does not claim a retained live process, an effective runtime state, model use,
/// or qualification.
///
/// # Errors
///
/// Returns [`LocalOllamaManagedPreflightError`] for the same fail-closed conditions as
/// [`run_local_ollama_managed_preflight`], including a relationship mismatch while
/// constructing the inert build binding.
#[expect(
    clippy::too_many_arguments,
    reason = "each independently frozen trust-boundary input remains explicit"
)]
pub async fn run_local_ollama_managed_preflight_with_build_binding(
    package: &RuntimePackageManifest,
    package_lease: &mut RuntimePackageLease,
    isolation: &PreparedIsolation,
    launch: &LaunchSpec,
    plan: &LocalOllamaBoundPreflightPlan,
    external_components: &[ExpectedExternalNativeComponent],
    limits: LocalOllamaManagedPreflightLimits,
    cancellation: &CancellationToken,
) -> Result<LocalOllamaManagedPreflightOutcome, LocalOllamaManagedPreflightError> {
    validate_static_inputs(package, package_lease, plan, external_components, limits)?;

    package_lease
        .revalidate(cancellation)
        .map_err(LocalOllamaManagedPreflightError::Package)?;
    let preparation = isolation.preparation_evidence();
    if !preparation.all_canaries_passed() {
        return Err(LocalOllamaManagedPreflightError::InvalidHelperBinding);
    }
    let helper = exact_helper_member(
        package,
        preparation.helper_digest(),
        preparation.helper_bytes(),
    )?;
    let executable = package_lease
        .clone_entrypoint_for_launch(cancellation)
        .map_err(LocalOllamaManagedPreflightError::Package)?;
    let lease = isolation
        .launch_retained(launch, executable, cancellation)
        .map_err(LocalOllamaManagedPreflightError::Isolation)?;

    run_retained(
        package,
        package_lease,
        isolation,
        launch,
        plan,
        helper,
        external_components,
        limits,
        lease,
        cancellation,
    )
    .await
}

#[expect(
    clippy::too_many_arguments,
    reason = "the live join keeps all frozen capabilities and policies explicit"
)]
async fn run_retained(
    package: &RuntimePackageManifest,
    package_lease: &mut RuntimePackageLease,
    isolation: &PreparedIsolation,
    launch: &LaunchSpec,
    plan: &LocalOllamaBoundPreflightPlan,
    helper: &rewrite_model::RuntimePackageMember,
    external_components: &[ExpectedExternalNativeComponent],
    limits: LocalOllamaManagedPreflightLimits,
    mut isolation_lease: RetainedIsolationLease,
    cancellation: &CancellationToken,
) -> Result<LocalOllamaManagedPreflightOutcome, LocalOllamaManagedPreflightError> {
    let operation = run_live_join(
        package,
        package_lease,
        isolation,
        launch,
        plan,
        helper,
        external_components,
        limits,
        &mut isolation_lease,
        cancellation,
    )
    .await;
    let operation = operation.and_then(|report| {
        let binding = bind_successful_managed_preflight(package, plan, &report)?;
        Ok(LocalOllamaManagedPreflightOutcome::new(report, binding))
    });
    let cleanup = isolation_lease.close(&CancellationToken::new());
    match (operation, cleanup) {
        (Err(operation), Err(cleanup)) => {
            Err(LocalOllamaManagedPreflightError::CleanupAfterFailure {
                operation: Box::new(operation),
                cleanup,
            })
        }
        (Ok(_outcome), Err(cleanup)) => Err(LocalOllamaManagedPreflightError::Cleanup(cleanup)),
        (operation, Ok(())) => operation,
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "the evidence join intentionally exposes every frozen authority input"
)]
#[expect(
    clippy::too_many_lines,
    reason = "the security-sensitive operation order remains visible in one linear join"
)]
async fn run_live_join(
    package: &RuntimePackageManifest,
    package_lease: &mut RuntimePackageLease,
    isolation: &PreparedIsolation,
    launch: &LaunchSpec,
    plan: &LocalOllamaBoundPreflightPlan,
    helper: &rewrite_model::RuntimePackageMember,
    external_components: &[ExpectedExternalNativeComponent],
    limits: LocalOllamaManagedPreflightLimits,
    isolation_lease: &mut RetainedIsolationLease,
    cancellation: &CancellationToken,
) -> Result<LocalOllamaManagedPreflightReport, LocalOllamaManagedPreflightError> {
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
    let mut process_lease = NativeManagedLinuxProcessObserver
        .attach(
            listener,
            diagnostics.into_file(),
            expectation,
            limits.process,
            cancellation,
        )
        .map_err(LocalOllamaManagedPreflightError::Witness)?;
    let initial_process_witness = process_lease.initial_evidence().clone();
    validate_process_binding(&initial_process_witness, package)?;

    let targets = local_ollama_preflight_targets(&plan.preflight)
        .map_err(|_error| LocalOllamaManagedPreflightError::InvalidInput)?;
    let expected_responses = targets.len().saturating_add(6);
    let session_bytes = usize::try_from(plan.maximum_session_body_bytes)
        .map_err(|_error| LocalOllamaManagedPreflightError::InvalidInput)?;
    let preflight = OllamaSingleConnectionPreflight::new(
        endpoint,
        targets,
        OllamaLimits::default(),
        session_bytes,
    )
    .map_err(|_error| LocalOllamaManagedPreflightError::InvalidInput)?;
    let mut observations = ConnectionObservationSequence::new(expected_responses);
    let transport_result = preflight
        .preflight_on_connected_stream_with_observer(
            OperationContext::new(cancellation, None),
            stream,
            |observation| observations.observe(&mut process_lease, cancellation, observation),
        )
        .await;
    let post_preflight_process = process_lease
        .reobserve(cancellation)
        .map_err(LocalOllamaManagedPreflightError::Witness)?;
    validate_process_binding(&post_preflight_process, package)?;
    let api_observation = match transport_result {
        Ok(value) => value,
        Err(OllamaObservedPreflightError::Observation(error)) => {
            return Err(LocalOllamaManagedPreflightError::BoundObservation(error));
        }
        Err(OllamaObservedPreflightError::Preflight(error)) => {
            return Err(LocalOllamaManagedPreflightError::Preflight(
                LocalOllamaPreflightError::Backend(error),
            ));
        }
    };
    observations
        .validate_complete()
        .map_err(LocalOllamaManagedPreflightError::BoundObservation)?;
    let preflight = local_ollama_preflight_report(&plan.preflight, api_observation)
        .map_err(LocalOllamaManagedPreflightError::Preflight)?;

    let runtime_version = plan
        .preflight
        .expected_runtime_version
        .parse::<OllamaVersion>()
        .map_err(|_error| LocalOllamaManagedPreflightError::InvalidInput)?;
    let package_id = package.runtime_package_manifest_id();
    let cloud_status = OllamaCloudDisableFeaturePolicy::assess(runtime_version, &package_id);
    if cloud_status == OllamaCloudDisableVersionStatus::FeatureUnavailable {
        return Err(LocalOllamaManagedPreflightError::CloudDisableFeatureUnavailable);
    }
    if OllamaCloudDisableFeaturePolicy::reviewed_runtime_count() == 0
        && cloud_status != OllamaCloudDisableVersionStatus::Unreviewed
    {
        return Err(LocalOllamaManagedPreflightError::InvalidEvidenceBinding);
    }

    let retained_members = package_lease
        .clone_members_for_native_observation(cancellation)
        .map_err(LocalOllamaManagedPreflightError::Package)?;
    let native_load = process_lease
        .observe_native_load(
            &NativeLoadObservationRequest {
                package,
                expected_package_id: &package_id,
                retained_package_members: &retained_members,
                expected_external_components: external_components,
                limits: limits.native_load,
            },
            cancellation,
        )
        .map_err(LocalOllamaManagedPreflightError::NativeLoad)?;
    let final_process_witness = process_lease
        .reobserve(cancellation)
        .map_err(LocalOllamaManagedPreflightError::Witness)?;
    validate_process_binding(&final_process_witness, package)?;
    let final_isolation = isolation_lease
        .reobserve(cancellation)
        .map_err(LocalOllamaManagedPreflightError::Isolation)?;
    validate_final_isolation(&initial_isolation, &final_isolation)?;
    package_lease
        .revalidate(cancellation)
        .map_err(LocalOllamaManagedPreflightError::Package)?;

    let connection_observations = observations.into_evidence();
    let connection_witness = connection_observations
        .last()
        .cloned()
        .ok_or(LocalOllamaManagedPreflightError::InvalidEvidenceBinding)?;
    let report_digests = report_evidence_digests(
        package_lease,
        isolation,
        launch,
        &initial_isolation,
        &final_isolation,
        &startup_output,
    )?;
    build_report(
        package,
        plan,
        helper,
        external_components,
        limits,
        report_digests,
        initial_process_witness,
        post_preflight_process,
        final_process_witness,
        connection_witness,
        connection_observations,
        cloud_status,
        preflight,
        native_load,
    )
}
