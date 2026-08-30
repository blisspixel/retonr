use std::cell::Cell;

use super::*;

fn after_receipt_preflight(
    bytes: &[u8],
    reads: &Cell<usize>,
) -> Result<CandidateGenerationReceiptV1Preflight, GenerationQualificationContractError> {
    let token = CandidateGenerationReceiptV1::preflight_json_bytes(bytes)?;
    reads.set(reads.get() + 1);
    Ok(token)
}

fn after_attempt_preflight(
    bytes: &[u8],
    reads: &Cell<usize>,
) -> Result<CandidateGenerationAttemptRecordV1Preflight, GenerationQualificationContractError> {
    let token = CandidateGenerationAttemptRecordV1::preflight_json_bytes(bytes)?;
    reads.set(reads.get() + 1);
    Ok(token)
}

fn receipt_invalid_encodings(
    encoded: &[u8],
) -> Vec<(Vec<u8>, GenerationQualificationContractError)> {
    let mut trailing = encoded.to_vec();
    trailing.push(b' ');
    vec![
        (
            vec![b' '; MAX_CANDIDATE_GENERATION_RECEIPT_JSON_BYTES + 1],
            GenerationQualificationContractError::EncodedRecordTooLarge,
        ),
        (
            b"{".to_vec(),
            GenerationQualificationContractError::InvalidEncoding,
        ),
        (
            replace_once(encoded, "\"schema_version\":1", "\"schema_version\":2"),
            GenerationQualificationContractError::UnsupportedSchema(2),
        ),
        (
            replace_once(
                encoded,
                "\"schema_version\":1",
                "\"schema_version\":1,\"unknown\":true",
            ),
            GenerationQualificationContractError::InvalidEncoding,
        ),
        (
            replace_once(
                encoded,
                "\"schema_version\":1",
                "\"schema_version\":1,\"schema_version\":1",
            ),
            GenerationQualificationContractError::InvalidEncoding,
        ),
        (
            trailing,
            GenerationQualificationContractError::NonCanonicalEncoding,
        ),
        (
            serde_json::to_vec(
                &serde_json::from_slice::<serde_json::Value>(encoded).expect("receipt value"),
            )
            .expect("reencoded receipt value"),
            GenerationQualificationContractError::NonCanonicalEncoding,
        ),
    ]
}

fn attempt_invalid_encodings(
    encoded: &[u8],
) -> Vec<(Vec<u8>, GenerationQualificationContractError)> {
    let mut trailing = encoded.to_vec();
    trailing.push(b'\n');
    vec![
        (
            vec![b' '; MAX_CANDIDATE_GENERATION_ATTEMPT_RECORD_JSON_BYTES + 1],
            GenerationQualificationContractError::EncodedRecordTooLarge,
        ),
        (
            b"[".to_vec(),
            GenerationQualificationContractError::InvalidEncoding,
        ),
        (
            replace_once(encoded, "\"schema_version\":1", "\"schema_version\":9"),
            GenerationQualificationContractError::UnsupportedSchema(9),
        ),
        (
            replace_once(
                encoded,
                "\"schema_version\":1",
                "\"schema_version\":1,\"unknown\":null",
            ),
            GenerationQualificationContractError::InvalidEncoding,
        ),
        (
            replace_once(
                encoded,
                "\"schema_version\":1",
                "\"schema_version\":1,\"schema_version\":1",
            ),
            GenerationQualificationContractError::InvalidEncoding,
        ),
        (
            trailing,
            GenerationQualificationContractError::NonCanonicalEncoding,
        ),
        (
            serde_json::to_vec(
                &serde_json::from_slice::<serde_json::Value>(encoded).expect("attempt value"),
            )
            .expect("reencoded attempt value"),
            GenerationQualificationContractError::NonCanonicalEncoding,
        ),
    ]
}

#[test]
fn receipt_preflight_rejects_untrusted_bytes_before_external_work() {
    let fixture = receipt_fixture();
    let encoded = serde_json::to_vec(&fixture.receipt).expect("receipt JSON");
    for (invalid, expected) in receipt_invalid_encodings(&encoded) {
        let reads = Cell::new(0);
        let observed = after_receipt_preflight(&invalid, &reads)
            .expect_err("invalid receipt must fail before external work");
        assert_eq!(observed, expected);
        assert_eq!(reads.get(), 0);
        assert_eq!(
            CandidateGenerationReceiptV1::from_json_bytes(
                &invalid,
                fixture.relations(),
                fixture.usage,
            )
            .expect_err("full decoder shares the preflight parser"),
            expected,
        );
    }
}

#[test]
fn attempt_preflight_rejects_untrusted_bytes_before_external_work() {
    let fixture = receipt_fixture();
    let completed = CandidateGenerationAttemptRecordV1::completed(
        &fixture.bundle.managed.base.attempts[0],
        &fixture.bundle.managed.precursor,
        &fixture.receipt,
    )
    .expect("completed attempt");
    let encoded = serde_json::to_vec(&completed).expect("attempt JSON");
    for (invalid, expected) in attempt_invalid_encodings(&encoded) {
        let reads = Cell::new(0);
        let observed = after_attempt_preflight(&invalid, &reads)
            .expect_err("invalid attempt must fail before external work");
        assert_eq!(observed, expected);
        assert_eq!(reads.get(), 0);
        assert_eq!(
            CandidateGenerationAttemptRecordV1::from_json_bytes(
                &invalid,
                &fixture.bundle.managed.base.attempts[0],
                Some(&fixture.bundle.managed.precursor),
                Some(&fixture.receipt),
            )
            .expect_err("full decoder shares the preflight parser"),
            expected,
        );
    }
}

#[test]
fn opaque_preflights_round_trip_only_through_exact_relationship_validation() {
    let fixture = receipt_fixture();
    let receipt_bytes = serde_json::to_vec(&fixture.receipt).expect("receipt JSON");
    let receipt_token = CandidateGenerationReceiptV1::preflight_json_bytes(&receipt_bytes)
        .expect("receipt preflight");
    assert_eq!(receipt_token.schema_version(), 1);
    assert_eq!(receipt_token.usage_observation(), fixture.usage);
    let embedded_usage = receipt_token.usage_observation();
    let receipt_debug = format!("{receipt_token:?}");
    assert!(!receipt_debug.contains(fixture.receipt.receipt_id().digest().as_str()));
    let receipt = CandidateGenerationReceiptV1::from_preflight(
        receipt_token,
        fixture.relations(),
        embedded_usage,
    )
    .expect("exact receipt relationships");
    assert_eq!(receipt, fixture.receipt);

    let completed = CandidateGenerationAttemptRecordV1::completed(
        &fixture.bundle.managed.base.attempts[0],
        &fixture.bundle.managed.precursor,
        &receipt,
    )
    .expect("completed attempt");
    let completed_bytes = serde_json::to_vec(&completed).expect("completed JSON");
    let completed_token =
        CandidateGenerationAttemptRecordV1::preflight_json_bytes(&completed_bytes)
            .expect("completed preflight");
    assert_eq!(completed_token.schema_version(), 1);
    assert_eq!(
        completed_token.disposition(),
        CandidateGenerationAttemptDispositionV1::Completed,
    );
    assert!(
        !format!("{completed_token:?}").contains(completed.attempt_record_id().digest().as_str())
    );
    assert_eq!(
        CandidateGenerationAttemptRecordV1::from_preflight(
            completed_token,
            &fixture.bundle.managed.base.attempts[0],
            Some(&fixture.bundle.managed.precursor),
            Some(&receipt),
        )
        .expect("completed relationships"),
        completed,
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
    .expect("failed attempt");
    let failed_bytes = serde_json::to_vec(&failed).expect("failed JSON");
    let failed_token = CandidateGenerationAttemptRecordV1::preflight_json_bytes(&failed_bytes)
        .expect("failed preflight");
    assert_eq!(
        failed_token.disposition(),
        CandidateGenerationAttemptDispositionV1::Failed,
    );
    assert_eq!(
        CandidateGenerationAttemptRecordV1::from_preflight(
            failed_token,
            &fixture.bundle.managed.base.attempts[0],
            Some(&fixture.bundle.managed.precursor),
            None,
        )
        .expect("failed relationships"),
        failed,
    );
}

#[test]
fn preflight_does_not_promote_canonical_but_untrusted_content() {
    let fixture = receipt_fixture();
    let receipt_bytes = serde_json::to_vec(&fixture.receipt).expect("receipt JSON");
    let changed_receipt = replace_once(
        &receipt_bytes,
        "\"model_used_proven\":false",
        "\"model_used_proven\":true",
    );
    let token = CandidateGenerationReceiptV1::preflight_json_bytes(&changed_receipt)
        .expect("canonical structural receipt");
    assert_eq!(
        CandidateGenerationReceiptV1::from_preflight(token, fixture.relations(), fixture.usage),
        Err(GenerationQualificationContractError::ReceiptRelationshipMismatch),
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
    .expect("failed attempt");
    let failed_bytes = serde_json::to_vec(&failed).expect("failed JSON");
    let invalid_failure = replace_once(
        &failed_bytes,
        "\"traffic_observed\":true,\"output_observed\":false",
        "\"traffic_observed\":false,\"output_observed\":true",
    );
    let token = CandidateGenerationAttemptRecordV1::preflight_json_bytes(&invalid_failure)
        .expect("canonical structural attempt");
    assert_eq!(
        token.disposition(),
        CandidateGenerationAttemptDispositionV1::Failed
    );
    assert_eq!(
        CandidateGenerationAttemptRecordV1::from_preflight(
            token,
            &fixture.bundle.managed.base.attempts[0],
            Some(&fixture.bundle.managed.precursor),
            None,
        ),
        Err(GenerationQualificationContractError::InvalidAttemptFailure),
    );
}
