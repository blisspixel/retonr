use crate::artifact_inventory::ArtifactInventoryError;

use super::ArtifactImportError;

pub(super) fn map_open_error(error: ArtifactInventoryError) -> ArtifactImportError {
    match error {
        ArtifactInventoryError::InvalidLimits => ArtifactImportError::InvalidLimits,
        ArtifactInventoryError::StorageNotInitialized
        | ArtifactInventoryError::UnsafeStorageLayout => ArtifactImportError::UnsafeStorageLayout,
        ArtifactInventoryError::StorageInUse => ArtifactImportError::StorageInUse,
        ArtifactInventoryError::StorageEntryLimitExceeded => {
            ArtifactImportError::StagingEntryLimitExceeded
        }
        ArtifactInventoryError::Cancelled => ArtifactImportError::Cancelled,
        ArtifactInventoryError::StorageIo(error) => ArtifactImportError::StorageIo(error),
        ArtifactInventoryError::State(error) => ArtifactImportError::State(error),
        ArtifactInventoryError::StateEntryLimitExceeded
        | ArtifactInventoryError::TotalVerificationLimitExceeded => {
            ArtifactImportError::StorageConflict
        }
        ArtifactInventoryError::ConcurrentModification => ArtifactImportError::StorageChanged,
    }
}

pub(super) fn map_recovery_error(error: ArtifactInventoryError) -> ArtifactImportError {
    match error {
        ArtifactInventoryError::StorageEntryLimitExceeded => {
            ArtifactImportError::StagingEntryLimitExceeded
        }
        other => map_open_error(other),
    }
}

pub(super) fn map_storage_error(error: ArtifactInventoryError) -> ArtifactImportError {
    match error {
        ArtifactInventoryError::StorageEntryLimitExceeded => {
            ArtifactImportError::StorageEntryLimitExceeded
        }
        ArtifactInventoryError::StorageNotInitialized
        | ArtifactInventoryError::StateEntryLimitExceeded
        | ArtifactInventoryError::TotalVerificationLimitExceeded => {
            ArtifactImportError::StorageConflict
        }
        ArtifactInventoryError::ConcurrentModification => ArtifactImportError::StorageChanged,
        other => map_open_error(other),
    }
}
