use rewrite_model_store::StoreError;
use rewrite_types::CancellationToken;

use super::{
    ArtifactSetInventoryError, ArtifactSetInventoryLimits, ArtifactSetInventoryProgress,
    ArtifactSetInventoryStage,
};

pub(super) fn validate_limits(
    limits: ArtifactSetInventoryLimits,
) -> Result<(), ArtifactSetInventoryError> {
    let valid = limits.maximum_state_entries > 0
        && limits.maximum_storage_entries > 0
        && limits.maximum_members > 0
        && limits.maximum_member_bytes > 0
        && limits.maximum_tree_entries > 0
        && limits.maximum_total_verification_bytes > 0
        && limits
            .maximum_state_entries
            .checked_add(1)
            .and_then(|value| i64::try_from(value).ok())
            .is_some()
        && limits.maximum_storage_entries.checked_add(1).is_some()
        && limits.maximum_members.checked_add(1).is_some()
        && limits.maximum_tree_entries.checked_add(1).is_some()
        && u64::try_from(limits.maximum_storage_entries).is_ok();
    if valid {
        Ok(())
    } else {
        Err(ArtifactSetInventoryError::InvalidLimits)
    }
}

pub(super) fn map_storage_open(error: crate::ArtifactInventoryError) -> ArtifactSetInventoryError {
    match error {
        crate::ArtifactInventoryError::InvalidLimits => ArtifactSetInventoryError::InvalidLimits,
        crate::ArtifactInventoryError::StorageNotInitialized => {
            ArtifactSetInventoryError::StorageNotInitialized
        }
        crate::ArtifactInventoryError::UnsafeStorageLayout => {
            ArtifactSetInventoryError::UnsafeStorageLayout
        }
        crate::ArtifactInventoryError::StorageInUse => ArtifactSetInventoryError::StorageInUse,
        crate::ArtifactInventoryError::StateEntryLimitExceeded => {
            ArtifactSetInventoryError::StateEntryLimitExceeded
        }
        crate::ArtifactInventoryError::StorageEntryLimitExceeded => {
            ArtifactSetInventoryError::StorageEntryLimitExceeded
        }
        crate::ArtifactInventoryError::TotalVerificationLimitExceeded => {
            ArtifactSetInventoryError::TotalVerificationLimitExceeded
        }
        crate::ArtifactInventoryError::ConcurrentModification => {
            ArtifactSetInventoryError::ConcurrentModification
        }
        crate::ArtifactInventoryError::Cancelled => ArtifactSetInventoryError::Cancelled,
        crate::ArtifactInventoryError::StorageIo(error) => {
            ArtifactSetInventoryError::StorageIo(error)
        }
        crate::ArtifactInventoryError::State(error) => ArtifactSetInventoryError::State(error),
    }
}

pub(super) fn map_store_error(error: StoreError) -> ArtifactSetInventoryError {
    match error {
        StoreError::InvalidLimit => ArtifactSetInventoryError::InvalidLimits,
        StoreError::InventoryLimitExceeded => ArtifactSetInventoryError::StateEntryLimitExceeded,
        other => ArtifactSetInventoryError::State(other),
    }
}

pub(super) fn map_final_store_error(error: StoreError) -> ArtifactSetInventoryError {
    match error {
        StoreError::InvalidLimit => ArtifactSetInventoryError::InvalidLimits,
        StoreError::InventoryLimitExceeded => ArtifactSetInventoryError::ConcurrentModification,
        other => ArtifactSetInventoryError::State(other),
    }
}

pub(super) fn ensure_not_cancelled(
    cancellation: &CancellationToken,
) -> Result<(), ArtifactSetInventoryError> {
    if cancellation.is_cancelled() {
        Err(ArtifactSetInventoryError::Cancelled)
    } else {
        Ok(())
    }
}

pub(super) fn report_progress(
    progress: &mut impl FnMut(ArtifactSetInventoryProgress),
    stage: ArtifactSetInventoryStage,
    completed_entries: u64,
    verified_bytes: u64,
) {
    progress(ArtifactSetInventoryProgress {
        stage,
        completed_entries,
        verified_bytes,
    });
}
