use rewrite_runtime_attestor::{
    CompiledFrozenExternalNativeComponentSet, ExternalNativeComponentReview,
    ExternalNativeComponentReviewDisposition, NativeLoadDiscovery,
};

use super::*;
use crate::{RuntimeAdmissionEvidenceFoundation, RuntimeAdmissionEvidenceFoundationInput};

#[test]
fn canonical_parser_rejects_unknown_and_noncanonical_fields() {
    let wire = CloudDisableWire {
        managed_environment_observed: true,
        runtime_version: "0.16.2".to_owned(),
        startup_marker_observed: true,
        version_status: CloudVersionStatus::Unreviewed,
    };
    let canonical = encode(&wire, 1_024).expect("canonical wire");
    assert_eq!(
        parse_canonical::<CloudDisableWire>(&canonical, 1_024).expect("reparse"),
        wire
    );
    let spaced = [b" ".as_slice(), canonical.as_slice()].concat();
    assert!(matches!(
        parse_canonical::<CloudDisableWire>(&spaced, 1_024),
        Err(RuntimeAdmissionRunnerError::InvalidRecordEncoding)
    ));
    let unknown = br#"{"managed_environment_observed":true,"runtime_version":"0.16.2","startup_marker_observed":true,"unknown":true,"version_status":"unreviewed"}"#;
    assert!(matches!(
        parse_canonical::<CloudDisableWire>(unknown, 1_024),
        Err(RuntimeAdmissionRunnerError::InvalidRecordEncoding)
    ));
}

#[test]
fn canonical_parser_enforces_fixed_limits_before_decoding() {
    assert!(matches!(
        parse_canonical::<CloudDisableWire>(&[], 1_024),
        Err(RuntimeAdmissionRunnerError::RecordLimitExceeded)
    ));
    assert!(matches!(
        parse_canonical::<CloudDisableWire>(&vec![b' '; 1_025], 1_024),
        Err(RuntimeAdmissionRunnerError::RecordLimitExceeded)
    ));
}

#[test]
fn native_record_round_trip_revalidates_the_typed_observation() {
    let package = super::super::super::tests::runtime_package("0.16.2");
    let observation = super::super::super::tests::native_observation(
        &package,
        Digest::sha256(b"managed process"),
    );
    let frozen = frozen_without_external_components(&package);
    let foundation =
        RuntimeAdmissionEvidenceFoundation::compile(RuntimeAdmissionEvidenceFoundationInput::new(
            package.artifact_set_id().clone(),
            package.artifact_set_id().clone(),
            Digest::sha256(b"source manifest"),
            Digest::sha256(b"build plan"),
            Digest::sha256(b"source report"),
            package.runtime_package_manifest_id(),
        ))
        .expect("foundation");

    validate_native_observation(&package, &frozen, &observation).expect("frozen observation");
    let wire = derive_native_wire(foundation.foundation_id(), &package, &frozen, &observation)
        .expect("native wire");
    let bytes = encode(
        &wire,
        super::super::MAX_RUNTIME_ADMISSION_NATIVE_LOAD_RECORD_BYTES,
    )
    .expect("canonical record");
    let reparsed: NativeLoadRecordWire = parse_canonical(
        &bytes,
        super::super::MAX_RUNTIME_ADMISSION_NATIVE_LOAD_RECORD_BYTES,
    )
    .expect("canonical reparse");
    let reparsed_observation =
        parse_native_observation(&reparsed.native_load, &package).expect("typed reparse");

    assert_eq!(wire, reparsed);
    assert_eq!(observation, reparsed_observation);
    assert_eq!(
        reparsed.native_load_observation_id,
        observation.native_load_observation_id()
    );
}

#[test]
fn managed_final_facts_compile_canonically_and_fail_closed() {
    let package = super::super::super::tests::runtime_package("0.16.2");
    let observation = super::super::super::tests::native_observation(
        &package,
        Digest::sha256(b"managed process"),
    );
    let frozen = frozen_without_external_components(&package);
    let foundation =
        RuntimeAdmissionEvidenceFoundation::compile(RuntimeAdmissionEvidenceFoundationInput::new(
            package.artifact_set_id().clone(),
            package.artifact_set_id().clone(),
            Digest::sha256(b"source manifest"),
            Digest::sha256(b"build plan"),
            Digest::sha256(b"source report"),
            package.runtime_package_manifest_id(),
        ))
        .expect("foundation");
    let native_record_id = RuntimeAdmissionNativeLoadRecordId(Digest::sha256(b"native record"));
    let process = Digest::sha256(b"managed process");
    let policy = Digest::sha256(b"isolation policy");
    let facts = ManagedFinalFacts {
        runtime_package_manifest_id: package.runtime_package_manifest_id(),
        frozen_external_component_set_id: frozen.frozen_set_id().clone(),
        native_runtime_package_manifest_id: package.runtime_package_manifest_id(),
        native_process_evidence_digest: process.clone(),
        native_load_observation_id: observation.native_load_observation_id(),
        initial_process_evidence_digest: process.clone(),
        final_process_evidence_digest: process.clone(),
        initial_connection_evidence_digest: Digest::sha256(b"initial connection"),
        final_connection_evidence_digest: Digest::sha256(b"final connection"),
        initial_isolation_evidence_digest: Digest::sha256(b"isolation evidence"),
        final_isolation_evidence_digest: Digest::sha256(b"isolation evidence"),
        isolation_unchanged: true,
        isolation_policy_digest: policy.clone(),
        startup_runtime_package_manifest_id: package.runtime_package_manifest_id(),
        startup_process_evidence_digest: process,
        startup_launch_spec_digest: Digest::sha256(b"launch"),
        startup_isolation_policy_digest: policy,
        standard_output_digest: Digest::sha256(b"stdout"),
        standard_error_digest: Digest::sha256(b"stderr"),
        standard_output_bytes: 6,
        standard_error_bytes: 6,
        cloud_runtime_package_manifest_id: package.runtime_package_manifest_id(),
        cloud_runtime_version: "0.16.2".to_owned(),
        probed_runtime_version: "0.16.2".to_owned(),
        cloud_version_status: OllamaCloudDisableVersionStatus::Unreviewed,
        managed_environment_observed: true,
        startup_marker_observed: true,
    };

    let wire = managed_wire_from_facts(
        foundation.foundation_id(),
        &package,
        &frozen,
        &native_record_id,
        facts.clone(),
    )
    .expect("managed wire");
    let bytes = encode(
        &wire,
        super::super::MAX_RUNTIME_ADMISSION_MANAGED_FINAL_RECORD_BYTES,
    )
    .expect("managed record");
    let reparsed: ManagedFinalRecordWire = parse_canonical(
        &bytes,
        super::super::MAX_RUNTIME_ADMISSION_MANAGED_FINAL_RECORD_BYTES,
    )
    .expect("managed reparse");
    assert_eq!(wire, reparsed);
    assert_eq!(
        reparsed.native_load_observation_id,
        observation.native_load_observation_id()
    );

    let mut changed = facts.clone();
    changed.isolation_unchanged = false;
    assert!(matches!(
        managed_wire_from_facts(
            foundation.foundation_id(),
            &package,
            &frozen,
            &native_record_id,
            changed,
        ),
        Err(RuntimeAdmissionRunnerError::InvalidEvidenceBinding)
    ));
    let mut unavailable = facts;
    unavailable.cloud_version_status = OllamaCloudDisableVersionStatus::FeatureUnavailable;
    assert!(matches!(
        managed_wire_from_facts(
            foundation.foundation_id(),
            &package,
            &frozen,
            &native_record_id,
            unavailable,
        ),
        Err(RuntimeAdmissionRunnerError::CloudDisableFeatureUnavailable)
    ));
}

fn frozen_without_external_components(
    package: &RuntimePackageManifest,
) -> VerifiedFrozenExternalNativeComponentSet {
    let discovery_bytes = serde_json::to_vec(&serde_json::json!({
        "authority": "none",
        "evidence_class": "linux_proc_map_files",
        "external_components": [],
        "observation_contract_id": "runtime-admission-test",
        "observation_contract_schema_version": 1,
        "process_evidence_digest": Digest::sha256(b"managed process"),
        "runtime_package_manifest_id": package.runtime_package_manifest_id(),
        "schema_version": 1,
        "status": "proposed"
    }))
    .expect("discovery bytes");
    let discovery = NativeLoadDiscovery::from_json_bytes(
        &discovery_bytes,
        &package.runtime_package_manifest_id(),
    )
    .expect("discovery");
    let reviewer_evidence = b"reviewer-owned native closure decision";
    let review = ExternalNativeComponentReview::compile(
        &discovery,
        ExternalNativeComponentReviewDisposition::Approved,
        reviewer_evidence,
    )
    .expect("review");
    let compiled =
        CompiledFrozenExternalNativeComponentSet::compile(&discovery, &review).expect("frozen set");
    VerifiedFrozenExternalNativeComponentSet::verify(
        compiled.canonical_json_bytes(),
        discovery.canonical_json_bytes(),
        review.canonical_json_bytes(),
        reviewer_evidence,
        &package.runtime_package_manifest_id(),
    )
    .expect("verified frozen set")
}
