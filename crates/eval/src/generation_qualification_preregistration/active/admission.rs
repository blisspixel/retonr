//! Read-only candidate admission gate for Prepared activation.

use std::time::Instant;

use rewrite_app::{CandidateActivationAdmissionError, CandidateGenerationEvidenceRepository};
use rewrite_model_store::StoreError;
use rewrite_types::CancellationToken;

use super::GenerationQualificationActivationError;
use crate::generation_qualification_preregistration::{
    GenerationQualificationPreregistrationRepository, PreparedGenerationQualificationOperation,
};

pub(super) fn reconcile(
    prepared: &PreparedGenerationQualificationOperation<'_, '_, '_, '_, '_>,
    repository: &GenerationQualificationPreregistrationRepository,
    evidence: &CandidateGenerationEvidenceRepository,
    cancellation: &CancellationToken,
) -> Result<(), GenerationQualificationActivationError> {
    if cancellation.is_cancelled() {
        return Err(GenerationQualificationActivationError::Cancelled);
    }
    if Instant::now() >= prepared.operation_deadline() {
        return Err(GenerationQualificationActivationError::DeadlineExceeded);
    }
    let plan = prepared.plan_foundation().plan();
    let attempts = repository
        .read_candidate_activation_admission(
            plan.qualification_plan_id(),
            plan.planned_attempt_ids(),
            evidence.root_id(),
        )
        .map_err(|error| map_store_error(&error))?;
    evidence
        .reconcile_candidate_activation_admission(
            plan.qualification_plan_id(),
            &attempts,
            prepared.operation_deadline(),
            cancellation,
        )
        .map_err(map_evidence_error)
}

fn map_store_error(error: &StoreError) -> GenerationQualificationActivationError {
    match error {
        StoreError::RecordTooLarge => {
            GenerationQualificationActivationError::CandidateInspectionBoundExceeded
        }
        StoreError::CorruptRecord => {
            GenerationQualificationActivationError::CandidateEvidenceCorrupt
        }
        _ => GenerationQualificationActivationError::CandidateInspectionUnavailable,
    }
}

fn map_evidence_error(
    error: CandidateActivationAdmissionError,
) -> GenerationQualificationActivationError {
    match error {
        CandidateActivationAdmissionError::DeadlineExceeded => {
            GenerationQualificationActivationError::DeadlineExceeded
        }
        CandidateActivationAdmissionError::Cancelled => {
            GenerationQualificationActivationError::Cancelled
        }
        CandidateActivationAdmissionError::UnexpectedStaging => {
            GenerationQualificationActivationError::CandidateUnexpectedStaging
        }
        CandidateActivationAdmissionError::PublicationOrphan => {
            GenerationQualificationActivationError::CandidatePublicationOrphan
        }
        CandidateActivationAdmissionError::StorageMismatch => {
            GenerationQualificationActivationError::CandidateEvidenceStorageMismatch
        }
        CandidateActivationAdmissionError::AmbiguousCommit => {
            GenerationQualificationActivationError::CandidateAmbiguousCommit
        }
        CandidateActivationAdmissionError::CheckpointOnly => {
            GenerationQualificationActivationError::CandidateCheckpointOnly
        }
        CandidateActivationAdmissionError::TerminalFailed => {
            GenerationQualificationActivationError::CandidateTerminalFailed
        }
        CandidateActivationAdmissionError::TerminalCompleted => {
            GenerationQualificationActivationError::CandidateTerminalCompleted
        }
        CandidateActivationAdmissionError::Corrupt => {
            GenerationQualificationActivationError::CandidateEvidenceCorrupt
        }
        CandidateActivationAdmissionError::BoundExceeded => {
            GenerationQualificationActivationError::CandidateInspectionBoundExceeded
        }
        CandidateActivationAdmissionError::Unavailable => {
            GenerationQualificationActivationError::CandidateInspectionUnavailable
        }
    }
}

#[cfg(test)]
#[path = "admission/tests.rs"]
mod tests;
