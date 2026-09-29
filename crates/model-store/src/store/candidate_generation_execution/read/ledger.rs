//! Read-only target attempt-ledger manifest derived from schema-10 rows.

use rewrite_model::{
    CandidateGenerationAttemptOutcomeV1, CandidateGenerationAttemptRecordV1,
    GenerationAttemptLedgerManifestV1, GenerationAttemptLedgerManifestV1Relations,
    GenerationQualificationPhaseScopeV1, GenerationQualificationPhaseStatusV1,
    MAX_CANDIDATE_GENERATION_ATTEMPT_RECORD_JSON_BYTES,
    ManagedOllamaCandidateGenerationEvidenceV2Input, PlannedCandidateAttemptV1,
};
use rusqlite::{Connection, params};

use super::super::GenerationQualificationPreregistrationReadInput;
use super::{StoredCandidateGenerationExecutionV1, load_completed, load_failed};
use crate::store::generation_qualification_preregistration::load_preregistration;
use crate::{StoreError, StoreResult};

pub(crate) fn rederive_attempt_ledger(
    connection: &Connection,
    preregistration: GenerationQualificationPreregistrationReadInput<'_>,
    managed_evidence_inputs: &[ManagedOllamaCandidateGenerationEvidenceV2Input],
) -> StoreResult<GenerationAttemptLedgerManifestV1> {
    let before = connection.total_changes();
    let stored =
        load_preregistration(connection, preregistration)?.ok_or(StoreError::MissingRecord)?;
    let records = target_prefix(connection, &stored, managed_evidence_inputs)?;
    let manifest = manifest_for(&stored, &records)?;
    if connection.total_changes() != before {
        return Err(StoreError::CorruptRecord);
    }
    Ok(manifest)
}

fn target_prefix(
    connection: &Connection,
    preregistration: &crate::store::generation_qualification_preregistration::StoredGenerationQualificationPreregistration,
    managed_evidence_inputs: &[ManagedOllamaCandidateGenerationEvidenceV2Input],
) -> StoreResult<Vec<CandidateGenerationAttemptRecordV1>> {
    let foundation = preregistration.plan_foundation();
    let target_id = preregistration
        .operation_policy()
        .target_generation_system_id();
    let target = foundation
        .planned_attempts()
        .iter()
        .filter(|attempt| attempt.generation_system_id() == target_id)
        .collect::<Vec<_>>();
    let mut records = Vec::new();
    let mut gap = false;
    let mut managed = managed_evidence_inputs.iter();
    for planned in target {
        match super::rows::load_attempt(
            connection,
            planned.planned_attempt_id().digest().as_str(),
            MAX_CANDIDATE_GENERATION_ATTEMPT_RECORD_JSON_BYTES,
        )? {
            None => gap = true,
            Some(_) if gap => return Err(StoreError::CorruptRecord),
            Some(row) => records.push(load_prefix_attempt(
                connection,
                preregistration,
                planned,
                &row,
                &mut managed,
            )?),
        }
    }
    if managed.next().is_some() {
        return Err(StoreError::CorruptRecord);
    }
    let stored_count = connection.query_row(
        "SELECT COUNT(*) FROM candidate_generation_attempt_records
         WHERE generation_qualification_plan_id = ?1",
        params![foundation.plan().qualification_plan_id().digest().as_str()],
        |row| row.get::<_, i64>(0),
    )?;
    if stored_count == i64::try_from(records.len()).unwrap_or(i64::MAX) {
        Ok(records)
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn load_prefix_attempt<'a>(
    connection: &Connection,
    preregistration: &crate::store::generation_qualification_preregistration::StoredGenerationQualificationPreregistration,
    planned: &PlannedCandidateAttemptV1,
    row: &super::rows::AttemptRow,
    managed: &mut impl Iterator<Item = &'a ManagedOllamaCandidateGenerationEvidenceV2Input>,
) -> StoreResult<CandidateGenerationAttemptRecordV1> {
    let stored = match row.outcome.as_str() {
        "failed" => load_failed(connection, preregistration.clone(), planned, row)?,
        "completed" => {
            let input = managed.next().ok_or(StoreError::CorruptRecord)?;
            load_completed(connection, preregistration.clone(), planned, input, row)?
        }
        _ => return Err(StoreError::CorruptRecord),
    };
    Ok(attempt_record(stored))
}

fn attempt_record(
    stored: StoredCandidateGenerationExecutionV1,
) -> CandidateGenerationAttemptRecordV1 {
    match stored {
        StoredCandidateGenerationExecutionV1::Failed { attempt, .. }
        | StoredCandidateGenerationExecutionV1::Completed { attempt, .. } => attempt,
    }
}

fn manifest_for(
    preregistration: &crate::store::generation_qualification_preregistration::StoredGenerationQualificationPreregistration,
    records: &[CandidateGenerationAttemptRecordV1],
) -> StoreResult<GenerationAttemptLedgerManifestV1> {
    let foundation = preregistration.plan_foundation();
    let policy = preregistration.operation_policy();
    let generation_system = foundation
        .generation_systems()
        .iter()
        .find(|system| system.generation_system_id() == policy.target_generation_system_id())
        .ok_or(StoreError::CorruptRecord)?;
    let target_count = foundation
        .planned_attempts()
        .iter()
        .filter(|attempt| attempt.generation_system_id() == policy.target_generation_system_id())
        .count();
    GenerationAttemptLedgerManifestV1::new(GenerationAttemptLedgerManifestV1Relations {
        scope: GenerationQualificationPhaseScopeV1 {
            generation_system,
            qualification_plan: foundation.plan(),
            suite: foundation.suite(),
        },
        phase_policy_digest: policy.attempt_ledger_policy_digest(),
        planned_attempts: foundation.planned_attempts(),
        attempt_records: records,
        status: derived_status(records, target_count),
    })
    .map_err(|_| StoreError::CorruptRecord)
}

fn derived_status(
    records: &[CandidateGenerationAttemptRecordV1],
    target_count: usize,
) -> GenerationQualificationPhaseStatusV1 {
    let all_completed = records.iter().all(|record| {
        matches!(
            record.outcome(),
            CandidateGenerationAttemptOutcomeV1::Completed { .. }
        )
    });
    if records.is_empty() {
        GenerationQualificationPhaseStatusV1::Skipped
    } else if all_completed && records.len() == target_count {
        GenerationQualificationPhaseStatusV1::Passed
    } else {
        GenerationQualificationPhaseStatusV1::Failed
    }
}
