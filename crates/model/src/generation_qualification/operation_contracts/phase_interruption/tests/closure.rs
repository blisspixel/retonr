use super::*;

#[test]
fn terminal_reason_status_compatibility_is_exact_and_completed_is_forbidden() {
    let fixture = OperationFixture::new();
    let platform = fixture.platform();
    let license = fixture.license();
    let skipped = fixture.phases(GenerationQualificationPhaseStatusV1::Skipped);
    let skipped_relations = receipt_relations(&fixture, &platform, &license, &skipped);

    let cancelled_input = receipt_input(
        1,
        0,
        GenerationQualificationOperationTerminalStatusV1::Cancelled,
        GenerationQualificationOperationFinalizationStatusV1::NotRequired,
    );
    let cancelled_receipt =
        GenerationQualificationOperationReceiptV1::new(skipped_relations, cancelled_input)
            .expect("cancelled receipt");
    let cancelled_relations = interruption_relations(
        &fixture,
        &cancelled_receipt,
        skipped_relations,
        cancelled_input,
    );
    GenerationQualificationPhaseInterruptionRecordV1::new(
        &cancelled_relations,
        interruption_input(
            &fixture,
            GenerationQualificationPhaseCheckpointV1::EvidenceAcquisition,
            GenerationQualificationPhaseInterruptionReasonV1::Cancelled,
        ),
    )
    .expect("matching cancelled interruption");
    assert_eq!(
        GenerationQualificationPhaseInterruptionRecordV1::new(
            &cancelled_relations,
            interruption_input(
                &fixture,
                GenerationQualificationPhaseCheckpointV1::EvidenceAcquisition,
                GenerationQualificationPhaseInterruptionReasonV1::AuthorityDrift,
            ),
        ),
        Err(GenerationQualificationOperationContractError::InvalidInterruptionClosure)
    );

    let deadline_nanoseconds =
        u64::from(fixture.base.policy.limits().maximum_elapsed_milliseconds()) * 1_000_000;
    let deadline_input = receipt_input(
        deadline_nanoseconds,
        0,
        GenerationQualificationOperationTerminalStatusV1::DeadlineExceeded,
        GenerationQualificationOperationFinalizationStatusV1::NotRequired,
    );
    let deadline_receipt =
        GenerationQualificationOperationReceiptV1::new(skipped_relations, deadline_input)
            .expect("deadline receipt");
    GenerationQualificationPhaseInterruptionRecordV1::new(
        &interruption_relations(
            &fixture,
            &deadline_receipt,
            skipped_relations,
            deadline_input,
        ),
        interruption_input(
            &fixture,
            GenerationQualificationPhaseCheckpointV1::EvidenceAcquisition,
            GenerationQualificationPhaseInterruptionReasonV1::DeadlineExceeded,
        ),
    )
    .expect("matching deadline interruption");

    let failed = fixture.phases(GenerationQualificationPhaseStatusV1::Failed);
    let failed_relations = receipt_relations(&fixture, &platform, &license, &failed);
    let completed_input = receipt_input(
        1,
        1,
        GenerationQualificationOperationTerminalStatusV1::Completed,
        GenerationQualificationOperationFinalizationStatusV1::Passed,
    );
    let completed_receipt =
        GenerationQualificationOperationReceiptV1::new(failed_relations, completed_input)
            .expect("completed receipt");
    assert_eq!(
        GenerationQualificationPhaseInterruptionRecordV1::new(
            &interruption_relations(
                &fixture,
                &completed_receipt,
                failed_relations,
                completed_input,
            ),
            interruption_input(
                &fixture,
                GenerationQualificationPhaseCheckpointV1::EvidenceCompilation,
                GenerationQualificationPhaseInterruptionReasonV1::EvidenceCompilationFailed,
            ),
        ),
        Err(GenerationQualificationOperationContractError::InvalidInterruptionClosure)
    );
}

#[test]
fn recursive_policy_receipt_and_complete_plan_closures_reject_substitution() {
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
    GenerationQualificationPhaseInterruptionRecordV1::new(&relations, input.clone())
        .expect("valid recursive closure");

    let mut changed_policy_input = fixture.base.policy_input.clone();
    changed_policy_input.attempt_ledger_policy_digest = digest("foreign ledger policy");
    assert!(
        GenerationQualificationPhaseInterruptionRecordV1::new(
            &GenerationQualificationPhaseInterruptionRecordV1Relations {
                operation_policy_input: &changed_policy_input,
                ..relations
            },
            input.clone(),
        )
        .is_err()
    );
    assert!(
        GenerationQualificationPhaseInterruptionRecordV1::new(
            &GenerationQualificationPhaseInterruptionRecordV1Relations {
                operation_receipt_input: GenerationQualificationOperationReceiptV1Input {
                    elapsed_nanoseconds: 2,
                    ..receipt_input
                },
                ..relations
            },
            input.clone(),
        )
        .is_err()
    );

    assert_plan_attempt_closure(&fixture, &relations, &input);

    let before_input = GenerationQualificationPhaseInterruptionRecordV1Input {
        phase: GenerationQualificationInterruptedPhaseV1::AttemptLedger,
        checkpoint: GenerationQualificationPhaseCheckpointV1::BeforePhase,
        planned_attempt_id: None,
        reason: GenerationQualificationPhaseInterruptionReasonV1::AuthorityDrift,
    };
    GenerationQualificationPhaseInterruptionRecordV1::new(&relations, before_input.clone())
        .expect("phase-wide checkpoint without attempt");
    assert_eq!(
        GenerationQualificationPhaseInterruptionRecordV1::new(
            &relations,
            GenerationQualificationPhaseInterruptionRecordV1Input {
                planned_attempt_id: Some(fixture.base.attempts[0].planned_attempt_id().clone(),),
                ..before_input
            },
        ),
        Err(GenerationQualificationOperationContractError::InvalidInterruptionClosure)
    );
}

fn assert_plan_attempt_closure(
    fixture: &OperationFixture,
    relations: &GenerationQualificationPhaseInterruptionRecordV1Relations<'_>,
    input: &GenerationQualificationPhaseInterruptionRecordV1Input,
) {
    let foreign_attempt_id: PlannedCandidateAttemptId =
        serde_json::from_value(Value::String(digest("foreign planned attempt").to_string()))
            .expect("typed foreign attempt ID");
    assert_eq!(
        GenerationQualificationPhaseInterruptionRecordV1::new(
            relations,
            GenerationQualificationPhaseInterruptionRecordV1Input {
                planned_attempt_id: Some(foreign_attempt_id),
                ..input.clone()
            },
        ),
        Err(GenerationQualificationOperationContractError::RelationshipMismatch)
    );
    assert_eq!(
        GenerationQualificationPhaseInterruptionRecordV1::new(
            relations,
            GenerationQualificationPhaseInterruptionRecordV1Input {
                planned_attempt_id: None,
                ..input.clone()
            },
        ),
        Err(GenerationQualificationOperationContractError::InvalidInterruptionClosure)
    );

    let mut reordered_attempts = fixture.base.attempts.clone();
    reordered_attempts.swap(0, 1);
    assert!(
        GenerationQualificationPhaseInterruptionRecordV1::new(
            &GenerationQualificationPhaseInterruptionRecordV1Relations {
                operation_policy_relations: GenerationQualificationOperationPolicyV1Relations {
                    planned_attempts: &reordered_attempts,
                    ..fixture.base.relations()
                },
                ..*relations
            },
            input.clone(),
        )
        .is_err()
    );
    assert!(
        GenerationQualificationPhaseInterruptionRecordV1::new(
            &GenerationQualificationPhaseInterruptionRecordV1Relations {
                operation_policy_relations: GenerationQualificationOperationPolicyV1Relations {
                    planned_attempts: &fixture.base.attempts[..fixture.base.attempts.len() - 1],
                    ..fixture.base.relations()
                },
                ..*relations
            },
            input.clone(),
        )
        .is_err()
    );
}

#[test]
fn resource_attempts_are_target_only_and_policy_digests_are_phase_derived() {
    let fixture = OperationFixture::new();
    let baseline_attempt = fixture
        .base
        .attempts
        .iter()
        .find(|attempt| {
            attempt.generation_system_id() == fixture.base.policy.baseline_generation_system_id()
        })
        .expect("baseline attempt");
    let target_attempt = fixture
        .base
        .attempts
        .iter()
        .find(|attempt| {
            attempt.generation_system_id() == fixture.base.policy.target_generation_system_id()
        })
        .expect("target attempt");
    assert_ne!(
        target_attempt.planned_attempt_id(),
        baseline_attempt.planned_attempt_id()
    );

    for (phase, expected) in [
        (
            GenerationQualificationInterruptedPhaseV1::AttemptLedger,
            fixture.base.policy.attempt_ledger_policy_digest(),
        ),
        (
            GenerationQualificationInterruptedPhaseV1::Repeatability,
            fixture.base.policy.repeatability_policy_digest(),
        ),
        (
            GenerationQualificationInterruptedPhaseV1::ResourceEvidence,
            fixture.base.policy.resource_policy_digest(),
        ),
        (
            GenerationQualificationInterruptedPhaseV1::HumanAdjudication,
            fixture.base.policy.human_adjudication_policy_digest(),
        ),
    ] {
        assert_eq!(phase_policy_digest(&fixture.base.policy, phase), expected);
    }

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
    assert_eq!(
        GenerationQualificationPhaseInterruptionRecordV1::new(
            &relations,
            GenerationQualificationPhaseInterruptionRecordV1Input {
                phase: GenerationQualificationInterruptedPhaseV1::ResourceEvidence,
                checkpoint: GenerationQualificationPhaseCheckpointV1::EvidenceAcquisition,
                planned_attempt_id: Some(baseline_attempt.planned_attempt_id().clone()),
                reason:
                    GenerationQualificationPhaseInterruptionReasonV1::RequiredObservationMissing,
            },
        ),
        Err(GenerationQualificationOperationContractError::RelationshipMismatch)
    );
}

#[test]
fn mandatory_finalization_preserves_primary_precedence_and_independent_cleanup() {
    let fixture = OperationFixture::new();
    let platform = fixture.platform();
    let license = fixture.license();
    let phases = fixture.phases(GenerationQualificationPhaseStatusV1::Skipped);
    let skipped_receipt_relations = receipt_relations(&fixture, &platform, &license, &phases);

    let cleanup_input = receipt_input(
        1,
        1,
        GenerationQualificationOperationTerminalStatusV1::Failed,
        GenerationQualificationOperationFinalizationStatusV1::Failed,
    );
    let cleanup_receipt =
        GenerationQualificationOperationReceiptV1::new(skipped_receipt_relations, cleanup_input)
            .expect("cleanup-failed receipt");
    let cleanup_relations = interruption_relations(
        &fixture,
        &cleanup_receipt,
        skipped_receipt_relations,
        cleanup_input,
    );
    GenerationQualificationPhaseInterruptionRecordV1::new(
        &cleanup_relations,
        interruption_input(
            &fixture,
            GenerationQualificationPhaseCheckpointV1::MandatoryFinalization,
            GenerationQualificationPhaseInterruptionReasonV1::CleanupFailed,
        ),
    )
    .expect("cleanup interruption");
    GenerationQualificationPhaseInterruptionRecordV1::new(
        &cleanup_relations,
        interruption_input(
            &fixture,
            GenerationQualificationPhaseCheckpointV1::MandatoryFinalization,
            GenerationQualificationPhaseInterruptionReasonV1::AuthorityDrift,
        ),
    )
    .expect("final package drift");

    let failed_phases = fixture.phases(GenerationQualificationPhaseStatusV1::Failed);
    let failed_receipt_relations = receipt_relations(&fixture, &platform, &license, &failed_phases);
    let failed_receipt =
        GenerationQualificationOperationReceiptV1::new(failed_receipt_relations, cleanup_input)
            .expect("retained failed-ledger receipt");
    GenerationQualificationPhaseInterruptionRecordV1::new(
        &interruption_relations(
            &fixture,
            &failed_receipt,
            failed_receipt_relations,
            cleanup_input,
        ),
        interruption_input(
            &fixture,
            GenerationQualificationPhaseCheckpointV1::MandatoryFinalization,
            GenerationQualificationPhaseInterruptionReasonV1::CleanupFailed,
        ),
    )
    .expect("retained failed attempt ledger remains valid interruption evidence");
}

#[test]
fn mandatory_finalization_keeps_deadline_primary_after_successful_cleanup() {
    let fixture = OperationFixture::new();
    let platform = fixture.platform();
    let license = fixture.license();
    let phases = fixture.phases(GenerationQualificationPhaseStatusV1::Skipped);
    let skipped_receipt_relations = receipt_relations(&fixture, &platform, &license, &phases);

    let deadline_nanoseconds =
        u64::from(fixture.base.policy.limits().maximum_elapsed_milliseconds()) * 1_000_000;
    let deadline_input = receipt_input(
        deadline_nanoseconds,
        1,
        GenerationQualificationOperationTerminalStatusV1::DeadlineExceeded,
        GenerationQualificationOperationFinalizationStatusV1::Passed,
    );
    let deadline_receipt =
        GenerationQualificationOperationReceiptV1::new(skipped_receipt_relations, deadline_input)
            .expect("post-cleanup deadline receipt");
    GenerationQualificationPhaseInterruptionRecordV1::new(
        &interruption_relations(
            &fixture,
            &deadline_receipt,
            skipped_receipt_relations,
            deadline_input,
        ),
        interruption_input(
            &fixture,
            GenerationQualificationPhaseCheckpointV1::MandatoryFinalization,
            GenerationQualificationPhaseInterruptionReasonV1::DeadlineExceeded,
        ),
    )
    .expect("deadline preserves passed cleanup");

    let passed_cleanup_input = receipt_input(
        1,
        1,
        GenerationQualificationOperationTerminalStatusV1::Failed,
        GenerationQualificationOperationFinalizationStatusV1::Passed,
    );
    let passed_cleanup_receipt = GenerationQualificationOperationReceiptV1::new(
        skipped_receipt_relations,
        passed_cleanup_input,
    )
    .expect("passed cleanup receipt");
    let passed_cleanup_relations = interruption_relations(
        &fixture,
        &passed_cleanup_receipt,
        skipped_receipt_relations,
        passed_cleanup_input,
    );
    for reason in [
        GenerationQualificationPhaseInterruptionReasonV1::CleanupFailed,
        GenerationQualificationPhaseInterruptionReasonV1::AuthorityDrift,
    ] {
        assert_eq!(
            GenerationQualificationPhaseInterruptionRecordV1::new(
                &passed_cleanup_relations,
                interruption_input(
                    &fixture,
                    GenerationQualificationPhaseCheckpointV1::MandatoryFinalization,
                    reason,
                ),
            ),
            Err(GenerationQualificationOperationContractError::InvalidInterruptionClosure)
        );
    }
}

#[test]
fn manifest_compilation_failure_is_representable_without_fabricating_resource_failure() {
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

    GenerationQualificationPhaseInterruptionRecordV1::new(
        &relations,
        GenerationQualificationPhaseInterruptionRecordV1Input {
            phase: GenerationQualificationInterruptedPhaseV1::AttemptLedger,
            checkpoint: GenerationQualificationPhaseCheckpointV1::ManifestCompilation,
            planned_attempt_id: None,
            reason: GenerationQualificationPhaseInterruptionReasonV1::EvidenceCompilationFailed,
        },
    )
    .expect("manifest compilation interruption");
    assert_eq!(
        GenerationQualificationPhaseInterruptionRecordV1::new(
            &relations,
            GenerationQualificationPhaseInterruptionRecordV1Input {
                phase: GenerationQualificationInterruptedPhaseV1::AttemptLedger,
                checkpoint: GenerationQualificationPhaseCheckpointV1::BeforePhase,
                planned_attempt_id: None,
                reason: GenerationQualificationPhaseInterruptionReasonV1::EvidenceCompilationFailed,
            },
        ),
        Err(GenerationQualificationOperationContractError::InvalidInterruptionClosure)
    );
}
