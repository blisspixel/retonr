use std::collections::HashSet;

use serde_json::Value;

use super::*;

fn cleanup_input(
    process: CandidateGenerationProcessCleanupStatusV1,
    runtime: CandidateGenerationPackageRevalidationStatusV1,
    model: CandidateGenerationPackageRevalidationStatusV1,
) -> CandidateGenerationCleanupRecordV1Input {
    CandidateGenerationCleanupRecordV1Input {
        process_cleanup_status: process,
        runtime_package_revalidation_status: runtime,
        model_package_revalidation_status: model,
    }
}

fn successful_cleanup(fixture: &ManagedFixture) -> CandidateGenerationCleanupRecordV1 {
    CandidateGenerationCleanupRecordV1::new(
        &fixture.precursor,
        &fixture.evidence,
        cleanup_input(
            CandidateGenerationProcessCleanupStatusV1::Succeeded,
            CandidateGenerationPackageRevalidationStatusV1::Verified,
            CandidateGenerationPackageRevalidationStatusV1::Verified,
        ),
    )
    .expect("successful cleanup")
}

fn expected_categories(
    process: CandidateGenerationProcessCleanupStatusV1,
    runtime: CandidateGenerationPackageRevalidationStatusV1,
    model: CandidateGenerationPackageRevalidationStatusV1,
) -> Vec<CandidateGenerationCleanupFailureCategoryV1> {
    let mut expected = Vec::new();
    if process == CandidateGenerationProcessCleanupStatusV1::Failed {
        expected.push(CandidateGenerationCleanupFailureCategoryV1::ProcessCleanupFailed);
    }
    match runtime {
        CandidateGenerationPackageRevalidationStatusV1::Verified => {}
        CandidateGenerationPackageRevalidationStatusV1::Changed => {
            expected.push(CandidateGenerationCleanupFailureCategoryV1::RuntimePackageChanged);
        }
        CandidateGenerationPackageRevalidationStatusV1::Failed => {
            expected.push(
                CandidateGenerationCleanupFailureCategoryV1::RuntimePackageRevalidationFailed,
            );
        }
    }
    match model {
        CandidateGenerationPackageRevalidationStatusV1::Verified => {}
        CandidateGenerationPackageRevalidationStatusV1::Changed => {
            expected.push(CandidateGenerationCleanupFailureCategoryV1::ModelPackageChanged);
        }
        CandidateGenerationPackageRevalidationStatusV1::Failed => expected
            .push(CandidateGenerationCleanupFailureCategoryV1::ModelPackageRevalidationFailed),
    }
    expected
}

#[test]
fn cleanup_preserves_exact_failure_closure_for_every_status_combination() {
    let fixture = managed_fixture(0);
    let process_statuses = [
        CandidateGenerationProcessCleanupStatusV1::Succeeded,
        CandidateGenerationProcessCleanupStatusV1::Failed,
    ];
    let package_statuses = [
        CandidateGenerationPackageRevalidationStatusV1::Verified,
        CandidateGenerationPackageRevalidationStatusV1::Changed,
        CandidateGenerationPackageRevalidationStatusV1::Failed,
    ];
    let mut identities = HashSet::new();
    for process in process_statuses {
        for runtime in package_statuses {
            for model in package_statuses {
                let record = CandidateGenerationCleanupRecordV1::new(
                    &fixture.precursor,
                    &fixture.evidence,
                    cleanup_input(process, runtime, model),
                )
                .expect("cleanup record");
                assert_eq!(
                    record.failure_categories(),
                    expected_categories(process, runtime, model)
                );
                assert_eq!(record.failure_categories().is_empty(), {
                    process == CandidateGenerationProcessCleanupStatusV1::Succeeded
                        && runtime == CandidateGenerationPackageRevalidationStatusV1::Verified
                        && model == CandidateGenerationPackageRevalidationStatusV1::Verified
                });
                assert!(identities.insert(record.cleanup_id().clone()));
            }
        }
    }
    assert_eq!(identities.len(), 18);
}

#[test]
fn cleanup_round_trips_exposes_statuses_and_has_stable_identity() {
    let fixture = managed_fixture(0);
    let record = successful_cleanup(&fixture);
    assert_eq!(record.schema_version(), 1);
    assert_eq!(record.precursor_id(), fixture.precursor.precursor_id());
    assert_eq!(
        record.managed_evidence_id(),
        fixture.evidence.managed_evidence_v2_id()
    );
    assert_eq!(
        record.process_cleanup_status(),
        CandidateGenerationProcessCleanupStatusV1::Succeeded
    );
    assert_eq!(
        record.runtime_package_revalidation_status(),
        CandidateGenerationPackageRevalidationStatusV1::Verified
    );
    assert_eq!(
        record.model_package_revalidation_status(),
        CandidateGenerationPackageRevalidationStatusV1::Verified
    );
    assert!(record.failure_categories().is_empty());
    assert_eq!(
        record.cleanup_id().digest().as_str(),
        "8886c5360014fa052f40dbb4897447fdac0dc9a026613f051afc635d068845b0"
    );

    let encoded = serde_json::to_vec(&record).expect("cleanup JSON");
    assert_eq!(
        CandidateGenerationCleanupRecordV1::from_json_bytes(
            &encoded,
            &fixture.precursor,
            &fixture.evidence,
        )
        .expect("cleanup decode"),
        record
    );
    record
        .validate_against(&fixture.precursor, &fixture.evidence)
        .expect("cleanup revalidates");

    let debug = format!("{record:?}");
    assert!(debug.contains(record.cleanup_id().digest().as_str()));
    assert!(debug.contains("Succeeded"));
    for hidden in [
        fixture.evidence.response_id().digest().as_str(),
        fixture
            .evidence
            .structured_request_binding_id()
            .digest()
            .as_str(),
        fixture
            .evidence
            .effective_runtime_state_join_id()
            .digest()
            .as_str(),
    ] {
        assert!(!debug.contains(hidden));
    }
}

#[test]
fn cleanup_rejects_cross_attempt_and_managed_evidence_substitution() {
    let fixture = managed_fixture(0);
    let other = managed_fixture(1);
    assert_eq!(
        CandidateGenerationCleanupRecordV1::new(
            &fixture.precursor,
            &other.evidence,
            cleanup_input(
                CandidateGenerationProcessCleanupStatusV1::Succeeded,
                CandidateGenerationPackageRevalidationStatusV1::Verified,
                CandidateGenerationPackageRevalidationStatusV1::Verified,
            ),
        ),
        Err(GenerationQualificationContractError::CleanupRelationshipMismatch)
    );

    let encoded = serde_json::to_vec(&successful_cleanup(&fixture)).expect("cleanup JSON");
    for changed in [
        replace_once(
            &encoded,
            fixture.precursor.precursor_id().digest().as_str(),
            other.precursor.precursor_id().digest().as_str(),
        ),
        replace_once(
            &encoded,
            fixture.evidence.managed_evidence_v2_id().digest().as_str(),
            other.evidence.managed_evidence_v2_id().digest().as_str(),
        ),
    ] {
        assert_eq!(
            CandidateGenerationCleanupRecordV1::from_json_bytes(
                &changed,
                &fixture.precursor,
                &fixture.evidence,
            ),
            Err(GenerationQualificationContractError::CleanupRelationshipMismatch)
        );
    }

    assert_ne!(
        successful_cleanup(&fixture).cleanup_id(),
        successful_cleanup(&other).cleanup_id()
    );
}

#[test]
fn cleanup_decoder_rejects_bounds_encoding_and_future_schema_first() {
    let fixture = managed_fixture(0);
    let record = successful_cleanup(&fixture);
    let encoded = serde_json::to_vec(&record).expect("cleanup JSON");
    assert_eq!(
        CandidateGenerationCleanupRecordV1::from_json_bytes(
            &vec![b' '; MAX_CANDIDATE_GENERATION_CLEANUP_JSON_BYTES + 1],
            &fixture.precursor,
            &fixture.evidence,
        ),
        Err(GenerationQualificationContractError::EncodedRecordTooLarge)
    );
    for invalid in [b"{".as_slice(), b"{} trailing".as_slice()] {
        assert_eq!(
            CandidateGenerationCleanupRecordV1::from_json_bytes(
                invalid,
                &fixture.precursor,
                &fixture.evidence,
            ),
            Err(GenerationQualificationContractError::InvalidEncoding)
        );
    }
    let duplicate = replace_once(
        &encoded,
        "\"schema_version\":1,",
        "\"schema_version\":1,\"schema_version\":1,",
    );
    assert_eq!(
        CandidateGenerationCleanupRecordV1::from_json_bytes(
            &duplicate,
            &fixture.precursor,
            &fixture.evidence,
        ),
        Err(GenerationQualificationContractError::InvalidEncoding)
    );
    let mut unknown = serde_json::to_value(&record).expect("cleanup value");
    unknown["authority"] = Value::Bool(true);
    assert_eq!(
        CandidateGenerationCleanupRecordV1::from_json_bytes(
            &serde_json::to_vec(&unknown).expect("unknown JSON"),
            &fixture.precursor,
            &fixture.evidence,
        ),
        Err(GenerationQualificationContractError::InvalidEncoding)
    );
    let mut whitespace = vec![b' '];
    whitespace.extend(&encoded);
    assert_eq!(
        CandidateGenerationCleanupRecordV1::from_json_bytes(
            &whitespace,
            &fixture.precursor,
            &fixture.evidence,
        ),
        Err(GenerationQualificationContractError::NonCanonicalEncoding)
    );
    let future = replace_once(&encoded, "\"schema_version\":1", "\"schema_version\":2");
    let future = replace_once(
        &future,
        "\"failure_categories\":[]",
        "\"failure_categories\":[\"process_cleanup_failed\"]",
    );
    assert_eq!(
        CandidateGenerationCleanupRecordV1::from_json_bytes(
            &future,
            &fixture.precursor,
            &fixture.evidence,
        ),
        Err(GenerationQualificationContractError::UnsupportedSchema(2))
    );
}

#[test]
fn cleanup_decoder_requires_exact_unique_canonical_failure_categories() {
    let fixture = managed_fixture(0);
    let failed = CandidateGenerationCleanupRecordV1::new(
        &fixture.precursor,
        &fixture.evidence,
        cleanup_input(
            CandidateGenerationProcessCleanupStatusV1::Failed,
            CandidateGenerationPackageRevalidationStatusV1::Changed,
            CandidateGenerationPackageRevalidationStatusV1::Failed,
        ),
    )
    .expect("failed cleanup");
    let encoded = serde_json::to_vec(&failed).expect("failed cleanup JSON");
    let exact = "[\"process_cleanup_failed\",\"runtime_package_changed\",\"model_package_revalidation_failed\"]";
    for replacement in [
        "[]",
        "[\"process_cleanup_failed\",\"runtime_package_changed\"]",
        "[\"runtime_package_changed\",\"process_cleanup_failed\",\"model_package_revalidation_failed\"]",
        "[\"process_cleanup_failed\",\"process_cleanup_failed\",\"model_package_revalidation_failed\"]",
        "[\"process_cleanup_failed\",\"runtime_package_changed\",\"model_package_revalidation_failed\",\"model_package_changed\"]",
    ] {
        assert_eq!(
            CandidateGenerationCleanupRecordV1::from_json_bytes(
                &replace_once(&encoded, exact, replacement),
                &fixture.precursor,
                &fixture.evidence,
            ),
            Err(GenerationQualificationContractError::InvalidCleanupStatusClosure)
        );
    }
    let success = serde_json::to_vec(&successful_cleanup(&fixture)).expect("cleanup JSON");
    assert_eq!(
        CandidateGenerationCleanupRecordV1::from_json_bytes(
            &replace_once(
                &success,
                "\"failure_categories\":[]",
                "\"failure_categories\":[\"process_cleanup_failed\"]",
            ),
            &fixture.precursor,
            &fixture.evidence,
        ),
        Err(GenerationQualificationContractError::InvalidCleanupStatusClosure)
    );
}

#[test]
fn cleanup_schema_is_machine_readable() {
    let schema = schemars::schema_for!(CandidateGenerationCleanupRecordV1);
    let value = serde_json::to_value(schema).expect("schema JSON");
    assert_eq!(
        value["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
}
