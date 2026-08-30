use super::probe::run_exact_probe;
use super::validation::{
    ValidatedPreconnectionInputs, ensure_not_cancelled, map_package_error,
    validate_final_identity_bindings, validate_final_preconnection_inputs,
    validate_final_process_binding, validate_managed_startup, validate_native_binding,
};
use super::{
    AttachedProcessEvidence, AttachedProcessLease, CancellationToken, Digest, IsolationEvidence,
    ListenerEndpoint, ManagedLinuxProcessExpectation, ManagedStartupOutput, NativeLoadObservation,
    NativeLoadObservationLimits, NativeLoadObservationRequest, NativeManagedLinuxProcessObserver,
    OllamaRuntimeProbeEvidence, RetainedTcpConnectionEvidence,
    RuntimeAdmissionCloudDisableLiveEvidence, RuntimeAdmissionFinalVerification,
    RuntimeAdmissionFinalVerificationRequest, RuntimeAdmissionManagedStartupEvidence,
    RuntimeAdmissionRunner, RuntimeAdmissionRunnerError, RuntimePackageLease,
    RuntimePackageManifest, RuntimePackageManifestId, TcpStream,
    VerifiedFrozenExternalNativeComponentSet,
};

impl RuntimeAdmissionRunner {
    /// Performs one fresh final live verification against an independently verified frozen set.
    ///
    /// Package, frozen-set, limit, and actual retained-launch validation completes
    /// before the runner requests the lease's single loopback channel. The runner
    /// then consumes that channel's stream, diagnostics capability, and startup
    /// output together, and performs exactly one `GET /api/version` request. The
    /// operation neither fetches, admits, qualifies, generates, nor mutates an
    /// allowlist.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeAdmissionRunnerError`] unless every static binding, exact startup fact,
    /// one-request response, retained connection, fresh frozen-set native observation, final
    /// managed process witness, final isolation witness, and retained package revalidation
    /// succeeds. The caller keeps explicit
    /// [`super::RuntimeAdmissionManagedRuntimeLease::close`] authority after either success or
    /// failure.
    pub async fn verify_final(
        request: RuntimeAdmissionFinalVerificationRequest<'_>,
    ) -> Result<RuntimeAdmissionFinalVerification, RuntimeAdmissionRunnerError> {
        let RuntimeAdmissionFinalVerificationRequest {
            package,
            package_lease,
            managed_runtime,
            frozen,
            endpoint,
            launch,
            limits,
            cancellation,
        } = request;
        let endpoint_socket = endpoint.socket_addr();
        let validated = validate_final_preconnection_inputs(
            package,
            package_lease,
            frozen,
            endpoint,
            launch,
            managed_runtime.launch_spec_digest(),
            managed_runtime.isolation_policy_digest(),
            limits,
            cancellation,
        )?;
        let initial_isolation = managed_runtime.isolation.initial_evidence();
        let expectation = managed_process_expectation(&initial_isolation)?;
        let channel = managed_runtime
            .isolation
            .connect_loopback(endpoint_socket, cancellation)
            .map_err(RuntimeAdmissionRunnerError::Isolation)?;
        let (connected_stream, diagnostics, startup) = channel.into_parts();
        let listener =
            ListenerEndpoint::new(endpoint_socket).map_err(RuntimeAdmissionRunnerError::Witness)?;
        let mut process = NativeManagedLinuxProcessObserver
            .attach(
                listener,
                diagnostics.into_file(),
                expectation,
                limits.managed_process,
                cancellation,
            )
            .map_err(RuntimeAdmissionRunnerError::Witness)?;
        let verified = verify_final_with_process(
            package,
            package_lease,
            &mut process,
            frozen,
            connected_stream,
            &startup,
            validated,
            limits.native_load,
            cancellation,
        )
        .await?;
        let final_isolation = managed_runtime
            .isolation
            .reobserve(cancellation)
            .map_err(RuntimeAdmissionRunnerError::Isolation)?;
        if initial_isolation != final_isolation {
            return Err(RuntimeAdmissionRunnerError::InvalidEvidenceBinding);
        }
        Ok(RuntimeAdmissionFinalVerification {
            runtime_package_manifest_id: verified.package_id,
            frozen_external_component_set_id: frozen.frozen_set_id().clone(),
            isolation_policy_digest: verified.isolation_policy_digest,
            initial_isolation_evidence: initial_isolation,
            final_isolation_evidence: final_isolation,
            initial_process_evidence: verified.initial_process,
            final_process_evidence: verified.final_process,
            initial_connection_evidence: verified.initial_connection,
            final_connection_evidence: verified.final_connection,
            native_load: verified.native_load,
            runtime_probe: verified.runtime_probe,
            managed_startup: verified.managed_startup,
            cloud_disable: verified.cloud_disable,
        })
    }
}

struct FinalVerificationCore {
    package_id: RuntimePackageManifestId,
    isolation_policy_digest: Digest,
    initial_process: AttachedProcessEvidence,
    final_process: AttachedProcessEvidence,
    initial_connection: RetainedTcpConnectionEvidence,
    final_connection: RetainedTcpConnectionEvidence,
    native_load: NativeLoadObservation,
    runtime_probe: OllamaRuntimeProbeEvidence,
    managed_startup: RuntimeAdmissionManagedStartupEvidence,
    cloud_disable: RuntimeAdmissionCloudDisableLiveEvidence,
}

#[expect(
    clippy::too_many_arguments,
    reason = "the private test seam keeps the internally joined production inputs explicit"
)]
async fn verify_final_with_process<L: AttachedProcessLease + ?Sized>(
    package: &RuntimePackageManifest,
    package_lease: &mut RuntimePackageLease,
    process: &mut L,
    frozen: &VerifiedFrozenExternalNativeComponentSet,
    connected_stream: TcpStream,
    startup: &ManagedStartupOutput,
    validated: ValidatedPreconnectionInputs,
    native_load_limits: NativeLoadObservationLimits,
    cancellation: &CancellationToken,
) -> Result<FinalVerificationCore, RuntimeAdmissionRunnerError> {
    ensure_not_cancelled(cancellation)?;
    let initial_process = process.initial_evidence().clone();
    validate_final_identity_bindings(
        package,
        &validated.package_id,
        frozen.runtime_package_manifest_id(),
        &initial_process,
    )?;
    let (startup_marker, managed_startup) = validate_managed_startup(
        startup,
        validated.package_id.clone(),
        initial_process.evidence_digest().clone(),
        validated.launch_spec_digest.clone(),
        validated.isolation_policy_digest.clone(),
    )?;
    let retained = package_lease
        .clone_members_for_native_observation(cancellation)
        .map_err(map_package_error)?;
    let (runtime_probe, initial_connection, final_connection) =
        run_exact_probe(&validated.probe, connected_stream, process, cancellation).await?;
    ensure_not_cancelled(cancellation)?;
    let native_load = process
        .observe_native_load(
            &NativeLoadObservationRequest {
                package,
                expected_package_id: &validated.package_id,
                retained_package_members: &retained,
                expected_external_components: frozen.expected_components(),
                limits: native_load_limits,
            },
            cancellation,
        )
        .map_err(RuntimeAdmissionRunnerError::NativeLoad)?;
    validate_native_binding(&native_load, &validated.package_id, &initial_process)?;
    let final_process = process
        .reobserve(cancellation)
        .map_err(RuntimeAdmissionRunnerError::Witness)?;
    validate_final_process_binding(&initial_process, &final_process, package)?;
    package_lease
        .revalidate(cancellation)
        .map_err(map_package_error)?;
    let cloud_disable = RuntimeAdmissionCloudDisableLiveEvidence {
        runtime_package_manifest_id: validated.package_id.clone(),
        runtime_version: runtime_probe.runtime_version(),
        version_status: validated.version_status,
        managed_environment: validated.managed_environment,
        startup_marker,
    };
    Ok(FinalVerificationCore {
        package_id: validated.package_id,
        isolation_policy_digest: validated.isolation_policy_digest,
        initial_process,
        final_process,
        initial_connection,
        final_connection,
        native_load,
        runtime_probe,
        managed_startup,
        cloud_disable,
    })
}

pub(super) fn managed_process_expectation(
    isolation: &IsolationEvidence,
) -> Result<ManagedLinuxProcessExpectation, RuntimeAdmissionRunnerError> {
    let target = isolation.target();
    ManagedLinuxProcessExpectation::new(
        target.outer_pid(),
        target.process_start_token(),
        target.executable_device(),
        target.executable_inode(),
        target.executable_bytes(),
        isolation.network_namespace().device(),
        isolation.network_namespace().inode(),
        target.namespace_user_id(),
    )
    .map_err(RuntimeAdmissionRunnerError::Witness)
}
