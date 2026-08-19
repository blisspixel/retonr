use std::{
    ffi::{OsStr, OsString},
    path::Path,
};

use rewrite_model::ArtifactSetManifest;
use rewrite_model_store::{ArtifactStateStore, StoreError, WriteDisposition};
use rewrite_types::CancellationToken;

mod contract;

use crate::artifact_set_import::{
    ArtifactSetImportError, ArtifactSetPlanBounds, SETS_DIRECTORY, plan_artifact_set,
    validate_plan_bounds, verify_final_tree,
};
use crate::artifact_storage::{
    ExactEntryCapacity, ExistingArtifactStorage, LifecycleLockMode, ManagedTreeLimits,
};
use crate::{ArtifactInventoryError, ArtifactReconciliationDisposition};
pub use contract::{
    ArtifactSetReconciliationError, ArtifactSetReconciliationLimits,
    ArtifactSetReconciliationProgress, ArtifactSetReconciliationRequest,
    ArtifactSetReconciliationResult, ArtifactSetReconciliationStage,
};

/// Existing-only service for registering one independently reverified set root.
pub struct ArtifactSetReconciliationService<'a> {
    storage: ExistingArtifactStorage,
    limits: ArtifactSetReconciliationLimits,
    store: &'a mut ArtifactStateStore,
}

impl<'a> ArtifactSetReconciliationService<'a> {
    /// Opens existing storage and takes its exclusive lifecycle lock.
    ///
    /// This operation creates nothing, performs no staging recovery, changes no
    /// managed bytes, and accesses no network.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactSetReconciliationError`] when limits are invalid, storage
    /// is absent or unsafe, or another lifecycle operation owns the lock.
    pub fn open_existing(
        root: impl AsRef<Path>,
        store: &'a mut ArtifactStateStore,
        limits: ArtifactSetReconciliationLimits,
    ) -> Result<Self, ArtifactSetReconciliationError> {
        validate_limits(limits)?;
        let storage = ExistingArtifactStorage::open(root, LifecycleLockMode::Exclusive)
            .map_err(map_open_error)?;
        Ok(Self {
            storage,
            limits,
            store,
        })
    }

    /// Reverifies and atomically registers one exact canonical artifact set.
    ///
    /// The complete set manifest is the only selection authority. A prior
    /// inventory report, path, tag, or storage key is never accepted as current
    /// evidence. The operation never copies, replaces, qualifies, or activates.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactSetReconciliationError`] for an invalid manifest, missing
    /// or conflicting tree, storage drift, cancellation, or durable-state failure.
    pub fn reconcile<F>(
        &mut self,
        request: &ArtifactSetReconciliationRequest,
        cancellation: &CancellationToken,
        mut progress: F,
    ) -> Result<ArtifactSetReconciliationResult, ArtifactSetReconciliationError>
    where
        F: FnMut(ArtifactSetReconciliationProgress),
    {
        ensure_not_cancelled(cancellation)?;
        let plan = plan_request(&request.manifest, self.limits)?;
        self.storage.validate_layout().map_err(map_open_error)?;
        let total = request.manifest.total_byte_size();
        report_progress(
            &mut progress,
            ArtifactSetReconciliationStage::InspectingSelection,
            0,
            total,
        );
        ensure_not_cancelled(cancellation)?;
        let sets = self
            .storage
            .root()
            .open_optional_child_directory(OsStr::new(SETS_DIRECTORY))
            .map_err(map_open_error)?
            .ok_or(ArtifactSetReconciliationError::OrphanNotFound)?;
        let name = OsString::from(&plan.storage_key);
        let set_root = match sets
            .exact_entry_capacity(&name, self.limits.maximum_storage_entries, cancellation)
            .map_err(map_open_error)?
        {
            ExactEntryCapacity::Present => sets
                .open_optional_child_directory(&name)
                .map_err(map_open_error)?
                .ok_or(ArtifactSetReconciliationError::OrphanNotFound)?,
            ExactEntryCapacity::Available => {
                return Err(ArtifactSetReconciliationError::OrphanNotFound);
            }
            ExactEntryCapacity::Full => {
                return Err(ArtifactSetReconciliationError::StorageEntryLimitExceeded);
            }
        };
        report_progress(
            &mut progress,
            ArtifactSetReconciliationStage::VerifyingOrphan,
            0,
            total,
        );
        let tree_limits =
            ManagedTreeLimits::new(self.limits.maximum_tree_entries).map_err(map_open_error)?;
        verify_final_tree(
            &set_root,
            &request.manifest,
            &plan,
            tree_limits,
            cancellation,
        )
        .map_err(map_verify_error)?;
        report_progress(
            &mut progress,
            ArtifactSetReconciliationStage::VerifyingOrphan,
            total,
            total,
        );
        self.storage.validate_layout().map_err(map_open_error)?;
        if sets.fingerprint().map_err(map_open_error)?
            != self
                .storage
                .root()
                .child_directory_fingerprint(OsStr::new(SETS_DIRECTORY))
                .map_err(map_open_error)?
        {
            return Err(ArtifactSetReconciliationError::StorageChanged);
        }
        ensure_not_cancelled(cancellation)?;
        let state = self
            .store
            .put_artifact_set_installation(&request.manifest, &plan.installed)
            .map_err(map_store_error)?;
        let disposition = match state.installed {
            WriteDisposition::Inserted => ArtifactReconciliationDisposition::Registered,
            WriteDisposition::AlreadyPresent => {
                ArtifactReconciliationDisposition::AlreadyRegistered
            }
        };
        Ok(ArtifactSetReconciliationResult {
            installed: plan.installed,
            installation: state.installation,
            disposition,
        })
    }
}

fn validate_limits(
    limits: ArtifactSetReconciliationLimits,
) -> Result<(), ArtifactSetReconciliationError> {
    let bounds = ArtifactSetPlanBounds {
        members: limits.maximum_members,
        member_bytes: limits.maximum_member_bytes,
        total_bytes: limits.maximum_total_bytes,
        tree_entries: limits.maximum_tree_entries,
    };
    if limits.maximum_storage_entries == 0
        || limits.maximum_storage_entries.checked_add(1).is_none()
        || validate_plan_bounds(bounds).is_err()
    {
        Err(ArtifactSetReconciliationError::InvalidLimits)
    } else {
        Ok(())
    }
}

fn plan_request(
    manifest: &ArtifactSetManifest,
    limits: ArtifactSetReconciliationLimits,
) -> Result<crate::artifact_set_import::ValidatedSetPlan, ArtifactSetReconciliationError> {
    let bounds = ArtifactSetPlanBounds {
        members: limits.maximum_members,
        member_bytes: limits.maximum_member_bytes,
        total_bytes: limits.maximum_total_bytes,
        tree_entries: limits.maximum_tree_entries,
    };
    plan_artifact_set(manifest, bounds).map_err(map_plan_error)
}

fn map_plan_error(error: ArtifactSetImportError) -> ArtifactSetReconciliationError {
    match error {
        ArtifactSetImportError::InvalidLimits => ArtifactSetReconciliationError::InvalidLimits,
        ArtifactSetImportError::InvalidManifest(error) => {
            ArtifactSetReconciliationError::InvalidManifest(error)
        }
        ArtifactSetImportError::InvalidInstallation(error) => {
            ArtifactSetReconciliationError::InvalidInstallation(error)
        }
        ArtifactSetImportError::TooManyMembers { actual, maximum } => {
            ArtifactSetReconciliationError::ArtifactSetTooLarge {
                actual: u64::try_from(actual).unwrap_or(u64::MAX),
                maximum: u64::try_from(maximum).unwrap_or(u64::MAX),
            }
        }
        ArtifactSetImportError::MemberTooLarge { actual, maximum }
        | ArtifactSetImportError::ArtifactSetTooLarge { actual, maximum } => {
            ArtifactSetReconciliationError::ArtifactSetTooLarge { actual, maximum }
        }
        ArtifactSetImportError::TreeEntryLimitExceeded => {
            ArtifactSetReconciliationError::StorageEntryLimitExceeded
        }
        ArtifactSetImportError::Cancelled => ArtifactSetReconciliationError::Cancelled,
        ArtifactSetImportError::StorageChanged => ArtifactSetReconciliationError::StorageChanged,
        ArtifactSetImportError::StorageIo(error) => {
            ArtifactSetReconciliationError::StorageIo(error)
        }
        ArtifactSetImportError::State(error) => ArtifactSetReconciliationError::State(error),
        _ => ArtifactSetReconciliationError::StorageConflict,
    }
}

fn map_verify_error(error: ArtifactSetImportError) -> ArtifactSetReconciliationError {
    match error {
        ArtifactSetImportError::Cancelled => ArtifactSetReconciliationError::Cancelled,
        ArtifactSetImportError::StorageChanged => ArtifactSetReconciliationError::StorageChanged,
        ArtifactSetImportError::StorageConflict
        | ArtifactSetImportError::SizeMismatch
        | ArtifactSetImportError::DigestMismatch
        | ArtifactSetImportError::SourceTreeMismatch => {
            ArtifactSetReconciliationError::StorageConflict
        }
        ArtifactSetImportError::UnsafeStorageLayout => {
            ArtifactSetReconciliationError::UnsafeStorageLayout
        }
        ArtifactSetImportError::StorageInUse => ArtifactSetReconciliationError::StorageInUse,
        ArtifactSetImportError::StorageEntryLimitExceeded
        | ArtifactSetImportError::TreeEntryLimitExceeded => {
            ArtifactSetReconciliationError::StorageEntryLimitExceeded
        }
        ArtifactSetImportError::StorageIo(error) => {
            ArtifactSetReconciliationError::StorageIo(error)
        }
        ArtifactSetImportError::State(error) => ArtifactSetReconciliationError::State(error),
        ArtifactSetImportError::InvalidLimits
        | ArtifactSetImportError::InvalidManifest(_)
        | ArtifactSetImportError::InvalidInstallation(_)
        | ArtifactSetImportError::TooManyMembers { .. }
        | ArtifactSetImportError::MemberTooLarge { .. }
        | ArtifactSetImportError::ArtifactSetTooLarge { .. } => map_plan_error(error),
        _ => ArtifactSetReconciliationError::StorageConflict,
    }
}

fn map_open_error(error: ArtifactInventoryError) -> ArtifactSetReconciliationError {
    match error {
        ArtifactInventoryError::InvalidLimits => ArtifactSetReconciliationError::InvalidLimits,
        ArtifactInventoryError::StorageNotInitialized => {
            ArtifactSetReconciliationError::StorageNotInitialized
        }
        ArtifactInventoryError::UnsafeStorageLayout => {
            ArtifactSetReconciliationError::UnsafeStorageLayout
        }
        ArtifactInventoryError::StorageInUse => ArtifactSetReconciliationError::StorageInUse,
        ArtifactInventoryError::StorageEntryLimitExceeded
        | ArtifactInventoryError::StateEntryLimitExceeded
        | ArtifactInventoryError::TotalVerificationLimitExceeded => {
            ArtifactSetReconciliationError::StorageEntryLimitExceeded
        }
        ArtifactInventoryError::Cancelled => ArtifactSetReconciliationError::Cancelled,
        ArtifactInventoryError::ConcurrentModification => {
            ArtifactSetReconciliationError::StorageChanged
        }
        ArtifactInventoryError::StorageIo(error) => {
            ArtifactSetReconciliationError::StorageIo(error)
        }
        ArtifactInventoryError::State(error) => ArtifactSetReconciliationError::State(error),
    }
}

fn map_store_error(error: StoreError) -> ArtifactSetReconciliationError {
    match error {
        StoreError::ImmutableConflict => ArtifactSetReconciliationError::StateConflict,
        StoreError::CorruptRecord | StoreError::MissingRecord => {
            ArtifactSetReconciliationError::StateCorrupt(error)
        }
        other => ArtifactSetReconciliationError::State(other),
    }
}

fn ensure_not_cancelled(
    cancellation: &CancellationToken,
) -> Result<(), ArtifactSetReconciliationError> {
    if cancellation.is_cancelled() {
        Err(ArtifactSetReconciliationError::Cancelled)
    } else {
        Ok(())
    }
}

fn report_progress(
    progress: &mut impl FnMut(ArtifactSetReconciliationProgress),
    stage: ArtifactSetReconciliationStage,
    completed_bytes: u64,
    total_bytes: u64,
) {
    progress(ArtifactSetReconciliationProgress {
        stage,
        completed_bytes,
        total_bytes,
    });
}

#[cfg(test)]
mod tests;
