use super::super::*;
use crate::runtime_admission_runner::records::*;
use serde_json::json;

pub(crate) fn review_operation_fixture(
    foundation: &VerifiedRuntimeAdmissionFoundationBinding,
    package: &RuntimePackageManifest,
) -> crate::RuntimeAdmissionFinalOperation {
    let process = Digest::sha256(b"review fixture process");
    let observation =
        crate::runtime_admission_runner::tests::native_observation(package, process.clone());
    let frozen: FrozenExternalNativeComponentSetId =
        serde_json::from_value(json!(Digest::sha256(b"review fixture frozen")))
            .expect("frozen identity");
    let native_wire: NativeLoadRecordWire = serde_json::from_value(json!({
        "authority":"none", "control":"native_closure", "foundation_id":foundation.foundation_id().digest(),
        "frozen_external_component_set_id":frozen, "native_load":observation,
        "native_load_observation_id":observation.native_load_observation_id(),
        "runtime_package_manifest_id":package.runtime_package_manifest_id(), "schema_version":1, "status":"passed"
    })).expect("native fixture wire");
    let native_bytes = encode(&native_wire, MAX_RUNTIME_ADMISSION_NATIVE_LOAD_RECORD_BYTES)
        .expect("native fixture bytes");
    let native_record = RuntimeAdmissionNativeLoadRecord {
        record_id: RuntimeAdmissionNativeLoadRecordId(record_digest(
            NATIVE_RECORD_ID_DOMAIN,
            &native_bytes,
        )),
        canonical_bytes: native_bytes,
    };
    let managed_wire: ManagedFinalRecordWire = serde_json::from_value(json!({
        "authority":"none", "cleanup":"complete", "cloud_disable":{
            "managed_environment_observed":true, "runtime_version":package.reported_version(),
            "startup_marker_observed":true, "version_status":"reviewed"
        }, "control":"managed_final", "final_connection_evidence_digest":process,
        "final_isolation_evidence_digest":process, "final_process_evidence_digest":process,
        "foundation_id":foundation.foundation_id().digest(), "frozen_external_component_set_id":frozen,
        "initial_connection_evidence_digest":process, "initial_isolation_evidence_digest":process,
        "initial_process_evidence_digest":process, "isolation_policy_digest":process,
        "managed_startup":{
            "isolation_policy_digest":process, "launch_spec_digest":process, "process_evidence_digest":process,
            "standard_error_bytes":0, "standard_error_digest":Digest::sha256(b""),
            "standard_output_bytes":0, "standard_output_digest":Digest::sha256(b"")
        }, "native_load_observation_id":observation.native_load_observation_id(),
        "native_load_record_id":native_record.record_id().digest(), "runtime_package_manifest_id":package.runtime_package_manifest_id(),
        "schema_version":1, "status":"passed"
    })).expect("managed fixture wire");
    let managed_bytes = encode(
        &managed_wire,
        MAX_RUNTIME_ADMISSION_MANAGED_FINAL_RECORD_BYTES,
    )
    .expect("managed fixture bytes");
    let managed_record = RuntimeAdmissionManagedFinalRecord {
        record_id: RuntimeAdmissionManagedFinalRecordId(record_digest(
            MANAGED_RECORD_ID_DOMAIN,
            &managed_bytes,
        )),
        canonical_bytes: managed_bytes,
    };
    let native_control = VerifiedPassedRuntimeAdmissionNativeClosureControl {
        foundation_id: foundation.foundation_id().clone(),
        record_id: native_record.record_id().clone(),
        control_digest: record_digest(NATIVE_CONTROL_DOMAIN, native_record.canonical_bytes()),
        native_load_observation_id: observation.native_load_observation_id(),
        frozen_external_component_set_id: frozen.clone(),
    };
    let (startup, cloud) = derive_passed_managed_controls(ManagedControlBindings {
        cloud_control_digest: record_digest(CLOUD_CONTROL_DOMAIN, managed_record.canonical_bytes()),
        cloud_status: OllamaCloudDisableVersionStatus::Reviewed,
        foundation_id: foundation.foundation_id().clone(),
        frozen_external_component_set_id: frozen,
        native_record_id: native_record.record_id().clone(),
        record_id: managed_record.record_id().clone(),
        startup_launch_spec_digest: process,
        startup_control_digest: record_digest(
            STARTUP_CONTROL_DOMAIN,
            managed_record.canonical_bytes(),
        ),
    })
    .expect("private reviewed fixture controls");
    crate::RuntimeAdmissionFinalOperation::test_from_review_records(
        native_record,
        managed_record,
        native_control,
        startup,
        cloud,
    )
}
