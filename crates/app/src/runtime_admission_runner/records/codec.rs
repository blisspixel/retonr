use rewrite_model::{
    NativeLoadObservation, NativeLoadObservationId, NativeLoadOrigin, RuntimePackageManifest,
    RuntimePackageManifestId,
};
use rewrite_ollama::OllamaCloudDisableVersionStatus;
use rewrite_runtime_attestor::{
    FrozenExternalNativeComponentSetId, VerifiedFrozenExternalNativeComponentSet,
};
use rewrite_types::Digest;
use serde::{Deserialize, Serialize};

use super::{
    MANAGED_RECORD_SCHEMA_VERSION, NATIVE_RECORD_SCHEMA_VERSION, RuntimeAdmissionNativeLoadRecordId,
};
use crate::{
    RuntimeAdmissionEvidenceFoundationId, RuntimeAdmissionFinalVerification,
    RuntimeAdmissionRunnerError, VerifiedRuntimeAdmissionFoundationBinding,
};

pub(super) fn validate_subjects(
    foundation: &VerifiedRuntimeAdmissionFoundationBinding,
    package: &RuntimePackageManifest,
    frozen: &VerifiedFrozenExternalNativeComponentSet,
) -> Result<(), RuntimeAdmissionRunnerError> {
    let package_id = package.runtime_package_manifest_id();
    if foundation.runtime_package_manifest_id() != &package_id {
        return Err(RuntimeAdmissionRunnerError::InvalidPackageBinding);
    }
    if frozen.runtime_package_manifest_id() != &package_id {
        return Err(RuntimeAdmissionRunnerError::InvalidFrozenSetBinding);
    }
    Ok(())
}

pub(super) fn validate_native_observation(
    package: &RuntimePackageManifest,
    frozen: &VerifiedFrozenExternalNativeComponentSet,
    observation: &NativeLoadObservation,
) -> Result<(), RuntimeAdmissionRunnerError> {
    if observation.runtime_package_manifest_id() != &package.runtime_package_manifest_id() {
        return Err(RuntimeAdmissionRunnerError::InvalidEvidenceBinding);
    }
    let external = observation
        .components()
        .iter()
        .filter(|component| {
            matches!(
                component.origin(),
                NativeLoadOrigin::ExternalPlatformComponent
            )
        })
        .collect::<Vec<_>>();
    if external.len() != frozen.expected_components().len()
        || external
            .iter()
            .zip(frozen.expected_components())
            .any(|(observed, expected)| {
                observed.artifact_id() != expected.artifact_id()
                    || observed.byte_size() != expected.byte_size()
                    || observed.mapping_class() != expected.mapping_class()
            })
    {
        return Err(RuntimeAdmissionRunnerError::InvalidFrozenSetBinding);
    }
    Ok(())
}

pub(super) fn derive_native_wire(
    foundation_id: &RuntimeAdmissionEvidenceFoundationId,
    package: &RuntimePackageManifest,
    frozen: &VerifiedFrozenExternalNativeComponentSet,
    observation: &NativeLoadObservation,
) -> Result<NativeLoadRecordWire, RuntimeAdmissionRunnerError> {
    let native_load = serde_json::to_value(observation)
        .map_err(|_| RuntimeAdmissionRunnerError::InvalidRecordEncoding)?;
    Ok(NativeLoadRecordWire {
        authority: Authority::None,
        control: NativeControl::NativeClosure,
        foundation_id: foundation_id.digest().clone(),
        frozen_external_component_set_id: frozen.frozen_set_id().clone(),
        native_load,
        native_load_observation_id: observation.native_load_observation_id(),
        runtime_package_manifest_id: package.runtime_package_manifest_id(),
        schema_version: NATIVE_RECORD_SCHEMA_VERSION,
        status: PassedStatus::Passed,
    })
}

pub(super) fn derive_managed_wire(
    foundation_id: &RuntimeAdmissionEvidenceFoundationId,
    package: &RuntimePackageManifest,
    frozen: &VerifiedFrozenExternalNativeComponentSet,
    final_evidence: &RuntimeAdmissionFinalVerification,
    native_record_id: &RuntimeAdmissionNativeLoadRecordId,
) -> Result<ManagedFinalRecordWire, RuntimeAdmissionRunnerError> {
    managed_wire_from_facts(
        foundation_id,
        package,
        frozen,
        native_record_id,
        managed_facts(final_evidence),
    )
}

fn managed_facts(final_evidence: &RuntimeAdmissionFinalVerification) -> ManagedFinalFacts {
    let startup = final_evidence.managed_startup();
    let cloud = final_evidence.cloud_disable();
    ManagedFinalFacts {
        runtime_package_manifest_id: final_evidence.runtime_package_manifest_id().clone(),
        frozen_external_component_set_id: final_evidence.frozen_external_component_set_id().clone(),
        native_runtime_package_manifest_id: final_evidence
            .native_load()
            .runtime_package_manifest_id()
            .clone(),
        native_process_evidence_digest: final_evidence
            .native_load()
            .process_evidence_digest()
            .clone(),
        native_load_observation_id: final_evidence.native_load().native_load_observation_id(),
        initial_process_evidence_digest: final_evidence
            .initial_process_evidence()
            .evidence_digest()
            .clone(),
        final_process_evidence_digest: final_evidence
            .final_process_evidence()
            .evidence_digest()
            .clone(),
        initial_connection_evidence_digest: final_evidence
            .initial_connection_evidence()
            .evidence_digest()
            .clone(),
        final_connection_evidence_digest: final_evidence
            .final_connection_evidence()
            .evidence_digest()
            .clone(),
        initial_isolation_evidence_digest: final_evidence
            .initial_isolation_evidence()
            .redacted_digest(),
        final_isolation_evidence_digest: final_evidence
            .final_isolation_evidence()
            .redacted_digest(),
        isolation_unchanged: final_evidence.initial_isolation_evidence()
            == final_evidence.final_isolation_evidence(),
        isolation_policy_digest: final_evidence.isolation_policy_digest().clone(),
        startup_runtime_package_manifest_id: startup.runtime_package_manifest_id().clone(),
        startup_process_evidence_digest: startup.process_evidence_digest().clone(),
        startup_launch_spec_digest: startup.launch_spec_digest().clone(),
        startup_isolation_policy_digest: startup.isolation_policy_digest().clone(),
        standard_output_digest: startup.standard_output_digest().clone(),
        standard_error_digest: startup.standard_error_digest().clone(),
        standard_output_bytes: startup.standard_output_bytes(),
        standard_error_bytes: startup.standard_error_bytes(),
        cloud_runtime_package_manifest_id: cloud.runtime_package_manifest_id().clone(),
        cloud_runtime_version: cloud.runtime_version().to_string(),
        probed_runtime_version: final_evidence.runtime_probe().runtime_version().to_string(),
        cloud_version_status: cloud.version_status(),
        managed_environment_observed: {
            let _environment = cloud.managed_environment();
            true
        },
        startup_marker_observed: {
            let _marker = cloud.startup_marker();
            true
        },
    }
}

fn managed_wire_from_facts(
    foundation_id: &RuntimeAdmissionEvidenceFoundationId,
    package: &RuntimePackageManifest,
    frozen: &VerifiedFrozenExternalNativeComponentSet,
    native_record_id: &RuntimeAdmissionNativeLoadRecordId,
    facts: ManagedFinalFacts,
) -> Result<ManagedFinalRecordWire, RuntimeAdmissionRunnerError> {
    let package_id = package.runtime_package_manifest_id();
    if facts.runtime_package_manifest_id != package_id
        || facts.frozen_external_component_set_id != *frozen.frozen_set_id()
        || facts.startup_runtime_package_manifest_id != package_id
        || facts.cloud_runtime_package_manifest_id != package_id
        || facts.native_runtime_package_manifest_id != package_id
        || facts.native_process_evidence_digest != facts.initial_process_evidence_digest
        || facts.startup_process_evidence_digest != facts.initial_process_evidence_digest
        || facts.startup_isolation_policy_digest != facts.isolation_policy_digest
        || facts.initial_process_evidence_digest != facts.final_process_evidence_digest
        || !facts.isolation_unchanged
        || facts.cloud_runtime_version != facts.probed_runtime_version
        || !facts.managed_environment_observed
        || !facts.startup_marker_observed
    {
        return Err(RuntimeAdmissionRunnerError::InvalidEvidenceBinding);
    }
    let version_status = match facts.cloud_version_status {
        OllamaCloudDisableVersionStatus::Unreviewed => CloudVersionStatus::Unreviewed,
        OllamaCloudDisableVersionStatus::Reviewed => CloudVersionStatus::Reviewed,
        OllamaCloudDisableVersionStatus::FeatureUnavailable => {
            return Err(RuntimeAdmissionRunnerError::CloudDisableFeatureUnavailable);
        }
    };
    Ok(ManagedFinalRecordWire {
        authority: Authority::None,
        cleanup: CleanupStatus::Complete,
        cloud_disable: CloudDisableWire {
            managed_environment_observed: true,
            runtime_version: facts.cloud_runtime_version,
            startup_marker_observed: true,
            version_status,
        },
        control: ManagedControl::ManagedFinal,
        final_connection_evidence_digest: facts.final_connection_evidence_digest,
        final_isolation_evidence_digest: facts.final_isolation_evidence_digest,
        final_process_evidence_digest: facts.final_process_evidence_digest,
        foundation_id: foundation_id.digest().clone(),
        frozen_external_component_set_id: frozen.frozen_set_id().clone(),
        initial_connection_evidence_digest: facts.initial_connection_evidence_digest,
        initial_isolation_evidence_digest: facts.initial_isolation_evidence_digest,
        initial_process_evidence_digest: facts.initial_process_evidence_digest,
        isolation_policy_digest: facts.isolation_policy_digest,
        managed_startup: ManagedStartupWire {
            isolation_policy_digest: facts.startup_isolation_policy_digest,
            launch_spec_digest: facts.startup_launch_spec_digest,
            process_evidence_digest: facts.startup_process_evidence_digest,
            standard_error_bytes: facts.standard_error_bytes,
            standard_error_digest: facts.standard_error_digest,
            standard_output_bytes: facts.standard_output_bytes,
            standard_output_digest: facts.standard_output_digest,
        },
        native_load_observation_id: facts.native_load_observation_id,
        native_load_record_id: native_record_id.digest().clone(),
        runtime_package_manifest_id: package_id,
        schema_version: MANAGED_RECORD_SCHEMA_VERSION,
        status: PassedStatus::Passed,
    })
}

#[derive(Clone)]
struct ManagedFinalFacts {
    runtime_package_manifest_id: RuntimePackageManifestId,
    frozen_external_component_set_id: FrozenExternalNativeComponentSetId,
    native_runtime_package_manifest_id: RuntimePackageManifestId,
    native_process_evidence_digest: Digest,
    native_load_observation_id: NativeLoadObservationId,
    initial_process_evidence_digest: Digest,
    final_process_evidence_digest: Digest,
    initial_connection_evidence_digest: Digest,
    final_connection_evidence_digest: Digest,
    initial_isolation_evidence_digest: Digest,
    final_isolation_evidence_digest: Digest,
    isolation_unchanged: bool,
    isolation_policy_digest: Digest,
    startup_runtime_package_manifest_id: RuntimePackageManifestId,
    startup_process_evidence_digest: Digest,
    startup_launch_spec_digest: Digest,
    startup_isolation_policy_digest: Digest,
    standard_output_digest: Digest,
    standard_error_digest: Digest,
    standard_output_bytes: u64,
    standard_error_bytes: u64,
    cloud_runtime_package_manifest_id: RuntimePackageManifestId,
    cloud_runtime_version: String,
    probed_runtime_version: String,
    cloud_version_status: OllamaCloudDisableVersionStatus,
    managed_environment_observed: bool,
    startup_marker_observed: bool,
}

pub(super) fn parse_native_observation(
    value: &serde_json::Value,
    package: &RuntimePackageManifest,
) -> Result<NativeLoadObservation, RuntimeAdmissionRunnerError> {
    let native_bytes = serde_json::to_vec(value)
        .map_err(|_| RuntimeAdmissionRunnerError::InvalidRecordEncoding)?;
    NativeLoadObservation::from_json_bytes(&native_bytes, package)
        .map_err(|_| RuntimeAdmissionRunnerError::InvalidRecordEncoding)
}

pub(super) fn encode<T: Serialize>(
    wire: &T,
    maximum_bytes: usize,
) -> Result<Vec<u8>, RuntimeAdmissionRunnerError> {
    let bytes =
        serde_json::to_vec(wire).map_err(|_| RuntimeAdmissionRunnerError::InvalidRecordEncoding)?;
    if bytes.is_empty() || bytes.len() > maximum_bytes {
        return Err(RuntimeAdmissionRunnerError::RecordLimitExceeded);
    }
    Ok(bytes)
}

pub(super) fn parse_canonical<T>(
    bytes: &[u8],
    maximum_bytes: usize,
) -> Result<T, RuntimeAdmissionRunnerError>
where
    T: for<'de> Deserialize<'de> + Serialize,
{
    if bytes.is_empty() || bytes.len() > maximum_bytes {
        return Err(RuntimeAdmissionRunnerError::RecordLimitExceeded);
    }
    let wire = serde_json::from_slice(bytes)
        .map_err(|_| RuntimeAdmissionRunnerError::InvalidRecordEncoding)?;
    if encode(&wire, maximum_bytes)? != bytes {
        return Err(RuntimeAdmissionRunnerError::InvalidRecordEncoding);
    }
    Ok(wire)
}

pub(super) fn record_digest(domain: &[u8], bytes: &[u8]) -> Digest {
    let mut material = Vec::with_capacity(domain.len() + 1 + bytes.len());
    material.extend_from_slice(domain);
    material.push(0);
    material.extend_from_slice(bytes);
    Digest::sha256(&material)
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Authority {
    None,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum PassedStatus {
    Passed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum NativeControl {
    NativeClosure,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NativeLoadRecordWire {
    authority: Authority,
    control: NativeControl,
    foundation_id: Digest,
    frozen_external_component_set_id: FrozenExternalNativeComponentSetId,
    pub(super) native_load: serde_json::Value,
    pub(super) native_load_observation_id: NativeLoadObservationId,
    runtime_package_manifest_id: RuntimePackageManifestId,
    schema_version: u32,
    status: PassedStatus,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ManagedControl {
    ManagedFinal,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum CleanupStatus {
    Complete,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum CloudVersionStatus {
    Unreviewed,
    Reviewed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CloudDisableWire {
    managed_environment_observed: bool,
    runtime_version: String,
    startup_marker_observed: bool,
    version_status: CloudVersionStatus,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManagedStartupWire {
    isolation_policy_digest: Digest,
    pub(super) launch_spec_digest: Digest,
    process_evidence_digest: Digest,
    standard_error_bytes: u64,
    standard_error_digest: Digest,
    standard_output_bytes: u64,
    standard_output_digest: Digest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManagedFinalRecordWire {
    authority: Authority,
    cleanup: CleanupStatus,
    cloud_disable: CloudDisableWire,
    control: ManagedControl,
    final_connection_evidence_digest: Digest,
    final_isolation_evidence_digest: Digest,
    final_process_evidence_digest: Digest,
    foundation_id: Digest,
    frozen_external_component_set_id: FrozenExternalNativeComponentSetId,
    initial_connection_evidence_digest: Digest,
    initial_isolation_evidence_digest: Digest,
    initial_process_evidence_digest: Digest,
    isolation_policy_digest: Digest,
    pub(super) managed_startup: ManagedStartupWire,
    pub(super) native_load_observation_id: NativeLoadObservationId,
    native_load_record_id: Digest,
    runtime_package_manifest_id: RuntimePackageManifestId,
    schema_version: u32,
    status: PassedStatus,
}

#[cfg(test)]
mod tests;
