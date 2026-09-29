//! Completed-cohort identity check for one schema-10 admission read.

use rewrite_model::{
    CandidateGenerationEvidenceBundleId, GenerationQualificationPlanId, PlannedCandidateAttemptId,
};
use rusqlite::{Connection, params};

use super::super::super::candidate_generation_evidence_storage::CandidateGenerationEvidenceStorageRootId;
use super::super::CandidateGenerationAttemptAdmissionClassV1;
use crate::StoreResult;
pub(super) fn completed_chain(
    connection: &Connection,
    plan_id: &GenerationQualificationPlanId,
    attempt_id: &PlannedCandidateAttemptId,
    storage_root_id: &CandidateGenerationEvidenceStorageRootId,
) -> StoreResult<(
    CandidateGenerationAttemptAdmissionClassV1,
    Option<CandidateGenerationEvidenceBundleId>,
)> {
    let mut statement = connection.prepare(
        "SELECT
             precursors.candidate_generation_attempt_precursor_id,
             records.candidate_generation_attempt_precursor_id,
             records.candidate_generation_receipt_id,
             managed.managed_candidate_generation_evidence_id,
             managed.response_id,
             cleanup.candidate_generation_cleanup_id,
             cleanup.managed_candidate_generation_evidence_id,
             bundles.candidate_generation_evidence_bundle_id,
             bundles.managed_candidate_generation_evidence_id,
             bundles.response_id,
             bundles.candidate_generation_cleanup_id,
             storage.storage_root_id,
             storage.relative_reference,
             readbacks.candidate_generation_evidence_bundle_readback_id,
             receipts.candidate_generation_receipt_id,
             receipts.candidate_generation_attempt_precursor_id,
             receipts.managed_candidate_generation_evidence_id,
             receipts.candidate_generation_cleanup_id,
             receipts.candidate_generation_evidence_bundle_id,
             receipts.candidate_generation_evidence_bundle_readback_id
         FROM candidate_generation_attempt_precursors AS precursors
         JOIN candidate_generation_attempt_records AS records
           ON records.generation_qualification_plan_id = precursors.generation_qualification_plan_id
          AND records.planned_candidate_attempt_id = precursors.planned_candidate_attempt_id
         JOIN managed_candidate_generation_evidence AS managed
           ON managed.candidate_generation_attempt_precursor_id =
               precursors.candidate_generation_attempt_precursor_id
         JOIN candidate_generation_cleanup_records AS cleanup
           ON cleanup.candidate_generation_attempt_precursor_id =
               precursors.candidate_generation_attempt_precursor_id
          AND cleanup.managed_candidate_generation_evidence_id =
               managed.managed_candidate_generation_evidence_id
         JOIN generation_evidence_bundles AS bundles
           ON bundles.candidate_generation_attempt_precursor_id =
               precursors.candidate_generation_attempt_precursor_id
          AND bundles.managed_candidate_generation_evidence_id =
               managed.managed_candidate_generation_evidence_id
          AND bundles.candidate_generation_cleanup_id = cleanup.candidate_generation_cleanup_id
         JOIN generation_evidence_bundle_storage AS storage
           ON storage.generation_qualification_plan_id = precursors.generation_qualification_plan_id
          AND storage.planned_candidate_attempt_id = precursors.planned_candidate_attempt_id
          AND storage.candidate_generation_evidence_bundle_id =
               bundles.candidate_generation_evidence_bundle_id
         JOIN generation_evidence_bundle_readbacks AS readbacks
           ON readbacks.candidate_generation_evidence_bundle_id =
               bundles.candidate_generation_evidence_bundle_id
         JOIN candidate_generation_receipts AS receipts
           ON receipts.generation_qualification_plan_id = precursors.generation_qualification_plan_id
          AND receipts.planned_candidate_attempt_id = precursors.planned_candidate_attempt_id
         WHERE precursors.generation_qualification_plan_id = ?1
           AND precursors.planned_candidate_attempt_id = ?2
         LIMIT 2",
    )?;
    let rows = statement.query_map(
        params![plan_id.digest().as_str(), attempt_id.digest().as_str()],
        chain_from_row,
    )?;
    let mut chains = Vec::new();
    for row in rows {
        chains.push(row?);
    }
    Ok(interpret_completed_chain(
        plan_id,
        attempt_id,
        storage_root_id,
        &chains,
    ))
}

fn chain_from_row(row: &rusqlite::Row<'_>) -> Result<CompletedChain, rusqlite::Error> {
    Ok(CompletedChain {
        precursor_id: row.get(0)?,
        attempt_precursor_id: row.get(1)?,
        attempt_receipt_id: row.get(2)?,
        managed_id: row.get(3)?,
        managed_response_id: row.get(4)?,
        cleanup_id: row.get(5)?,
        cleanup_managed_id: row.get(6)?,
        bundle_id: row.get(7)?,
        bundle_managed_id: row.get(8)?,
        bundle_response_id: row.get(9)?,
        bundle_cleanup_id: row.get(10)?,
        storage_root_id: row.get(11)?,
        relative_reference: row.get(12)?,
        readback_id: row.get(13)?,
        receipt_id: row.get(14)?,
        receipt_precursor_id: row.get(15)?,
        receipt_managed_id: row.get(16)?,
        receipt_cleanup_id: row.get(17)?,
        receipt_bundle_id: row.get(18)?,
        receipt_readback_id: row.get(19)?,
    })
}

struct CompletedChain {
    precursor_id: String,
    attempt_precursor_id: String,
    attempt_receipt_id: String,
    managed_id: String,
    managed_response_id: String,
    cleanup_id: String,
    cleanup_managed_id: String,
    bundle_id: String,
    bundle_managed_id: String,
    bundle_response_id: String,
    bundle_cleanup_id: String,
    storage_root_id: String,
    relative_reference: String,
    readback_id: String,
    receipt_id: String,
    receipt_precursor_id: String,
    receipt_managed_id: String,
    receipt_cleanup_id: String,
    receipt_bundle_id: String,
    receipt_readback_id: String,
}

fn interpret_completed_chain(
    plan_id: &GenerationQualificationPlanId,
    attempt_id: &PlannedCandidateAttemptId,
    storage_root_id: &CandidateGenerationEvidenceStorageRootId,
    chains: &[CompletedChain],
) -> (
    CandidateGenerationAttemptAdmissionClassV1,
    Option<CandidateGenerationEvidenceBundleId>,
) {
    let Some(chain) = chains.first() else {
        return (CandidateGenerationAttemptAdmissionClassV1::Ambiguous, None);
    };
    if chains.len() != 1 || !chain_ids_agree(chain) {
        return (CandidateGenerationAttemptAdmissionClassV1::Corrupt, None);
    }
    let expected_reference = format!(
        "bundles/v1/{}/{}/{}",
        plan_id.digest().as_str(),
        attempt_id.digest().as_str(),
        chain.bundle_id
    );
    if chain.relative_reference != expected_reference {
        return (CandidateGenerationAttemptAdmissionClassV1::Corrupt, None);
    }
    if chain.storage_root_id != storage_root_id.as_str() {
        return (CandidateGenerationAttemptAdmissionClassV1::Ambiguous, None);
    }
    match parse_bundle_id(&chain.bundle_id) {
        Some(bundle_id) => (
            CandidateGenerationAttemptAdmissionClassV1::TerminalCompleted,
            Some(bundle_id),
        ),
        None => (CandidateGenerationAttemptAdmissionClassV1::Corrupt, None),
    }
}

fn chain_ids_agree(chain: &CompletedChain) -> bool {
    chain.attempt_precursor_id == chain.precursor_id
        && chain.receipt_precursor_id == chain.precursor_id
        && chain.attempt_receipt_id == chain.receipt_id
        && chain.cleanup_managed_id == chain.managed_id
        && chain.bundle_managed_id == chain.managed_id
        && chain.receipt_managed_id == chain.managed_id
        && chain.bundle_response_id == chain.managed_response_id
        && chain.bundle_cleanup_id == chain.cleanup_id
        && chain.receipt_cleanup_id == chain.cleanup_id
        && chain.receipt_bundle_id == chain.bundle_id
        && chain.receipt_readback_id == chain.readback_id
}

fn parse_bundle_id(value: &str) -> Option<CandidateGenerationEvidenceBundleId> {
    serde_json::from_value(serde_json::Value::String(value.to_owned())).ok()
}
