use rewrite_model::{
    ArtifactSetRelativePath, CandidateGenerationAttemptOutcomeV1,
    CandidateGenerationAttemptRecordV1, CandidateGenerationEvidenceBundleManifestV1,
    CandidateGenerationEvidenceBundleManifestV1Relations,
    CandidateGenerationEvidenceBundleReadbackV1, CandidateGenerationReceiptV1,
    CandidateGenerationReceiptV1Relations, MAX_CANDIDATE_GENERATION_ATTEMPT_RECORD_JSON_BYTES,
    MAX_CANDIDATE_GENERATION_CLEANUP_JSON_BYTES,
    MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_JSON_BYTES,
    MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_READBACK_JSON_BYTES,
    MAX_CANDIDATE_GENERATION_RECEIPT_JSON_BYTES,
    MAX_MANAGED_OLLAMA_CANDIDATE_GENERATION_EVIDENCE_V2_JSON_BYTES,
    ManagedOllamaCandidateGenerationEvidenceV2,
    ManagedOllamaCandidateGenerationEvidenceV2Relations,
};
use rusqlite::Connection;

use super::{
    CandidateGenerationExecutionV1Input, CandidateGenerationExecutionV1ReadInput,
    StoredCandidateGenerationExecutionV1,
};
use crate::store::candidate_generation_attempt_precursor::load_precursor;
use crate::store::candidate_generation_evidence_storage::{
    CandidateGenerationEvidenceBundleStorageV1, CandidateGenerationEvidenceStorageRootId,
    CandidateGenerationEvidenceStorageV1Limits,
};
use crate::store::generation_qualification_preregistration::load_preregistration;
use crate::store::generation_system_foundation::load_foundation as load_system_foundation;
use crate::{StoreError, StoreResult};

mod rows;
mod validation;

use validation::{
    require_exact_json, require_id, require_indexes, required_json_row, validate_managed_row,
};

#[expect(
    clippy::large_types_passed_by_value,
    reason = "the copyable input is one exact transaction closure"
)]
pub(super) fn read_input(
    input: CandidateGenerationExecutionV1Input<'_>,
) -> CandidateGenerationExecutionV1ReadInput<'_> {
    match input {
        CandidateGenerationExecutionV1Input::Failed {
            preregistration,
            attempt,
            ..
        } => CandidateGenerationExecutionV1ReadInput::Failed {
            preregistration,
            planned_attempt_id: super::codec::attempt_planned_id(attempt),
        },
        CandidateGenerationExecutionV1Input::Completed {
            preregistration,
            managed_evidence_input,
            attempt,
            ..
        } => CandidateGenerationExecutionV1ReadInput::Completed {
            preregistration,
            planned_attempt_id: super::codec::attempt_planned_id(attempt),
            managed_evidence_input,
        },
    }
}

pub(super) fn load_execution(
    connection: &Connection,
    input: CandidateGenerationExecutionV1ReadInput<'_>,
) -> StoreResult<Option<StoredCandidateGenerationExecutionV1>> {
    let preregistration = load_preregistration(connection, input.preregistration())?
        .ok_or(StoreError::MissingRecord)?;
    let foundation = preregistration.plan_foundation();
    let planned =
        super::codec::selected_attempt(foundation.planned_attempts(), input.planned_attempt_id())?;
    let Some(attempt_row) = rows::load_attempt(
        connection,
        planned.planned_attempt_id().digest().as_str(),
        MAX_CANDIDATE_GENERATION_ATTEMPT_RECORD_JSON_BYTES,
    )?
    else {
        return Ok(None);
    };
    require_indexes(
        &attempt_row.value,
        attempt_row.value.id.as_str(),
        &[
            foundation.plan().qualification_plan_id().digest().as_str(),
            planned.planned_attempt_id().digest().as_str(),
        ],
    )?;
    match input {
        CandidateGenerationExecutionV1ReadInput::Failed { .. } => {
            load_failed(connection, preregistration.clone(), planned, &attempt_row).map(Some)
        }
        CandidateGenerationExecutionV1ReadInput::Completed {
            managed_evidence_input,
            ..
        } => load_completed(
            connection,
            preregistration.clone(),
            planned,
            managed_evidence_input,
            &attempt_row,
        )
        .map(Some),
    }
}

fn load_failed(
    connection: &Connection,
    preregistration: crate::StoredGenerationQualificationPreregistration,
    planned: &rewrite_model::PlannedCandidateAttemptV1,
    row: &rows::AttemptRow,
) -> StoreResult<StoredCandidateGenerationExecutionV1> {
    if row.outcome != "failed" || row.receipt_id.is_some() {
        return Err(StoreError::ImmutableConflict);
    }
    let foundation = preregistration.plan_foundation();
    let stored_precursor = load_precursor(
        connection,
        foundation,
        foundation.plan().qualification_plan_id(),
        planned.planned_attempt_id(),
    )?;
    match (&stored_precursor, &row.precursor_id) {
        (None, None) => {}
        (Some(value), Some(id)) if value.precursor_id().digest().as_str() == id => {}
        _ => return Err(StoreError::CorruptRecord),
    }
    let preflight =
        CandidateGenerationAttemptRecordV1::preflight_json_bytes(&row.value.canonical_json)
            .map_err(|_| StoreError::CorruptRecord)?;
    let attempt = CandidateGenerationAttemptRecordV1::from_preflight(
        preflight,
        planned,
        stored_precursor.as_ref(),
        None,
    )
    .map_err(|_| StoreError::CorruptRecord)?;
    if !matches!(
        attempt.outcome(),
        CandidateGenerationAttemptOutcomeV1::Failed { .. }
    ) {
        return Err(StoreError::CorruptRecord);
    }
    require_exact_json(&attempt, &row.value.canonical_json)?;
    require_id(&row.value.id, attempt.attempt_record_id().digest().as_str())?;
    Ok(StoredCandidateGenerationExecutionV1::Failed {
        preregistration,
        precursor: stored_precursor,
        attempt,
    })
}

#[expect(
    clippy::too_many_lines,
    reason = "one ordered precursor through cleanup relationship validation"
)]
fn load_completed(
    connection: &Connection,
    preregistration: crate::StoredGenerationQualificationPreregistration,
    planned: &rewrite_model::PlannedCandidateAttemptV1,
    managed_input: &rewrite_model::ManagedOllamaCandidateGenerationEvidenceV2Input,
    attempt_row: &rows::AttemptRow,
) -> StoreResult<StoredCandidateGenerationExecutionV1> {
    if attempt_row.outcome != "completed" {
        return Err(StoreError::ImmutableConflict);
    }
    let precursor_id = attempt_row
        .precursor_id
        .as_deref()
        .ok_or(StoreError::CorruptRecord)?;
    let receipt_id = attempt_row
        .receipt_id
        .as_deref()
        .ok_or(StoreError::CorruptRecord)?;
    let foundation = preregistration.plan_foundation();
    let precursor = load_precursor(
        connection,
        foundation,
        foundation.plan().qualification_plan_id(),
        planned.planned_attempt_id(),
    )?
    .ok_or(StoreError::CorruptRecord)?;
    require_id(precursor_id, precursor.precursor_id().digest().as_str())?;
    let receipt_row = required_json_row(
        connection,
        "candidate_generation_receipts",
        "candidate_generation_receipt_id",
        receipt_id,
        &[
            "generation_qualification_plan_id",
            "planned_candidate_attempt_id",
            "candidate_generation_attempt_precursor_id",
            "managed_candidate_generation_evidence_id",
            "candidate_generation_cleanup_id",
            "candidate_generation_evidence_bundle_id",
            "candidate_generation_evidence_bundle_readback_id",
        ],
        MAX_CANDIDATE_GENERATION_RECEIPT_JSON_BYTES,
    )?;
    let chain = CompletionChain::from_receipt_row(&receipt_row)?;
    let system = foundation
        .generation_systems()
        .iter()
        .find(|value| value.generation_system_id() == planned.generation_system_id())
        .ok_or(StoreError::CorruptRecord)?;
    let system_foundation = load_system_foundation(connection, system.generation_system_id())?
        .ok_or(StoreError::CorruptRecord)?;
    if system_foundation.generation_system() != system {
        return Err(StoreError::CorruptRecord);
    }
    let managed_row = required_json_row(
        connection,
        "managed_candidate_generation_evidence",
        "managed_candidate_generation_evidence_id",
        &chain.managed_id,
        &[
            "candidate_generation_attempt_precursor_id",
            "bracket_observation_v1_id",
            "effective_package_evidence_v2_id",
            "effective_runtime_state_id",
            "effective_runtime_state_join_id",
            "generation_request_binding_id",
            "structured_request_binding_id",
            "response_id",
        ],
        MAX_MANAGED_OLLAMA_CANDIDATE_GENERATION_EVIDENCE_V2_JSON_BYTES,
    )?;
    let managed_evidence = ManagedOllamaCandidateGenerationEvidenceV2::from_json_bytes(
        &managed_row.canonical_json,
        ManagedOllamaCandidateGenerationEvidenceV2Relations {
            precursor: &precursor,
            planned_attempt: planned,
            generation_system: system,
            effective_package_evidence_v2: system_foundation.effective_package_evidence_v2(),
        },
        managed_input,
    )
    .map_err(|_| StoreError::CorruptRecord)?;
    validate_managed_row(&managed_row, &managed_evidence)?;
    let cleanup_row = required_json_row(
        connection,
        "candidate_generation_cleanup_records",
        "candidate_generation_cleanup_id",
        &chain.cleanup_id,
        &[
            "candidate_generation_attempt_precursor_id",
            "managed_candidate_generation_evidence_id",
        ],
        MAX_CANDIDATE_GENERATION_CLEANUP_JSON_BYTES,
    )?;
    let cleanup = rewrite_model::CandidateGenerationCleanupRecordV1::from_json_bytes(
        &cleanup_row.canonical_json,
        &precursor,
        &managed_evidence,
    )
    .map_err(|_| StoreError::CorruptRecord)?;
    require_indexes(
        &cleanup_row,
        cleanup.cleanup_id().digest().as_str(),
        &[
            cleanup.precursor_id().digest().as_str(),
            cleanup.managed_evidence_id().digest().as_str(),
        ],
    )?;
    require_exact_json(&cleanup, &cleanup_row.canonical_json)?;
    load_completion_tail(
        connection,
        preregistration,
        planned,
        precursor,
        managed_evidence,
        cleanup,
        &receipt_row,
        &chain,
        attempt_row,
    )
}

#[expect(
    clippy::struct_field_names,
    reason = "each persisted completion-chain field is an identity"
)]
struct CompletionChain {
    managed_id: String,
    cleanup_id: String,
    bundle_id: String,
    readback_id: String,
}

impl CompletionChain {
    fn from_receipt_row(row: &rows::JsonRow) -> StoreResult<Self> {
        if row.indexes.len() != 7 {
            return Err(StoreError::CorruptRecord);
        }
        Ok(Self {
            managed_id: row.indexes[3].clone(),
            cleanup_id: row.indexes[4].clone(),
            bundle_id: row.indexes[5].clone(),
            readback_id: row.indexes[6].clone(),
        })
    }
}

#[expect(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "one exact completed relationship closure"
)]
fn load_completion_tail(
    connection: &Connection,
    preregistration: crate::StoredGenerationQualificationPreregistration,
    planned: &rewrite_model::PlannedCandidateAttemptV1,
    precursor: rewrite_model::CandidateGenerationAttemptPrecursorV1,
    managed_evidence: ManagedOllamaCandidateGenerationEvidenceV2,
    cleanup: rewrite_model::CandidateGenerationCleanupRecordV1,
    receipt_row: &rows::JsonRow,
    chain: &CompletionChain,
    attempt_row: &rows::AttemptRow,
) -> StoreResult<StoredCandidateGenerationExecutionV1> {
    let bundle_row = required_json_row(
        connection,
        "generation_evidence_bundles",
        "candidate_generation_evidence_bundle_id",
        &chain.bundle_id,
        &[
            "candidate_generation_attempt_precursor_id",
            "managed_candidate_generation_evidence_id",
            "response_id",
            "candidate_generation_cleanup_id",
        ],
        MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_JSON_BYTES,
    )?;
    let bundle_preflight = CandidateGenerationEvidenceBundleManifestV1::preflight_json_bytes(
        &bundle_row.canonical_json,
    )
    .map_err(|_| StoreError::CorruptRecord)?;
    let response_artifact = bundle_preflight
        .structured_response_artifact()
        .map_err(|_| StoreError::CorruptRecord)?;
    let foundation = preregistration.plan_foundation();
    let bundle = CandidateGenerationEvidenceBundleManifestV1::from_preflight(
        bundle_preflight,
        CandidateGenerationEvidenceBundleManifestV1Relations {
            qualification_plan: foundation.plan(),
            planned_attempt: planned,
            precursor: &precursor,
            managed_evidence: &managed_evidence,
            cleanup: &cleanup,
            structured_response_artifact: &response_artifact,
        },
    )
    .map_err(|_| StoreError::CorruptRecord)?;
    require_indexes(
        &bundle_row,
        bundle.evidence_bundle_id().digest().as_str(),
        &[
            bundle.precursor_id().digest().as_str(),
            bundle.managed_evidence_id().digest().as_str(),
            bundle.response_id().digest().as_str(),
            bundle.cleanup_id().digest().as_str(),
        ],
    )?;
    require_exact_json(&bundle, &bundle_row.canonical_json)?;
    let storage = load_storage(connection, foundation, planned, &bundle)?;
    let readback_row = required_json_row(
        connection,
        "generation_evidence_bundle_readbacks",
        "candidate_generation_evidence_bundle_readback_id",
        &chain.readback_id,
        &["candidate_generation_evidence_bundle_id"],
        MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_READBACK_JSON_BYTES,
    )?;
    let readback = CandidateGenerationEvidenceBundleReadbackV1::from_json_bytes(
        &readback_row.canonical_json,
        &bundle,
    )
    .map_err(|_| StoreError::CorruptRecord)?;
    require_indexes(
        &readback_row,
        readback.readback_id().digest().as_str(),
        &[readback.bundle_id().digest().as_str()],
    )?;
    require_exact_json(&readback, &readback_row.canonical_json)?;
    let system = foundation
        .generation_systems()
        .iter()
        .find(|value| value.generation_system_id() == planned.generation_system_id())
        .ok_or(StoreError::CorruptRecord)?;
    let (case, cluster, repetition) = super::codec::selected_case_relations(foundation, planned)?;
    let receipt_preflight =
        CandidateGenerationReceiptV1::preflight_json_bytes(&receipt_row.canonical_json)
            .map_err(|_| StoreError::CorruptRecord)?;
    let usage = receipt_preflight.usage_observation();
    let receipt = CandidateGenerationReceiptV1::from_preflight(
        receipt_preflight,
        CandidateGenerationReceiptV1Relations {
            qualification_plan: foundation.plan(),
            suite: foundation.suite(),
            case,
            cluster,
            repetition,
            planned_attempt: planned,
            precursor: &precursor,
            generation_system: system,
            managed_evidence: &managed_evidence,
            cleanup: &cleanup,
            bundle: &bundle,
            readback: &readback,
        },
        usage,
    )
    .map_err(|_| StoreError::CorruptRecord)?;
    require_indexes(
        receipt_row,
        receipt.receipt_id().digest().as_str(),
        &[
            receipt.qualification_plan_id().digest().as_str(),
            receipt.planned_attempt_id().digest().as_str(),
            receipt.precursor_id().digest().as_str(),
            receipt.managed_evidence_id().digest().as_str(),
            receipt.cleanup_id().digest().as_str(),
            receipt.bundle_id().digest().as_str(),
            receipt.readback_id().digest().as_str(),
        ],
    )?;
    require_exact_json(&receipt, &receipt_row.canonical_json)?;
    let attempt_preflight =
        CandidateGenerationAttemptRecordV1::preflight_json_bytes(&attempt_row.value.canonical_json)
            .map_err(|_| StoreError::CorruptRecord)?;
    let attempt = CandidateGenerationAttemptRecordV1::from_preflight(
        attempt_preflight,
        planned,
        Some(&precursor),
        Some(&receipt),
    )
    .map_err(|_| StoreError::CorruptRecord)?;
    require_id(
        &attempt_row.value.id,
        attempt.attempt_record_id().digest().as_str(),
    )?;
    require_exact_json(&attempt, &attempt_row.value.canonical_json)?;
    Ok(StoredCandidateGenerationExecutionV1::Completed {
        preregistration,
        precursor,
        managed_evidence,
        cleanup,
        bundle,
        storage,
        readback,
        receipt,
        attempt,
    })
}

fn load_storage(
    connection: &Connection,
    foundation: &crate::GenerationQualificationPlanFoundationV1,
    planned: &rewrite_model::PlannedCandidateAttemptV1,
    bundle: &CandidateGenerationEvidenceBundleManifestV1,
) -> StoreResult<CandidateGenerationEvidenceBundleStorageV1> {
    let row = rows::load_storage(connection, bundle.evidence_bundle_id().digest().as_str())?
        .ok_or(StoreError::CorruptRecord)?;
    let expected = [
        bundle.evidence_bundle_id().digest().as_str(),
        foundation.plan().qualification_plan_id().digest().as_str(),
        planned.planned_attempt_id().digest().as_str(),
    ];
    if row.indexes.len() != 4 || row.indexes[..3].iter().map(String::as_str).ne(expected) {
        return Err(StoreError::CorruptRecord);
    }
    let root_id = CandidateGenerationEvidenceStorageRootId::new(row.indexes[3].clone())
        .map_err(|_| StoreError::CorruptRecord)?;
    let relative = ArtifactSetRelativePath::new(row.relative_reference)
        .map_err(|_| StoreError::CorruptRecord)?;
    let limits = CandidateGenerationEvidenceStorageV1Limits::new(
        u32::try_from(row.maximum_tree_entries).map_err(|_| StoreError::CorruptRecord)?,
        u16::try_from(row.maximum_tree_depth).map_err(|_| StoreError::CorruptRecord)?,
        u64::try_from(row.maximum_aggregate_bytes).map_err(|_| StoreError::CorruptRecord)?,
    )
    .map_err(|_| StoreError::CorruptRecord)?;
    CandidateGenerationEvidenceBundleStorageV1::from_stored_reference(
        root_id,
        foundation.plan().qualification_plan_id().clone(),
        planned.planned_attempt_id().clone(),
        bundle.evidence_bundle_id().clone(),
        relative,
        limits,
    )
    .map_err(|_| StoreError::CorruptRecord)
}
