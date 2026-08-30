use super::{
    ADMITTED_RUNTIME_FAMILY, ArtifactSetId, AttachedProcessEvidence, AttachedProcessEvidenceClass,
    AttachedProcessLaunchMode, AttachedProcessWitnessLimits, CancellationToken, Digest, LaunchSpec,
    MAXIMUM_DESCRIPTORS_PER_PROCESS, MAXIMUM_ENTRYPOINT_BYTES, MAXIMUM_NATIVE_LOAD_HASH_BYTES,
    MAXIMUM_NATIVE_LOAD_OBSERVATION_MILLIS, MAXIMUM_NATIVE_LOADED_COMPONENTS,
    MAXIMUM_NATIVE_MAPPING_METADATA_BYTES, MAXIMUM_NATIVE_MAPPING_REGIONS,
    MAXIMUM_OBSERVATION_MILLIS, MAXIMUM_OBSERVED_PROCESSES, MAXIMUM_SOCKET_TABLE_BYTES,
    MAXIMUM_SOCKET_TABLE_ENTRIES, ManagedStartupOutput, NativeLoadObservation,
    NativeLoadObservationLimits, OllamaCloudDisableEvidenceError, OllamaCloudDisableFeaturePolicy,
    OllamaCloudDisableStartupMarker, OllamaCloudDisableVersionStatus, OllamaEndpoint,
    OllamaManagedCloudDisableEnvironment, OllamaSingleConnectionRuntimeProbe, OllamaVersion, OsStr,
    PACKAGE_ATTESTATION_SCHEMA_VERSION, PackageAttestationError, PackageAttestationScope,
    RuntimeAdmissionManagedStartupEvidence, RuntimeAdmissionRunnerError,
    RuntimeAdmissionRunnerLimits, RuntimeOperatingSystem, RuntimePackageAttestationEvidence,
    RuntimePackageLease, RuntimePackageManifest, RuntimePackageManifestId,
    RuntimePackageMemberRole, VerifiedFrozenExternalNativeComponentSet,
};

pub(super) struct ValidatedPreconnectionInputs {
    pub(super) package_id: RuntimePackageManifestId,
    pub(super) version_status: OllamaCloudDisableVersionStatus,
    pub(super) managed_environment: OllamaManagedCloudDisableEnvironment,
    pub(super) launch_spec_digest: Digest,
    pub(super) isolation_policy_digest: Digest,
    pub(super) probe: OllamaSingleConnectionRuntimeProbe,
}

#[expect(
    clippy::too_many_arguments,
    reason = "preconnection validation keeps every trust-boundary binding explicit"
)]
pub(super) fn validate_final_preconnection_inputs(
    package: &RuntimePackageManifest,
    package_lease: &RuntimePackageLease,
    frozen: &VerifiedFrozenExternalNativeComponentSet,
    endpoint: OllamaEndpoint,
    launch: &LaunchSpec,
    actual_launch_spec_digest: &Digest,
    actual_isolation_policy_digest: &Digest,
    limits: RuntimeAdmissionRunnerLimits,
    cancellation: &CancellationToken,
) -> Result<ValidatedPreconnectionInputs, RuntimeAdmissionRunnerError> {
    ensure_not_cancelled(cancellation)?;
    if !valid_managed_process_limits(limits.managed_process)
        || !valid_native_limits(limits.native_load)
    {
        return Err(RuntimeAdmissionRunnerError::InvalidInput);
    }
    let package_id = validate_package_binding(package, package_lease)?;
    validate_final_package_and_frozen_binding(
        package,
        &package_id,
        frozen.runtime_package_manifest_id(),
    )?;
    let launch_spec_digest = validate_actual_launch_binding(launch, actual_launch_spec_digest)?;
    let value = launch
        .environment_value(OsStr::new("OLLAMA_NO_CLOUD"))
        .and_then(OsStr::to_str)
        .ok_or(OllamaCloudDisableEvidenceError::MissingEnvironmentDeclaration)
        .map_err(RuntimeAdmissionRunnerError::CloudDisable)?;
    let managed_environment = OllamaManagedCloudDisableEnvironment::parse(&[value])
        .map_err(RuntimeAdmissionRunnerError::CloudDisable)?;
    let runtime_version = package
        .reported_version()
        .parse::<OllamaVersion>()
        .map_err(|_error| RuntimeAdmissionRunnerError::InvalidInput)?;
    let version_status = OllamaCloudDisableFeaturePolicy::assess(runtime_version, &package_id);
    if version_status == OllamaCloudDisableVersionStatus::FeatureUnavailable {
        return Err(RuntimeAdmissionRunnerError::CloudDisableFeatureUnavailable);
    }
    let probe = OllamaSingleConnectionRuntimeProbe::new(endpoint, runtime_version, limits.ollama)
        .map_err(RuntimeAdmissionRunnerError::Probe)?;
    Ok(ValidatedPreconnectionInputs {
        package_id,
        version_status,
        managed_environment,
        launch_spec_digest,
        isolation_policy_digest: actual_isolation_policy_digest.clone(),
        probe,
    })
}

pub(super) fn validate_actual_launch_binding(
    reviewed_launch: &LaunchSpec,
    actual_launch_spec_digest: &Digest,
) -> Result<Digest, RuntimeAdmissionRunnerError> {
    let reviewed_digest = reviewed_launch.redacted_digest();
    if &reviewed_digest == actual_launch_spec_digest {
        Ok(reviewed_digest)
    } else {
        Err(RuntimeAdmissionRunnerError::InvalidLaunchBinding)
    }
}

fn validate_final_package_and_frozen_binding(
    package: &RuntimePackageManifest,
    package_id: &RuntimePackageManifestId,
    frozen_package_id: &RuntimePackageManifestId,
) -> Result<(), RuntimeAdmissionRunnerError> {
    if package.runtime_family() != ADMITTED_RUNTIME_FAMILY
        || package.target().operating_system() != RuntimeOperatingSystem::Linux
    {
        return Err(RuntimeAdmissionRunnerError::InvalidInput);
    }
    if package_id != frozen_package_id {
        return Err(RuntimeAdmissionRunnerError::InvalidFrozenSetBinding);
    }
    Ok(())
}

pub(super) fn validate_final_identity_bindings(
    package: &RuntimePackageManifest,
    package_id: &RuntimePackageManifestId,
    frozen_package_id: &RuntimePackageManifestId,
    process: &AttachedProcessEvidence,
) -> Result<(), RuntimeAdmissionRunnerError> {
    validate_final_package_and_frozen_binding(package, package_id, frozen_package_id)?;
    validate_managed_process_binding(process, package)
}

pub(super) fn validate_package_binding(
    package: &RuntimePackageManifest,
    lease: &RuntimePackageLease,
) -> Result<RuntimePackageManifestId, RuntimeAdmissionRunnerError> {
    validate_package_facts(
        package,
        lease.evidence(),
        lease.installation_key().artifact_set_id(),
    )
}

fn validate_package_facts(
    package: &RuntimePackageManifest,
    evidence: &RuntimePackageAttestationEvidence,
    installed_artifact_set_id: &ArtifactSetId,
) -> Result<RuntimePackageManifestId, RuntimeAdmissionRunnerError> {
    let package_id = package.runtime_package_manifest_id();
    let (code_member_count, code_byte_size) = package
        .members()
        .iter()
        .filter(|member| {
            member.roles().iter().any(|role| {
                matches!(
                    role,
                    RuntimePackageMemberRole::Entrypoint
                        | RuntimePackageMemberRole::NativeDependency
                        | RuntimePackageMemberRole::HelperExecutable
                )
            })
        })
        .try_fold((0_u32, 0_u64), |(count, bytes), member| {
            Some((
                count.checked_add(1)?,
                bytes.checked_add(member.byte_size())?,
            ))
        })
        .ok_or(RuntimeAdmissionRunnerError::InvalidPackageBinding)?;
    if evidence.schema_version() != PACKAGE_ATTESTATION_SCHEMA_VERSION
        || evidence.scope() != PackageAttestationScope::StaticManagedBytes
        || evidence.artifact_set_id() != package.artifact_set_id()
        || installed_artifact_set_id != package.artifact_set_id()
        || evidence.runtime_package_manifest_id() != &package_id
        || evidence.entrypoint_artifact_id() != package.entrypoint().artifact_id()
        || evidence.code_member_count() != code_member_count
        || evidence.code_byte_size() != code_byte_size
    {
        return Err(RuntimeAdmissionRunnerError::InvalidPackageBinding);
    }
    Ok(package_id)
}

pub(super) fn validate_discovery_process_binding(
    evidence: &AttachedProcessEvidence,
    package: &RuntimePackageManifest,
) -> Result<(), RuntimeAdmissionRunnerError> {
    if evidence.entrypoint_digest() != package.entrypoint().artifact_id().digest()
        || evidence.entrypoint_bytes() != package.entrypoint().byte_size()
    {
        return Err(RuntimeAdmissionRunnerError::InvalidProcessBinding);
    }
    Ok(())
}

fn validate_managed_process_binding(
    evidence: &AttachedProcessEvidence,
    package: &RuntimePackageManifest,
) -> Result<(), RuntimeAdmissionRunnerError> {
    if evidence.evidence_class() != AttachedProcessEvidenceClass::LinuxManagedNamespaceSockDiag
        || evidence.launch_mode() != AttachedProcessLaunchMode::ManagedLinuxIsolation
    {
        return Err(RuntimeAdmissionRunnerError::InvalidProcessBinding);
    }
    validate_discovery_process_binding(evidence, package)
}

pub(super) fn validate_managed_startup(
    startup: &ManagedStartupOutput,
    package_id: RuntimePackageManifestId,
    process_evidence_digest: Digest,
    launch_spec_digest: Digest,
    isolation_policy_digest: Digest,
) -> Result<
    (
        OllamaCloudDisableStartupMarker,
        RuntimeAdmissionManagedStartupEvidence,
    ),
    RuntimeAdmissionRunnerError,
> {
    compile_managed_startup_evidence(
        startup.standard_output(),
        startup.standard_error(),
        startup.standard_output_truncated(),
        startup.standard_error_truncated(),
        package_id,
        process_evidence_digest,
        launch_spec_digest,
        isolation_policy_digest,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "the private deterministic seam mirrors the bounded startup evidence fields"
)]
pub(super) fn compile_managed_startup_evidence(
    standard_output: &[u8],
    standard_error: &[u8],
    standard_output_truncated: bool,
    standard_error_truncated: bool,
    package_id: RuntimePackageManifestId,
    process_evidence_digest: Digest,
    launch_spec_digest: Digest,
    isolation_policy_digest: Digest,
) -> Result<
    (
        OllamaCloudDisableStartupMarker,
        RuntimeAdmissionManagedStartupEvidence,
    ),
    RuntimeAdmissionRunnerError,
> {
    if standard_output_truncated || standard_error_truncated {
        return Err(RuntimeAdmissionRunnerError::TruncatedStartupOutput);
    }
    let startup_marker =
        OllamaCloudDisableStartupMarker::parse_streams(standard_output, standard_error)
            .map_err(RuntimeAdmissionRunnerError::CloudDisable)?;
    let standard_output_bytes = u64::try_from(standard_output.len())
        .map_err(|_error| RuntimeAdmissionRunnerError::InvalidInput)?;
    let standard_error_bytes = u64::try_from(standard_error.len())
        .map_err(|_error| RuntimeAdmissionRunnerError::InvalidInput)?;
    let evidence = RuntimeAdmissionManagedStartupEvidence {
        runtime_package_manifest_id: package_id,
        process_evidence_digest,
        launch_spec_digest,
        isolation_policy_digest,
        standard_output_digest: Digest::sha256(standard_output),
        standard_error_digest: Digest::sha256(standard_error),
        standard_output_bytes,
        standard_error_bytes,
    };
    Ok((startup_marker, evidence))
}

pub(super) fn valid_managed_process_limits(limits: AttachedProcessWitnessLimits) -> bool {
    limits.maximum_socket_table_bytes > 0
        && limits.maximum_socket_table_bytes <= MAXIMUM_SOCKET_TABLE_BYTES
        && limits.maximum_socket_table_entries > 0
        && limits.maximum_socket_table_entries <= MAXIMUM_SOCKET_TABLE_ENTRIES
        && limits.maximum_processes > 0
        && limits.maximum_processes <= MAXIMUM_OBSERVED_PROCESSES
        && limits.maximum_descriptors_per_process > 0
        && limits.maximum_descriptors_per_process <= MAXIMUM_DESCRIPTORS_PER_PROCESS
        && limits.maximum_entrypoint_bytes > 0
        && limits.maximum_entrypoint_bytes <= MAXIMUM_ENTRYPOINT_BYTES
        && !limits.maximum_elapsed.is_zero()
        && limits.maximum_elapsed <= std::time::Duration::from_millis(MAXIMUM_OBSERVATION_MILLIS)
}

pub(super) fn valid_native_limits(limits: NativeLoadObservationLimits) -> bool {
    limits.maximum_mapping_regions > 0
        && limits.maximum_mapping_regions <= MAXIMUM_NATIVE_MAPPING_REGIONS
        && limits.maximum_mapping_metadata_bytes > 0
        && limits.maximum_mapping_metadata_bytes <= MAXIMUM_NATIVE_MAPPING_METADATA_BYTES
        && limits.maximum_components > 0
        && limits.maximum_components <= MAXIMUM_NATIVE_LOADED_COMPONENTS
        && limits.maximum_aggregate_hash_bytes > 0
        && limits.maximum_aggregate_hash_bytes <= MAXIMUM_NATIVE_LOAD_HASH_BYTES
        && !limits.maximum_elapsed.is_zero()
        && limits.maximum_elapsed
            <= std::time::Duration::from_millis(MAXIMUM_NATIVE_LOAD_OBSERVATION_MILLIS)
}

pub(super) fn validate_native_binding(
    observation: &NativeLoadObservation,
    package_id: &RuntimePackageManifestId,
    initial_process: &AttachedProcessEvidence,
) -> Result<(), RuntimeAdmissionRunnerError> {
    if observation.runtime_package_manifest_id() != package_id
        || observation.process_evidence_digest() != initial_process.evidence_digest()
    {
        return Err(RuntimeAdmissionRunnerError::InvalidEvidenceBinding);
    }
    Ok(())
}

pub(super) fn validate_final_process_binding(
    initial: &AttachedProcessEvidence,
    final_evidence: &AttachedProcessEvidence,
    package: &RuntimePackageManifest,
) -> Result<(), RuntimeAdmissionRunnerError> {
    validate_managed_process_binding(final_evidence, package)?;
    if initial.evidence_digest() != final_evidence.evidence_digest() {
        return Err(RuntimeAdmissionRunnerError::InvalidEvidenceBinding);
    }
    Ok(())
}

pub(super) fn ensure_not_cancelled(
    cancellation: &CancellationToken,
) -> Result<(), RuntimeAdmissionRunnerError> {
    if cancellation.is_cancelled() {
        Err(RuntimeAdmissionRunnerError::Cancelled)
    } else {
        Ok(())
    }
}

pub(super) fn map_package_error(error: PackageAttestationError) -> RuntimeAdmissionRunnerError {
    if matches!(error, PackageAttestationError::Cancelled) {
        RuntimeAdmissionRunnerError::Cancelled
    } else {
        RuntimeAdmissionRunnerError::Package(error)
    }
}
