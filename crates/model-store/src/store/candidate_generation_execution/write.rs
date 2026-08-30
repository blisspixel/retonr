use rewrite_model::CandidateGenerationAttemptOutcomeV1;
use rusqlite::{Connection, params};

use super::{
    CandidateGenerationExecutionV1Input, CandidateGenerationExecutionV1WriteDisposition,
    WriteDisposition, codec::EncodedExecution,
};
use crate::{StoreError, StoreResult};

#[expect(
    clippy::large_types_passed_by_value,
    reason = "the copyable input is one exact transaction closure"
)]
pub(super) fn write_execution(
    connection: &Connection,
    input: CandidateGenerationExecutionV1Input<'_>,
    encoded: &EncodedExecution,
) -> StoreResult<CandidateGenerationExecutionV1WriteDisposition> {
    match input {
        CandidateGenerationExecutionV1Input::Failed { .. } => {
            write_failed(connection, input, encoded)
        }
        CandidateGenerationExecutionV1Input::Completed { .. } => {
            write_completed(connection, input, encoded)
        }
    }
}

#[expect(
    clippy::large_types_passed_by_value,
    reason = "the copyable input is one exact failed closure"
)]
fn write_failed(
    connection: &Connection,
    input: CandidateGenerationExecutionV1Input<'_>,
    encoded: &EncodedExecution,
) -> StoreResult<CandidateGenerationExecutionV1WriteDisposition> {
    let CandidateGenerationExecutionV1Input::Failed {
        preregistration,
        precursor,
        attempt,
    } = input
    else {
        return Err(StoreError::CorruptRecord);
    };
    let attempt_disposition = insert_attempt(
        connection,
        preregistration
            .operation_policy_relations
            .plan
            .qualification_plan_id()
            .digest(),
        attempt,
        &encoded.attempt,
    )?;
    Ok(CandidateGenerationExecutionV1WriteDisposition {
        precursor: precursor.map(|_| WriteDisposition::AlreadyPresent),
        managed_evidence: None,
        cleanup: None,
        bundle: None,
        storage: None,
        readback: None,
        receipt: None,
        attempt: attempt_disposition,
    })
}

#[expect(
    clippy::large_types_passed_by_value,
    reason = "the copyable input is one exact completed closure"
)]
fn write_completed(
    connection: &Connection,
    input: CandidateGenerationExecutionV1Input<'_>,
    encoded: &EncodedExecution,
) -> StoreResult<CandidateGenerationExecutionV1WriteDisposition> {
    let CandidateGenerationExecutionV1Input::Completed {
        preregistration,
        managed_evidence,
        cleanup,
        bundle,
        storage,
        readback,
        receipt,
        attempt,
        ..
    } = input
    else {
        return Err(StoreError::CorruptRecord);
    };
    let managed = insert_managed(
        connection,
        managed_evidence,
        encoded
            .managed_evidence
            .as_deref()
            .ok_or(StoreError::CorruptRecord)?,
    )?;
    let cleanup = insert_cleanup(
        connection,
        cleanup,
        encoded
            .cleanup
            .as_deref()
            .ok_or(StoreError::CorruptRecord)?,
    )?;
    let bundle = insert_bundle(
        connection,
        bundle,
        encoded.bundle.as_deref().ok_or(StoreError::CorruptRecord)?,
    )?;
    let storage = insert_storage(connection, storage)?;
    let readback = insert_readback(
        connection,
        readback,
        encoded
            .readback
            .as_deref()
            .ok_or(StoreError::CorruptRecord)?,
    )?;
    let receipt = insert_receipt(
        connection,
        receipt,
        encoded
            .receipt
            .as_deref()
            .ok_or(StoreError::CorruptRecord)?,
    )?;
    let attempt = insert_attempt(
        connection,
        preregistration
            .operation_policy_relations
            .plan
            .qualification_plan_id()
            .digest(),
        attempt,
        &encoded.attempt,
    )?;
    require_uniform_cohort(&[
        managed, cleanup, bundle, storage, readback, receipt, attempt,
    ])?;
    Ok(CandidateGenerationExecutionV1WriteDisposition {
        precursor: Some(WriteDisposition::AlreadyPresent),
        managed_evidence: Some(managed),
        cleanup: Some(cleanup),
        bundle: Some(bundle),
        storage: Some(storage),
        readback: Some(readback),
        receipt: Some(receipt),
        attempt,
    })
}

fn require_uniform_cohort(dispositions: &[WriteDisposition]) -> StoreResult<()> {
    let first = dispositions.first().ok_or(StoreError::CorruptRecord)?;
    if dispositions.iter().all(|value| value == first) {
        Ok(())
    } else {
        Err(StoreError::ImmutableConflict)
    }
}

fn insert_managed(
    connection: &Connection,
    value: &rewrite_model::ManagedOllamaCandidateGenerationEvidenceV2,
    canonical_json: &[u8],
) -> StoreResult<WriteDisposition> {
    insert(
        connection,
        "INSERT OR IGNORE INTO managed_candidate_generation_evidence (
        managed_candidate_generation_evidence_id, candidate_generation_attempt_precursor_id,
        bracket_observation_v1_id, effective_package_evidence_v2_id,
        effective_runtime_state_id, effective_runtime_state_join_id,
        generation_request_binding_id, structured_request_binding_id, response_id, canonical_json
    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            digest(value.managed_evidence_v2_id().digest()),
            digest(value.precursor_id().digest()),
            digest(value.bracket_observation_v1_id().digest()),
            digest(value.effective_package_evidence_v2_id().digest()),
            digest(value.effective_runtime_state_id().digest()),
            digest(value.effective_runtime_state_join_id().digest()),
            digest(value.generation_request_binding_id().digest()),
            digest(value.structured_request_binding_id().digest()),
            digest(value.response_id().digest()),
            canonical_json,
        ],
    )
}

fn insert_cleanup(
    connection: &Connection,
    value: &rewrite_model::CandidateGenerationCleanupRecordV1,
    canonical_json: &[u8],
) -> StoreResult<WriteDisposition> {
    insert(
        connection,
        "INSERT OR IGNORE INTO candidate_generation_cleanup_records (
        candidate_generation_cleanup_id, candidate_generation_attempt_precursor_id,
        managed_candidate_generation_evidence_id, canonical_json
    ) VALUES (?1, ?2, ?3, ?4)",
        params![
            digest(value.cleanup_id().digest()),
            digest(value.precursor_id().digest()),
            digest(value.managed_evidence_id().digest()),
            canonical_json,
        ],
    )
}

fn insert_bundle(
    connection: &Connection,
    value: &rewrite_model::CandidateGenerationEvidenceBundleManifestV1,
    canonical_json: &[u8],
) -> StoreResult<WriteDisposition> {
    insert(
        connection,
        "INSERT OR IGNORE INTO generation_evidence_bundles (
        candidate_generation_evidence_bundle_id, candidate_generation_attempt_precursor_id,
        managed_candidate_generation_evidence_id, response_id,
        candidate_generation_cleanup_id, canonical_json
    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            digest(value.evidence_bundle_id().digest()),
            digest(value.precursor_id().digest()),
            digest(value.managed_evidence_id().digest()),
            digest(value.response_id().digest()),
            digest(value.cleanup_id().digest()),
            canonical_json,
        ],
    )
}

fn insert_storage(
    connection: &Connection,
    value: &super::CandidateGenerationEvidenceBundleStorageV1,
) -> StoreResult<WriteDisposition> {
    let limits = value.limits();
    insert(
        connection,
        "INSERT OR IGNORE INTO generation_evidence_bundle_storage (
        candidate_generation_evidence_bundle_id, generation_qualification_plan_id,
        planned_candidate_attempt_id, storage_root_id, relative_reference,
        maximum_tree_entries, maximum_tree_depth, maximum_aggregate_bytes
    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            digest(value.evidence_bundle_id().digest()),
            digest(value.qualification_plan_id().digest()),
            digest(value.planned_attempt_id().digest()),
            value.storage_root_id().as_str(),
            value.relative_reference().as_str(),
            i64::from(limits.maximum_tree_entries()),
            i64::from(limits.maximum_tree_depth()),
            i64::try_from(limits.maximum_aggregate_bytes())
                .map_err(|_| StoreError::RecordTooLarge)?,
        ],
    )
}

fn insert_readback(
    connection: &Connection,
    value: &rewrite_model::CandidateGenerationEvidenceBundleReadbackV1,
    canonical_json: &[u8],
) -> StoreResult<WriteDisposition> {
    insert(
        connection,
        "INSERT OR IGNORE INTO generation_evidence_bundle_readbacks (
        candidate_generation_evidence_bundle_readback_id,
        candidate_generation_evidence_bundle_id, canonical_json
    ) VALUES (?1, ?2, ?3)",
        params![
            digest(value.readback_id().digest()),
            digest(value.bundle_id().digest()),
            canonical_json
        ],
    )
}

fn insert_receipt(
    connection: &Connection,
    value: &rewrite_model::CandidateGenerationReceiptV1,
    canonical_json: &[u8],
) -> StoreResult<WriteDisposition> {
    insert(
        connection,
        "INSERT OR IGNORE INTO candidate_generation_receipts (
        candidate_generation_receipt_id, generation_qualification_plan_id,
        planned_candidate_attempt_id, candidate_generation_attempt_precursor_id,
        managed_candidate_generation_evidence_id, candidate_generation_cleanup_id,
        candidate_generation_evidence_bundle_id,
        candidate_generation_evidence_bundle_readback_id, canonical_json
    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            digest(value.receipt_id().digest()),
            digest(value.qualification_plan_id().digest()),
            digest(value.planned_attempt_id().digest()),
            digest(value.precursor_id().digest()),
            digest(value.managed_evidence_id().digest()),
            digest(value.cleanup_id().digest()),
            digest(value.bundle_id().digest()),
            digest(value.readback_id().digest()),
            canonical_json,
        ],
    )
}

fn insert_attempt(
    connection: &Connection,
    plan_id: &impl std::fmt::Display,
    value: &rewrite_model::CandidateGenerationAttemptRecordV1,
    canonical_json: &[u8],
) -> StoreResult<WriteDisposition> {
    let (outcome, planned_id, precursor_id, receipt_id) = match value.outcome() {
        CandidateGenerationAttemptOutcomeV1::Completed {
            planned_attempt_id,
            precursor_id,
            receipt_id,
        } => (
            "completed",
            planned_attempt_id.digest(),
            Some(precursor_id.digest()),
            Some(receipt_id.digest()),
        ),
        CandidateGenerationAttemptOutcomeV1::Failed {
            planned_attempt_id,
            precursor_id,
            ..
        } => (
            "failed",
            planned_attempt_id.digest(),
            precursor_id
                .as_ref()
                .map(rewrite_model::CandidateGenerationAttemptPrecursorId::digest),
            None,
        ),
    };
    insert(
        connection,
        "INSERT OR IGNORE INTO candidate_generation_attempt_records (
        candidate_generation_attempt_record_id, generation_qualification_plan_id,
        planned_candidate_attempt_id, outcome, candidate_generation_attempt_precursor_id,
        candidate_generation_receipt_id, canonical_json
    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            digest(value.attempt_record_id().digest()),
            digest(plan_id),
            digest(planned_id),
            outcome,
            precursor_id.map(digest),
            receipt_id.map(digest),
            canonical_json,
        ],
    )
}

fn insert(
    connection: &Connection,
    sql: &str,
    parameters: impl rusqlite::Params,
) -> StoreResult<WriteDisposition> {
    match connection.execute(sql, parameters)? {
        1 => Ok(WriteDisposition::Inserted),
        0 => Ok(WriteDisposition::AlreadyPresent),
        _ => Err(StoreError::CorruptRecord),
    }
}

fn digest(value: &impl std::fmt::Display) -> String {
    value.to_string()
}
