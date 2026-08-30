use super::*;

const _: () = {
    assert!(MAX_GENERATION_QUALIFICATION_PHASE_INTERRUPTION_JSON_BYTES >= 16 * 1_024);
    assert!(MAX_GENERATION_QUALIFICATION_PHASE_INTERRUPTION_CANONICAL_BYTES >= 4 * 1_024);
};

#[test]
fn interruption_round_trips_with_frozen_identity_and_exact_accessors() {
    let fixture = OperationFixture::new();
    let platform = fixture.platform();
    let license = fixture.license();
    let phases = fixture.phases(GenerationQualificationPhaseStatusV1::Skipped);
    let receipt_input = receipt_input(
        1,
        0,
        GenerationQualificationOperationTerminalStatusV1::Failed,
        GenerationQualificationOperationFinalizationStatusV1::NotRequired,
    );
    let receipt_relations = receipt_relations(&fixture, &platform, &license, &phases);
    let receipt = GenerationQualificationOperationReceiptV1::new(receipt_relations, receipt_input)
        .expect("failed receipt");
    let relations = interruption_relations(&fixture, &receipt, receipt_relations, receipt_input);
    let input = interruption_input(
        &fixture,
        GenerationQualificationPhaseCheckpointV1::EvidenceAcquisition,
        GenerationQualificationPhaseInterruptionReasonV1::AuthorityDrift,
    );
    let record = GenerationQualificationPhaseInterruptionRecordV1::new(&relations, input.clone())
        .expect("interruption record");
    let bytes = serde_json::to_vec(&record).expect("interruption JSON");

    assert_eq!(record.schema_version(), 1);
    assert_eq!(
        record.operation_receipt_id(),
        receipt.operation_receipt_id()
    );
    assert_eq!(
        record.operation_policy_id(),
        fixture.base.policy.operation_policy_id()
    );
    assert_eq!(
        record.target_generation_system_id(),
        fixture.base.policy.target_generation_system_id()
    );
    assert_eq!(
        record.generation_qualification_plan_id(),
        fixture.base.plan.qualification_plan_id()
    );
    assert_eq!(
        record.suite_manifest_id(),
        fixture.base.suite.suite_manifest_id()
    );
    assert_eq!(
        record.phase_policy_digest(),
        fixture.base.policy.attempt_ledger_policy_digest()
    );
    assert_eq!(
        record.phase(),
        GenerationQualificationInterruptedPhaseV1::AttemptLedger
    );
    assert_eq!(
        record.checkpoint(),
        GenerationQualificationPhaseCheckpointV1::EvidenceAcquisition
    );
    assert_eq!(
        record.planned_attempt_id(),
        Some(fixture.base.attempts[0].planned_attempt_id())
    );
    assert_eq!(
        record.reason(),
        GenerationQualificationPhaseInterruptionReasonV1::AuthorityDrift
    );
    assert_eq!(
        GenerationQualificationPhaseInterruptionRecordV1::from_json_bytes(
            &bytes, &relations, &input
        )
        .expect("canonical decode"),
        record
    );
    record
        .validate_against(&relations, input)
        .expect("recursive revalidation");
    assert_eq!(
        record.phase_interruption_record_id().digest().as_str(),
        "2e8e1976efb75b58891bbd48504fb1d684558b289154a2320890eb4dec6f920e"
    );
    assert_ne!(
        record.phase_interruption_record_id().digest(),
        receipt.operation_receipt_id().digest()
    );
    assert_ne!(
        GENERATION_QUALIFICATION_PHASE_INTERRUPTION_RECORD_ID_DOMAIN,
        GENERATION_QUALIFICATION_OPERATION_RECEIPT_ID_DOMAIN
    );
    assert!(bytes.len() < MAX_GENERATION_QUALIFICATION_PHASE_INTERRUPTION_JSON_BYTES);
    assert_canonical_json(&fixture, &receipt, bytes);
}

fn assert_canonical_json(
    fixture: &OperationFixture,
    receipt: &GenerationQualificationOperationReceiptV1,
    bytes: Vec<u8>,
) {
    let text = String::from_utf8(bytes).expect("UTF-8 JSON");
    assert_eq!(
        text,
        format!(
            concat!(
                "{{\"schema_version\":1,\"record_kind\":\"phase_interruption\",",
                "\"operation_receipt_id\":\"{}\",\"operation_policy_id\":\"{}\",",
                "\"target_generation_system_id\":\"{}\",",
                "\"generation_qualification_plan_id\":\"{}\",",
                "\"suite_manifest_id\":\"{}\",\"phase_policy_digest\":\"{}\",",
                "\"phase\":\"attempt_ledger\",\"checkpoint\":\"evidence_acquisition\",",
                "\"planned_attempt_id\":\"{}\",\"reason\":\"authority_drift\"}}"
            ),
            receipt.operation_receipt_id().digest(),
            fixture.base.policy.operation_policy_id().digest(),
            fixture.base.policy.target_generation_system_id().digest(),
            fixture.base.plan.qualification_plan_id().digest(),
            fixture.base.suite.suite_manifest_id().digest(),
            fixture.base.policy.attempt_ledger_policy_digest(),
            fixture.base.attempts[0].planned_attempt_id().digest(),
        )
    );
}

#[test]
fn decoder_is_strict_bounded_and_canonical() {
    let fixture = OperationFixture::new();
    let platform = fixture.platform();
    let license = fixture.license();
    let phases = fixture.phases(GenerationQualificationPhaseStatusV1::Skipped);
    let receipt_input = receipt_input(
        1,
        0,
        GenerationQualificationOperationTerminalStatusV1::Failed,
        GenerationQualificationOperationFinalizationStatusV1::NotRequired,
    );
    let receipt_relations = receipt_relations(&fixture, &platform, &license, &phases);
    let receipt = GenerationQualificationOperationReceiptV1::new(receipt_relations, receipt_input)
        .expect("failed receipt");
    let relations = interruption_relations(&fixture, &receipt, receipt_relations, receipt_input);
    let input = interruption_input(
        &fixture,
        GenerationQualificationPhaseCheckpointV1::EvidenceAcquisition,
        GenerationQualificationPhaseInterruptionReasonV1::AuthorityDrift,
    );
    let record = GenerationQualificationPhaseInterruptionRecordV1::new(&relations, input.clone())
        .expect("interruption record");
    let bytes = serde_json::to_vec(&record).expect("interruption JSON");
    let text = String::from_utf8(bytes.clone()).expect("UTF-8 JSON");

    assert_eq!(
        GenerationQualificationPhaseInterruptionRecordV1::from_json_bytes(
            &vec![b' '; MAX_GENERATION_QUALIFICATION_PHASE_INTERRUPTION_JSON_BYTES + 1],
            &relations,
            &input,
        ),
        Err(GenerationQualificationOperationContractError::EncodedRecordTooLarge)
    );
    assert_eq!(
        GenerationQualificationPhaseInterruptionRecordV1::from_json_bytes(b"{", &relations, &input),
        Err(GenerationQualificationOperationContractError::InvalidEncoding)
    );
    let duplicate = text.replacen(
        "\"schema_version\":1",
        "\"schema_version\":1,\"schema_version\":1",
        1,
    );
    assert_eq!(
        GenerationQualificationPhaseInterruptionRecordV1::from_json_bytes(
            duplicate.as_bytes(),
            &relations,
            &input,
        ),
        Err(GenerationQualificationOperationContractError::InvalidEncoding)
    );
    let mut unknown = serde_json::to_value(&record).expect("interruption value");
    unknown["authority"] = Value::Bool(true);
    assert_eq!(
        decode_value(&unknown, &relations, &input),
        Err(GenerationQualificationOperationContractError::InvalidEncoding)
    );
    let mut missing = serde_json::to_value(&record).expect("interruption value");
    missing
        .as_object_mut()
        .expect("record object")
        .remove("planned_attempt_id");
    assert_eq!(
        decode_value(&missing, &relations, &input),
        Err(GenerationQualificationOperationContractError::InvalidEncoding)
    );
    let mut wrong_kind = serde_json::to_value(&record).expect("interruption value");
    wrong_kind["record_kind"] = Value::String("qualification".to_owned());
    assert_eq!(
        decode_value(&wrong_kind, &relations, &input),
        Err(GenerationQualificationOperationContractError::InvalidEncoding)
    );
    assert_eq!(
        GenerationQualificationPhaseInterruptionRecordV1::from_json_bytes(
            &reorder_first_two_fields(&bytes),
            &relations,
            &input,
        ),
        Err(GenerationQualificationOperationContractError::NonCanonicalEncoding)
    );
    let mut trailing = bytes;
    trailing.push(b' ');
    assert_eq!(
        GenerationQualificationPhaseInterruptionRecordV1::from_json_bytes(
            &trailing, &relations, &input
        ),
        Err(GenerationQualificationOperationContractError::NonCanonicalEncoding)
    );
    let mut unsupported = serde_json::to_value(&record).expect("interruption value");
    unsupported["schema_version"] = Value::from(2);
    assert_eq!(
        decode_value(&unsupported, &relations, &input),
        Err(GenerationQualificationOperationContractError::UnsupportedSchema)
    );
}

#[test]
fn every_wire_relationship_and_runner_input_is_independently_rederived() {
    let fixture = OperationFixture::new();
    let platform = fixture.platform();
    let license = fixture.license();
    let phases = fixture.phases(GenerationQualificationPhaseStatusV1::Skipped);
    let receipt_input = receipt_input(
        1,
        0,
        GenerationQualificationOperationTerminalStatusV1::Failed,
        GenerationQualificationOperationFinalizationStatusV1::NotRequired,
    );
    let receipt_relations = receipt_relations(&fixture, &platform, &license, &phases);
    let receipt = GenerationQualificationOperationReceiptV1::new(receipt_relations, receipt_input)
        .expect("failed receipt");
    let relations = interruption_relations(&fixture, &receipt, receipt_relations, receipt_input);
    let input = interruption_input(
        &fixture,
        GenerationQualificationPhaseCheckpointV1::EvidenceAcquisition,
        GenerationQualificationPhaseInterruptionReasonV1::AuthorityDrift,
    );
    let record = GenerationQualificationPhaseInterruptionRecordV1::new(&relations, input.clone())
        .expect("interruption record");
    let substitute = Value::String(digest("substituted interruption field").to_string());

    for field in [
        "operation_receipt_id",
        "operation_policy_id",
        "target_generation_system_id",
        "generation_qualification_plan_id",
        "suite_manifest_id",
        "phase_policy_digest",
    ] {
        let mut changed = serde_json::to_value(&record).expect("interruption value");
        changed[field] = substitute.clone();
        assert_eq!(
            decode_value(&changed, &relations, &input),
            Err(GenerationQualificationOperationContractError::RelationshipMismatch),
            "accepted substituted {field}"
        );
    }

    for (field, replacement) in [
        ("phase", Value::String("repeatability".to_owned())),
        (
            "checkpoint",
            Value::String("evidence_compilation".to_owned()),
        ),
        ("planned_attempt_id", Value::Null),
        ("reason", Value::String("cancelled".to_owned())),
    ] {
        let mut changed = serde_json::to_value(&record).expect("interruption value");
        changed[field] = replacement;
        assert_eq!(
            decode_value(&changed, &relations, &input),
            Err(GenerationQualificationOperationContractError::RelationshipMismatch),
            "accepted substituted runner input {field}"
        );
    }
}
