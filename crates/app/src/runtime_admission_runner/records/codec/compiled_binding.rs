use rewrite_model::RuntimePackageManifest;
use rewrite_ollama::OllamaCloudDisableVersionStatus;

use super::{
    CloudVersionStatus, MANAGED_RECORD_SCHEMA_VERSION, ManagedFinalRecordWire,
    NATIVE_RECORD_SCHEMA_VERSION, NativeLoadRecordWire, parse_canonical, parse_native_observation,
    record_digest,
};
use crate::{
    RuntimeAdmissionFinalOperation, RuntimeAdmissionRunnerError,
    VerifiedRuntimeAdmissionFoundationBinding,
};

#[cfg(test)]
mod fixture;
#[cfg(test)]
pub(crate) use fixture::review_operation_fixture;
#[cfg(test)]
mod tests;

pub(crate) fn verify_compiled_for_review(
    operation: &RuntimeAdmissionFinalOperation,
    foundation: &VerifiedRuntimeAdmissionFoundationBinding,
    package: &RuntimePackageManifest,
) -> Result<(), RuntimeAdmissionRunnerError> {
    use super::super::{
        CLOUD_CONTROL_DOMAIN, MANAGED_RECORD_ID_DOMAIN,
        MAX_RUNTIME_ADMISSION_MANAGED_FINAL_RECORD_BYTES,
        MAX_RUNTIME_ADMISSION_NATIVE_LOAD_RECORD_BYTES, NATIVE_CONTROL_DOMAIN,
        NATIVE_RECORD_ID_DOMAIN, STARTUP_CONTROL_DOMAIN,
    };
    let native_record = operation.native_load_record();
    let managed_record = operation.managed_final_record();
    let native = operation.native_closure();
    let startup = operation.managed_startup();
    let cloud = operation.cloud_disable();
    let native_wire: NativeLoadRecordWire = parse_canonical(
        native_record.canonical_bytes(),
        MAX_RUNTIME_ADMISSION_NATIVE_LOAD_RECORD_BYTES,
    )?;
    let managed_wire: ManagedFinalRecordWire = parse_canonical(
        managed_record.canonical_bytes(),
        MAX_RUNTIME_ADMISSION_MANAGED_FINAL_RECORD_BYTES,
    )?;
    let observation = parse_native_observation(&native_wire.native_load, package)?;
    let package_id = package.runtime_package_manifest_id();
    let frozen = native.frozen_external_component_set_id();
    if foundation.runtime_package_manifest_id() != &package_id
        || native.foundation_id() != foundation.foundation_id()
        || startup.foundation_id() != foundation.foundation_id()
        || cloud.foundation_id() != foundation.foundation_id()
        || native_wire.foundation_id != *foundation.foundation_id().digest()
        || managed_wire.foundation_id != native_wire.foundation_id
        || native_wire.runtime_package_manifest_id != package_id
        || managed_wire.runtime_package_manifest_id != package_id
        || native_wire.frozen_external_component_set_id != *frozen
        || managed_wire.frozen_external_component_set_id != *frozen
        || startup.frozen_external_component_set_id() != frozen
        || cloud.frozen_external_component_set_id() != frozen
        || native_wire.schema_version != NATIVE_RECORD_SCHEMA_VERSION
        || managed_wire.schema_version != MANAGED_RECORD_SCHEMA_VERSION
        || native_wire.native_load_observation_id != observation.native_load_observation_id()
        || native.native_load_observation_id() != &native_wire.native_load_observation_id
        || managed_wire.native_load_observation_id != native_wire.native_load_observation_id
        || observation.process_evidence_digest() != &managed_wire.initial_process_evidence_digest
        || managed_wire.initial_process_evidence_digest
            != managed_wire.final_process_evidence_digest
        || managed_wire.initial_isolation_evidence_digest
            != managed_wire.final_isolation_evidence_digest
        || managed_wire.managed_startup.process_evidence_digest
            != managed_wire.initial_process_evidence_digest
        || managed_wire.managed_startup.isolation_policy_digest
            != managed_wire.isolation_policy_digest
        || startup.startup_launch_spec_digest() != &managed_wire.managed_startup.launch_spec_digest
        || !managed_wire.cloud_disable.managed_environment_observed
        || !managed_wire.cloud_disable.startup_marker_observed
        || managed_wire.cloud_disable.runtime_version != package.reported_version()
        || managed_wire.cloud_disable.version_status != CloudVersionStatus::Reviewed
        || cloud.version_status() != OllamaCloudDisableVersionStatus::Reviewed
        || native.record_id() != native_record.record_id()
        || startup.record_id() != managed_record.record_id()
        || cloud.record_id() != managed_record.record_id()
        || startup.native_record_id() != native_record.record_id()
        || cloud.native_record_id() != native_record.record_id()
        || managed_wire.native_load_record_id != *native_record.record_id().digest()
        || record_digest(NATIVE_RECORD_ID_DOMAIN, native_record.canonical_bytes())
            != *native_record.record_id().digest()
        || record_digest(MANAGED_RECORD_ID_DOMAIN, managed_record.canonical_bytes())
            != *managed_record.record_id().digest()
        || record_digest(NATIVE_CONTROL_DOMAIN, native_record.canonical_bytes())
            != *native.control_digest()
        || record_digest(STARTUP_CONTROL_DOMAIN, managed_record.canonical_bytes())
            != *startup.control_digest()
        || record_digest(CLOUD_CONTROL_DOMAIN, managed_record.canonical_bytes())
            != *cloud.control_digest()
    {
        return Err(RuntimeAdmissionRunnerError::InvalidEvidenceBinding);
    }
    Ok(())
}
