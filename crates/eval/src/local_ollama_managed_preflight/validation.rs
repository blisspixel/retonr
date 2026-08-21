use std::{ffi::OsStr, time::Duration};

use rewrite_app::{
    PACKAGE_ATTESTATION_SCHEMA_VERSION, PackageAttestationScope, RuntimePackageAttestationEvidence,
    RuntimePackageLease,
};
use rewrite_model::{
    NativeMappingClass, RuntimeOperatingSystem, RuntimePackageLoadPolicy, RuntimePackageManifest,
    RuntimePackageMember, RuntimePackageMemberRole,
};
use rewrite_ollama::{
    OllamaCloudDisableEvidenceError, OllamaCloudDisableStartupMarker,
    OllamaManagedCloudDisableEnvironment,
};
use rewrite_runtime_attestor::{
    AttachedProcessEvidence, AttachedProcessEvidenceClass, AttachedProcessLaunchMode,
    AttachedProcessWitnessLimits, ExpectedExternalNativeComponent, MAXIMUM_DESCRIPTORS_PER_PROCESS,
    MAXIMUM_ENTRYPOINT_BYTES, MAXIMUM_NATIVE_LOAD_HASH_BYTES,
    MAXIMUM_NATIVE_LOAD_OBSERVATION_MILLIS, MAXIMUM_NATIVE_LOADED_COMPONENTS,
    MAXIMUM_NATIVE_MAPPING_METADATA_BYTES, MAXIMUM_NATIVE_MAPPING_REGIONS,
    MAXIMUM_OBSERVATION_MILLIS, MAXIMUM_OBSERVED_PROCESSES, MAXIMUM_SOCKET_TABLE_BYTES,
    MAXIMUM_SOCKET_TABLE_ENTRIES, ManagedLinuxProcessExpectation, NativeLoadObservationLimits,
};
use rewrite_runtime_isolation::{IsolationEvidence, LaunchSpec, ManagedStartupOutput};
use rewrite_types::Digest;

use crate::{
    LocalOllamaBoundPreflightPlan, LocalOllamaPreflightMode,
    local_ollama_bound_preflight::validate_bound_plan,
};

use super::{LocalOllamaManagedPreflightError, LocalOllamaManagedPreflightLimits};

pub(super) fn validate_static_inputs(
    package: &RuntimePackageManifest,
    lease: &RuntimePackageLease,
    plan: &LocalOllamaBoundPreflightPlan,
    external: &[ExpectedExternalNativeComponent],
    limits: LocalOllamaManagedPreflightLimits,
) -> Result<(), LocalOllamaManagedPreflightError> {
    validate_bound_plan(plan).map_err(LocalOllamaManagedPreflightError::BoundObservation)?;
    if !valid_managed_plan_binding(package, plan, limits.process)
        || !valid_process_limits(limits.process)
        || !valid_native_limits(limits.native_load)
        || !valid_external_components(external, limits.native_load.maximum_components)
    {
        return Err(LocalOllamaManagedPreflightError::InvalidInput);
    }
    validate_package_facts(
        package,
        lease.evidence(),
        lease.installation_key().artifact_set_id(),
    )
}

fn valid_managed_plan_binding(
    package: &RuntimePackageManifest,
    plan: &LocalOllamaBoundPreflightPlan,
    process_limits: AttachedProcessWitnessLimits,
) -> bool {
    package.runtime_family() == "ollama"
        && package.reported_version() == plan.preflight.expected_runtime_version
        && package.target().operating_system() == RuntimeOperatingSystem::Linux
        && plan.preflight.mode == LocalOllamaPreflightMode::Verify
        && plan.expected_entrypoint_digest.as_ref()
            == Some(package.entrypoint().artifact_id().digest())
        && process_limits.maximum_entrypoint_bytes == plan.maximum_entrypoint_bytes
}

fn validate_package_facts(
    package: &RuntimePackageManifest,
    evidence: &RuntimePackageAttestationEvidence,
    installed_artifact_set_id: &rewrite_model::ArtifactSetId,
) -> Result<(), LocalOllamaManagedPreflightError> {
    let package_id = package.runtime_package_manifest_id();
    let mut code = package.members().iter().filter(|member| {
        member.roles().iter().any(|role| {
            matches!(
                role,
                RuntimePackageMemberRole::Entrypoint
                    | RuntimePackageMemberRole::NativeDependency
                    | RuntimePackageMemberRole::HelperExecutable
            )
        })
    });
    let (count, bytes) = code
        .try_fold((0_u32, 0_u64), |(count, bytes), member| {
            Some((
                count.checked_add(1)?,
                bytes.checked_add(member.byte_size())?,
            ))
        })
        .ok_or(LocalOllamaManagedPreflightError::InvalidPackageBinding)?;
    if evidence.schema_version() != PACKAGE_ATTESTATION_SCHEMA_VERSION
        || evidence.scope() != PackageAttestationScope::StaticManagedBytes
        || evidence.artifact_set_id() != package.artifact_set_id()
        || installed_artifact_set_id != package.artifact_set_id()
        || evidence.runtime_package_manifest_id() != &package_id
        || evidence.entrypoint_artifact_id() != package.entrypoint().artifact_id()
        || evidence.code_member_count() != count
        || evidence.code_byte_size() != bytes
    {
        return Err(LocalOllamaManagedPreflightError::InvalidPackageBinding);
    }
    Ok(())
}

pub(super) fn exact_helper_member<'a>(
    package: &'a RuntimePackageManifest,
    helper_digest: &Digest,
    helper_bytes: u64,
) -> Result<&'a RuntimePackageMember, LocalOllamaManagedPreflightError> {
    let mut matches = package
        .members()
        .iter()
        .filter(|member| helper_matches(member, helper_digest, helper_bytes));
    let helper = matches
        .next()
        .ok_or(LocalOllamaManagedPreflightError::InvalidHelperBinding)?;
    if matches.next().is_some() {
        return Err(LocalOllamaManagedPreflightError::InvalidHelperBinding);
    }
    Ok(helper)
}

fn helper_matches(member: &RuntimePackageMember, digest: &Digest, bytes: u64) -> bool {
    member
        .roles()
        .contains(&RuntimePackageMemberRole::HelperExecutable)
        && !member
            .roles()
            .contains(&RuntimePackageMemberRole::Entrypoint)
        && member.load_policy() == RuntimePackageLoadPolicy::MustNotBeCodeLoaded
        && member.artifact_id().digest() == digest
        && member.byte_size() == bytes
}

pub(super) fn managed_expectation(
    evidence: &IsolationEvidence,
) -> Result<ManagedLinuxProcessExpectation, LocalOllamaManagedPreflightError> {
    let target = evidence.target();
    let network = evidence.network_namespace();
    ManagedLinuxProcessExpectation::new(
        target.outer_pid(),
        target.process_start_token(),
        target.executable_device(),
        target.executable_inode(),
        target.executable_bytes(),
        network.device(),
        network.inode(),
        target.namespace_user_id(),
    )
    .map_err(LocalOllamaManagedPreflightError::Witness)
}

pub(super) fn validate_isolation_binding(
    evidence: &IsolationEvidence,
    package: &RuntimePackageManifest,
) -> Result<(), LocalOllamaManagedPreflightError> {
    if !evidence.preparation().all_canaries_passed()
        || evidence.target().executable_bytes() != package.entrypoint().byte_size()
    {
        return Err(LocalOllamaManagedPreflightError::InvalidEvidenceBinding);
    }
    Ok(())
}

pub(super) fn validate_process_binding(
    evidence: &AttachedProcessEvidence,
    package: &RuntimePackageManifest,
) -> Result<(), LocalOllamaManagedPreflightError> {
    if evidence.evidence_class() != AttachedProcessEvidenceClass::LinuxManagedNamespaceSockDiag
        || evidence.launch_mode() != AttachedProcessLaunchMode::ManagedLinuxIsolation
        || evidence.entrypoint_digest() != package.entrypoint().artifact_id().digest()
        || evidence.entrypoint_bytes() != package.entrypoint().byte_size()
    {
        return Err(LocalOllamaManagedPreflightError::InvalidEvidenceBinding);
    }
    Ok(())
}

pub(super) fn validate_final_isolation(
    initial: &IsolationEvidence,
    final_evidence: &IsolationEvidence,
) -> Result<(), LocalOllamaManagedPreflightError> {
    if initial.redacted_digest() != final_evidence.redacted_digest() {
        return Err(LocalOllamaManagedPreflightError::InvalidEvidenceBinding);
    }
    Ok(())
}

pub(super) fn validate_cloud_launch(
    launch: &LaunchSpec,
    output: &ManagedStartupOutput,
) -> Result<(), LocalOllamaManagedPreflightError> {
    validate_cloud_observations(
        launch
            .environment_value(OsStr::new("OLLAMA_NO_CLOUD"))
            .and_then(OsStr::to_str),
        output.standard_output(),
        output.standard_error(),
        output.standard_output_truncated(),
        output.standard_error_truncated(),
    )
}

fn validate_cloud_observations(
    environment_value: Option<&str>,
    standard_output: &[u8],
    standard_error: &[u8],
    standard_output_truncated: bool,
    standard_error_truncated: bool,
) -> Result<(), LocalOllamaManagedPreflightError> {
    let value = environment_value
        .ok_or(OllamaCloudDisableEvidenceError::MissingEnvironmentDeclaration)
        .map_err(LocalOllamaManagedPreflightError::CloudDisable)?;
    OllamaManagedCloudDisableEnvironment::parse(&[value])
        .map_err(LocalOllamaManagedPreflightError::CloudDisable)?;
    if standard_output_truncated || standard_error_truncated {
        return Err(LocalOllamaManagedPreflightError::TruncatedStartupOutput);
    }
    OllamaCloudDisableStartupMarker::parse_streams(standard_output, standard_error)
        .map_err(LocalOllamaManagedPreflightError::CloudDisable)?;
    Ok(())
}

fn valid_process_limits(limits: AttachedProcessWitnessLimits) -> bool {
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
        && valid_elapsed(limits.maximum_elapsed, MAXIMUM_OBSERVATION_MILLIS)
}

fn valid_native_limits(limits: NativeLoadObservationLimits) -> bool {
    limits.maximum_mapping_regions > 0
        && limits.maximum_mapping_regions <= MAXIMUM_NATIVE_MAPPING_REGIONS
        && limits.maximum_mapping_metadata_bytes > 0
        && limits.maximum_mapping_metadata_bytes <= MAXIMUM_NATIVE_MAPPING_METADATA_BYTES
        && limits.maximum_components > 0
        && limits.maximum_components <= MAXIMUM_NATIVE_LOADED_COMPONENTS
        && limits.maximum_aggregate_hash_bytes > 0
        && limits.maximum_aggregate_hash_bytes <= MAXIMUM_NATIVE_LOAD_HASH_BYTES
        && valid_elapsed(
            limits.maximum_elapsed,
            MAXIMUM_NATIVE_LOAD_OBSERVATION_MILLIS,
        )
}

fn valid_elapsed(value: Duration, maximum_millis: u64) -> bool {
    !value.is_zero() && value <= Duration::from_millis(maximum_millis)
}

fn valid_external_components(
    components: &[ExpectedExternalNativeComponent],
    maximum: usize,
) -> bool {
    if components.len() > maximum {
        return false;
    }
    components.iter().all(|component| {
        component.byte_size() > 0
            && component.mapping_class() == NativeMappingClass::ExecutableMapped
    }) && components.windows(2).all(|pair| {
        pair[0].artifact_id().digest().as_str() < pair[1].artifact_id().digest().as_str()
    })
}

#[cfg(test)]
#[path = "validation/tests.rs"]
mod tests;
