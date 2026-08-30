use super::*;

#[test]
fn receipt_round_trips_exposes_exact_closure_and_has_stable_identity() {
    let fixture = OperationFixture::new();
    let platform = fixture.platform(GenerationQualificationPlatformStatusV1::Supported);
    let license = fixture.license(GenerationQualificationLicenseDecisionV1::LocalUseOnly);
    let phases = fixture.failed_ledger_phases();
    let input = receipt_input(
        1,
        GenerationQualificationOperationTerminalStatusV1::Completed,
        GenerationQualificationOperationFinalizationStatusV1::Passed,
        1,
    );
    let relations = receipt_relations(&fixture, &platform, &license, &phases);
    let receipt = GenerationQualificationOperationReceiptV1::new(relations, input)
        .expect("operation receipt");

    assert_eq!(receipt.schema_version(), 1);
    assert_eq!(
        receipt.target_generation_system_id(),
        fixture.base.policy.target_generation_system_id()
    );
    assert_eq!(
        receipt.baseline_generation_system_id(),
        fixture.base.policy.baseline_generation_system_id()
    );
    assert_eq!(
        receipt.operation_policy_id(),
        fixture.base.policy.operation_policy_id()
    );
    assert_eq!(
        receipt.request_projection_id(),
        fixture.projection.request_projection_id()
    );
    assert_eq!(
        receipt.platform_evidence_id(),
        platform.platform_evidence_id()
    );
    assert_eq!(receipt.license_evidence_id(), license.license_evidence_id());
    assert_eq!(
        receipt.attempt_ledger_manifest(),
        (
            phases.ledger.attempt_ledger_manifest_id(),
            phases.ledger.evidence_root_digest()
        )
    );
    assert_eq!(
        receipt.repeatability_evidence_manifest(),
        (
            phases.repeatability.repeatability_evidence_manifest_id(),
            phases.repeatability.evidence_root_digest()
        )
    );
    assert_eq!(
        receipt.resource_evidence_manifest(),
        (
            phases.resource.resource_evidence_manifest_id(),
            phases.resource.evidence_root_digest()
        )
    );
    assert_eq!(
        receipt.human_adjudication_evidence_manifest(),
        (
            phases.human.human_adjudication_evidence_manifest_id(),
            phases.human.evidence_root_digest()
        )
    );
    assert_eq!(receipt.elapsed_nanoseconds(), 1);
    assert_eq!(receipt.peak_concurrent_attempts(), 1);
    assert_eq!(
        receipt.terminal_status(),
        GenerationQualificationOperationTerminalStatusV1::Completed
    );
    assert_eq!(
        receipt.finalization_status(),
        GenerationQualificationOperationFinalizationStatusV1::Passed
    );
    let bytes = serde_json::to_vec(&receipt).expect("receipt JSON");
    assert_eq!(
        GenerationQualificationOperationReceiptV1::from_json_bytes(&bytes, relations, input)
            .expect("receipt decode"),
        receipt
    );
    receipt
        .validate_against(relations, input)
        .expect("receipt revalidation");
    assert_eq!(
        receipt.operation_receipt_id().digest().as_str(),
        "2adf55b46c6614f30a62c807218ee12a999ac3fa60e0587ca5ceccf59b98bee8"
    );
    assert_ne!(
        receipt.operation_receipt_id().digest(),
        fixture.projection.request_projection_id().digest()
    );
}
#[test]
fn decoder_is_strict_bounded_canonical_and_has_no_terminal_digest() {
    let fixture = OperationFixture::new();
    let platform = fixture.platform(GenerationQualificationPlatformStatusV1::Supported);
    let license = fixture.license(GenerationQualificationLicenseDecisionV1::LocalUseOnly);
    let phases = fixture.failed_ledger_phases();
    let input = receipt_input(
        1,
        GenerationQualificationOperationTerminalStatusV1::Completed,
        GenerationQualificationOperationFinalizationStatusV1::Passed,
        1,
    );
    let relations = receipt_relations(&fixture, &platform, &license, &phases);
    let receipt = GenerationQualificationOperationReceiptV1::new(relations, input)
        .expect("operation receipt");
    let bytes = serde_json::to_vec(&receipt).expect("receipt JSON");
    let text = String::from_utf8(bytes.clone()).expect("UTF-8 JSON");
    assert!(!text.contains("terminal_digest"));

    assert_eq!(
        GenerationQualificationOperationReceiptV1::from_json_bytes(
            &vec![b' '; MAX_GENERATION_QUALIFICATION_OPERATION_RECEIPT_JSON_BYTES + 1],
            relations,
            input,
        ),
        Err(GenerationQualificationOperationContractError::EncodedRecordTooLarge)
    );
    assert_eq!(
        GenerationQualificationOperationReceiptV1::from_json_bytes(b"{", relations, input),
        Err(GenerationQualificationOperationContractError::InvalidEncoding)
    );
    let mut unknown = receipt_value(&receipt);
    unknown["terminal_digest"] = Value::String(digest("forbidden terminal digest").to_string());
    assert_eq!(
        decode_value(&unknown, relations, input),
        Err(GenerationQualificationOperationContractError::InvalidEncoding)
    );
    let duplicate = text.replacen(
        "\"schema_version\":1",
        "\"schema_version\":1,\"schema_version\":1",
        1,
    );
    assert_eq!(
        GenerationQualificationOperationReceiptV1::from_json_bytes(
            duplicate.as_bytes(),
            relations,
            input,
        ),
        Err(GenerationQualificationOperationContractError::InvalidEncoding)
    );
    let mut missing = receipt_value(&receipt);
    missing
        .as_object_mut()
        .expect("object")
        .remove("operation_policy_id");
    assert_eq!(
        decode_value(&missing, relations, input),
        Err(GenerationQualificationOperationContractError::InvalidEncoding)
    );
    assert_eq!(
        GenerationQualificationOperationReceiptV1::from_json_bytes(
            &reorder_first_two_fields(&bytes),
            relations,
            input,
        ),
        Err(GenerationQualificationOperationContractError::NonCanonicalEncoding)
    );
    let mut trailing = bytes;
    trailing.push(b' ');
    assert_eq!(
        GenerationQualificationOperationReceiptV1::from_json_bytes(&trailing, relations, input),
        Err(GenerationQualificationOperationContractError::NonCanonicalEncoding)
    );
    let mut unsupported = receipt_value(&receipt);
    unsupported["schema_version"] = Value::from(2);
    assert_eq!(
        decode_value(&unsupported, relations, input),
        Err(GenerationQualificationOperationContractError::UnsupportedSchema)
    );
}

#[test]
fn every_scope_policy_root_and_runner_input_substitution_is_rejected() {
    let fixture = OperationFixture::new();
    let platform = fixture.platform(GenerationQualificationPlatformStatusV1::Supported);
    let license = fixture.license(GenerationQualificationLicenseDecisionV1::LocalUseOnly);
    let phases = fixture.failed_ledger_phases();
    let input = receipt_input(
        1,
        GenerationQualificationOperationTerminalStatusV1::Completed,
        GenerationQualificationOperationFinalizationStatusV1::Passed,
        1,
    );
    let relations = receipt_relations(&fixture, &platform, &license, &phases);
    let receipt = GenerationQualificationOperationReceiptV1::new(relations, input)
        .expect("operation receipt");
    let substitute = Value::String(digest("receipt substitution").as_str().to_owned());

    for field in [
        "target_generation_system_id",
        "baseline_generation_system_id",
        "operation_policy_id",
        "request_projection_id",
        "platform_evidence_id",
        "license_evidence_id",
        "attempt_ledger_manifest_id",
        "attempt_ledger_root_digest",
        "repeatability_evidence_manifest_id",
        "repeatability_evidence_root_digest",
        "resource_evidence_manifest_id",
        "resource_evidence_root_digest",
        "human_adjudication_evidence_manifest_id",
        "human_adjudication_evidence_root_digest",
    ] {
        let mut changed = receipt_value(&receipt);
        changed[field] = substitute.clone();
        assert_eq!(
            decode_value(&changed, relations, input),
            Err(GenerationQualificationOperationContractError::RelationshipMismatch),
            "accepted substituted {field}"
        );
    }

    for (field, value) in [
        ("elapsed_nanoseconds", Value::from(2)),
        ("peak_concurrent_attempts", Value::from(0)),
        ("terminal_status", Value::String("failed".to_owned())),
        ("finalization_status", Value::String("failed".to_owned())),
    ] {
        let mut changed = receipt_value(&receipt);
        changed[field] = value;
        assert_eq!(
            decode_value(&changed, relations, input),
            Err(GenerationQualificationOperationContractError::RelationshipMismatch),
            "accepted untrusted runner field {field}"
        );
    }
}

#[test]
fn deadline_boundary_is_exact_in_nanoseconds() {
    let fixture = OperationFixture::new();
    let platform = fixture.platform(GenerationQualificationPlatformStatusV1::Supported);
    let license = fixture.license(GenerationQualificationLicenseDecisionV1::LocalUseOnly);
    let phases = fixture.skipped_phases();
    let relations = receipt_relations(&fixture, &platform, &license, &phases);
    let threshold =
        u64::from(fixture.base.policy.limits().maximum_elapsed_milliseconds()) * 1_000_000;

    GenerationQualificationOperationReceiptV1::new(
        relations,
        receipt_input(
            0,
            GenerationQualificationOperationTerminalStatusV1::Failed,
            GenerationQualificationOperationFinalizationStatusV1::NotRequired,
            threshold - 1,
        ),
    )
    .expect("below deadline");
    for elapsed in [threshold, threshold + 1] {
        GenerationQualificationOperationReceiptV1::new(
            relations,
            receipt_input(
                0,
                GenerationQualificationOperationTerminalStatusV1::DeadlineExceeded,
                GenerationQualificationOperationFinalizationStatusV1::NotRequired,
                elapsed,
            ),
        )
        .expect("deadline terminal");
    }
    assert_eq!(
        GenerationQualificationOperationReceiptV1::new(
            relations,
            receipt_input(
                0,
                GenerationQualificationOperationTerminalStatusV1::DeadlineExceeded,
                GenerationQualificationOperationFinalizationStatusV1::NotRequired,
                threshold - 1,
            ),
        ),
        Err(GenerationQualificationOperationContractError::DeadlineMismatch)
    );
    assert_eq!(
        GenerationQualificationOperationReceiptV1::new(
            relations,
            receipt_input(
                0,
                GenerationQualificationOperationTerminalStatusV1::Failed,
                GenerationQualificationOperationFinalizationStatusV1::NotRequired,
                threshold,
            ),
        ),
        Err(GenerationQualificationOperationContractError::DeadlineMismatch)
    );
}
