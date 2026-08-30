use std::{fs::TryLockError, io};

use rewrite_model_store::CandidateGenerationEvidenceStorageContractError;
use thiserror::Error;

use crate::ArtifactInventoryError;
use crate::CandidateGenerationEvidenceBundleError;

/// Failure while initializing or reopening app-owned generation-evidence storage.
#[derive(Debug, Error)]
pub enum CandidateGenerationEvidenceRepositoryError {
    /// The app data directory or selected storage operation failed.
    #[error("candidate generation evidence storage operation failed")]
    StorageIo(#[source] io::Error),
    /// The app data directory does not contain an initialized evidence root.
    #[error("candidate generation evidence storage is not initialized")]
    NotInitialized,
    /// The app data directory already contains an initialized evidence root.
    #[error("candidate generation evidence storage is already initialized")]
    AlreadyInitialized,
    /// A fixed name, object type, link, case alias, or retained boundary is unsafe.
    #[error("candidate generation evidence storage boundary is unsafe")]
    UnsafeBoundary,
    /// Another process owns an incompatible root lifecycle lock.
    #[error("candidate generation evidence storage is in use")]
    StorageInUse,
    /// The root marker is malformed, noncanonical, or outside its fixed bound.
    #[error("candidate generation evidence storage root marker is invalid")]
    InvalidRootMarker,
    /// The initialized root has a different stable local identity.
    #[error("candidate generation evidence storage root identity does not match")]
    RootIdentityMismatch,
    /// A retained root object or named storage boundary changed.
    #[error("candidate generation evidence storage changed during the operation")]
    StorageChanged,
    /// The durable store-local reference contract rejected the requested binding.
    #[error("candidate generation evidence storage reference is invalid")]
    StorageContract(#[source] CandidateGenerationEvidenceStorageContractError),
    /// Bundle publication or complete byte reacquisition failed.
    #[error("candidate generation evidence bundle operation failed")]
    EvidenceBundle(#[source] CandidateGenerationEvidenceBundleError),
}

pub(super) fn map_exclusive_lock(
    error: TryLockError,
) -> CandidateGenerationEvidenceRepositoryError {
    match error {
        TryLockError::WouldBlock => CandidateGenerationEvidenceRepositoryError::StorageInUse,
        TryLockError::Error(error) => CandidateGenerationEvidenceRepositoryError::StorageIo(error),
    }
}

pub(super) fn map_initial_storage(
    error: ArtifactInventoryError,
) -> CandidateGenerationEvidenceRepositoryError {
    match error {
        ArtifactInventoryError::StorageIo(error) => {
            CandidateGenerationEvidenceRepositoryError::StorageIo(error)
        }
        ArtifactInventoryError::StorageNotInitialized => {
            CandidateGenerationEvidenceRepositoryError::NotInitialized
        }
        ArtifactInventoryError::StorageInUse => {
            CandidateGenerationEvidenceRepositoryError::StorageInUse
        }
        ArtifactInventoryError::ConcurrentModification => {
            CandidateGenerationEvidenceRepositoryError::StorageChanged
        }
        ArtifactInventoryError::Cancelled
        | ArtifactInventoryError::InvalidLimits
        | ArtifactInventoryError::State(_)
        | ArtifactInventoryError::StateEntryLimitExceeded
        | ArtifactInventoryError::StorageEntryLimitExceeded
        | ArtifactInventoryError::TotalVerificationLimitExceeded
        | ArtifactInventoryError::UnsafeStorageLayout => {
            CandidateGenerationEvidenceRepositoryError::UnsafeBoundary
        }
    }
}

pub(super) fn map_active_storage(
    error: ArtifactInventoryError,
) -> CandidateGenerationEvidenceRepositoryError {
    match map_initial_storage(error) {
        CandidateGenerationEvidenceRepositoryError::NotInitialized => {
            CandidateGenerationEvidenceRepositoryError::StorageChanged
        }
        other => other,
    }
}
