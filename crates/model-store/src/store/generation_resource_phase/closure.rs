//! Full parent snapshot and exact semantic resource closure reconstruction.

use super::GenerationResourcePhaseV1Input;
use crate::{StoreError, StoreResult};
use rewrite_model::{
    CandidateGenerationAttemptOutcomeV1, GenerationQualificationPhaseScopeV1,
    GenerationQualificationPhaseStatusV1, GenerationResourceEvidenceManifestV1,
    GenerationResourceEvidenceManifestV1Relations, MAX_GENERATION_QUALIFICATION_PHASE_ITEMS,
};
use rusqlite::Connection;

pub(super) fn validate(
    connection: &Connection,
    input: GenerationResourcePhaseV1Input<'_>,
) -> StoreResult<()> {
    let parent = input.repeatability;
    if crate::store::generation_attempt_ledger::load_repeatability_phase(connection, *parent)?
        .as_ref()
        != Some(parent.manifest)
    {
        return Err(StoreError::MissingRecord);
    }
    let preregistration =
        crate::store::generation_qualification_preregistration::load_preregistration(
            connection,
            parent.ledger.preregistration,
        )?
        .ok_or(StoreError::MissingRecord)?;
    let foundation = preregistration.plan_foundation();
    let policy = preregistration.operation_policy();
    let target = foundation
        .generation_systems()
        .iter()
        .find(|system| system.generation_system_id() == policy.target_generation_system_id())
        .ok_or(StoreError::CorruptRecord)?;
    let expected_count = foundation
        .repetitions()
        .len()
        .checked_mul(foundation.suite().case_ids().len())
        .filter(|count| *count > 0 && *count <= MAX_GENERATION_QUALIFICATION_PHASE_ITEMS)
        .ok_or(StoreError::CorruptRecord)?;
    if input.ordered_results.len() != expected_count {
        return Err(StoreError::CorruptRecord);
    }
    let ledger = crate::store::candidate_generation_execution::rederive_attempt_ledger(
        connection,
        parent.ledger.preregistration,
        parent.ledger.managed_evidence_inputs,
    )?;
    let mut cursor = 0;
    for repetition in foundation.repetitions() {
        for case_id in foundation.suite().case_ids() {
            let record = &input.ordered_results[cursor];
            let planned = foundation
                .planned_attempts()
                .iter()
                .filter(|attempt| {
                    attempt.generation_system_id() == target.generation_system_id()
                        && attempt.repetition_id() == repetition.repetition_id()
                        && attempt.case_id() == case_id
                })
                .collect::<Vec<_>>();
            let [planned] = planned.as_slice() else {
                return Err(StoreError::CorruptRecord);
            };
            let attempt = ledger
                .attempt_records
                .iter()
                .find(|attempt| attempt.attempt_record_id() == record.attempt_record_id())
                .ok_or(StoreError::MissingRecord)?;
            let CandidateGenerationAttemptOutcomeV1::Completed {
                planned_attempt_id,
                receipt_id,
                ..
            } = attempt.outcome()
            else {
                return Err(StoreError::CorruptRecord);
            };
            if record.generation_system_id() != target.generation_system_id()
                || record.generation_qualification_plan_id()
                    != foundation.plan().qualification_plan_id()
                || record.suite_manifest_id() != foundation.suite().suite_manifest_id()
                || record.case_id() != case_id
                || record.repetition_id() != repetition.repetition_id()
                || record.planned_attempt_id() != planned.planned_attempt_id()
                || planned_attempt_id != record.planned_attempt_id()
                || receipt_id != record.candidate_generation_receipt_id()
                || record.resource_policy_digest() != policy.resource_policy_digest()
            {
                return Err(StoreError::CorruptRecord);
            }
            cursor += 1;
        }
    }
    validate_manifest(
        input,
        GenerationQualificationPhaseScopeV1 {
            generation_system: target,
            qualification_plan: foundation.plan(),
            suite: foundation.suite(),
        },
        policy.resource_policy_digest(),
    )
}

fn validate_manifest(
    input: GenerationResourcePhaseV1Input<'_>,
    scope: GenerationQualificationPhaseScopeV1<'_>,
    policy_digest: &rewrite_types::Digest,
) -> StoreResult<()> {
    let digests = input
        .ordered_results
        .iter()
        .map(|record| record.resource_attempt_result_id().digest().clone())
        .collect::<Vec<_>>();
    let status = if input
        .ordered_results
        .iter()
        .any(|record| !record.exceeded_limits().is_empty())
    {
        GenerationQualificationPhaseStatusV1::Failed
    } else {
        GenerationQualificationPhaseStatusV1::Passed
    };
    let expected =
        GenerationResourceEvidenceManifestV1::new(GenerationResourceEvidenceManifestV1Relations {
            scope,
            phase_policy_digest: policy_digest,
            evidence_record_digests: &digests,
            status,
        })
        .map_err(|_| StoreError::CorruptRecord)?;
    if &expected != input.manifest {
        return Err(StoreError::CorruptRecord);
    }
    Ok(())
}
