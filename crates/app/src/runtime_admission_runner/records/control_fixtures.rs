use super::*;

pub(crate) fn test_execution_control_fixtures(
    foundation_id: RuntimeAdmissionEvidenceFoundationId,
    cloud_status: OllamaCloudDisableVersionStatus,
    native_record_matches: bool,
    frozen_set_matches: bool,
) -> (
    VerifiedPassedRuntimeAdmissionNativeClosureControl,
    VerifiedPassedRuntimeAdmissionManagedStartupControl,
    VerifiedPassedRuntimeAdmissionCloudDisableControl,
) {
    test_execution_control_fixtures_with_launch(
        foundation_id,
        cloud_status,
        native_record_matches,
        frozen_set_matches,
        Digest::sha256(b"launch"),
    )
}

pub(crate) fn test_execution_control_fixtures_with_launch(
    foundation_id: RuntimeAdmissionEvidenceFoundationId,
    cloud_status: OllamaCloudDisableVersionStatus,
    native_record_matches: bool,
    frozen_set_matches: bool,
    startup_launch_spec_digest: Digest,
) -> (
    VerifiedPassedRuntimeAdmissionNativeClosureControl,
    VerifiedPassedRuntimeAdmissionManagedStartupControl,
    VerifiedPassedRuntimeAdmissionCloudDisableControl,
) {
    let native_record_id = RuntimeAdmissionNativeLoadRecordId(Digest::sha256(b"native record"));
    let managed_record_id = RuntimeAdmissionManagedFinalRecordId(Digest::sha256(b"managed record"));
    let native_load_observation_id =
        serde_json::from_value(serde_json::json!(Digest::sha256(b"native observation")))
            .expect("test native observation identity");
    let frozen_external_component_set_id: FrozenExternalNativeComponentSetId =
        serde_json::from_value(serde_json::json!(Digest::sha256(
            b"frozen external components"
        )))
        .expect("test frozen set identity");
    let managed_native_record_id = if native_record_matches {
        native_record_id.clone()
    } else {
        RuntimeAdmissionNativeLoadRecordId(Digest::sha256(b"other native record"))
    };
    let managed_frozen_external_component_set_id = if frozen_set_matches {
        frozen_external_component_set_id.clone()
    } else {
        serde_json::from_value(serde_json::json!(Digest::sha256(b"other frozen set")))
            .expect("other test frozen set identity")
    };
    let native = VerifiedPassedRuntimeAdmissionNativeClosureControl {
        foundation_id: foundation_id.clone(),
        record_id: native_record_id.clone(),
        control_digest: Digest::sha256(b"native control"),
        native_load_observation_id,
        frozen_external_component_set_id: frozen_external_component_set_id.clone(),
    };
    let bindings = ManagedControlBindings {
        cloud_control_digest: Digest::sha256(b"cloud control"),
        cloud_status,
        foundation_id,
        frozen_external_component_set_id: managed_frozen_external_component_set_id,
        native_record_id: managed_native_record_id,
        record_id: managed_record_id,
        startup_launch_spec_digest,
        startup_control_digest: Digest::sha256(b"startup control"),
    };
    let (startup, cloud) = if cloud_status == OllamaCloudDisableVersionStatus::Reviewed {
        derive_passed_managed_controls(bindings).expect("reviewed test controls")
    } else {
        derive_managed_controls(bindings)
    };
    (native, startup, cloud)
}
