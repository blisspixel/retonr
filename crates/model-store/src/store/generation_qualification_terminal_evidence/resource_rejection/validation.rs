use super::{
    Connection, GenerationQualificationPhaseStatusV1, GenerationQualificationRecordV1,
    GenerationQualificationStatusV1, GenerationResourceRejectionV1Input, StoreError, StoreResult,
};
use rewrite_model::{
    GenerationQualificationOperationFinalizationStatusV1,
    GenerationQualificationOperationTerminalStatusV1,
};

pub(super) fn validate(
    connection: &Connection,
    input: &GenerationResourceRejectionV1Input<'_>,
) -> StoreResult<()> {
    let phase = input.resource_phase;
    let parent = phase.repeatability;
    let q = input.qualification_relations;
    let r = q.operation_receipt_relations;
    let fact = q.operation_receipt_input;
    if phase.manifest.status() != GenerationQualificationPhaseStatusV1::Failed
        || r.resource_manifest != phase.manifest
        || r.repeatability_manifest != parent.manifest
        || r.attempt_ledger_manifest != parent.ledger.manifest
        || r.operation_policy.operation_policy_id()
            != parent.ledger.preregistration.operation_policy_id
        || r.request_projection.request_projection_id()
            != parent.ledger.preregistration.request_projection_id
        || q.repeatability_manifest_relations.results != parent.ordered_results
        || q.repeatability_result_relations.len() != parent.expected_parents.len()
        || r.human_adjudication_manifest.status() != GenerationQualificationPhaseStatusV1::Skipped
        || !q
            .human_adjudication_manifest_relations
            .evidence_record_digests
            .is_empty()
        || fact.terminal_status != GenerationQualificationOperationTerminalStatusV1::Completed
        || fact.finalization_status != GenerationQualificationOperationFinalizationStatusV1::Passed
        || fact.peak_concurrent_attempts != 1
    {
        return Err(StoreError::CorruptRecord);
    }
    // Reconstruct the bounded complete parent tree before allocating any caller
    // slice projection or iterating the claimed recursive result closures.
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
    let derived = GenerationQualificationRecordV1::new(q).map_err(|_| StoreError::CorruptRecord)?;
    if &derived != input.record || derived.status() != GenerationQualificationStatusV1::Rejected {
        return Err(StoreError::CorruptRecord);
    }
    Ok(())
}
