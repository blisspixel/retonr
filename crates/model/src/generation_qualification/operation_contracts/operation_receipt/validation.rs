use super::super::{
    GenerationQualificationLicenseDecisionV1, GenerationQualificationOperationContractError,
    GenerationQualificationPlatformStatusV1,
};
use super::{
    GenerationQualificationOperationFinalizationStatusV1, GenerationQualificationOperationPolicyV1,
    GenerationQualificationOperationReceiptV1Input,
    GenerationQualificationOperationReceiptV1Relations,
    GenerationQualificationOperationTerminalStatusV1,
};
use crate::generation_qualification::GenerationQualificationPhaseStatusV1;

pub(super) fn validate_scope(
    relations: GenerationQualificationOperationReceiptV1Relations<'_>,
) -> Result<(), GenerationQualificationOperationContractError> {
    let policy = relations.operation_policy;
    let projection = relations.request_projection;
    let target = policy.target_generation_system_id();
    let plan = policy.generation_qualification_plan_id();
    let suite = policy.suite_manifest_id();
    if projection.operation_policy_id() != policy.operation_policy_id()
        || projection.target_generation_system_id() != target
        || projection.baseline_generation_system_id() != policy.baseline_generation_system_id()
        || projection.generation_qualification_plan_id() != plan
        || projection.suite_manifest_id() != suite
        || relations.platform_evidence.operation_policy_id() != policy.operation_policy_id()
        || relations.platform_evidence.request_projection_id() != projection.request_projection_id()
        || relations.platform_evidence.target_generation_system_id() != target
        || relations.license_evidence.operation_policy_id() != policy.operation_policy_id()
        || relations.license_evidence.request_projection_id() != projection.request_projection_id()
        || relations.license_evidence.target_generation_system_id() != target
        || relations.attempt_ledger_manifest.generation_system_id() != target
        || relations.repeatability_manifest.generation_system_id() != target
        || relations.resource_manifest.generation_system_id() != target
        || relations.human_adjudication_manifest.generation_system_id() != target
        || relations
            .attempt_ledger_manifest
            .generation_qualification_plan_id()
            != plan
        || relations
            .repeatability_manifest
            .generation_qualification_plan_id()
            != plan
        || relations
            .resource_manifest
            .generation_qualification_plan_id()
            != plan
        || relations
            .human_adjudication_manifest
            .generation_qualification_plan_id()
            != plan
        || relations.attempt_ledger_manifest.suite_manifest_id() != suite
        || relations.repeatability_manifest.suite_manifest_id() != suite
        || relations.resource_manifest.suite_manifest_id() != suite
        || relations.human_adjudication_manifest.suite_manifest_id() != suite
        || relations.attempt_ledger_manifest.phase_policy_digest()
            != policy.attempt_ledger_policy_digest()
        || relations.repeatability_manifest.phase_policy_digest()
            != policy.repeatability_policy_digest()
        || relations.resource_manifest.phase_policy_digest() != policy.resource_policy_digest()
        || relations.human_adjudication_manifest.phase_policy_digest()
            != policy.human_adjudication_policy_digest()
    {
        return Err(GenerationQualificationOperationContractError::ScopeMismatch);
    }
    Ok(())
}

pub(super) fn validate_phase_progression(
    relations: GenerationQualificationOperationReceiptV1Relations<'_>,
    input: GenerationQualificationOperationReceiptV1Input,
) -> Result<(), GenerationQualificationOperationContractError> {
    use GenerationQualificationPhaseStatusV1::{Failed, Passed, Skipped};
    let ledger = relations.attempt_ledger_manifest.status();
    let repeatability = relations.repeatability_manifest.status();
    let resource = relations.resource_manifest.status();
    let human = relations.human_adjudication_manifest.status();
    let valid = match ledger {
        Skipped => repeatability == Skipped && resource == Skipped && human == Skipped,
        Failed => {
            matches!(repeatability, Failed | Skipped) && resource == Skipped && human == Skipped
        }
        Passed => match repeatability {
            Skipped | Failed => resource == Skipped && human == Skipped,
            Passed => match resource {
                Skipped | Failed => human == Skipped,
                Passed => matches!(human, Passed | Failed | Skipped),
            },
        },
    };
    let preacquisition = input.peak_concurrent_attempts == 0;
    let all_skipped = [ledger, repeatability, resource, human]
        .into_iter()
        .all(|status| status == Skipped);
    let completed_managed = input.terminal_status
        == GenerationQualificationOperationTerminalStatusV1::Completed
        && input.finalization_status
            == GenerationQualificationOperationFinalizationStatusV1::Passed;
    let completed_phase_closure = match ledger {
        Skipped => false,
        Failed => matches!(repeatability, Failed | Skipped),
        Passed => match repeatability {
            Skipped => false,
            Failed => true,
            Passed => match resource {
                Skipped => false,
                Failed => true,
                Passed => matches!(human, Passed | Failed),
            },
        },
    };
    if valid && (!preacquisition || all_skipped) && (!completed_managed || completed_phase_closure)
    {
        Ok(())
    } else {
        Err(GenerationQualificationOperationContractError::InvalidTerminalClosure)
    }
}

pub(super) fn validate_deadline(
    policy: &GenerationQualificationOperationPolicyV1,
    input: GenerationQualificationOperationReceiptV1Input,
) -> Result<(), GenerationQualificationOperationContractError> {
    let threshold = u64::from(policy.limits().maximum_elapsed_milliseconds())
        .checked_mul(1_000_000)
        .ok_or(GenerationQualificationOperationContractError::EncodingOverflow)?;
    let elapsed_deadline = input.elapsed_nanoseconds >= threshold;
    let terminal_deadline =
        input.terminal_status == GenerationQualificationOperationTerminalStatusV1::DeadlineExceeded;
    if elapsed_deadline == terminal_deadline {
        Ok(())
    } else {
        Err(GenerationQualificationOperationContractError::DeadlineMismatch)
    }
}

pub(super) fn validate_terminal(
    relations: GenerationQualificationOperationReceiptV1Relations<'_>,
    input: GenerationQualificationOperationReceiptV1Input,
) -> Result<(), GenerationQualificationOperationContractError> {
    use GenerationQualificationOperationFinalizationStatusV1::{Failed, NotRequired, Passed};
    use GenerationQualificationOperationTerminalStatusV1::Completed;
    if input.peak_concurrent_attempts > 1 {
        return Err(GenerationQualificationOperationContractError::InvalidTerminalClosure);
    }
    let platform_rejected =
        relations.platform_evidence.status() == GenerationQualificationPlatformStatusV1::Rejected;
    let license_rejected =
        relations.license_evidence.decision() == GenerationQualificationLicenseDecisionV1::Rejected;
    let positive_gates = !platform_rejected && !license_rejected;
    let valid = match (input.peak_concurrent_attempts, input.finalization_status) {
        (0, NotRequired) => {
            input.terminal_status != Completed || platform_rejected || license_rejected
        }
        (1, Passed | Failed) => {
            positive_gates
                && (input.terminal_status != Completed || input.finalization_status == Passed)
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(GenerationQualificationOperationContractError::FinalizationMismatch)
    }
}
