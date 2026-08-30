use super::*;

#[test]
fn peak_zero_terminal_and_negative_gate_matrix_is_exact() {
    let fixture = OperationFixture::new();
    let positive_platform = fixture.platform(GenerationQualificationPlatformStatusV1::Supported);
    let positive_license = fixture.license(GenerationQualificationLicenseDecisionV1::LocalUseOnly);
    let phases = fixture.skipped_phases();
    let positive = receipt_relations(&fixture, &positive_platform, &positive_license, &phases);
    for terminal in [
        GenerationQualificationOperationTerminalStatusV1::Cancelled,
        GenerationQualificationOperationTerminalStatusV1::Failed,
    ] {
        GenerationQualificationOperationReceiptV1::new(
            positive,
            receipt_input(
                0,
                terminal,
                GenerationQualificationOperationFinalizationStatusV1::NotRequired,
                1,
            ),
        )
        .expect("preacquisition terminal");
    }
    assert_eq!(
        GenerationQualificationOperationReceiptV1::new(
            positive,
            receipt_input(
                0,
                GenerationQualificationOperationTerminalStatusV1::Completed,
                GenerationQualificationOperationFinalizationStatusV1::NotRequired,
                1,
            ),
        ),
        Err(GenerationQualificationOperationContractError::FinalizationMismatch)
    );

    let rejected_platform = fixture.platform(GenerationQualificationPlatformStatusV1::Rejected);
    let rejected_license = fixture.license(GenerationQualificationLicenseDecisionV1::Rejected);
    for relations in [
        receipt_relations(&fixture, &rejected_platform, &positive_license, &phases),
        receipt_relations(&fixture, &positive_platform, &rejected_license, &phases),
        receipt_relations(&fixture, &rejected_platform, &rejected_license, &phases),
    ] {
        GenerationQualificationOperationReceiptV1::new(
            relations,
            receipt_input(
                0,
                GenerationQualificationOperationTerminalStatusV1::Completed,
                GenerationQualificationOperationFinalizationStatusV1::NotRequired,
                1,
            ),
        )
        .expect("policy-directed negative gate");
    }

    for finalization in [
        GenerationQualificationOperationFinalizationStatusV1::Passed,
        GenerationQualificationOperationFinalizationStatusV1::Failed,
    ] {
        assert_eq!(
            GenerationQualificationOperationReceiptV1::new(
                positive,
                receipt_input(
                    0,
                    GenerationQualificationOperationTerminalStatusV1::Failed,
                    finalization,
                    1,
                ),
            ),
            Err(GenerationQualificationOperationContractError::FinalizationMismatch)
        );
    }
}
#[test]
fn peak_one_finalization_matrix_and_crossed_gates_are_exact() {
    let fixture = OperationFixture::new();
    let positive_platform = fixture.platform(GenerationQualificationPlatformStatusV1::Supported);
    let positive_license = fixture.license(GenerationQualificationLicenseDecisionV1::LocalUseOnly);
    let phases = fixture.failed_ledger_phases();
    let positive = receipt_relations(&fixture, &positive_platform, &positive_license, &phases);

    GenerationQualificationOperationReceiptV1::new(
        positive,
        receipt_input(
            1,
            GenerationQualificationOperationTerminalStatusV1::Completed,
            GenerationQualificationOperationFinalizationStatusV1::Passed,
            1,
        ),
    )
    .expect("completed finalized terminal");
    assert_eq!(
        GenerationQualificationOperationReceiptV1::new(
            positive,
            receipt_input(
                1,
                GenerationQualificationOperationTerminalStatusV1::Completed,
                GenerationQualificationOperationFinalizationStatusV1::Failed,
                1,
            ),
        ),
        Err(GenerationQualificationOperationContractError::FinalizationMismatch)
    );
    for terminal in [
        GenerationQualificationOperationTerminalStatusV1::Cancelled,
        GenerationQualificationOperationTerminalStatusV1::Failed,
    ] {
        for finalization in [
            GenerationQualificationOperationFinalizationStatusV1::Passed,
            GenerationQualificationOperationFinalizationStatusV1::Failed,
        ] {
            GenerationQualificationOperationReceiptV1::new(
                positive,
                receipt_input(1, terminal, finalization, 1),
            )
            .expect("noncompleted managed terminal");
        }
    }
    assert_eq!(
        GenerationQualificationOperationReceiptV1::new(
            positive,
            receipt_input(
                2,
                GenerationQualificationOperationTerminalStatusV1::Failed,
                GenerationQualificationOperationFinalizationStatusV1::Failed,
                1,
            ),
        ),
        Err(GenerationQualificationOperationContractError::InvalidTerminalClosure)
    );
    assert_eq!(
        GenerationQualificationOperationReceiptV1::new(
            positive,
            receipt_input(
                1,
                GenerationQualificationOperationTerminalStatusV1::Failed,
                GenerationQualificationOperationFinalizationStatusV1::NotRequired,
                1,
            ),
        ),
        Err(GenerationQualificationOperationContractError::FinalizationMismatch)
    );

    let rejected_platform = fixture.platform(GenerationQualificationPlatformStatusV1::Rejected);
    let rejected_license = fixture.license(GenerationQualificationLicenseDecisionV1::Rejected);
    for relations in [
        receipt_relations(&fixture, &rejected_platform, &positive_license, &phases),
        receipt_relations(&fixture, &positive_platform, &rejected_license, &phases),
    ] {
        for finalization in [
            GenerationQualificationOperationFinalizationStatusV1::Passed,
            GenerationQualificationOperationFinalizationStatusV1::Failed,
        ] {
            assert_eq!(
                GenerationQualificationOperationReceiptV1::new(
                    relations,
                    receipt_input(
                        1,
                        GenerationQualificationOperationTerminalStatusV1::Failed,
                        finalization,
                        1,
                    ),
                ),
                Err(GenerationQualificationOperationContractError::FinalizationMismatch)
            );
        }
    }
}

#[test]
fn completed_requires_a_policy_directed_phase_terminal_but_other_terminals_may_abort() {
    let fixture = OperationFixture::new();
    let platform = fixture.platform(GenerationQualificationPlatformStatusV1::Supported);
    let license = fixture.license(GenerationQualificationLicenseDecisionV1::LocalUseOnly);
    let phases = fixture.passed_ledger_then_skipped_phases();
    let relations = receipt_relations(&fixture, &platform, &license, &phases);
    assert_eq!(
        GenerationQualificationOperationReceiptV1::new(
            relations,
            receipt_input(
                1,
                GenerationQualificationOperationTerminalStatusV1::Completed,
                GenerationQualificationOperationFinalizationStatusV1::Passed,
                1,
            ),
        ),
        Err(GenerationQualificationOperationContractError::InvalidTerminalClosure)
    );
    for terminal in [
        GenerationQualificationOperationTerminalStatusV1::Cancelled,
        GenerationQualificationOperationTerminalStatusV1::Failed,
    ] {
        GenerationQualificationOperationReceiptV1::new(
            relations,
            receipt_input(
                1,
                terminal,
                GenerationQualificationOperationFinalizationStatusV1::Passed,
                1,
            ),
        )
        .expect("noncompleted phase abort");
    }
}

#[test]
fn invalid_phase_progression_is_rejected() {
    let fixture = OperationFixture::new();
    let platform = fixture.platform(GenerationQualificationPlatformStatusV1::Supported);
    let license = fixture.license(GenerationQualificationLicenseDecisionV1::LocalUseOnly);
    for statuses in [
        (
            GenerationQualificationPhaseStatusV1::Skipped,
            GenerationQualificationPhaseStatusV1::Skipped,
            GenerationQualificationPhaseStatusV1::Passed,
            GenerationQualificationPhaseStatusV1::Skipped,
        ),
        (
            GenerationQualificationPhaseStatusV1::Failed,
            GenerationQualificationPhaseStatusV1::Skipped,
            GenerationQualificationPhaseStatusV1::Failed,
            GenerationQualificationPhaseStatusV1::Skipped,
        ),
        (
            GenerationQualificationPhaseStatusV1::Failed,
            GenerationQualificationPhaseStatusV1::Failed,
            GenerationQualificationPhaseStatusV1::Skipped,
            GenerationQualificationPhaseStatusV1::Passed,
        ),
    ] {
        let phase_set = phases(
            &fixture.base,
            statuses.0,
            statuses.1,
            statuses.2,
            statuses.3,
        );
        assert_eq!(
            GenerationQualificationOperationReceiptV1::new(
                receipt_relations(&fixture, &platform, &license, &phase_set),
                receipt_input(
                    1,
                    GenerationQualificationOperationTerminalStatusV1::Failed,
                    GenerationQualificationOperationFinalizationStatusV1::Passed,
                    1,
                ),
            ),
            Err(GenerationQualificationOperationContractError::InvalidTerminalClosure)
        );
    }
}

#[test]
fn changed_projection_or_gate_evidence_cannot_be_cross_joined() {
    let fixture = OperationFixture::new();
    let platform = fixture.platform(GenerationQualificationPlatformStatusV1::Supported);
    let license = fixture.license(GenerationQualificationLicenseDecisionV1::LocalUseOnly);
    let phases = fixture.failed_ledger_phases();
    let mut changed_inputs = fixture.request_inputs.clone();
    changed_inputs[0].structured_completion_request_binding_id =
        StructuredCompletionRequestBindingId::from_derived_digest(digest("changed request"));
    let changed_projection = GenerationQualificationRequestProjectionV1::new(
        projection_relations(&fixture.base),
        &changed_inputs,
    )
    .expect("changed projection");
    assert_eq!(
        GenerationQualificationOperationReceiptV1::new(
            GenerationQualificationOperationReceiptV1Relations {
                request_projection: &changed_projection,
                ..receipt_relations(&fixture, &platform, &license, &phases)
            },
            receipt_input(
                1,
                GenerationQualificationOperationTerminalStatusV1::Failed,
                GenerationQualificationOperationFinalizationStatusV1::Passed,
                1,
            ),
        ),
        Err(GenerationQualificationOperationContractError::ScopeMismatch)
    );

    let rejected_platform = fixture.platform(GenerationQualificationPlatformStatusV1::Rejected);
    let input = receipt_input(
        0,
        GenerationQualificationOperationTerminalStatusV1::Completed,
        GenerationQualificationOperationFinalizationStatusV1::NotRequired,
        1,
    );
    let rejected_phases = fixture.skipped_phases();
    let rejected_relations =
        receipt_relations(&fixture, &rejected_platform, &license, &rejected_phases);
    let changed = GenerationQualificationOperationReceiptV1::new(rejected_relations, input)
        .expect("rejected receipt");
    let positive_platform = fixture.platform(GenerationQualificationPlatformStatusV1::Supported);
    let skipped = fixture.skipped_phases();
    assert_eq!(
        GenerationQualificationOperationReceiptV1::from_json_bytes(
            &serde_json::to_vec(&changed).expect("changed receipt JSON"),
            receipt_relations(&fixture, &positive_platform, &license, &skipped),
            receipt_input(
                0,
                GenerationQualificationOperationTerminalStatusV1::Failed,
                GenerationQualificationOperationFinalizationStatusV1::NotRequired,
                1,
            ),
        ),
        Err(GenerationQualificationOperationContractError::RelationshipMismatch)
    );
}

#[test]
fn debug_output_is_content_free() {
    let fixture = OperationFixture::new();
    let platform = fixture.platform(GenerationQualificationPlatformStatusV1::Supported);
    let license = fixture.license(GenerationQualificationLicenseDecisionV1::LocalUseOnly);
    let phases = fixture.failed_ledger_phases();
    let receipt = GenerationQualificationOperationReceiptV1::new(
        receipt_relations(&fixture, &platform, &license, &phases),
        receipt_input(
            1,
            GenerationQualificationOperationTerminalStatusV1::Completed,
            GenerationQualificationOperationFinalizationStatusV1::Passed,
            1,
        ),
    )
    .expect("receipt");
    let rendered = format!("{receipt:?}");
    assert!(!rendered.contains(platform.platform_evidence_id().digest().as_str()));
    assert!(!rendered.contains(license.license_evidence_id().digest().as_str()));
    assert_eq!(
        GenerationQualificationOperationContractError::FinalizationMismatch.to_string(),
        "generation qualification operation finalization does not match"
    );
}
