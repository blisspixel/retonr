use super::validation::{
    ensure_not_cancelled, map_package_error, validate_discovery_process_binding,
    validate_package_binding,
};
use super::{
    AttachedProcessLease, CancellationToken, NativeLoadDiscovery, NativeLoadDiscoveryRequest,
    NativeLoadObservationLimits, RetainedNativePackageMember, RuntimeAdmissionRunner,
    RuntimeAdmissionRunnerError, RuntimePackageLease, RuntimePackageManifest,
    RuntimePackageManifestId,
};

impl RuntimeAdmissionRunner {
    /// Performs bounded non-authoritative external native-component discovery.
    ///
    /// The returned proposal has `authority: none` and cannot be passed to final verification.
    /// A separately reviewed [`super::VerifiedFrozenExternalNativeComponentSet`] is required
    /// there.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeAdmissionRunnerError`] for package/process mismatch, cancellation,
    /// retained-package drift, unsupported observation, access/visibility failure, or invalid
    /// observer output.
    pub fn discover_native_closure<L: AttachedProcessLease + ?Sized>(
        package: &RuntimePackageManifest,
        package_lease: &mut RuntimePackageLease,
        process: &mut L,
        limits: NativeLoadObservationLimits,
        cancellation: &CancellationToken,
    ) -> Result<NativeLoadDiscovery, RuntimeAdmissionRunnerError> {
        ensure_not_cancelled(cancellation)?;
        let package_id = validate_package_binding(package, package_lease)?;
        validate_discovery_process_binding(process.initial_evidence(), package)?;
        let retained = package_lease
            .clone_members_for_native_observation(cancellation)
            .map_err(map_package_error)?;
        let discovery = discover_with_retained_members(
            package,
            &package_id,
            &retained,
            process,
            limits,
            cancellation,
        )?;
        package_lease
            .revalidate(cancellation)
            .map_err(map_package_error)?;
        Ok(discovery)
    }
}

pub(super) fn discover_with_retained_members<L: AttachedProcessLease + ?Sized>(
    package: &RuntimePackageManifest,
    package_id: &RuntimePackageManifestId,
    retained: &[RetainedNativePackageMember],
    process: &mut L,
    limits: NativeLoadObservationLimits,
    cancellation: &CancellationToken,
) -> Result<NativeLoadDiscovery, RuntimeAdmissionRunnerError> {
    ensure_not_cancelled(cancellation)?;
    let discovery = process
        .discover_external_native_components(
            &NativeLoadDiscoveryRequest {
                package,
                expected_package_id: package_id,
                retained_package_members: retained,
                limits,
            },
            cancellation,
        )
        .map_err(RuntimeAdmissionRunnerError::NativeLoad)?;
    if discovery.runtime_package_manifest_id() != package_id
        || discovery.process_evidence_digest() != process.initial_evidence().evidence_digest()
    {
        return Err(RuntimeAdmissionRunnerError::InvalidEvidenceBinding);
    }
    Ok(discovery)
}
