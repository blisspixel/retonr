use rewrite_model::CandidateGenerationAttemptOutcomeV1;

use super::{
    RetainedCandidateBatch, VerifiedCandidateBatchSetInput,
    VerifiedCandidateBatchSetRelationship as Relationship,
};

pub(super) fn validate_fixed_closure<B: RetainedCandidateBatch>(
    input: &VerifiedCandidateBatchSetInput,
    batches: &[B],
) -> Result<(), Relationship> {
    validate_top_level(input, batches.len())?;
    for (index, (case_id, batch)) in input.suite.case_ids().iter().zip(batches).enumerate() {
        let receipt = batch.receipt();
        if receipt.case_id() != case_id {
            return Err(Relationship::SemanticCaseOrder);
        }
        if receipt.qualification_plan_id() != input.qualification_plan.qualification_plan_id()
            || receipt.suite_manifest_id() != input.suite.suite_manifest_id()
        {
            return Err(Relationship::QualificationClosure);
        }
        if receipt.repetition_id() != input.repetition.repetition_id() {
            return Err(Relationship::RepetitionClosure);
        }
        if receipt.generation_system_id() != input.generation_system.generation_system_id() {
            return Err(Relationship::GenerationSystemClosure);
        }
        let planned = selected_attempt(input, case_id)?;
        validate_completed_batch(batch, receipt, planned.planned_attempt_id())?;
        let selection = &input.selection_policy.entries()[index];
        if selection.case_id() != case_id
            || usize::from(selection.selected_ordinal()) >= batch.candidate_count()
            || receipt
                .candidate_entries()
                .get(usize::from(selection.selected_ordinal()))
                .is_none_or(|entry| entry.ordinal() != selection.selected_ordinal())
        {
            return Err(Relationship::SelectionClosure);
        }
    }
    Ok(())
}

fn validate_top_level(
    input: &VerifiedCandidateBatchSetInput,
    batch_count: usize,
) -> Result<(), Relationship> {
    if input.planned_attempts.len() != input.qualification_plan.planned_attempt_ids().len()
        || input
            .planned_attempts
            .iter()
            .zip(input.qualification_plan.planned_attempt_ids())
            .any(|(attempt, id)| attempt.planned_attempt_id() != id)
    {
        return Err(Relationship::PlannedAttemptClosure);
    }
    if batch_count != input.suite.case_ids().len() {
        return Err(Relationship::BatchCount);
    }
    if input.qualification_plan.suite_manifest_id() != input.suite.suite_manifest_id()
        || input.repetition.suite_manifest_id() != input.suite.suite_manifest_id()
        || input.selection_policy.suite_manifest_id() != input.suite.suite_manifest_id()
        || input.selection_policy.entries().len() != input.suite.case_ids().len()
    {
        return Err(Relationship::QualificationClosure);
    }
    if !input
        .qualification_plan
        .generation_system_ids()
        .contains(input.generation_system.generation_system_id())
    {
        return Err(Relationship::GenerationSystemClosure);
    }
    Ok(())
}

fn selected_attempt<'a>(
    input: &'a VerifiedCandidateBatchSetInput,
    case_id: &rewrite_model::GenerationCaseId,
) -> Result<&'a rewrite_model::PlannedCandidateAttemptV1, Relationship> {
    let mut matching = input.planned_attempts.iter().filter(|attempt| {
        attempt.case_id() == case_id
            && attempt.repetition_id() == input.repetition.repetition_id()
            && attempt.generation_system_id() == input.generation_system.generation_system_id()
    });
    let selected = matching
        .next()
        .ok_or(Relationship::SelectedAttemptClosure)?;
    if matching.next().is_some() {
        return Err(Relationship::SelectedAttemptClosure);
    }
    Ok(selected)
}

fn validate_completed_batch<B: RetainedCandidateBatch>(
    batch: &B,
    receipt: &rewrite_model::CandidateGenerationReceiptV1,
    planned_attempt_id: &rewrite_model::PlannedCandidateAttemptId,
) -> Result<(), Relationship> {
    if receipt.planned_attempt_id() != planned_attempt_id {
        return Err(Relationship::SelectedAttemptClosure);
    }
    match batch.attempt_record().outcome() {
        CandidateGenerationAttemptOutcomeV1::Completed {
            planned_attempt_id: completed_planned,
            receipt_id,
            ..
        } if completed_planned == planned_attempt_id && receipt_id == receipt.receipt_id() => {
            Ok(())
        }
        CandidateGenerationAttemptOutcomeV1::Completed { .. }
        | CandidateGenerationAttemptOutcomeV1::Failed { .. } => {
            Err(Relationship::SelectedAttemptClosure)
        }
    }
}
