use rewrite_types::Digest;

use super::{
    GenerationQualificationInterruptedPhaseV1, GenerationQualificationOperationContractError,
    GenerationQualificationOperationPolicyV1, GenerationQualificationPhaseCheckpointV1,
    GenerationQualificationPhaseInterruptionReasonV1,
    GenerationQualificationPhaseInterruptionRecordV1Input,
    GenerationQualificationPhaseInterruptionRecordV1Relations,
};
use crate::generation_qualification::{
    GenerationQualificationOperationFinalizationStatusV1,
    GenerationQualificationOperationTerminalStatusV1, GenerationQualificationPhaseStatusV1,
    PlannedCandidateAttemptV1,
};

pub(super) fn validate_relations(
    relations: &GenerationQualificationPhaseInterruptionRecordV1Relations<'_>,
    input: &GenerationQualificationPhaseInterruptionRecordV1Input,
) -> Result<(), GenerationQualificationOperationContractError> {
    relations.operation_policy.validate_against(
        relations.operation_policy_relations,
        relations.operation_policy_input,
    )?;
    relations.operation_receipt.validate_against(
        relations.operation_receipt_relations,
        relations.operation_receipt_input,
    )?;
    validate_exact_closure(relations)?;
    validate_terminal(relations, input.reason)?;
    let planned_attempt = validate_planned_attempt(relations, input)?;
    validate_checkpoint_attempt(relations, input, planned_attempt)?;
    validate_manifest_progression(input.phase, input.checkpoint, phase_statuses(relations))?;
    validate_reason(relations, input)?;
    Ok(())
}

fn validate_exact_closure(
    relations: &GenerationQualificationPhaseInterruptionRecordV1Relations<'_>,
) -> Result<(), GenerationQualificationOperationContractError> {
    let policy = relations.operation_policy;
    let receipt = relations.operation_receipt;
    let receipt_policy = relations.operation_receipt_relations.operation_policy;
    if receipt.operation_policy_id() != policy.operation_policy_id()
        || receipt_policy != policy
        || receipt_policy.operation_policy_id() != policy.operation_policy_id()
        || receipt.target_generation_system_id() != policy.target_generation_system_id()
        || receipt.baseline_generation_system_id() != policy.baseline_generation_system_id()
        || relations
            .operation_policy_relations
            .plan
            .qualification_plan_id()
            != policy.generation_qualification_plan_id()
        || relations
            .operation_policy_relations
            .suite
            .suite_manifest_id()
            != policy.suite_manifest_id()
    {
        return Err(GenerationQualificationOperationContractError::RelationshipMismatch);
    }
    Ok(())
}

fn validate_terminal(
    relations: &GenerationQualificationPhaseInterruptionRecordV1Relations<'_>,
    reason: GenerationQualificationPhaseInterruptionReasonV1,
) -> Result<(), GenerationQualificationOperationContractError> {
    use GenerationQualificationOperationTerminalStatusV1::{
        Cancelled, Completed, DeadlineExceeded, Failed,
    };
    let terminal = relations.operation_receipt.terminal_status();
    let expected = match reason {
        GenerationQualificationPhaseInterruptionReasonV1::Cancelled => Cancelled,
        GenerationQualificationPhaseInterruptionReasonV1::DeadlineExceeded => DeadlineExceeded,
        GenerationQualificationPhaseInterruptionReasonV1::AuthorityDrift
        | GenerationQualificationPhaseInterruptionReasonV1::RequiredObservationMissing
        | GenerationQualificationPhaseInterruptionReasonV1::RequiredObservationInvalid
        | GenerationQualificationPhaseInterruptionReasonV1::MeasurementOverflow
        | GenerationQualificationPhaseInterruptionReasonV1::EvidenceCompilationFailed
        | GenerationQualificationPhaseInterruptionReasonV1::CleanupFailed => Failed,
    };
    if terminal == Completed || terminal != expected {
        Err(GenerationQualificationOperationContractError::InvalidInterruptionClosure)
    } else {
        Ok(())
    }
}

fn validate_planned_attempt<'a>(
    relations: &'a GenerationQualificationPhaseInterruptionRecordV1Relations<'a>,
    input: &GenerationQualificationPhaseInterruptionRecordV1Input,
) -> Result<Option<&'a PlannedCandidateAttemptV1>, GenerationQualificationOperationContractError> {
    let Some(expected_id) = input.planned_attempt_id.as_ref() else {
        return Ok(None);
    };
    let policy_relations = relations.operation_policy_relations;
    let index = policy_relations
        .plan
        .planned_attempt_ids()
        .iter()
        .position(|id| id == expected_id)
        .ok_or(GenerationQualificationOperationContractError::RelationshipMismatch)?;
    let attempt = policy_relations
        .planned_attempts
        .get(index)
        .ok_or(GenerationQualificationOperationContractError::RelationshipMismatch)?;
    if attempt.planned_attempt_id() != expected_id {
        return Err(GenerationQualificationOperationContractError::RelationshipMismatch);
    }
    Ok(Some(attempt))
}

fn validate_checkpoint_attempt(
    relations: &GenerationQualificationPhaseInterruptionRecordV1Relations<'_>,
    input: &GenerationQualificationPhaseInterruptionRecordV1Input,
    planned_attempt: Option<&PlannedCandidateAttemptV1>,
) -> Result<(), GenerationQualificationOperationContractError> {
    use GenerationQualificationInterruptedPhaseV1::{AttemptLedger, ResourceEvidence};
    use GenerationQualificationPhaseCheckpointV1::{
        EvidenceAcquisition, EvidenceCompilation, MandatoryFinalization,
    };

    let requires_attempt = matches!(
        (input.phase, input.checkpoint),
        (
            AttemptLedger | ResourceEvidence,
            EvidenceAcquisition | EvidenceCompilation
        ) | (AttemptLedger, MandatoryFinalization)
    );
    if requires_attempt != planned_attempt.is_some() {
        return Err(GenerationQualificationOperationContractError::InvalidInterruptionClosure);
    }
    if input.phase == ResourceEvidence
        && planned_attempt.is_some_and(|attempt| {
            attempt.generation_system_id()
                != relations.operation_policy.target_generation_system_id()
        })
    {
        return Err(GenerationQualificationOperationContractError::RelationshipMismatch);
    }
    Ok(())
}

fn phase_statuses(
    relations: &GenerationQualificationPhaseInterruptionRecordV1Relations<'_>,
) -> [GenerationQualificationPhaseStatusV1; 4] {
    [
        relations
            .operation_receipt_relations
            .attempt_ledger_manifest
            .status(),
        relations
            .operation_receipt_relations
            .repeatability_manifest
            .status(),
        relations
            .operation_receipt_relations
            .resource_manifest
            .status(),
        relations
            .operation_receipt_relations
            .human_adjudication_manifest
            .status(),
    ]
}

pub(super) fn validate_manifest_progression(
    phase: GenerationQualificationInterruptedPhaseV1,
    checkpoint: GenerationQualificationPhaseCheckpointV1,
    statuses: [GenerationQualificationPhaseStatusV1; 4],
) -> Result<(), GenerationQualificationOperationContractError> {
    use GenerationQualificationPhaseCheckpointV1::{
        BeforePhase, EvidenceAcquisition, EvidenceCompilation, FinalAuthorityRevalidation,
        MandatoryFinalization, ManifestCompilation,
    };
    use GenerationQualificationPhaseStatusV1::{Failed, Passed, Skipped};

    let phase_index = match phase {
        GenerationQualificationInterruptedPhaseV1::AttemptLedger => 0,
        GenerationQualificationInterruptedPhaseV1::Repeatability => 1,
        GenerationQualificationInterruptedPhaseV1::ResourceEvidence => 2,
        GenerationQualificationInterruptedPhaseV1::HumanAdjudication => 3,
    };
    let prior_passed = statuses[..phase_index]
        .iter()
        .all(|status| *status == Passed);
    let later_skipped = statuses[phase_index + 1..]
        .iter()
        .all(|status| *status == Skipped);
    let current_valid = match checkpoint {
        BeforePhase | ManifestCompilation => statuses[phase_index] == Skipped,
        EvidenceAcquisition | EvidenceCompilation => {
            matches!(statuses[phase_index], Skipped | Failed)
        }
        FinalAuthorityRevalidation | MandatoryFinalization => true,
    };
    if prior_passed && later_skipped && current_valid {
        Ok(())
    } else {
        Err(GenerationQualificationOperationContractError::InvalidInterruptionClosure)
    }
}

fn validate_reason(
    relations: &GenerationQualificationPhaseInterruptionRecordV1Relations<'_>,
    input: &GenerationQualificationPhaseInterruptionRecordV1Input,
) -> Result<(), GenerationQualificationOperationContractError> {
    use GenerationQualificationOperationFinalizationStatusV1::NotRequired;
    let receipt = relations.operation_receipt;
    let valid = reason_closure_is_valid(
        input.phase,
        input.checkpoint,
        input.reason,
        relations
            .operation_receipt_relations
            .resource_manifest
            .status(),
        receipt.peak_concurrent_attempts(),
        receipt.finalization_status(),
    );
    if !valid
        || (input.checkpoint == GenerationQualificationPhaseCheckpointV1::MandatoryFinalization
            && receipt.finalization_status() == NotRequired)
    {
        Err(GenerationQualificationOperationContractError::InvalidInterruptionClosure)
    } else {
        Ok(())
    }
}

fn reason_closure_is_valid(
    phase: GenerationQualificationInterruptedPhaseV1,
    checkpoint: GenerationQualificationPhaseCheckpointV1,
    reason: GenerationQualificationPhaseInterruptionReasonV1,
    resource_status: GenerationQualificationPhaseStatusV1,
    peak_concurrent_attempts: u32,
    finalization_status: GenerationQualificationOperationFinalizationStatusV1,
) -> bool {
    use GenerationQualificationInterruptedPhaseV1::{
        AttemptLedger, Repeatability, ResourceEvidence,
    };
    use GenerationQualificationOperationFinalizationStatusV1::{Failed, Passed};
    use GenerationQualificationPhaseCheckpointV1::{
        EvidenceAcquisition, EvidenceCompilation, MandatoryFinalization,
    };
    use GenerationQualificationPhaseInterruptionReasonV1::{
        AuthorityDrift, Cancelled, CleanupFailed, DeadlineExceeded, EvidenceCompilationFailed,
        MeasurementOverflow, RequiredObservationInvalid, RequiredObservationMissing,
    };
    use GenerationQualificationPhaseStatusV1::Skipped;

    match reason {
        RequiredObservationMissing | RequiredObservationInvalid => {
            phase == ResourceEvidence
                && matches!(checkpoint, EvidenceAcquisition | EvidenceCompilation)
                && resource_status == Skipped
        }
        MeasurementOverflow => {
            phase == ResourceEvidence
                && checkpoint == EvidenceCompilation
                && resource_status == Skipped
        }
        EvidenceCompilationFailed => {
            matches!(
                checkpoint,
                EvidenceCompilation | GenerationQualificationPhaseCheckpointV1::ManifestCompilation
            ) && (phase != ResourceEvidence || resource_status == Skipped)
        }
        CleanupFailed => {
            matches!(phase, AttemptLedger | Repeatability)
                && checkpoint == MandatoryFinalization
                && peak_concurrent_attempts == 1
                && finalization_status == Failed
        }
        AuthorityDrift if checkpoint == MandatoryFinalization => {
            matches!(phase, AttemptLedger | Repeatability)
                && peak_concurrent_attempts == 1
                && finalization_status == Failed
        }
        Cancelled | DeadlineExceeded if checkpoint == MandatoryFinalization => {
            matches!(phase, AttemptLedger | Repeatability)
                && peak_concurrent_attempts == 1
                && matches!(finalization_status, Passed | Failed)
        }
        AuthorityDrift | Cancelled | DeadlineExceeded => true,
    }
}

pub(super) const fn phase_policy_digest(
    policy: &GenerationQualificationOperationPolicyV1,
    phase: GenerationQualificationInterruptedPhaseV1,
) -> &Digest {
    match phase {
        GenerationQualificationInterruptedPhaseV1::AttemptLedger => {
            policy.attempt_ledger_policy_digest()
        }
        GenerationQualificationInterruptedPhaseV1::Repeatability => {
            policy.repeatability_policy_digest()
        }
        GenerationQualificationInterruptedPhaseV1::ResourceEvidence => {
            policy.resource_policy_digest()
        }
        GenerationQualificationInterruptedPhaseV1::HumanAdjudication => {
            policy.human_adjudication_policy_digest()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_progression_matrix_is_closed() {
        use GenerationQualificationInterruptedPhaseV1::{
            AttemptLedger, HumanAdjudication, Repeatability,
        };
        use GenerationQualificationPhaseCheckpointV1::{
            BeforePhase, EvidenceAcquisition, FinalAuthorityRevalidation, MandatoryFinalization,
            ManifestCompilation,
        };
        use GenerationQualificationPhaseStatusV1::{Failed, Passed, Skipped};

        for current in [Skipped, Failed, Passed] {
            let statuses = [Passed, Passed, Passed, current];
            let before = validate_manifest_progression(HumanAdjudication, BeforePhase, statuses);
            assert_eq!(before.is_ok(), current == Skipped);
            let acquiring =
                validate_manifest_progression(HumanAdjudication, EvidenceAcquisition, statuses);
            assert_eq!(acquiring.is_ok(), current != Passed);
            let compiling =
                validate_manifest_progression(HumanAdjudication, ManifestCompilation, statuses);
            assert_eq!(compiling.is_ok(), current == Skipped);
            let final_validation = validate_manifest_progression(
                HumanAdjudication,
                FinalAuthorityRevalidation,
                statuses,
            );
            assert!(final_validation.is_ok());
            assert!(
                validate_manifest_progression(HumanAdjudication, MandatoryFinalization, statuses)
                    .is_ok()
            );
        }

        assert_eq!(
            validate_manifest_progression(
                Repeatability,
                EvidenceAcquisition,
                [Failed, Skipped, Skipped, Skipped]
            ),
            Err(GenerationQualificationOperationContractError::InvalidInterruptionClosure)
        );
        assert_eq!(
            validate_manifest_progression(
                AttemptLedger,
                EvidenceAcquisition,
                [Skipped, Failed, Skipped, Skipped]
            ),
            Err(GenerationQualificationOperationContractError::InvalidInterruptionClosure)
        );
    }

    #[test]
    fn resource_operational_failures_require_a_skipped_resource_manifest() {
        use GenerationQualificationInterruptedPhaseV1::ResourceEvidence;
        use GenerationQualificationOperationFinalizationStatusV1::NotRequired;
        use GenerationQualificationPhaseCheckpointV1::{
            EvidenceAcquisition, EvidenceCompilation, ManifestCompilation,
        };
        use GenerationQualificationPhaseInterruptionReasonV1::{
            EvidenceCompilationFailed, MeasurementOverflow, RequiredObservationInvalid,
            RequiredObservationMissing,
        };
        use GenerationQualificationPhaseStatusV1::{Failed, Skipped};

        for reason in [RequiredObservationMissing, RequiredObservationInvalid] {
            assert!(reason_closure_is_valid(
                ResourceEvidence,
                EvidenceAcquisition,
                reason,
                Skipped,
                0,
                NotRequired,
            ));
            assert!(!reason_closure_is_valid(
                ResourceEvidence,
                EvidenceAcquisition,
                reason,
                Failed,
                0,
                NotRequired,
            ));
        }
        assert!(reason_closure_is_valid(
            ResourceEvidence,
            EvidenceCompilation,
            EvidenceCompilationFailed,
            Skipped,
            0,
            NotRequired,
        ));
        assert!(!reason_closure_is_valid(
            ResourceEvidence,
            EvidenceCompilation,
            EvidenceCompilationFailed,
            Failed,
            0,
            NotRequired,
        ));
        assert!(reason_closure_is_valid(
            ResourceEvidence,
            ManifestCompilation,
            EvidenceCompilationFailed,
            Skipped,
            0,
            NotRequired,
        ));
        assert!(!reason_closure_is_valid(
            ResourceEvidence,
            ManifestCompilation,
            EvidenceCompilationFailed,
            Failed,
            0,
            NotRequired,
        ));
        assert!(reason_closure_is_valid(
            ResourceEvidence,
            EvidenceCompilation,
            MeasurementOverflow,
            Skipped,
            0,
            NotRequired,
        ));
        assert!(!reason_closure_is_valid(
            ResourceEvidence,
            EvidenceAcquisition,
            MeasurementOverflow,
            Skipped,
            0,
            NotRequired,
        ));
    }
}
