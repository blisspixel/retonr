use crate::ArtifactInventoryError;

use super::ArtifactSetImportError;

pub(crate) fn map_storage_open(error: ArtifactInventoryError) -> ArtifactSetImportError {
    match error {
        ArtifactInventoryError::InvalidLimits => ArtifactSetImportError::InvalidLimits,
        ArtifactInventoryError::StorageNotInitialized
        | ArtifactInventoryError::UnsafeStorageLayout => {
            ArtifactSetImportError::UnsafeStorageLayout
        }
        ArtifactInventoryError::StorageInUse => ArtifactSetImportError::StorageInUse,
        ArtifactInventoryError::StorageEntryLimitExceeded
        | ArtifactInventoryError::StateEntryLimitExceeded
        | ArtifactInventoryError::TotalVerificationLimitExceeded => {
            ArtifactSetImportError::StorageEntryLimitExceeded
        }
        ArtifactInventoryError::Cancelled => ArtifactSetImportError::Cancelled,
        ArtifactInventoryError::ConcurrentModification => ArtifactSetImportError::StorageChanged,
        ArtifactInventoryError::StorageIo(error) => ArtifactSetImportError::StorageIo(error),
        ArtifactInventoryError::State(error) => ArtifactSetImportError::State(error),
    }
}

pub(super) fn map_source_tree(error: ArtifactInventoryError) -> ArtifactSetImportError {
    match error {
        ArtifactInventoryError::StorageEntryLimitExceeded => {
            ArtifactSetImportError::TreeEntryLimitExceeded
        }
        ArtifactInventoryError::Cancelled => ArtifactSetImportError::Cancelled,
        ArtifactInventoryError::ConcurrentModification => ArtifactSetImportError::StorageChanged,
        ArtifactInventoryError::StorageIo(error) => ArtifactSetImportError::SourceIo(error),
        ArtifactInventoryError::StorageNotInitialized
        | ArtifactInventoryError::UnsafeStorageLayout
        | ArtifactInventoryError::StateEntryLimitExceeded
        | ArtifactInventoryError::TotalVerificationLimitExceeded
        | ArtifactInventoryError::InvalidLimits
        | ArtifactInventoryError::StorageInUse
        | ArtifactInventoryError::State(_) => ArtifactSetImportError::UnsafeSourceTree,
    }
}

pub(crate) fn map_managed_tree(error: ArtifactInventoryError) -> ArtifactSetImportError {
    match error {
        ArtifactInventoryError::StorageEntryLimitExceeded => {
            ArtifactSetImportError::TreeEntryLimitExceeded
        }
        ArtifactInventoryError::Cancelled => ArtifactSetImportError::Cancelled,
        ArtifactInventoryError::ConcurrentModification => ArtifactSetImportError::StorageChanged,
        ArtifactInventoryError::StorageIo(error) => ArtifactSetImportError::StorageIo(error),
        ArtifactInventoryError::StorageNotInitialized
        | ArtifactInventoryError::UnsafeStorageLayout
        | ArtifactInventoryError::StateEntryLimitExceeded
        | ArtifactInventoryError::TotalVerificationLimitExceeded
        | ArtifactInventoryError::InvalidLimits
        | ArtifactInventoryError::StorageInUse
        | ArtifactInventoryError::State(_) => ArtifactSetImportError::StorageConflict,
    }
}

pub(super) fn map_staging(error: ArtifactInventoryError) -> ArtifactSetImportError {
    match error {
        ArtifactInventoryError::StorageEntryLimitExceeded => {
            ArtifactSetImportError::StagingEntryLimitExceeded
        }
        other => map_managed_tree(other),
    }
}

pub(crate) fn map_set_capacity(error: ArtifactInventoryError) -> ArtifactSetImportError {
    match error {
        ArtifactInventoryError::StorageEntryLimitExceeded => {
            ArtifactSetImportError::StorageEntryLimitExceeded
        }
        other => map_managed_tree(other),
    }
}
