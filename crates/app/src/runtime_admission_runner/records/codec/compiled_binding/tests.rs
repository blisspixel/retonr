use super::*;
use crate::runtime_admission_runner::records::*;
use crate::{RuntimeAdmissionEvidenceFoundation, RuntimeAdmissionEvidenceFoundationInput};
use rewrite_types::Digest;
use serde_json::{Value, json};

fn foundation(package: &RuntimePackageManifest) -> VerifiedRuntimeAdmissionFoundationBinding {
    let foundation =
        RuntimeAdmissionEvidenceFoundation::compile(RuntimeAdmissionEvidenceFoundationInput::new(
            package.artifact_set_id().clone(),
            package.artifact_set_id().clone(),
            Digest::sha256(b"manifest"),
            Digest::sha256(b"plan"),
            Digest::sha256(b"report"),
            package.runtime_package_manifest_id(),
        ))
        .expect("foundation");
    VerifiedRuntimeAdmissionFoundationBinding::test_fixture(&foundation)
}

fn changed_records(
    operation: &RuntimeAdmissionFinalOperation,
    native_bytes: Vec<u8>,
    managed_bytes: Vec<u8>,
) -> RuntimeAdmissionFinalOperation {
    let mut native = operation.native_load_record().clone();
    native.canonical_bytes = native_bytes;
    let mut managed = operation.managed_final_record().clone();
    managed.canonical_bytes = managed_bytes;
    RuntimeAdmissionFinalOperation::test_from_review_records(
        native,
        managed,
        operation.native_closure().clone(),
        operation.managed_startup().clone(),
        operation.cloud_disable().clone(),
    )
}

#[test]
fn exact_cleanup_gated_records_reparse_against_their_passed_controls() {
    let package = crate::runtime_admission_runner::tests::runtime_package("0.16.2");
    let binding = foundation(&package);
    let operation = review_operation_fixture(&binding, &package);
    verify_compiled_for_review(&operation, &binding, &package).expect("exact records");
    let other_package = crate::runtime_admission_runner::tests::runtime_package("0.16.3");
    assert!(verify_compiled_for_review(&operation, &binding, &other_package).is_err());
    assert!(verify_compiled_for_review(&operation, &foundation(&other_package), &package).is_err());
}

#[test]
fn all_execution_subject_substitutions_and_unreviewed_status_fail_rejoin() {
    let package = crate::runtime_admission_runner::tests::runtime_package("0.16.2");
    let binding = foundation(&package);
    let operation = review_operation_fixture(&binding, &package);
    let other = Digest::sha256(b"foreign subject");
    for native in [true, false] {
        let original = if native {
            operation.native_load_record().canonical_bytes()
        } else {
            operation.managed_final_record().canonical_bytes()
        };
        let value: Value = serde_json::from_slice(original).expect("wire");
        let mut mutations = vec![
            (vec!["foundation_id"], json!(other)),
            (vec!["runtime_package_manifest_id"], json!(other)),
            (vec!["frozen_external_component_set_id"], json!(other)),
            (vec!["schema_version"], json!(2)),
            (vec!["native_load_observation_id"], json!(other)),
        ];
        if !native {
            mutations.extend([
                (vec!["native_load_record_id"], json!(other)),
                (vec!["final_process_evidence_digest"], json!(other)),
                (vec!["final_isolation_evidence_digest"], json!(other)),
                (
                    vec!["managed_startup", "process_evidence_digest"],
                    json!(other),
                ),
                (vec!["managed_startup", "launch_spec_digest"], json!(other)),
                (
                    vec!["managed_startup", "isolation_policy_digest"],
                    json!(other),
                ),
                (vec!["cloud_disable", "runtime_version"], json!("0.16.3")),
                (vec!["cloud_disable", "version_status"], json!("unreviewed")),
                (
                    vec!["cloud_disable", "managed_environment_observed"],
                    json!(false),
                ),
                (
                    vec!["cloud_disable", "startup_marker_observed"],
                    json!(false),
                ),
            ]);
        }
        for (path, replacement) in mutations {
            let mut substituted = value.clone();
            let mut target = &mut substituted;
            for field in &path {
                target = &mut target[*field];
            }
            *target = replacement;
            let bytes = serde_json::to_vec(&substituted).expect("changed canonical wire");
            let substituted_operation = if native {
                changed_records(
                    &operation,
                    bytes,
                    operation.managed_final_record().canonical_bytes().to_vec(),
                )
            } else {
                changed_records(
                    &operation,
                    operation.native_load_record().canonical_bytes().to_vec(),
                    bytes,
                )
            };
            assert!(
                verify_compiled_for_review(&substituted_operation, &binding, &package).is_err(),
                "substitution {path:?}"
            );
        }
    }
}

#[test]
fn wrong_record_purpose_noncanonical_bytes_and_different_control_digests_fail() {
    let package = crate::runtime_admission_runner::tests::runtime_package("0.16.2");
    let binding = foundation(&package);
    let operation = review_operation_fixture(&binding, &package);
    let exchanged = changed_records(
        &operation,
        operation.managed_final_record().canonical_bytes().to_vec(),
        operation.native_load_record().canonical_bytes().to_vec(),
    );
    assert!(verify_compiled_for_review(&exchanged, &binding, &package).is_err());
    let whitespace = changed_records(
        &operation,
        [
            b" ".as_slice(),
            operation.native_load_record().canonical_bytes(),
        ]
        .concat(),
        operation.managed_final_record().canonical_bytes().to_vec(),
    );
    assert!(verify_compiled_for_review(&whitespace, &binding, &package).is_err());
    let mut foreign = operation.cloud_disable().clone();
    foreign.control_digest = Digest::sha256(b"different cloud control");
    let foreign_control = RuntimeAdmissionFinalOperation::test_from_review_records(
        operation.native_load_record().clone(),
        operation.managed_final_record().clone(),
        operation.native_closure().clone(),
        operation.managed_startup().clone(),
        foreign,
    );
    assert!(verify_compiled_for_review(&foreign_control, &binding, &package).is_err());
}
