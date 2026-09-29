//! Read-only comparison of candidate admission metadata with this evidence root.

use std::ffi::OsStr;
use std::time::Instant;

use rewrite_model::{GenerationQualificationPlanId, MAX_PLANNED_GENERATION_ATTEMPTS};
use rewrite_model_store::CandidateGenerationAttemptAdmissionV1;
use rewrite_types::CancellationToken;
use thiserror::Error;

use crate::ArtifactInventoryError;
use crate::MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_DESTINATION_ENTRIES;
use crate::MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_STAGING_ROOTS;

use super::layout::{FixedEntryKind, fixed_child_state};
use super::{CandidateGenerationEvidenceRepository, CandidateGenerationEvidenceRepositoryError};

/// Content-free refusal from a read-only candidate activation admission check.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum CandidateActivationAdmissionError {
    /// The caller deadline elapsed before admission finished.
    #[error("candidate activation admission deadline was reached")]
    DeadlineExceeded,
    /// The caller cancelled admission before it finished.
    #[error("candidate activation admission was cancelled")]
    Cancelled,
    /// `.staging` contains at least one direct child.
    #[error("candidate activation admission found unexpected evidence staging")]
    UnexpectedStaging,
    /// A bundle directory exists without matching terminal metadata.
    #[error("candidate activation admission found an evidence publication orphan")]
    PublicationOrphan,
    /// Terminal completed metadata has no matching bundle directory on this root.
    #[error("candidate activation admission evidence storage does not match")]
    StorageMismatch,
    /// Metadata and the evidence root do not form one legal class.
    #[error("candidate activation admission found an ambiguous candidate commit")]
    AmbiguousCommit,
    /// A precursor exists and no terminal evidence is present.
    #[error("candidate activation admission found a checkpoint-only candidate attempt")]
    CheckpointOnly,
    /// A failed attempt exists and no bundle directory is present.
    #[error("candidate activation admission found a failed candidate attempt")]
    TerminalFailed,
    /// A completed attempt matches its bundle directory.
    #[error("candidate activation admission found a completed candidate attempt")]
    TerminalCompleted,
    /// The evidence layout is not a trustworthy admission input.
    #[error("candidate activation admission candidate evidence is corrupt")]
    Corrupt,
    /// A bounded directory listing exceeded its fixed ceiling.
    #[error("candidate activation admission inspection bound was exceeded")]
    BoundExceeded,
    /// The evidence root could not be read.
    #[error("candidate activation admission inspection is unavailable")]
    Unavailable,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum AttemptDirectory {
    Absent,
    Empty,
    ExactBundle,
    Other,
}

impl CandidateGenerationEvidenceRepository {
    /// Compares one plan-order admission snapshot with this evidence root.
    ///
    /// The only success is an entirely pristine plan: every attempt is not
    /// started, this plan has no bundle directories, and `.staging` is empty.
    /// The method does not create, rename, delete, or promote anything.
    ///
    /// # Errors
    ///
    /// Returns [`CandidateActivationAdmissionError`] for a non-pristine plan,
    /// an evidence mismatch, cancellation, or an unreadable root.
    pub fn reconcile_candidate_activation_admission(
        &self,
        plan_id: &GenerationQualificationPlanId,
        attempts: &[CandidateGenerationAttemptAdmissionV1],
        deadline: Instant,
        cancellation: &CancellationToken,
    ) -> Result<(), CandidateActivationAdmissionError> {
        check_gate(deadline, cancellation)?;
        if attempts.is_empty() || attempts.len() > MAX_PLANNED_GENERATION_ATTEMPTS {
            return Err(if attempts.is_empty() {
                CandidateActivationAdmissionError::Corrupt
            } else {
                CandidateActivationAdmissionError::BoundExceeded
            });
        }
        self.revalidate()
            .map_err(|error| map_repository_error(&error))?;
        require_clear_staging(self, cancellation)?;
        for attempt in attempts {
            check_gate(deadline, cancellation)?;
            let directory = attempt_directory(self, plan_id, attempt, cancellation)?;
            decide_attempt(attempt, directory)?;
        }
        check_gate(deadline, cancellation)?;
        require_no_unplanned_attempt_directories(self, plan_id, attempts, cancellation)
    }
}

fn check_gate(
    deadline: Instant,
    cancellation: &CancellationToken,
) -> Result<(), CandidateActivationAdmissionError> {
    if Instant::now() >= deadline {
        Err(CandidateActivationAdmissionError::DeadlineExceeded)
    } else if cancellation.is_cancelled() {
        Err(CandidateActivationAdmissionError::Cancelled)
    } else {
        Ok(())
    }
}

fn require_clear_staging(
    repository: &CandidateGenerationEvidenceRepository,
    cancellation: &CancellationToken,
) -> Result<(), CandidateActivationAdmissionError> {
    let entries = list_children(
        &repository.staging,
        MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_STAGING_ROOTS,
        cancellation,
    )?;
    if entries.is_empty() {
        Ok(())
    } else {
        Err(CandidateActivationAdmissionError::UnexpectedStaging)
    }
}

fn attempt_directory(
    repository: &CandidateGenerationEvidenceRepository,
    plan_id: &GenerationQualificationPlanId,
    attempt: &CandidateGenerationAttemptAdmissionV1,
    cancellation: &CancellationToken,
) -> Result<AttemptDirectory, CandidateActivationAdmissionError> {
    let Some(plan_directory) = open_plan_directory(repository, plan_id, cancellation)? else {
        return Ok(AttemptDirectory::Absent);
    };
    match child_directory(
        &plan_directory,
        attempt.planned_attempt_id().digest().as_str(),
        cancellation,
    )? {
        None => Ok(AttemptDirectory::Absent),
        Some(attempt_directory) => classify_attempt_directory(
            &attempt_directory,
            attempt
                .evidence_bundle_id()
                .map(|bundle| bundle.digest().as_str()),
            cancellation,
        ),
    }
}

fn open_plan_directory(
    repository: &CandidateGenerationEvidenceRepository,
    plan_id: &GenerationQualificationPlanId,
    cancellation: &CancellationToken,
) -> Result<Option<crate::artifact_storage::PinnedDirectory>, CandidateActivationAdmissionError> {
    if !named_directory_is_present(
        &repository.layout_version,
        plan_id.digest().as_str(),
        cancellation,
    )? {
        return Ok(None);
    }
    repository
        .layout_version
        .open_child_directory(OsStr::new(plan_id.digest().as_str()))
        .map(Some)
        .map_err(|error| map_inventory_error(&error))
}

fn classify_attempt_directory(
    attempt_directory: &crate::artifact_storage::PinnedDirectory,
    expected_bundle_id: Option<&str>,
    cancellation: &CancellationToken,
) -> Result<AttemptDirectory, CandidateActivationAdmissionError> {
    let entries = list_children(
        attempt_directory,
        MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_DESTINATION_ENTRIES,
        cancellation,
    )?;
    if entries.is_empty() {
        return Ok(AttemptDirectory::Empty);
    }
    let mut exact = false;
    for entry in &entries {
        if entry.indirect || entry.direct_regular_file {
            return Err(CandidateActivationAdmissionError::Corrupt);
        }
        let Some(name) = entry.name.to_str() else {
            return Err(CandidateActivationAdmissionError::Corrupt);
        };
        if expected_bundle_id.is_some_and(|expected| name == expected) {
            exact = true;
        }
    }
    Ok(if entries.len() == 1 && exact {
        AttemptDirectory::ExactBundle
    } else {
        AttemptDirectory::Other
    })
}

fn decide_attempt(
    attempt: &CandidateGenerationAttemptAdmissionV1,
    directory: AttemptDirectory,
) -> Result<(), CandidateActivationAdmissionError> {
    use rewrite_model_store::CandidateGenerationAttemptAdmissionClassV1::{
        Ambiguous, CheckpointOnly, Corrupt, NotStarted, TerminalCompleted, TerminalFailed,
    };
    match (attempt.class(), directory) {
        (NotStarted, AttemptDirectory::Absent) => Ok(()),
        (NotStarted | CheckpointOnly, AttemptDirectory::Other | AttemptDirectory::ExactBundle) => {
            Err(CandidateActivationAdmissionError::PublicationOrphan)
        }
        (CheckpointOnly, AttemptDirectory::Absent) => {
            Err(CandidateActivationAdmissionError::CheckpointOnly)
        }
        (TerminalFailed, AttemptDirectory::Absent) => {
            Err(CandidateActivationAdmissionError::TerminalFailed)
        }
        (TerminalCompleted, AttemptDirectory::ExactBundle) => {
            Err(CandidateActivationAdmissionError::TerminalCompleted)
        }
        (TerminalCompleted, AttemptDirectory::Absent) => {
            Err(CandidateActivationAdmissionError::StorageMismatch)
        }
        (TerminalFailed, AttemptDirectory::Other | AttemptDirectory::ExactBundle)
        | (TerminalCompleted, AttemptDirectory::Other)
        | (Ambiguous, _) => Err(CandidateActivationAdmissionError::AmbiguousCommit),
        (Corrupt, _) | (_, AttemptDirectory::Empty) => {
            Err(CandidateActivationAdmissionError::Corrupt)
        }
    }
}

fn require_no_unplanned_attempt_directories(
    repository: &CandidateGenerationEvidenceRepository,
    plan_id: &GenerationQualificationPlanId,
    attempts: &[CandidateGenerationAttemptAdmissionV1],
    cancellation: &CancellationToken,
) -> Result<(), CandidateActivationAdmissionError> {
    let Some(plan_directory) = open_plan_directory(repository, plan_id, cancellation)? else {
        return Ok(());
    };
    let entries = list_children(
        &plan_directory,
        MAX_PLANNED_GENERATION_ATTEMPTS,
        cancellation,
    )?;
    for entry in entries {
        if entry.indirect || entry.direct_regular_file {
            return Err(CandidateActivationAdmissionError::Corrupt);
        }
        let Some(name) = entry.name.to_str() else {
            return Err(CandidateActivationAdmissionError::Corrupt);
        };
        let planned = attempts
            .iter()
            .any(|attempt| attempt.planned_attempt_id().digest().as_str() == name);
        if !planned {
            return Err(CandidateActivationAdmissionError::PublicationOrphan);
        }
    }
    Ok(())
}

fn named_directory_is_present(
    parent: &crate::artifact_storage::PinnedDirectory,
    name: &str,
    cancellation: &CancellationToken,
) -> Result<bool, CandidateActivationAdmissionError> {
    if cancellation.is_cancelled() {
        return Err(CandidateActivationAdmissionError::Cancelled);
    }
    match fixed_child_state(
        parent,
        name,
        FixedEntryKind::Directory,
        MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_DESTINATION_ENTRIES,
    ) {
        Ok(super::layout::FixedChildState::Present) => Ok(true),
        Ok(super::layout::FixedChildState::Absent) => Ok(false),
        Err(error) => Err(map_repository_error(&error)),
    }
}

fn child_directory(
    parent: &crate::artifact_storage::PinnedDirectory,
    name: &str,
    cancellation: &CancellationToken,
) -> Result<Option<crate::artifact_storage::PinnedDirectory>, CandidateActivationAdmissionError> {
    if !named_directory_is_present(parent, name, cancellation)? {
        return Ok(None);
    }
    parent
        .open_child_directory(OsStr::new(name))
        .map(Some)
        .map_err(|error| map_inventory_error(&error))
}

fn list_children(
    directory: &crate::artifact_storage::PinnedDirectory,
    maximum_entries: usize,
    cancellation: &CancellationToken,
) -> Result<Vec<crate::artifact_storage::RawDirectoryEntry>, CandidateActivationAdmissionError> {
    directory
        .raw_entries(maximum_entries, cancellation)
        .map_err(|error| map_inventory_error(&error))
}

fn map_inventory_error(error: &ArtifactInventoryError) -> CandidateActivationAdmissionError {
    match error {
        ArtifactInventoryError::Cancelled => CandidateActivationAdmissionError::Cancelled,
        ArtifactInventoryError::StorageEntryLimitExceeded => {
            CandidateActivationAdmissionError::BoundExceeded
        }
        ArtifactInventoryError::StorageIo(_) | ArtifactInventoryError::StorageInUse => {
            CandidateActivationAdmissionError::Unavailable
        }
        ArtifactInventoryError::StorageNotInitialized
        | ArtifactInventoryError::ConcurrentModification
        | ArtifactInventoryError::InvalidLimits
        | ArtifactInventoryError::State(_)
        | ArtifactInventoryError::StateEntryLimitExceeded
        | ArtifactInventoryError::TotalVerificationLimitExceeded
        | ArtifactInventoryError::UnsafeStorageLayout => CandidateActivationAdmissionError::Corrupt,
    }
}

fn map_repository_error(
    error: &CandidateGenerationEvidenceRepositoryError,
) -> CandidateActivationAdmissionError {
    match error {
        CandidateGenerationEvidenceRepositoryError::StorageIo(_)
        | CandidateGenerationEvidenceRepositoryError::StorageInUse
        | CandidateGenerationEvidenceRepositoryError::NotInitialized
        | CandidateGenerationEvidenceRepositoryError::AlreadyInitialized => {
            CandidateActivationAdmissionError::Unavailable
        }
        CandidateGenerationEvidenceRepositoryError::UnsafeBoundary
        | CandidateGenerationEvidenceRepositoryError::InvalidRootMarker
        | CandidateGenerationEvidenceRepositoryError::RootIdentityMismatch
        | CandidateGenerationEvidenceRepositoryError::StorageChanged
        | CandidateGenerationEvidenceRepositoryError::StorageContract(_)
        | CandidateGenerationEvidenceRepositoryError::EvidenceBundle(_) => {
            CandidateActivationAdmissionError::Corrupt
        }
    }
}

#[cfg(test)]
#[path = "admission/tests.rs"]
mod tests;
