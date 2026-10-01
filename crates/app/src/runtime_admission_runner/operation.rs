use rewrite_model::{RuntimeOperatingSystem, RuntimePackageLoadPolicy, RuntimePackageMemberRole};
use rewrite_runtime_attestor::AttachedProcessLease;
use rewrite_runtime_isolation::PreparedIsolation;

use super::observed_records::RuntimeAdmissionObservedFinalOperation;
use super::validation::{
    ensure_not_cancelled, map_package_error, valid_managed_process_limits, valid_native_limits,
    validate_actual_launch_binding, validate_final_identity_bindings,
    validate_final_process_binding, validate_package_binding,
};
use super::verify::managed_process_expectation;
use super::{
    ADMITTED_RUNTIME_FAMILY, CancellationToken, LaunchSpec, ListenerEndpoint, NativeLoadDiscovery,
    NativeManagedLinuxProcessObserver, OllamaEndpoint, RuntimeAdmissionFinalVerificationRequest,
    RuntimeAdmissionManagedRuntimeLease, RuntimeAdmissionRunner, RuntimeAdmissionRunnerError,
    RuntimeAdmissionRunnerLimits, RuntimePackageLease, RuntimePackageManifest,
    VerifiedFrozenExternalNativeComponentSet,
};
use crate::VerifiedRuntimeAdmissionFoundationBinding;

use super::records::{
    RuntimeAdmissionManagedFinalRecord, RuntimeAdmissionManagedFinalRecordVerifier,
    RuntimeAdmissionNativeLoadRecord, RuntimeAdmissionNativeLoadRecordVerifier,
    VerifiedPassedRuntimeAdmissionCloudDisableControl,
    VerifiedPassedRuntimeAdmissionManagedStartupControl,
    VerifiedPassedRuntimeAdmissionNativeClosureControl,
};

/// Inputs for one fresh managed native-closure discovery operation.
pub struct RuntimeAdmissionNativeClosureDiscoveryRequest<'a> {
    /// Exact typed package whose retained entrypoint will be launched.
    pub package: &'a RuntimePackageManifest,
    /// Retained exact package capabilities kept live through cleanup.
    pub package_lease: &'a mut RuntimePackageLease,
    /// Prepared and actively probed isolation capability.
    pub isolation: &'a PreparedIsolation,
    /// Reviewed launch declaration used for the exact retained entrypoint.
    pub launch: &'a LaunchSpec,
    /// Exact loopback endpoint used to establish managed readiness.
    pub endpoint: OllamaEndpoint,
    /// Explicit managed-process and native-observation ceilings.
    pub limits: RuntimeAdmissionRunnerLimits,
    /// Cooperative operation cancellation. Cleanup never reuses this token.
    pub cancellation: &'a CancellationToken,
}

/// Canonical inert discovery publication produced after complete cleanup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAdmissionNativeClosureDiscovery {
    discovery: NativeLoadDiscovery,
}

impl RuntimeAdmissionNativeClosureDiscovery {
    /// Returns the independently reparsed discovery.
    #[must_use]
    pub const fn discovery(&self) -> &NativeLoadDiscovery {
        &self.discovery
    }

    /// Returns exact bounded canonical discovery bytes.
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        self.discovery.canonical_json_bytes()
    }
}

/// Inputs for one final frozen-set-gated managed verification and cleanup.
pub struct RuntimeAdmissionFinalOperationRequest<'a> {
    /// Exact typed package whose retained entrypoint will be launched.
    pub package: &'a RuntimePackageManifest,
    /// Retained exact package capabilities kept live through cleanup.
    pub package_lease: &'a mut RuntimePackageLease,
    /// Durable source-build foundation independently bound to this package.
    pub foundation: &'a VerifiedRuntimeAdmissionFoundationBinding,
    /// Independently reviewed frozen external native-component set.
    pub frozen: &'a VerifiedFrozenExternalNativeComponentSet,
    /// Prepared and actively probed isolation capability.
    pub isolation: &'a PreparedIsolation,
    /// Reviewed launch declaration used for the exact retained entrypoint.
    pub launch: &'a LaunchSpec,
    /// Exact loopback endpoint used by the one-request final probe.
    pub endpoint: OllamaEndpoint,
    /// Explicit managed-process, native-observation, and probe ceilings.
    pub limits: RuntimeAdmissionRunnerLimits,
    /// Cooperative operation cancellation. Cleanup never reuses this token.
    pub cancellation: &'a CancellationToken,
}

/// Canonical records and inert passed controls returned only after complete cleanup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAdmissionFinalOperation {
    native_load_record: RuntimeAdmissionNativeLoadRecord,
    managed_final_record: RuntimeAdmissionManagedFinalRecord,
    native_closure: VerifiedPassedRuntimeAdmissionNativeClosureControl,
    managed_startup: VerifiedPassedRuntimeAdmissionManagedStartupControl,
    cloud_disable: VerifiedPassedRuntimeAdmissionCloudDisableControl,
}

impl RuntimeAdmissionFinalOperation {
    #[cfg(test)]
    pub(crate) fn test_from_review_records(
        native_load_record: RuntimeAdmissionNativeLoadRecord,
        managed_final_record: RuntimeAdmissionManagedFinalRecord,
        native_closure: VerifiedPassedRuntimeAdmissionNativeClosureControl,
        managed_startup: VerifiedPassedRuntimeAdmissionManagedStartupControl,
        cloud_disable: VerifiedPassedRuntimeAdmissionCloudDisableControl,
    ) -> Self {
        Self {
            native_load_record,
            managed_final_record,
            native_closure,
            managed_startup,
            cloud_disable,
        }
    }

    #[cfg(test)]
    pub(crate) fn test_review_fixture(
        foundation: &VerifiedRuntimeAdmissionFoundationBinding,
        package: &RuntimePackageManifest,
    ) -> Self {
        super::records::review_operation_fixture(foundation, package)
    }

    pub(crate) fn verify_records_for_review(
        &self,
        foundation: &VerifiedRuntimeAdmissionFoundationBinding,
        package: &RuntimePackageManifest,
    ) -> Result<(), RuntimeAdmissionRunnerError> {
        super::records::verify_compiled_for_review(self, foundation, package)
    }

    /// Returns canonical native-load publication material.
    #[must_use]
    pub const fn native_load_record(&self) -> &RuntimeAdmissionNativeLoadRecord {
        &self.native_load_record
    }

    /// Returns canonical managed-final publication material.
    #[must_use]
    pub const fn managed_final_record(&self) -> &RuntimeAdmissionManagedFinalRecord {
        &self.managed_final_record
    }

    /// Returns the independently verified foundation-bound native-closure result.
    #[must_use]
    pub const fn native_closure(&self) -> &VerifiedPassedRuntimeAdmissionNativeClosureControl {
        &self.native_closure
    }

    /// Returns the independently verified foundation-bound managed-startup result.
    #[must_use]
    pub const fn managed_startup(&self) -> &VerifiedPassedRuntimeAdmissionManagedStartupControl {
        &self.managed_startup
    }

    /// Returns the independently verified foundation-bound cloud-disable result.
    #[must_use]
    pub const fn cloud_disable(&self) -> &VerifiedPassedRuntimeAdmissionCloudDisableControl {
        &self.cloud_disable
    }
}

impl RuntimeAdmissionRunner {
    /// Launches the retained package, discovers native closure, and always cleans up.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeAdmissionRunnerError`] for invalid bindings, observation
    /// failure, cancellation, or incomplete managed-process cleanup.
    pub fn discover_managed_native_closure(
        mut request: RuntimeAdmissionNativeClosureDiscoveryRequest<'_>,
    ) -> Result<RuntimeAdmissionNativeClosureDiscovery, RuntimeAdmissionRunnerError> {
        validate_launch_context(
            request.package,
            request.package_lease,
            request.isolation,
            request.limits,
            request.cancellation,
        )?;
        let executable = request
            .package_lease
            .clone_entrypoint_for_launch(request.cancellation)
            .map_err(map_package_error)?;
        let isolation = request
            .isolation
            .launch_retained(request.launch, executable, request.cancellation)
            .map_err(RuntimeAdmissionRunnerError::Isolation)?;
        let mut managed = RuntimeAdmissionManagedRuntimeLease::new(isolation);
        let operation = discover_launched(&mut managed, &mut request);
        let discovery = finish_with_mandatory_finalization(
            operation,
            |fresh| managed.close(fresh),
            |fresh| {
                request
                    .package_lease
                    .revalidate(fresh)
                    .map_err(map_package_error)
            },
            request.cancellation,
        )?;
        Ok(RuntimeAdmissionNativeClosureDiscovery { discovery })
    }

    /// Launches, performs final verification, always cleans up, then verifies records.
    ///
    /// This operation does not mutate an allowlist, policy, qualification, role,
    /// or generation state.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeAdmissionRunnerError`] unless the foundation, package,
    /// frozen set, live evidence, cleanup, canonical records, and independent
    /// record verification all succeed.
    pub async fn verify_managed_final_and_close(
        mut request: RuntimeAdmissionFinalOperationRequest<'_>,
    ) -> Result<RuntimeAdmissionFinalOperation, RuntimeAdmissionRunnerError> {
        let final_evidence = verify_and_close(&mut request).await?;
        let native_load_record = RuntimeAdmissionNativeLoadRecord::compile(
            request.foundation,
            request.package,
            request.frozen,
            final_evidence.native_load(),
        )?;
        let native_closure = RuntimeAdmissionNativeLoadRecordVerifier::verify(
            native_load_record.canonical_bytes(),
            request.foundation,
            request.package,
            request.frozen,
            final_evidence.native_load(),
        )?;
        let managed_final_record = RuntimeAdmissionManagedFinalRecord::compile(
            request.foundation,
            request.package,
            request.frozen,
            &final_evidence,
            native_load_record.record_id(),
        )?;
        let (managed_startup, cloud_disable) = RuntimeAdmissionManagedFinalRecordVerifier::verify(
            managed_final_record.canonical_bytes(),
            request.foundation,
            request.package,
            request.frozen,
            &final_evidence,
            &native_closure,
        )?;
        ensure_not_cancelled(request.cancellation)?;
        Ok(RuntimeAdmissionFinalOperation {
            native_load_record,
            managed_final_record,
            native_closure,
            managed_startup,
            cloud_disable,
        })
    }

    /// Observes the exact frozen-set-gated managed runtime and closes it before
    /// compiling canonical native and managed observation records.
    ///
    /// An `Unreviewed` cloud status is preserved as observation evidence. This
    /// separate result grants no passed cloud control, startup-control authority,
    /// admission, generation, or policy authority. Production policy is unchanged.
    /// Mandatory cleanup and package revalidation both use fresh contexts even
    /// when the primary operation or another finalizer fails.
    ///
    /// # Errors
    /// Returns [`RuntimeAdmissionRunnerError`] for exact binding failures,
    /// observation errors, cancellation, cleanup, package drift, malformed
    /// canonical records, or independent reparse disagreement.
    pub async fn observe_managed_final_and_close(
        mut request: RuntimeAdmissionFinalOperationRequest<'_>,
    ) -> Result<RuntimeAdmissionObservedFinalOperation, RuntimeAdmissionRunnerError> {
        let final_evidence = verify_and_close(&mut request).await?;
        let observed = RuntimeAdmissionObservedFinalOperation::compile(
            request.foundation,
            request.package,
            request.frozen,
            &final_evidence,
        )?;
        ensure_not_cancelled(request.cancellation)?;
        Ok(observed)
    }
}

async fn verify_and_close(
    request: &mut RuntimeAdmissionFinalOperationRequest<'_>,
) -> Result<super::RuntimeAdmissionFinalVerification, RuntimeAdmissionRunnerError> {
    if request.foundation.runtime_package_manifest_id()
        != &request.package.runtime_package_manifest_id()
    {
        return Err(RuntimeAdmissionRunnerError::InvalidPackageBinding);
    }
    validate_launch_context(
        request.package,
        request.package_lease,
        request.isolation,
        request.limits,
        request.cancellation,
    )?;
    let executable = request
        .package_lease
        .clone_entrypoint_for_launch(request.cancellation)
        .map_err(map_package_error)?;
    let isolation = request
        .isolation
        .launch_retained(request.launch, executable, request.cancellation)
        .map_err(RuntimeAdmissionRunnerError::Isolation)?;
    let mut managed = RuntimeAdmissionManagedRuntimeLease::new(isolation);
    let operation =
        RuntimeAdmissionRunner::verify_final(RuntimeAdmissionFinalVerificationRequest {
            package: request.package,
            package_lease: request.package_lease,
            managed_runtime: &mut managed,
            frozen: request.frozen,
            endpoint: request.endpoint.clone(),
            launch: request.launch,
            limits: request.limits,
            cancellation: request.cancellation,
        })
        .await;
    finish_with_mandatory_finalization(
        operation,
        |fresh| managed.close(fresh),
        |fresh| {
            request
                .package_lease
                .revalidate(fresh)
                .map_err(map_package_error)
        },
        request.cancellation,
    )
}

fn finish_with_mandatory_finalization<T>(
    operation: Result<T, RuntimeAdmissionRunnerError>,
    cleanup: impl FnOnce(&CancellationToken) -> Result<(), super::IsolationError>,
    finalize_package: impl FnOnce(&CancellationToken) -> Result<(), RuntimeAdmissionRunnerError>,
    cancellation: &CancellationToken,
) -> Result<T, RuntimeAdmissionRunnerError> {
    let closed = finish_with_fresh_cleanup(operation, cleanup);
    let package = finalize_package(&CancellationToken::new());
    match (closed, package) {
        (Ok(value), Ok(())) => {
            ensure_not_cancelled(cancellation)?;
            Ok(value)
        }
        (Err(operation), Ok(())) => Err(operation),
        (Ok(_), Err(finalization)) => Err(finalization),
        (Err(operation), Err(finalization)) => {
            Err(RuntimeAdmissionRunnerError::FinalizationAfterFailure {
                operation: Box::new(operation),
                finalization: Box::new(finalization),
            })
        }
    }
}

fn validate_launch_context(
    package: &RuntimePackageManifest,
    package_lease: &RuntimePackageLease,
    isolation: &PreparedIsolation,
    limits: RuntimeAdmissionRunnerLimits,
    cancellation: &CancellationToken,
) -> Result<(), RuntimeAdmissionRunnerError> {
    ensure_not_cancelled(cancellation)?;
    if package.runtime_family() != ADMITTED_RUNTIME_FAMILY
        || package.target().operating_system() != RuntimeOperatingSystem::Linux
        || !valid_managed_process_limits(limits.managed_process)
        || !valid_native_limits(limits.native_load)
    {
        return Err(RuntimeAdmissionRunnerError::InvalidInput);
    }
    validate_package_binding(package, package_lease)?;
    let preparation = isolation.preparation_evidence();
    let mut helpers = package.members().iter().filter(|member| {
        member
            .roles()
            .contains(&RuntimePackageMemberRole::HelperExecutable)
    });
    let helper = helpers
        .next()
        .ok_or(RuntimeAdmissionRunnerError::InvalidPackageBinding)?;
    if helpers.next().is_some()
        || helper.roles() != [RuntimePackageMemberRole::HelperExecutable]
        || helper.load_policy() != RuntimePackageLoadPolicy::MustNotBeCodeLoaded
        || helper.artifact_id().digest() != preparation.helper_digest()
        || helper.byte_size() != preparation.helper_bytes()
        || !preparation.all_canaries_passed()
    {
        return Err(RuntimeAdmissionRunnerError::InvalidPackageBinding);
    }
    Ok(())
}

fn discover_launched(
    managed: &mut RuntimeAdmissionManagedRuntimeLease,
    request: &mut RuntimeAdmissionNativeClosureDiscoveryRequest<'_>,
) -> Result<NativeLoadDiscovery, RuntimeAdmissionRunnerError> {
    validate_actual_launch_binding(request.launch, managed.launch_spec_digest())?;
    let initial_isolation = managed.isolation.initial_evidence();
    let expectation = managed_process_expectation(&initial_isolation)?;
    let endpoint = request.endpoint.socket_addr();
    let channel = managed
        .isolation
        .connect_loopback(endpoint, request.cancellation)
        .map_err(RuntimeAdmissionRunnerError::Isolation)?;
    let (stream, diagnostics, _startup) = channel.into_parts();
    let _retained_stream = stream;
    let listener = ListenerEndpoint::new(endpoint).map_err(RuntimeAdmissionRunnerError::Witness)?;
    let mut process = NativeManagedLinuxProcessObserver
        .attach(
            listener,
            diagnostics.into_file(),
            expectation,
            request.limits.managed_process,
            request.cancellation,
        )
        .map_err(RuntimeAdmissionRunnerError::Witness)?;
    let package_id = request.package.runtime_package_manifest_id();
    validate_final_identity_bindings(
        request.package,
        &package_id,
        &package_id,
        process.initial_evidence(),
    )?;
    let discovery = RuntimeAdmissionRunner::discover_native_closure(
        request.package,
        request.package_lease,
        &mut process,
        request.limits.native_load,
        request.cancellation,
    )?;
    let final_process = process
        .reobserve(request.cancellation)
        .map_err(RuntimeAdmissionRunnerError::Witness)?;
    validate_final_process_binding(process.initial_evidence(), &final_process, request.package)?;
    let final_isolation = managed
        .isolation
        .reobserve(request.cancellation)
        .map_err(RuntimeAdmissionRunnerError::Isolation)?;
    if initial_isolation != final_isolation {
        return Err(RuntimeAdmissionRunnerError::InvalidEvidenceBinding);
    }
    let reparsed = NativeLoadDiscovery::from_json_bytes(
        discovery.canonical_json_bytes(),
        &request.package.runtime_package_manifest_id(),
    )
    .map_err(RuntimeAdmissionRunnerError::NativeLoad)?;
    if reparsed != discovery {
        return Err(RuntimeAdmissionRunnerError::InvalidRecordEncoding);
    }
    Ok(reparsed)
}

fn finish_with_fresh_cleanup<T, F>(
    operation: Result<T, RuntimeAdmissionRunnerError>,
    cleanup: F,
) -> Result<T, RuntimeAdmissionRunnerError>
where
    F: FnOnce(&CancellationToken) -> Result<(), super::IsolationError>,
{
    let cleanup = cleanup(&CancellationToken::new());
    match (operation, cleanup) {
        (Ok(value), Ok(())) => Ok(value),
        (Ok(_), Err(error)) => Err(RuntimeAdmissionRunnerError::Cleanup(error)),
        (Err(error), Ok(())) => Err(error),
        (Err(operation), Err(cleanup)) => Err(RuntimeAdmissionRunnerError::CleanupAfterFailure {
            operation: Box::new(operation),
            cleanup,
        }),
    }
}

#[cfg(test)]
mod tests;
