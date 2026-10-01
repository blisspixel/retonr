use serde_json::{Value, json};

use super::*;

fn managed_wire(cloud_status: &str) -> ManagedFinalRecordWire {
    let digest = Digest::sha256(b"observed fixture subject");
    serde_json::from_value(json!({
        "authority": "none",
        "cleanup": "complete",
        "cloud_disable": {
            "managed_environment_observed": true,
            "runtime_version": "0.16.2",
            "startup_marker_observed": true,
            "version_status": cloud_status
        },
        "control": "managed_final",
        "final_connection_evidence_digest": digest,
        "final_isolation_evidence_digest": digest,
        "final_process_evidence_digest": digest,
        "foundation_id": digest,
        "frozen_external_component_set_id": digest,
        "initial_connection_evidence_digest": digest,
        "initial_isolation_evidence_digest": digest,
        "initial_process_evidence_digest": digest,
        "isolation_policy_digest": digest,
        "managed_startup": {
            "isolation_policy_digest": digest,
            "launch_spec_digest": digest,
            "process_evidence_digest": digest,
            "standard_error_bytes": 6,
            "standard_error_digest": digest,
            "standard_output_bytes": 6,
            "standard_output_digest": digest
        },
        "native_load_observation_id": digest,
        "native_load_record_id": digest,
        "runtime_package_manifest_id": digest,
        "schema_version": 1,
        "status": "passed"
    }))
    .expect("bounded typed managed record fixture")
}

#[test]
fn unreviewed_observation_preserves_status_without_granting_a_passed_control() {
    for status in ["unreviewed", "reviewed"] {
        let expected = managed_wire(status);
        let bytes = encode(&expected, MAX_RUNTIME_ADMISSION_MANAGED_FINAL_RECORD_BYTES)
            .expect("canonical observation");
        let record = verify_observed_managed_wire(&bytes, &expected)
            .expect("independent observation reparse");
        assert_eq!(record.canonical_bytes(), bytes);
        assert_eq!(
            record.record_id().digest(),
            &record_digest(MANAGED_RECORD_ID_DOMAIN, &bytes)
        );
        let decoded: Value =
            serde_json::from_slice(record.canonical_bytes()).expect("content-free observation");
        assert_eq!(decoded["cloud_disable"]["version_status"], status);
    }
    assert!(matches!(
        require_reviewed_cloud_disable(OllamaCloudDisableVersionStatus::Unreviewed),
        Err(RuntimeAdmissionRunnerError::CloudDisableRuntimeUnreviewed)
    ));
}

#[test]
fn every_observed_subject_and_nested_fact_mutation_rejects_the_record() {
    let expected = managed_wire("unreviewed");
    let original = serde_json::to_value(&expected).expect("typed observed wire");
    for path in [
        "/foundation_id",
        "/runtime_package_manifest_id",
        "/frozen_external_component_set_id",
        "/native_load_record_id",
        "/native_load_observation_id",
        "/initial_process_evidence_digest",
        "/final_process_evidence_digest",
        "/initial_connection_evidence_digest",
        "/final_connection_evidence_digest",
        "/initial_isolation_evidence_digest",
        "/final_isolation_evidence_digest",
        "/isolation_policy_digest",
        "/managed_startup/launch_spec_digest",
        "/managed_startup/process_evidence_digest",
        "/managed_startup/isolation_policy_digest",
        "/managed_startup/standard_output_digest",
        "/managed_startup/standard_error_digest",
    ] {
        let mut changed = original.clone();
        *changed.pointer_mut(path).expect("fixture identity path") =
            json!(Digest::sha256(b"other exact observation"));
        let bytes = serde_json::to_vec(&changed).expect("canonical changed observation");
        assert!(
            matches!(
                verify_observed_managed_wire(&bytes, &expected),
                Err(RuntimeAdmissionRunnerError::InvalidEvidenceBinding)
            ),
            "subject {path}"
        );
    }
    for (path, replacement) in [
        ("/managed_startup/standard_output_bytes", json!(7)),
        ("/managed_startup/standard_error_bytes", json!(7)),
        ("/cloud_disable/runtime_version", json!("0.16.3")),
        ("/cloud_disable/version_status", json!("reviewed")),
        ("/cloud_disable/managed_environment_observed", json!(false)),
        ("/cloud_disable/startup_marker_observed", json!(false)),
        ("/schema_version", json!(2)),
    ] {
        let mut changed = original.clone();
        *changed.pointer_mut(path).expect("fixture fact path") = replacement;
        let bytes = serde_json::to_vec(&changed).expect("canonical changed observation");
        assert!(
            verify_observed_managed_wire(&bytes, &expected).is_err(),
            "fact {path}"
        );
    }
}

#[test]
fn observational_parser_rejects_noncanonical_unknown_duplicate_and_authoritative_bytes() {
    let expected = managed_wire("unreviewed");
    let canonical = encode(&expected, MAX_RUNTIME_ADMISSION_MANAGED_FINAL_RECORD_BYTES)
        .expect("canonical observation");
    let mut newline = canonical.clone();
    newline.push(b'\n');
    assert!(matches!(
        verify_observed_managed_wire(&newline, &expected),
        Err(RuntimeAdmissionRunnerError::InvalidRecordEncoding)
    ));
    let mut unknown = serde_json::to_value(&expected).expect("fixture wire");
    unknown["extra"] = json!("unexpected evidence");
    assert!(
        verify_observed_managed_wire(
            &serde_json::to_vec(&unknown).expect("unknown bytes"),
            &expected
        )
        .is_err()
    );
    let duplicate = format!(
        "{{\"schema_version\":1,{}",
        std::str::from_utf8(&canonical[1..]).expect("fixture UTF-8")
    );
    assert!(verify_observed_managed_wire(duplicate.as_bytes(), &expected).is_err());
    for (path, value) in [
        ("/authority", "admitted"),
        ("/cleanup", "incomplete"),
        ("/status", "failed"),
        ("/cloud_disable/version_status", "feature_unavailable"),
    ] {
        let mut changed = serde_json::to_value(&expected).expect("fixture wire");
        *changed.pointer_mut(path).expect("fixture status path") = json!(value);
        assert!(
            verify_observed_managed_wire(
                &serde_json::to_vec(&changed).expect("invalid status bytes"),
                &expected
            )
            .is_err()
        );
    }
    assert!(matches!(
        verify_observed_managed_wire(
            &vec![b' '; MAX_RUNTIME_ADMISSION_MANAGED_FINAL_RECORD_BYTES + 1],
            &expected
        ),
        Err(RuntimeAdmissionRunnerError::RecordLimitExceeded)
    ));
}
