use super::{OperationFixture, receipt_input, receipt_relations};
use crate::generation_qualification::{
    GenerationQualificationLicenseDecisionV1, GenerationQualificationOperationContractError,
    GenerationQualificationOperationFinalizationStatusV1,
    GenerationQualificationOperationTerminalStatusV1, GenerationQualificationPhaseCheckpointV1,
    GenerationQualificationPhaseInterruptionReasonV1,
    GenerationQualificationPhaseInterruptionRecordV1Input, GenerationQualificationPlatformStatusV1,
    GenerationQualificationTerminalInterruptionPlan, GenerationQualificationTerminalSetPlan,
    PlannedGenerationQualificationTerminalSet,
};

#[test]
fn peak_zero_completed_gate_has_no_interruption_and_a_supplied_one_is_refused() {
    let fixture = OperationFixture::new();
    let platform = fixture.platform(GenerationQualificationPlatformStatusV1::Rejected);
    let license = fixture.license(GenerationQualificationLicenseDecisionV1::LocalUseOnly);
    let phases = fixture.skipped_phases();
    let relations = receipt_relations(&fixture, &platform, &license, &phases);
    let input = receipt_input(
        0,
        GenerationQualificationOperationTerminalStatusV1::Completed,
        GenerationQualificationOperationFinalizationStatusV1::NotRequired,
        1,
    );
    let planned =
        PlannedGenerationQualificationTerminalSet::plan(plan(&fixture, relations, input, false))
            .expect("negative gate");
    assert!(planned.interruption().is_none());
    assert_eq!(planned.receipt().terminal_status(), input.terminal_status);
    assert_eq!(
        PlannedGenerationQualificationTerminalSet::plan(plan(&fixture, relations, input, true)),
        Err(GenerationQualificationOperationContractError::InvalidInterruptionClosure)
    );
}

#[test]
fn peak_zero_noncompleted_receipt_may_omit_or_bind_a_matching_interruption() {
    let fixture = OperationFixture::new();
    let platform = fixture.platform(GenerationQualificationPlatformStatusV1::Supported);
    let license = fixture.license(GenerationQualificationLicenseDecisionV1::LocalUseOnly);
    let phases = fixture.skipped_phases();
    let relations = receipt_relations(&fixture, &platform, &license, &phases);
    let input = receipt_input(
        0,
        GenerationQualificationOperationTerminalStatusV1::Cancelled,
        GenerationQualificationOperationFinalizationStatusV1::NotRequired,
        1,
    );
    let omitted =
        PlannedGenerationQualificationTerminalSet::plan(plan(&fixture, relations, input, false))
            .expect("policy denial style closure");
    assert!(omitted.interruption().is_none());
    let bound =
        PlannedGenerationQualificationTerminalSet::plan(plan(&fixture, relations, input, true))
            .expect("cancelled interruption");
    let interruption = bound.interruption().expect("bound interruption");
    assert_eq!(
        interruption.operation_receipt_id(),
        bound.receipt().operation_receipt_id()
    );
    assert_eq!(
        interruption.reason(),
        GenerationQualificationPhaseInterruptionReasonV1::Cancelled
    );
}

#[test]
fn peak_one_matrix_binds_interruption_only_to_a_noncompleted_receipt() {
    let fixture = OperationFixture::new();
    let platform = fixture.platform(GenerationQualificationPlatformStatusV1::Supported);
    let license = fixture.license(GenerationQualificationLicenseDecisionV1::LocalUseOnly);
    let phases = fixture.failed_ledger_phases();
    let relations = receipt_relations(&fixture, &platform, &license, &phases);
    let completed = receipt_input(
        1,
        GenerationQualificationOperationTerminalStatusV1::Completed,
        GenerationQualificationOperationFinalizationStatusV1::Passed,
        1,
    );
    let failed = receipt_input(
        1,
        GenerationQualificationOperationTerminalStatusV1::Failed,
        GenerationQualificationOperationFinalizationStatusV1::Failed,
        1,
    );
    assert!(
        PlannedGenerationQualificationTerminalSet::plan(plan(
            &fixture, relations, completed, false
        ))
        .expect("finalized completion")
        .interruption()
        .is_none()
    );
    assert_eq!(
        PlannedGenerationQualificationTerminalSet::plan(plan(&fixture, relations, completed, true)),
        Err(GenerationQualificationOperationContractError::InvalidInterruptionClosure)
    );
    let planned =
        PlannedGenerationQualificationTerminalSet::plan(plan(&fixture, relations, failed, true))
            .expect("failed finalization");
    assert_eq!(
        planned
            .interruption()
            .expect("failed interruption")
            .operation_receipt_id(),
        planned.receipt().operation_receipt_id()
    );
    assert_eq!(
        PlannedGenerationQualificationTerminalSet::plan(GenerationQualificationTerminalSetPlan {
            receipt_relations: relations,
            receipt_input: failed,
            interruption: Some(interruption_plan(
                &fixture,
                GenerationQualificationPhaseInterruptionReasonV1::Cancelled,
            )),
        }),
        Err(GenerationQualificationOperationContractError::InvalidInterruptionClosure)
    );
}

fn plan<'a>(
    fixture: &'a OperationFixture,
    receipt_relations: crate::generation_qualification::GenerationQualificationOperationReceiptV1Relations<'a>,
    receipt_input: crate::generation_qualification::GenerationQualificationOperationReceiptV1Input,
    include_interruption: bool,
) -> GenerationQualificationTerminalSetPlan<'a> {
    let reason = match receipt_input.terminal_status {
        GenerationQualificationOperationTerminalStatusV1::Cancelled => {
            GenerationQualificationPhaseInterruptionReasonV1::Cancelled
        }
        GenerationQualificationOperationTerminalStatusV1::DeadlineExceeded => {
            GenerationQualificationPhaseInterruptionReasonV1::DeadlineExceeded
        }
        GenerationQualificationOperationTerminalStatusV1::Failed
        | GenerationQualificationOperationTerminalStatusV1::Completed => {
            GenerationQualificationPhaseInterruptionReasonV1::AuthorityDrift
        }
    };
    GenerationQualificationTerminalSetPlan {
        receipt_relations,
        receipt_input,
        interruption: include_interruption.then(|| interruption_plan(fixture, reason)),
    }
}

fn interruption_plan(
    fixture: &OperationFixture,
    reason: GenerationQualificationPhaseInterruptionReasonV1,
) -> GenerationQualificationTerminalInterruptionPlan<'_> {
    GenerationQualificationTerminalInterruptionPlan {
        operation_policy_relations: fixture.base.relations(),
        operation_policy_input: &fixture.base.policy_input,
        input: GenerationQualificationPhaseInterruptionRecordV1Input {
            phase: crate::generation_qualification::GenerationQualificationInterruptedPhaseV1::AttemptLedger,
            checkpoint: GenerationQualificationPhaseCheckpointV1::EvidenceAcquisition,
            planned_attempt_id: Some(fixture.base.attempts[0].planned_attempt_id().clone()),
            reason,
        },
    }
}
