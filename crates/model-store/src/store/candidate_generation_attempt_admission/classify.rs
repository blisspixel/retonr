//! Schema-10 indexed admission classification.

use rewrite_model::{GenerationQualificationPlanId, PlannedCandidateAttemptId};
use rusqlite::{Connection, OptionalExtension as _, params};

use super::super::candidate_generation_evidence_storage::CandidateGenerationEvidenceStorageRootId;
use super::{
    AttemptFacts, CandidateGenerationAttemptAdmissionClassV1, CandidateGenerationAttemptAdmissionV1,
};
use crate::{StoreError, StoreResult};

pub(super) fn read_admissions(
    connection: &Connection,
    plan_id: &GenerationQualificationPlanId,
    planned_attempt_ids: &[PlannedCandidateAttemptId],
    storage_root_id: &CandidateGenerationEvidenceStorageRootId,
) -> StoreResult<Vec<CandidateGenerationAttemptAdmissionV1>> {
    require_exact_plan_order(connection, plan_id, planned_attempt_ids)?;
    require_closed_attempt_scope(connection, plan_id)?;
    require_no_orphaned_completion_rows(connection)?;
    let mut admissions = Vec::with_capacity(planned_attempt_ids.len());
    for attempt_id in planned_attempt_ids {
        let facts = load_facts(connection, plan_id, attempt_id)?;
        admissions.push(classify_attempt(
            connection,
            plan_id,
            attempt_id,
            storage_root_id,
            &facts,
        )?);
    }
    Ok(admissions)
}

fn require_exact_plan_order(
    connection: &Connection,
    plan_id: &GenerationQualificationPlanId,
    planned_attempt_ids: &[PlannedCandidateAttemptId],
) -> StoreResult<()> {
    let stored_count = connection
        .query_row(
            "SELECT planned_attempt_count
             FROM generation_qualification_plans
             WHERE generation_qualification_plan_id = ?1",
            params![plan_id.digest().as_str()],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .ok_or(StoreError::CorruptRecord)?;
    let mut statement = connection.prepare(
        "SELECT plan_ordinal, planned_candidate_attempt_id
         FROM generation_qualification_plan_attempts
         WHERE generation_qualification_plan_id = ?1
         ORDER BY plan_ordinal ASC
         LIMIT ?2",
    )?;
    let rows = statement.query_map(
        params![
            plan_id.digest().as_str(),
            i64::try_from(planned_attempt_ids.len().saturating_add(1))
                .map_err(|_| StoreError::CorruptRecord)?
        ],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
    )?;
    let mut stored = Vec::new();
    for row in rows {
        stored.push(row?);
    }
    if i64::try_from(stored.len()).ok() != Some(stored_count)
        || stored.len() != planned_attempt_ids.len()
    {
        return Err(StoreError::CorruptRecord);
    }
    for (index, (ordinal, attempt_id)) in stored.iter().enumerate() {
        if *ordinal != i64::try_from(index).unwrap_or(i64::MAX)
            || attempt_id != planned_attempt_ids[index].digest().as_str()
        {
            return Err(StoreError::CorruptRecord);
        }
    }
    Ok(())
}

fn require_closed_attempt_scope(
    connection: &Connection,
    plan_id: &GenerationQualificationPlanId,
) -> StoreResult<()> {
    let extras = connection.query_row(
        "SELECT COUNT(*) FROM (
             SELECT planned_candidate_attempt_id
             FROM candidate_generation_attempt_precursors
             WHERE generation_qualification_plan_id = ?1
             UNION
             SELECT planned_candidate_attempt_id
             FROM candidate_generation_attempt_records
             WHERE generation_qualification_plan_id = ?1
             UNION
             SELECT planned_candidate_attempt_id
             FROM generation_evidence_bundle_storage
             WHERE generation_qualification_plan_id = ?1
             UNION
             SELECT planned_candidate_attempt_id
             FROM candidate_generation_receipts
             WHERE generation_qualification_plan_id = ?1
         ) AS observed
         WHERE planned_candidate_attempt_id NOT IN (
             SELECT planned_candidate_attempt_id
             FROM generation_qualification_plan_attempts
             WHERE generation_qualification_plan_id = ?1
         )",
        params![plan_id.digest().as_str()],
        |row| row.get::<_, i64>(0),
    )?;
    if extras == 0 {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn require_no_orphaned_completion_rows(connection: &Connection) -> StoreResult<()> {
    for sql in [
        "SELECT 1 FROM managed_candidate_generation_evidence AS managed
         WHERE NOT EXISTS (
             SELECT 1 FROM candidate_generation_attempt_precursors AS precursor
             WHERE precursor.candidate_generation_attempt_precursor_id =
                 managed.candidate_generation_attempt_precursor_id
         ) LIMIT 1",
        "SELECT 1 FROM candidate_generation_cleanup_records AS cleanup
         WHERE NOT EXISTS (
             SELECT 1 FROM candidate_generation_attempt_precursors AS precursor
             WHERE precursor.candidate_generation_attempt_precursor_id =
                 cleanup.candidate_generation_attempt_precursor_id
         ) LIMIT 1",
        "SELECT 1 FROM generation_evidence_bundles AS bundle
         WHERE NOT EXISTS (
             SELECT 1 FROM candidate_generation_attempt_precursors AS precursor
             WHERE precursor.candidate_generation_attempt_precursor_id =
                 bundle.candidate_generation_attempt_precursor_id
         ) LIMIT 1",
        "SELECT 1 FROM generation_evidence_bundle_readbacks AS readback
         WHERE NOT EXISTS (
             SELECT 1 FROM generation_evidence_bundles AS bundle
             WHERE bundle.candidate_generation_evidence_bundle_id =
                 readback.candidate_generation_evidence_bundle_id
         ) LIMIT 1",
    ] {
        if connection
            .query_row(sql, [], |row| row.get::<_, i64>(0))
            .optional()?
            .is_some()
        {
            return Err(StoreError::CorruptRecord);
        }
    }
    Ok(())
}

fn load_facts(
    connection: &Connection,
    plan_id: &GenerationQualificationPlanId,
    attempt_id: &PlannedCandidateAttemptId,
) -> StoreResult<AttemptFacts> {
    let facts = connection.query_row(
        "SELECT
             precursors.candidate_generation_attempt_precursor_id,
             records.outcome,
             records.candidate_generation_attempt_precursor_id,
             records.candidate_generation_receipt_id,
             (
                 SELECT COUNT(*) FROM managed_candidate_generation_evidence AS managed
                 WHERE precursors.candidate_generation_attempt_precursor_id IS NOT NULL
                   AND managed.candidate_generation_attempt_precursor_id =
                       precursors.candidate_generation_attempt_precursor_id
             ),
             (
                 SELECT COUNT(*) FROM candidate_generation_cleanup_records AS cleanup
                 WHERE precursors.candidate_generation_attempt_precursor_id IS NOT NULL
                   AND cleanup.candidate_generation_attempt_precursor_id =
                       precursors.candidate_generation_attempt_precursor_id
             ),
             (
                 SELECT COUNT(*) FROM generation_evidence_bundles AS bundles
                 WHERE precursors.candidate_generation_attempt_precursor_id IS NOT NULL
                   AND bundles.candidate_generation_attempt_precursor_id =
                       precursors.candidate_generation_attempt_precursor_id
             ),
             (
                 SELECT COUNT(*) FROM generation_evidence_bundle_storage AS storage
                 WHERE storage.generation_qualification_plan_id = ?1
                   AND storage.planned_candidate_attempt_id = ?2
             ),
             (
                 SELECT COUNT(*)
                 FROM generation_evidence_bundle_readbacks AS readbacks
                 JOIN generation_evidence_bundles AS bundles
                   ON readbacks.candidate_generation_evidence_bundle_id =
                       bundles.candidate_generation_evidence_bundle_id
                 WHERE precursors.candidate_generation_attempt_precursor_id IS NOT NULL
                   AND bundles.candidate_generation_attempt_precursor_id =
                       precursors.candidate_generation_attempt_precursor_id
             ),
             (
                 SELECT COUNT(*) FROM candidate_generation_receipts AS receipts
                 WHERE receipts.generation_qualification_plan_id = ?1
                   AND receipts.planned_candidate_attempt_id = ?2
             )
         FROM (SELECT ?1 AS generation_qualification_plan_id, ?2 AS planned_candidate_attempt_id)
             AS selected
         LEFT JOIN candidate_generation_attempt_precursors AS precursors
           ON precursors.generation_qualification_plan_id = selected.generation_qualification_plan_id
          AND precursors.planned_candidate_attempt_id = selected.planned_candidate_attempt_id
         LEFT JOIN candidate_generation_attempt_records AS records
           ON records.generation_qualification_plan_id = selected.generation_qualification_plan_id
          AND records.planned_candidate_attempt_id = selected.planned_candidate_attempt_id",
        params![plan_id.digest().as_str(), attempt_id.digest().as_str()],
        |row| {
            Ok(AttemptFacts {
                precursor_id: row.get(0)?,
                outcome: row.get(1)?,
                attempt_precursor_id: row.get(2)?,
                attempt_receipt_id: row.get(3)?,
                managed_count: row.get(4)?,
                cleanup_count: row.get(5)?,
                bundle_count: row.get(6)?,
                storage_count: row.get(7)?,
                readback_count: row.get(8)?,
                receipt_count: row.get(9)?,
            })
        },
    )?;
    Ok(facts)
}

fn classify_attempt(
    connection: &Connection,
    plan_id: &GenerationQualificationPlanId,
    attempt_id: &PlannedCandidateAttemptId,
    storage_root_id: &CandidateGenerationEvidenceStorageRootId,
    facts: &AttemptFacts,
) -> StoreResult<CandidateGenerationAttemptAdmissionV1> {
    let class = coarse_class(facts);
    let (class, bundle_id) = if matches!(
        class,
        CandidateGenerationAttemptAdmissionClassV1::TerminalCompleted
    ) {
        chain::completed_chain(connection, plan_id, attempt_id, storage_root_id)?
    } else {
        (class, None)
    };
    CandidateGenerationAttemptAdmissionV1::from_observation(attempt_id.clone(), class, bundle_id)
}

fn coarse_class(facts: &AttemptFacts) -> CandidateGenerationAttemptAdmissionClassV1 {
    use CandidateGenerationAttemptAdmissionClassV1::{
        Ambiguous, CheckpointOnly, Corrupt, NotStarted,
    };
    if count_exceeds_one(facts) || outcome_is_unrecognized(facts) {
        return Corrupt;
    }
    let clear = counts_are_clear(facts);
    let full = counts_are_full(facts);
    let precursor = facts.precursor_id.is_some();
    match facts.outcome.as_deref() {
        None if clear
            && !precursor
            && facts.attempt_precursor_id.is_none()
            && facts.attempt_receipt_id.is_none() =>
        {
            NotStarted
        }
        None if clear
            && precursor
            && facts.attempt_precursor_id.is_none()
            && facts.attempt_receipt_id.is_none() =>
        {
            CheckpointOnly
        }
        Some("failed") => classify_failed(facts, precursor, clear),
        Some("completed") => classify_completed(facts, precursor, full),
        None => Ambiguous,
        Some(_) => Corrupt,
    }
}

fn count_exceeds_one(facts: &AttemptFacts) -> bool {
    facts.managed_count > 1
        || facts.cleanup_count > 1
        || facts.bundle_count > 1
        || facts.storage_count > 1
        || facts.readback_count > 1
        || facts.receipt_count > 1
}

fn outcome_is_unrecognized(facts: &AttemptFacts) -> bool {
    matches!(
        facts.outcome.as_deref(),
        Some(outcome) if outcome != "failed" && outcome != "completed"
    )
}

const fn counts_are_clear(facts: &AttemptFacts) -> bool {
    facts.managed_count == 0
        && facts.cleanup_count == 0
        && facts.bundle_count == 0
        && facts.storage_count == 0
        && facts.readback_count == 0
        && facts.receipt_count == 0
}

const fn counts_are_full(facts: &AttemptFacts) -> bool {
    facts.managed_count == 1
        && facts.cleanup_count == 1
        && facts.bundle_count == 1
        && facts.storage_count == 1
        && facts.readback_count == 1
        && facts.receipt_count == 1
}

fn classify_failed(
    facts: &AttemptFacts,
    precursor: bool,
    clear: bool,
) -> CandidateGenerationAttemptAdmissionClassV1 {
    use CandidateGenerationAttemptAdmissionClassV1::{Ambiguous, Corrupt, TerminalFailed};
    if facts.attempt_receipt_id.is_some() || !failed_precursor_matches(facts, precursor) {
        return Corrupt;
    }
    if clear {
        TerminalFailed
    } else if precursor {
        Ambiguous
    } else {
        Corrupt
    }
}

fn classify_completed(
    facts: &AttemptFacts,
    precursor: bool,
    full: bool,
) -> CandidateGenerationAttemptAdmissionClassV1 {
    use CandidateGenerationAttemptAdmissionClassV1::{Ambiguous, Corrupt, TerminalCompleted};
    if !precursor || facts.attempt_precursor_id.is_none() || facts.attempt_receipt_id.is_none() {
        return Corrupt;
    }
    if full { TerminalCompleted } else { Ambiguous }
}

fn failed_precursor_matches(facts: &AttemptFacts, precursor_present: bool) -> bool {
    match (
        precursor_present,
        facts.precursor_id.as_deref(),
        facts.attempt_precursor_id.as_deref(),
    ) {
        (false, None, None) => true,
        (true, Some(precursor), Some(attempt_precursor)) => precursor == attempt_precursor,
        _ => false,
    }
}

#[path = "chain.rs"]
mod chain;
