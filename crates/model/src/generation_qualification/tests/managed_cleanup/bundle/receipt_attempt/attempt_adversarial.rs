use std::collections::HashSet;

use super::*;

#[test]
fn attempt_decoder_rejects_untrusted_future_and_substituted_records() {
    let fixture = receipt_fixture();
    let planned = &fixture.bundle.managed.base.attempts[0];
    let precursor = &fixture.bundle.managed.precursor;
    let completed =
        CandidateGenerationAttemptRecordV1::completed(planned, precursor, &fixture.receipt)
            .expect("completed");
    let encoded = serde_json::to_vec(&completed).expect("attempt JSON");
    assert_eq!(
        CandidateGenerationAttemptRecordV1::from_json_bytes(
            &vec![b' '; MAX_CANDIDATE_GENERATION_ATTEMPT_RECORD_JSON_BYTES + 1],
            planned,
            Some(precursor),
            Some(&fixture.receipt),
        ),
        Err(GenerationQualificationContractError::EncodedRecordTooLarge),
    );
    assert_eq!(
        CandidateGenerationAttemptRecordV1::from_json_bytes(
            b"{",
            planned,
            Some(precursor),
            Some(&fixture.receipt)
        ),
        Err(GenerationQualificationContractError::InvalidEncoding),
    );
    let future = replace_once(&encoded, "\"schema_version\":1", "\"schema_version\":2");
    assert_eq!(
        CandidateGenerationAttemptRecordV1::from_json_bytes(&future, planned, None, None),
        Err(GenerationQualificationContractError::UnsupportedSchema(2)),
    );
    let mut spaced = encoded.clone();
    spaced.push(b' ');
    assert_eq!(
        CandidateGenerationAttemptRecordV1::from_json_bytes(
            &spaced,
            planned,
            Some(precursor),
            Some(&fixture.receipt)
        ),
        Err(GenerationQualificationContractError::NonCanonicalEncoding),
    );
    let duplicate = replace_once(
        &encoded,
        "\"schema_version\":1",
        "\"schema_version\":1,\"schema_version\":1",
    );
    assert_eq!(
        CandidateGenerationAttemptRecordV1::from_json_bytes(
            &duplicate,
            planned,
            Some(precursor),
            Some(&fixture.receipt)
        ),
        Err(GenerationQualificationContractError::InvalidEncoding),
    );
    let changed = replace_once(
        &encoded,
        fixture.receipt.receipt_id().digest().as_str(),
        digest("substituted receipt").as_str(),
    );
    assert_eq!(
        CandidateGenerationAttemptRecordV1::from_json_bytes(
            &changed,
            planned,
            Some(precursor),
            Some(&fixture.receipt)
        ),
        Err(GenerationQualificationContractError::AttemptRecordRelationshipMismatch),
    );
    assert_eq!(
        CandidateGenerationAttemptRecordV1::from_json_bytes(
            &encoded,
            planned,
            Some(precursor),
            None
        ),
        Err(GenerationQualificationContractError::AttemptRecordRelationshipMismatch),
    );
    assert_eq!(
        CandidateGenerationAttemptRecordV1::from_json_bytes(
            &encoded,
            planned,
            None,
            Some(&fixture.receipt),
        ),
        Err(GenerationQualificationContractError::AttemptRecordRelationshipMismatch),
    );
}

#[test]
fn failed_attempt_decoder_rejects_a_receipt() {
    let fixture = receipt_fixture();
    let planned = &fixture.bundle.managed.base.attempts[0];
    let precursor = &fixture.bundle.managed.precursor;
    let failed = CandidateGenerationAttemptRecordV1::failed(
        planned,
        Some(precursor),
        failure_input(
            CandidateGenerationAttemptFailurePhaseV1::ResponseValidation,
            CandidateGenerationAttemptFailureCategoryV1::ResponseInvalid,
            true,
            false,
            CandidateGenerationAttemptCleanupDispositionV1::Succeeded,
        ),
    )
    .expect("failed");
    assert_eq!(
        CandidateGenerationAttemptRecordV1::from_json_bytes(
            &serde_json::to_vec(&failed).expect("failed JSON"),
            planned,
            Some(precursor),
            Some(&fixture.receipt),
        ),
        Err(GenerationQualificationContractError::AttemptRecordRelationshipMismatch),
    );
}

#[test]
fn attempt_relationships_reject_foreign_precursor_and_receipt() {
    let fixture = receipt_fixture();
    let foreign = managed_fixture(1);
    assert_eq!(
        CandidateGenerationAttemptRecordV1::completed(
            &fixture.bundle.managed.base.attempts[0],
            &foreign.precursor,
            &fixture.receipt,
        ),
        Err(GenerationQualificationContractError::AttemptRecordRelationshipMismatch),
    );
    assert_eq!(
        CandidateGenerationAttemptRecordV1::failed(
            &fixture.bundle.managed.base.attempts[0],
            Some(&foreign.precursor),
            failure_input(
                CandidateGenerationAttemptFailurePhaseV1::Launch,
                CandidateGenerationAttemptFailureCategoryV1::LaunchFailed,
                false,
                false,
                CandidateGenerationAttemptCleanupDispositionV1::Succeeded,
            ),
        ),
        Err(GenerationQualificationContractError::AttemptRecordRelationshipMismatch),
    );
}

#[test]
fn every_failure_category_has_a_distinct_stable_identity_input() {
    let fixture = receipt_fixture();
    let categories = [
        CandidateGenerationAttemptFailureCategoryV1::Cancelled,
        CandidateGenerationAttemptFailureCategoryV1::DeadlineExceeded,
        CandidateGenerationAttemptFailureCategoryV1::RequestInvalid,
        CandidateGenerationAttemptFailureCategoryV1::RelationshipMismatch,
        CandidateGenerationAttemptFailureCategoryV1::UnsupportedPlatform,
        CandidateGenerationAttemptFailureCategoryV1::PackageChanged,
        CandidateGenerationAttemptFailureCategoryV1::PackageRevalidationFailed,
        CandidateGenerationAttemptFailureCategoryV1::LaunchFailed,
        CandidateGenerationAttemptFailureCategoryV1::TransportFailed,
        CandidateGenerationAttemptFailureCategoryV1::ResponseInvalid,
        CandidateGenerationAttemptFailureCategoryV1::ObservationMismatch,
        CandidateGenerationAttemptFailureCategoryV1::ManagedEvidenceInvalid,
        CandidateGenerationAttemptFailureCategoryV1::CleanupFailed,
        CandidateGenerationAttemptFailureCategoryV1::PublicationFailed,
        CandidateGenerationAttemptFailureCategoryV1::ReadbackFailed,
        CandidateGenerationAttemptFailureCategoryV1::ReceiptInvalid,
    ];
    let mut identities = HashSet::new();
    for category in categories {
        let disposition = if category == CandidateGenerationAttemptFailureCategoryV1::CleanupFailed
        {
            CandidateGenerationAttemptCleanupDispositionV1::Failed
        } else {
            CandidateGenerationAttemptCleanupDispositionV1::Succeeded
        };
        let record = CandidateGenerationAttemptRecordV1::failed(
            &fixture.bundle.managed.base.attempts[0],
            Some(&fixture.bundle.managed.precursor),
            failure_input(
                CandidateGenerationAttemptFailurePhaseV1::ResponseValidation,
                category,
                true,
                false,
                disposition,
            ),
        )
        .expect("category record");
        assert!(identities.insert(record.attempt_record_id().clone()));
    }
    assert_eq!(identities.len(), 16);
}

#[test]
fn attempt_debug_is_redacted() {
    let fixture = receipt_fixture();
    let record = CandidateGenerationAttemptRecordV1::completed(
        &fixture.bundle.managed.base.attempts[0],
        &fixture.bundle.managed.precursor,
        &fixture.receipt,
    )
    .expect("completed");
    let debug = format!("{record:?}");
    assert!(debug.contains(record.attempt_record_id().digest().as_str()));
    assert!(!debug.contains(fixture.receipt.receipt_id().digest().as_str()));
    assert!(
        !debug.contains(
            fixture
                .bundle
                .managed
                .precursor
                .precursor_id()
                .digest()
                .as_str()
        )
    );
    let failed = CandidateGenerationAttemptRecordV1::failed(
        &fixture.bundle.managed.base.attempts[0],
        Some(&fixture.bundle.managed.precursor),
        failure_input(
            CandidateGenerationAttemptFailurePhaseV1::ResponseValidation,
            CandidateGenerationAttemptFailureCategoryV1::ResponseInvalid,
            true,
            false,
            CandidateGenerationAttemptCleanupDispositionV1::Succeeded,
        ),
    )
    .expect("failed");
    assert!(format!("{failed:?}").contains("failed"));
}

#[test]
fn attempt_wire_field_and_enum_tag_orders_are_stable() {
    let fixture = receipt_fixture();
    let completed = CandidateGenerationAttemptRecordV1::completed(
        &fixture.bundle.managed.base.attempts[0],
        &fixture.bundle.managed.precursor,
        &fixture.receipt,
    )
    .expect("completed");
    let completed_json =
        String::from_utf8(serde_json::to_vec(&completed).expect("JSON")).expect("UTF-8");
    let fields = [
        "schema_version",
        "outcome",
        "completed",
        "planned_attempt_id",
        "precursor_id",
        "receipt_id",
    ];
    let positions =
        fields.map(|field| completed_json.find(&format!("\"{field}\"")).expect("field"));
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));

    let phases = [
        CandidateGenerationAttemptFailurePhaseV1::RequestCompilation,
        CandidateGenerationAttemptFailurePhaseV1::PrecursorCompilation,
        CandidateGenerationAttemptFailurePhaseV1::Launch,
        CandidateGenerationAttemptFailurePhaseV1::PreTrafficRevalidation,
        CandidateGenerationAttemptFailurePhaseV1::GenerationTraffic,
        CandidateGenerationAttemptFailurePhaseV1::ResponseValidation,
        CandidateGenerationAttemptFailurePhaseV1::FinalObservation,
        CandidateGenerationAttemptFailurePhaseV1::ManagedEvidenceCompilation,
        CandidateGenerationAttemptFailurePhaseV1::Cleanup,
        CandidateGenerationAttemptFailurePhaseV1::BundlePublication,
        CandidateGenerationAttemptFailurePhaseV1::BundleReadback,
        CandidateGenerationAttemptFailurePhaseV1::ReceiptCompilation,
    ];
    assert_eq!(
        serde_json::to_value(phases).expect("phases"),
        serde_json::json!([
            "request_compilation",
            "precursor_compilation",
            "launch",
            "pre_traffic_revalidation",
            "generation_traffic",
            "response_validation",
            "final_observation",
            "managed_evidence_compilation",
            "cleanup",
            "bundle_publication",
            "bundle_readback",
            "receipt_compilation"
        ]),
    );
}

#[test]
fn failure_category_and_cleanup_tags_are_stable() {
    let categories = [
        CandidateGenerationAttemptFailureCategoryV1::Cancelled,
        CandidateGenerationAttemptFailureCategoryV1::DeadlineExceeded,
        CandidateGenerationAttemptFailureCategoryV1::RequestInvalid,
        CandidateGenerationAttemptFailureCategoryV1::RelationshipMismatch,
        CandidateGenerationAttemptFailureCategoryV1::UnsupportedPlatform,
        CandidateGenerationAttemptFailureCategoryV1::PackageChanged,
        CandidateGenerationAttemptFailureCategoryV1::PackageRevalidationFailed,
        CandidateGenerationAttemptFailureCategoryV1::LaunchFailed,
        CandidateGenerationAttemptFailureCategoryV1::TransportFailed,
        CandidateGenerationAttemptFailureCategoryV1::ResponseInvalid,
        CandidateGenerationAttemptFailureCategoryV1::ObservationMismatch,
        CandidateGenerationAttemptFailureCategoryV1::ManagedEvidenceInvalid,
        CandidateGenerationAttemptFailureCategoryV1::CleanupFailed,
        CandidateGenerationAttemptFailureCategoryV1::PublicationFailed,
        CandidateGenerationAttemptFailureCategoryV1::ReadbackFailed,
        CandidateGenerationAttemptFailureCategoryV1::ReceiptInvalid,
    ];
    assert_eq!(
        serde_json::to_value(categories).expect("categories"),
        serde_json::json!([
            "cancelled",
            "deadline_exceeded",
            "request_invalid",
            "relationship_mismatch",
            "unsupported_platform",
            "package_changed",
            "package_revalidation_failed",
            "launch_failed",
            "transport_failed",
            "response_invalid",
            "observation_mismatch",
            "managed_evidence_invalid",
            "cleanup_failed",
            "publication_failed",
            "readback_failed",
            "receipt_invalid"
        ]),
    );
    assert_eq!(
        serde_json::to_value([
            CandidateGenerationAttemptCleanupDispositionV1::NotRequired,
            CandidateGenerationAttemptCleanupDispositionV1::Succeeded,
            CandidateGenerationAttemptCleanupDispositionV1::Failed,
        ])
        .expect("cleanup dispositions"),
        serde_json::json!(["not_required", "succeeded", "failed"]),
    );
}
