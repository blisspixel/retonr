use rewrite_model::{
    CandidateGenerationAttemptOutcomeV1, CandidateGenerationAttemptRecordV1,
    CandidateGenerationEvidenceBundleManifestV1,
    CandidateGenerationEvidenceBundleManifestV1Relations,
    CandidateGenerationEvidenceBundleReadbackV1, CandidateGenerationReceiptV1,
    CandidateGenerationReceiptV1Relations, GenerationCaseManifestV1, GenerationClusterRecordV1,
    GenerationRepetitionRecordV1, MAX_CANDIDATE_GENERATION_ATTEMPT_RECORD_JSON_BYTES,
    MAX_CANDIDATE_GENERATION_CLEANUP_JSON_BYTES,
    MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_JSON_BYTES,
    MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_READBACK_JSON_BYTES,
    MAX_CANDIDATE_GENERATION_RECEIPT_JSON_BYTES,
    MAX_MANAGED_OLLAMA_CANDIDATE_GENERATION_EVIDENCE_V2_JSON_BYTES,
    ManagedOllamaCandidateGenerationEvidenceV2,
    ManagedOllamaCandidateGenerationEvidenceV2Relations, PlannedCandidateAttemptId,
    PlannedCandidateAttemptV1,
};
use rusqlite::Connection;

use super::{CandidateGenerationExecutionV1Input, StoredCandidateGenerationExecutionV1};
use crate::store::candidate_generation_attempt_precursor::load_precursor;
use crate::store::generation_qualification_preregistration::load_preregistration;
use crate::store::generation_system_foundation::load_foundation as load_system_foundation;
use crate::{StoreError, StoreResult};

pub(super) struct EncodedExecution {
    pub(super) managed_evidence: Option<Vec<u8>>,
    pub(super) cleanup: Option<Vec<u8>>,
    pub(super) bundle: Option<Vec<u8>>,
    pub(super) readback: Option<Vec<u8>>,
    pub(super) receipt: Option<Vec<u8>>,
    pub(super) attempt: Vec<u8>,
}

pub(super) fn attempt_planned_id(
    attempt: &CandidateGenerationAttemptRecordV1,
) -> &PlannedCandidateAttemptId {
    match attempt.outcome() {
        CandidateGenerationAttemptOutcomeV1::Completed {
            planned_attempt_id, ..
        }
        | CandidateGenerationAttemptOutcomeV1::Failed {
            planned_attempt_id, ..
        } => planned_attempt_id,
    }
}

#[expect(
    clippy::large_types_passed_by_value,
    reason = "the copyable input is one exact transaction closure"
)]
pub(super) fn encode(
    input: CandidateGenerationExecutionV1Input<'_>,
) -> StoreResult<EncodedExecution> {
    match input {
        CandidateGenerationExecutionV1Input::Failed { attempt, .. } => Ok(EncodedExecution {
            managed_evidence: None,
            cleanup: None,
            bundle: None,
            readback: None,
            receipt: None,
            attempt: encode_record(attempt, MAX_CANDIDATE_GENERATION_ATTEMPT_RECORD_JSON_BYTES)?,
        }),
        CandidateGenerationExecutionV1Input::Completed {
            managed_evidence,
            cleanup,
            bundle,
            readback,
            receipt,
            attempt,
            ..
        } => Ok(EncodedExecution {
            managed_evidence: Some(encode_record(
                managed_evidence,
                MAX_MANAGED_OLLAMA_CANDIDATE_GENERATION_EVIDENCE_V2_JSON_BYTES,
            )?),
            cleanup: Some(encode_record(
                cleanup,
                MAX_CANDIDATE_GENERATION_CLEANUP_JSON_BYTES,
            )?),
            bundle: Some(encode_record(
                bundle,
                MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_JSON_BYTES,
            )?),
            readback: Some(encode_record(
                readback,
                MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_READBACK_JSON_BYTES,
            )?),
            receipt: Some(encode_record(
                receipt,
                MAX_CANDIDATE_GENERATION_RECEIPT_JSON_BYTES,
            )?),
            attempt: encode_record(attempt, MAX_CANDIDATE_GENERATION_ATTEMPT_RECORD_JSON_BYTES)?,
        }),
    }
}

fn encode_record<T: serde::Serialize>(value: &T, maximum: usize) -> StoreResult<Vec<u8>> {
    let bytes = serde_json::to_vec(value)?;
    if bytes.is_empty() {
        Err(StoreError::CorruptRecord)
    } else if bytes.len() > maximum {
        Err(StoreError::RecordTooLarge)
    } else {
        Ok(bytes)
    }
}

#[expect(
    clippy::large_types_passed_by_value,
    clippy::too_many_lines,
    reason = "one copyable input and one ordered terminal trust-boundary validation"
)]
pub(super) fn validate_input(
    connection: &Connection,
    input: CandidateGenerationExecutionV1Input<'_>,
) -> StoreResult<()> {
    let preregistration = load_preregistration(connection, input.preregistration())?
        .ok_or(StoreError::MissingRecord)?;
    let foundation = preregistration.plan_foundation();
    let planned = selected_attempt(foundation.planned_attempts(), input.planned_attempt_id())?;
    let system = foundation
        .generation_systems()
        .iter()
        .find(|value| value.generation_system_id() == planned.generation_system_id())
        .ok_or(StoreError::CorruptRecord)?;
    let system_foundation = load_system_foundation(connection, system.generation_system_id())?
        .ok_or(StoreError::MissingRecord)?;
    if system_foundation.generation_system() != system {
        return Err(StoreError::CorruptRecord);
    }
    match input {
        CandidateGenerationExecutionV1Input::Failed {
            precursor, attempt, ..
        } => {
            let stored_precursor = load_optional_precursor(
                connection,
                foundation,
                planned,
                precursor.map(|value| value.precursor_id().digest().as_str()),
            )?;
            if stored_precursor.as_ref() != precursor {
                return Err(StoreError::ImmutableConflict);
            }
            let bytes = serde_json::to_vec(attempt)?;
            let preflight = CandidateGenerationAttemptRecordV1::preflight_json_bytes(&bytes)
                .map_err(StoreError::InvalidCandidateGenerationExecution)?;
            let decoded = CandidateGenerationAttemptRecordV1::from_preflight(
                preflight,
                planned,
                stored_precursor.as_ref(),
                None,
            )
            .map_err(StoreError::InvalidCandidateGenerationExecution)?;
            if &decoded != attempt
                || !matches!(
                    decoded.outcome(),
                    CandidateGenerationAttemptOutcomeV1::Failed { .. }
                )
            {
                return Err(StoreError::InvalidCandidateGenerationExecution(
                    rewrite_model::GenerationQualificationContractError::AttemptRecordRelationshipMismatch,
                ));
            }
            Ok(())
        }
        CandidateGenerationExecutionV1Input::Completed {
            precursor,
            managed_evidence,
            managed_evidence_input,
            cleanup,
            bundle,
            storage,
            readback,
            receipt,
            attempt,
            ..
        } => {
            let stored_precursor = load_precursor(
                connection,
                foundation,
                foundation.plan().qualification_plan_id(),
                planned.planned_attempt_id(),
            )?
            .ok_or(StoreError::MissingRecord)?;
            if &stored_precursor != precursor {
                return Err(StoreError::ImmutableConflict);
            }
            let managed_relations = ManagedOllamaCandidateGenerationEvidenceV2Relations {
                precursor,
                planned_attempt: planned,
                generation_system: system,
                effective_package_evidence_v2: system_foundation.effective_package_evidence_v2(),
            };
            let managed_bytes = serde_json::to_vec(managed_evidence)?;
            let decoded_managed = ManagedOllamaCandidateGenerationEvidenceV2::from_json_bytes(
                &managed_bytes,
                managed_relations,
                managed_evidence_input,
            )
            .map_err(StoreError::InvalidCandidateGenerationExecution)?;
            if &decoded_managed != managed_evidence {
                return Err(StoreError::ImmutableConflict);
            }
            let cleanup_bytes = serde_json::to_vec(cleanup)?;
            let decoded_cleanup =
                rewrite_model::CandidateGenerationCleanupRecordV1::from_json_bytes(
                    &cleanup_bytes,
                    precursor,
                    managed_evidence,
                )
                .map_err(StoreError::InvalidCandidateGenerationExecution)?;
            if &decoded_cleanup != cleanup {
                return Err(StoreError::ImmutableConflict);
            }
            let bundle_bytes = serde_json::to_vec(bundle)?;
            let bundle_preflight =
                CandidateGenerationEvidenceBundleManifestV1::preflight_json_bytes(&bundle_bytes)
                    .map_err(StoreError::InvalidCandidateGenerationExecution)?;
            let response_artifact = bundle_preflight
                .structured_response_artifact()
                .map_err(StoreError::InvalidCandidateGenerationExecution)?;
            let decoded_bundle = CandidateGenerationEvidenceBundleManifestV1::from_preflight(
                bundle_preflight,
                CandidateGenerationEvidenceBundleManifestV1Relations {
                    qualification_plan: foundation.plan(),
                    planned_attempt: planned,
                    precursor,
                    managed_evidence,
                    cleanup,
                    structured_response_artifact: &response_artifact,
                },
            )
            .map_err(StoreError::InvalidCandidateGenerationExecution)?;
            if &decoded_bundle != bundle
                || storage.qualification_plan_id() != foundation.plan().qualification_plan_id()
                || storage.planned_attempt_id() != planned.planned_attempt_id()
                || storage.evidence_bundle_id() != bundle.evidence_bundle_id()
            {
                return Err(StoreError::ImmutableConflict);
            }
            let readback_bytes = serde_json::to_vec(readback)?;
            let decoded_readback = CandidateGenerationEvidenceBundleReadbackV1::from_json_bytes(
                &readback_bytes,
                bundle,
            )
            .map_err(StoreError::InvalidCandidateGenerationExecution)?;
            if &decoded_readback != readback {
                return Err(StoreError::ImmutableConflict);
            }
            validate_receipt_and_attempt(
                foundation,
                planned,
                system,
                precursor,
                managed_evidence,
                cleanup,
                bundle,
                readback,
                receipt,
                attempt,
            )
        }
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "one exact receipt relationship closure"
)]
fn validate_receipt_and_attempt(
    foundation: &crate::GenerationQualificationPlanFoundationV1,
    planned: &PlannedCandidateAttemptV1,
    system: &rewrite_model::GenerationSystemRecordV1,
    precursor: &rewrite_model::CandidateGenerationAttemptPrecursorV1,
    managed: &rewrite_model::ManagedOllamaCandidateGenerationEvidenceV2,
    cleanup: &rewrite_model::CandidateGenerationCleanupRecordV1,
    bundle: &rewrite_model::CandidateGenerationEvidenceBundleManifestV1,
    readback: &rewrite_model::CandidateGenerationEvidenceBundleReadbackV1,
    receipt: &rewrite_model::CandidateGenerationReceiptV1,
    attempt: &rewrite_model::CandidateGenerationAttemptRecordV1,
) -> StoreResult<()> {
    let (case, cluster, repetition) = selected_case_relations(foundation, planned)?;
    let receipt_bytes = serde_json::to_vec(receipt)?;
    let receipt_preflight = CandidateGenerationReceiptV1::preflight_json_bytes(&receipt_bytes)
        .map_err(StoreError::InvalidCandidateGenerationExecution)?;
    let usage = receipt_preflight.usage_observation();
    let decoded_receipt = CandidateGenerationReceiptV1::from_preflight(
        receipt_preflight,
        CandidateGenerationReceiptV1Relations {
            qualification_plan: foundation.plan(),
            suite: foundation.suite(),
            case,
            cluster,
            repetition,
            planned_attempt: planned,
            precursor,
            generation_system: system,
            managed_evidence: managed,
            cleanup,
            bundle,
            readback,
        },
        usage,
    )
    .map_err(StoreError::InvalidCandidateGenerationExecution)?;
    let attempt_bytes = serde_json::to_vec(attempt)?;
    let attempt_preflight =
        CandidateGenerationAttemptRecordV1::preflight_json_bytes(&attempt_bytes)
            .map_err(StoreError::InvalidCandidateGenerationExecution)?;
    let decoded_attempt = CandidateGenerationAttemptRecordV1::from_preflight(
        attempt_preflight,
        planned,
        Some(precursor),
        Some(receipt),
    )
    .map_err(StoreError::InvalidCandidateGenerationExecution)?;
    if &decoded_receipt == receipt
        && &decoded_attempt == attempt
        && matches!(
            decoded_attempt.outcome(),
            CandidateGenerationAttemptOutcomeV1::Completed { .. }
        )
    {
        Ok(())
    } else {
        Err(StoreError::ImmutableConflict)
    }
}

pub(super) fn selected_attempt<'a>(
    attempts: &'a [PlannedCandidateAttemptV1],
    expected: &PlannedCandidateAttemptId,
) -> StoreResult<&'a PlannedCandidateAttemptV1> {
    attempts
        .iter()
        .find(|value| value.planned_attempt_id() == expected)
        .ok_or(StoreError::MissingRecord)
}

pub(super) fn selected_case_relations<'a>(
    foundation: &'a crate::GenerationQualificationPlanFoundationV1,
    planned: &PlannedCandidateAttemptV1,
) -> StoreResult<(
    &'a GenerationCaseManifestV1,
    &'a GenerationClusterRecordV1,
    &'a GenerationRepetitionRecordV1,
)> {
    let case = foundation
        .cases()
        .iter()
        .find(|value| value.case_id() == planned.case_id())
        .ok_or(StoreError::CorruptRecord)?;
    let cluster = foundation
        .clusters()
        .iter()
        .find(|value| value.cluster_id() == planned.cluster_id())
        .ok_or(StoreError::CorruptRecord)?;
    let repetition = foundation
        .repetitions()
        .iter()
        .find(|value| value.repetition_id() == planned.repetition_id())
        .ok_or(StoreError::CorruptRecord)?;
    Ok((case, cluster, repetition))
}

fn load_optional_precursor(
    connection: &Connection,
    foundation: &crate::GenerationQualificationPlanFoundationV1,
    planned: &PlannedCandidateAttemptV1,
    expected_id: Option<&str>,
) -> StoreResult<Option<rewrite_model::CandidateGenerationAttemptPrecursorV1>> {
    let precursor = load_precursor(
        connection,
        foundation,
        foundation.plan().qualification_plan_id(),
        planned.planned_attempt_id(),
    )?;
    match (precursor, expected_id) {
        (None, None) => Ok(None),
        (Some(value), Some(expected)) if value.precursor_id().digest().as_str() == expected => {
            Ok(Some(value))
        }
        (None, Some(_)) => Err(StoreError::MissingRecord),
        (Some(_), None | Some(_)) => Err(StoreError::ImmutableConflict),
    }
}

#[expect(
    clippy::large_types_passed_by_value,
    reason = "the copyable input is one exact transaction closure"
)]
pub(super) fn matches_input(
    stored: &StoredCandidateGenerationExecutionV1,
    input: CandidateGenerationExecutionV1Input<'_>,
) -> bool {
    match (stored, input) {
        (
            StoredCandidateGenerationExecutionV1::Failed {
                precursor, attempt, ..
            },
            CandidateGenerationExecutionV1Input::Failed {
                precursor: expected_precursor,
                attempt: expected_attempt,
                ..
            },
        ) => precursor.as_ref() == expected_precursor && attempt == expected_attempt,
        (
            StoredCandidateGenerationExecutionV1::Completed {
                precursor,
                managed_evidence,
                cleanup,
                bundle,
                storage,
                readback,
                receipt,
                attempt,
                ..
            },
            CandidateGenerationExecutionV1Input::Completed {
                precursor: expected_precursor,
                managed_evidence: expected_managed,
                cleanup: expected_cleanup,
                bundle: expected_bundle,
                storage: expected_storage,
                readback: expected_readback,
                receipt: expected_receipt,
                attempt: expected_attempt,
                ..
            },
        ) => {
            precursor == expected_precursor
                && managed_evidence == expected_managed
                && cleanup == expected_cleanup
                && bundle == expected_bundle
                && storage == expected_storage
                && readback == expected_readback
                && receipt == expected_receipt
                && attempt == expected_attempt
        }
        _ => false,
    }
}
