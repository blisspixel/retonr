use super::GenerationQualificationTerminalCloseoutV1Input;
use crate::{StoreError, StoreResult};
use rewrite_model::{
    GenerationQualificationOperationFinalizationStatusV1,
    GenerationQualificationOperationTerminalStatusV1, GenerationQualificationPhaseStatusV1,
    GenerationQualificationRecordV1, GenerationQualificationStatusV1,
};
use rusqlite::Connection;

pub(super) fn validate(
    connection: &Connection,
    input: &GenerationQualificationTerminalCloseoutV1Input<'_>,
) -> StoreResult<()> {
    let phase = input.resource_phase;
    let parent = phase.repeatability;
    let q = input.qualification_relations;
    let r = q.operation_receipt_relations;
    let fact = q.operation_receipt_input;
    if r.resource_manifest != phase.manifest
        || r.repeatability_manifest != parent.manifest
        || r.attempt_ledger_manifest != parent.ledger.manifest
        || r.operation_policy.operation_policy_id()
            != parent.ledger.preregistration.operation_policy_id
        || r.request_projection.request_projection_id()
            != parent.ledger.preregistration.request_projection_id
        || q.repeatability_manifest_relations.results != parent.ordered_results
        || q.repeatability_result_relations.len() != parent.expected_parents.len()
        || fact.terminal_status != GenerationQualificationOperationTerminalStatusV1::Completed
        || fact.finalization_status != GenerationQualificationOperationFinalizationStatusV1::Passed
        || fact.peak_concurrent_attempts != 1
    {
        return Err(StoreError::CorruptRecord);
    }
    if crate::store::generation_resource_phase::load(connection, phase)?.is_none() {
        return Err(StoreError::MissingRecord);
    }
    for (relations, expected) in q
        .repeatability_result_relations
        .iter()
        .zip(parent.expected_parents)
    {
        if relations.candidate_receipt_set != Some(&expected.target_receipt_set)
            || relations.deterministic_evaluation != Some(&expected.deterministic_evaluation)
            || relations.candidate_judge_join != Some(expected.judge_execution.join())
        {
            return Err(StoreError::CorruptRecord);
        }
    }
    let resource_digests = phase
        .ordered_results
        .iter()
        .map(|record| record.resource_attempt_result_id().digest().clone())
        .collect::<Vec<_>>();
    if q.resource_manifest_relations.evidence_record_digests != resource_digests {
        return Err(StoreError::CorruptRecord);
    }

    validate_human_status(input)?;

    let derived = GenerationQualificationRecordV1::new(q).map_err(|_| StoreError::CorruptRecord)?;
    if &derived != input.record {
        return Err(StoreError::CorruptRecord);
    }
    Ok(())
}

fn validate_human_status(
    input: &GenerationQualificationTerminalCloseoutV1Input<'_>,
) -> StoreResult<()> {
    let phase = input.resource_phase;
    let q = input.qualification_relations;
    let r = q.operation_receipt_relations;
    if let Some(denial) = input.human_policy_denial.as_ref() {
        if phase.manifest.status() != GenerationQualificationPhaseStatusV1::Passed
            || r.human_adjudication_manifest.status()
                != GenerationQualificationPhaseStatusV1::Failed
            || r.human_adjudication_manifest.evidence_item_count() != 1
            || q.human_adjudication_manifest_relations
                .evidence_record_digests
                != [denial
                    .record
                    .human_adjudication_policy_denial_record_id()
                    .digest()
                    .clone()]
            || denial.record.generation_system_id()
                != q.operation_policy_relations
                    .target_system
                    .generation_system
                    .generation_system_id()
            || denial.record.generation_qualification_plan_id()
                != q.operation_policy_relations.plan.qualification_plan_id()
            || denial.record.suite_manifest_id()
                != q.operation_policy_relations.suite.suite_manifest_id()
            || denial.record.phase_policy_digest()
                != r.operation_policy.human_adjudication_policy_digest()
            || input.record.status() != GenerationQualificationStatusV1::Rejected
        {
            return Err(StoreError::CorruptRecord);
        }
    } else if r.human_adjudication_manifest.status() == GenerationQualificationPhaseStatusV1::Passed
    {
        if phase.manifest.status() != GenerationQualificationPhaseStatusV1::Passed
            || input.record.status() != GenerationQualificationStatusV1::Qualified
        {
            return Err(StoreError::CorruptRecord);
        }
    } else if r.human_adjudication_manifest.status() == GenerationQualificationPhaseStatusV1::Failed
    {
        if phase.manifest.status() != GenerationQualificationPhaseStatusV1::Passed
            || input.record.status() != GenerationQualificationStatusV1::Rejected
        {
            return Err(StoreError::CorruptRecord);
        }
    } else if r.human_adjudication_manifest.status()
        == GenerationQualificationPhaseStatusV1::Skipped
    {
        if phase.manifest.status() != GenerationQualificationPhaseStatusV1::Failed
            || !q
                .human_adjudication_manifest_relations
                .evidence_record_digests
                .is_empty()
            || input.record.status() != GenerationQualificationStatusV1::Rejected
        {
            return Err(StoreError::CorruptRecord);
        }
    } else {
        return Err(StoreError::CorruptRecord);
    }
    Ok(())
}
