use rewrite_model_store::StoreError;
use rewrite_types::CancellationToken;

use crate::artifact_set_import::ArtifactSetImportError;
use crate::{ArtifactInventoryError, ArtifactRemovalDisposition};

use super::{
    ArtifactSetRemovalError, ArtifactSetRemovalProgress, ArtifactSetRemovalRecoveryError,
    ArtifactSetRemovalRequest, ArtifactSetRemovalResult, ArtifactSetRemovalStage,
};

pub(super) fn result(
    request: &ArtifactSetRemovalRequest,
    disposition: ArtifactRemovalDisposition,
) -> ArtifactSetRemovalResult {
    ArtifactSetRemovalResult {
        selection: request.selection.clone(),
        disposition,
    }
}

pub(super) fn report_progress(
    progress: &mut impl FnMut(ArtifactSetRemovalProgress),
    stage: ArtifactSetRemovalStage,
    completed_bytes: u64,
    total_bytes: u64,
) {
    progress(ArtifactSetRemovalProgress {
        stage,
        completed_bytes,
        total_bytes,
    });
}

pub(super) fn ensure_not_cancelled(
    cancellation: &CancellationToken,
) -> Result<(), ArtifactSetRemovalError> {
    if cancellation.is_cancelled() {
        Err(ArtifactSetRemovalError::Cancelled)
    } else {
        Ok(())
    }
}

pub(super) fn map_plan_error(error: ArtifactSetImportError) -> ArtifactSetRemovalError {
    match error {
        ArtifactSetImportError::InvalidLimits => ArtifactSetRemovalError::InvalidLimits,
        ArtifactSetImportError::InvalidManifest(error) => {
            ArtifactSetRemovalError::InvalidManifest(error)
        }
        ArtifactSetImportError::InvalidInstallation(error) => {
            ArtifactSetRemovalError::InvalidInstallation(error)
        }
        ArtifactSetImportError::TooManyMembers { actual, maximum } => {
            ArtifactSetRemovalError::ArtifactSetTooLarge {
                actual: u64::try_from(actual).unwrap_or(u64::MAX),
                maximum: u64::try_from(maximum).unwrap_or(u64::MAX),
            }
        }
        ArtifactSetImportError::MemberTooLarge { actual, maximum }
        | ArtifactSetImportError::ArtifactSetTooLarge { actual, maximum } => {
            ArtifactSetRemovalError::ArtifactSetTooLarge { actual, maximum }
        }
        ArtifactSetImportError::TreeEntryLimitExceeded => {
            ArtifactSetRemovalError::StorageEntryLimitExceeded
        }
        ArtifactSetImportError::Cancelled => ArtifactSetRemovalError::Cancelled,
        ArtifactSetImportError::StorageChanged => ArtifactSetRemovalError::StorageChanged,
        ArtifactSetImportError::StorageIo(error) => ArtifactSetRemovalError::StorageIo(error),
        ArtifactSetImportError::State(error) => ArtifactSetRemovalError::State(error),
        _ => ArtifactSetRemovalError::StorageConflict,
    }
}

pub(super) fn map_verify_error(error: ArtifactSetImportError) -> ArtifactSetRemovalError {
    match error {
        ArtifactSetImportError::Cancelled => ArtifactSetRemovalError::Cancelled,
        ArtifactSetImportError::StorageChanged => ArtifactSetRemovalError::StorageChanged,
        ArtifactSetImportError::StorageConflict
        | ArtifactSetImportError::SizeMismatch
        | ArtifactSetImportError::DigestMismatch
        | ArtifactSetImportError::SourceTreeMismatch => ArtifactSetRemovalError::StorageConflict,
        ArtifactSetImportError::UnsafeStorageLayout => ArtifactSetRemovalError::UnsafeStorageLayout,
        ArtifactSetImportError::StorageInUse => ArtifactSetRemovalError::StorageInUse,
        ArtifactSetImportError::StorageEntryLimitExceeded
        | ArtifactSetImportError::TreeEntryLimitExceeded => {
            ArtifactSetRemovalError::StorageEntryLimitExceeded
        }
        ArtifactSetImportError::StorageIo(error) => ArtifactSetRemovalError::StorageIo(error),
        ArtifactSetImportError::State(error) => ArtifactSetRemovalError::State(error),
        other => map_plan_error(other),
    }
}

pub(super) fn map_open_error(error: ArtifactInventoryError) -> ArtifactSetRemovalError {
    match error {
        ArtifactInventoryError::InvalidLimits => ArtifactSetRemovalError::InvalidLimits,
        ArtifactInventoryError::StorageNotInitialized => {
            ArtifactSetRemovalError::StorageNotInitialized
        }
        ArtifactInventoryError::UnsafeStorageLayout => ArtifactSetRemovalError::UnsafeStorageLayout,
        ArtifactInventoryError::StorageInUse => ArtifactSetRemovalError::StorageInUse,
        ArtifactInventoryError::StorageEntryLimitExceeded
        | ArtifactInventoryError::StateEntryLimitExceeded
        | ArtifactInventoryError::TotalVerificationLimitExceeded => {
            ArtifactSetRemovalError::StorageEntryLimitExceeded
        }
        ArtifactInventoryError::Cancelled => ArtifactSetRemovalError::Cancelled,
        ArtifactInventoryError::ConcurrentModification => ArtifactSetRemovalError::StorageChanged,
        ArtifactInventoryError::StorageIo(error) => ArtifactSetRemovalError::StorageIo(error),
        ArtifactInventoryError::State(error) => ArtifactSetRemovalError::State(error),
    }
}

pub(super) fn map_storage_error(error: ArtifactInventoryError) -> ArtifactSetRemovalError {
    match error {
        ArtifactInventoryError::ConcurrentModification
        | ArtifactInventoryError::StorageNotInitialized
        | ArtifactInventoryError::UnsafeStorageLayout => ArtifactSetRemovalError::StorageChanged,
        other => map_open_error(other),
    }
}

pub(super) fn map_store_error(error: StoreError) -> ArtifactSetRemovalError {
    match error {
        StoreError::StaleInstallation | StoreError::MissingRecord => {
            ArtifactSetRemovalError::StaleSelection
        }
        error @ (StoreError::Serialization(_)
        | StoreError::InvalidManifest(_)
        | StoreError::InvalidInstallation(_)
        | StoreError::InvalidArtifactSet(_)
        | StoreError::InvalidArtifactSetInstallation(_)
        | StoreError::RecordTooLarge
        | StoreError::CorruptRecord) => ArtifactSetRemovalError::StateCorrupt(error),
        other => ArtifactSetRemovalError::State(other),
    }
}

pub(super) fn map_recovery_storage_error(
    error: ArtifactInventoryError,
) -> ArtifactSetRemovalRecoveryError {
    match error {
        ArtifactInventoryError::StorageIo(error) => {
            ArtifactSetRemovalRecoveryError::StorageIo(error)
        }
        _ => ArtifactSetRemovalRecoveryError::Storage,
    }
}

pub(super) fn map_prepared_error(error: ArtifactSetRemovalError) -> ArtifactSetRemovalError {
    match error {
        ArtifactSetRemovalError::StorageIo(source) => ArtifactSetRemovalError::RecoveryRequired(
            ArtifactSetRemovalRecoveryError::StorageIo(source),
        ),
        _ => ArtifactSetRemovalError::RecoveryRequired(ArtifactSetRemovalRecoveryError::Storage),
    }
}
