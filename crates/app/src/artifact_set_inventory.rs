use std::ffi::OsStr;

use rewrite_model_store::ArtifactStateStore;
use rewrite_types::CancellationToken;

mod contract;
mod inspect;
mod map;

use crate::artifact_set_import::SETS_DIRECTORY;
use crate::artifact_storage::{ExistingArtifactStorage, LifecycleLockMode, PinnedDirectory};
pub use contract::{
    ArtifactSetInventoryError, ArtifactSetInventoryLimits, ArtifactSetInventoryProgress,
    ArtifactSetInventoryReport, ArtifactSetInventoryStage, ArtifactSetTreeConflict,
    OversizedArtifactSet, RegisteredArtifactSetBytes, RegisteredArtifactSetInspection,
    UnexpectedArtifactSetEntryCounts, VerifiedArtifactSetOrphan,
};
use inspect::{InventoryBuilder, snapshot_sets};
use map::{
    ensure_not_cancelled, map_final_store_error, map_storage_open, map_store_error,
    report_progress, validate_limits,
};

/// Read-only, point-in-time inspection of application-owned artifact-set storage.
pub struct ArtifactSetInventoryService<'a> {
    storage: ExistingArtifactStorage,
    limits: ArtifactSetInventoryLimits,
    store: &'a ArtifactStateStore,
}

impl<'a> ArtifactSetInventoryService<'a> {
    /// Opens existing storage and acquires its shared lifecycle lock.
    ///
    /// This operation never creates storage, cleans staging files, repairs state,
    /// removes bytes, or accesses the network. A missing `sets` directory is an
    /// empty observation, not initialization.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactSetInventoryError`] when limits are invalid, storage is
    /// not initialized, a managed boundary is unsafe, or an exclusive lifecycle
    /// operation owns the lock.
    pub fn open(
        root: impl AsRef<std::path::Path>,
        store: &'a ArtifactStateStore,
        limits: ArtifactSetInventoryLimits,
    ) -> Result<Self, ArtifactSetInventoryError> {
        validate_limits(limits)?;
        let storage = ExistingArtifactStorage::open(root, LifecycleLockMode::Shared)
            .map_err(map_storage_open)?;
        Ok(Self {
            storage,
            limits,
            store,
        })
    }

    /// Builds one bounded, deterministic set-reconciliation report without mutation.
    ///
    /// Verified orphan set roots are point-in-time reclamation candidates only.
    /// They are never treated as durable authority and are not removed automatically.
    /// This report does not grant a lease, qualify a package, or authorize a role.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactSetInventoryError`] with no partial report when state is
    /// corrupt, limits are exceeded, cancellation is observed, or storage changes
    /// during the operation.
    pub fn inventory<F>(
        &self,
        cancellation: &CancellationToken,
        mut progress: F,
    ) -> Result<ArtifactSetInventoryReport, ArtifactSetInventoryError>
    where
        F: FnMut(ArtifactSetInventoryProgress),
    {
        ensure_not_cancelled(cancellation)?;
        report_progress(
            &mut progress,
            ArtifactSetInventoryStage::OpeningStorage,
            0,
            0,
        );
        let initial_layout = self.storage.validate_layout().map_err(map_storage_open)?;
        let initial_sets = self.open_sets()?;
        ensure_not_cancelled(cancellation)?;

        report_progress(&mut progress, ArtifactSetInventoryStage::LoadingState, 0, 0);
        let states = self
            .store
            .artifact_set_inventory(self.limits.maximum_state_entries)
            .map_err(map_store_error)?;
        ensure_not_cancelled(cancellation)?;

        report_progress(
            &mut progress,
            ArtifactSetInventoryStage::FreezingStorage,
            0,
            0,
        );
        let initial_entries = snapshot_sets(initial_sets.as_ref(), self.limits, cancellation)?;
        let mut builder = InventoryBuilder::new(&states, &initial_entries, self.limits);
        builder.inspect_registered(initial_sets.as_ref(), cancellation, &mut progress)?;
        builder.inspect_uninstalled(initial_sets.as_ref(), cancellation, &mut progress)?;
        ensure_not_cancelled(cancellation)?;

        report_progress(
            &mut progress,
            ArtifactSetInventoryStage::RecheckingStorageAndState,
            builder.completed_entries,
            builder.verified_bytes,
        );
        ensure_not_cancelled(cancellation)?;
        let final_sets = self.open_sets()?;
        let final_entries = snapshot_sets(final_sets.as_ref(), self.limits, cancellation)?;
        if initial_sets.is_some() != final_sets.is_some()
            || initial_entries != final_entries
            || initial_layout != self.storage.validate_layout().map_err(map_storage_open)?
        {
            return Err(ArtifactSetInventoryError::ConcurrentModification);
        }
        if let (Some(initial), Some(final_sets)) = (initial_sets.as_ref(), final_sets.as_ref())
            && initial.fingerprint().map_err(map_storage_open)?
                != final_sets.fingerprint().map_err(map_storage_open)?
        {
            return Err(ArtifactSetInventoryError::ConcurrentModification);
        }
        let final_states = self
            .store
            .artifact_set_inventory(self.limits.maximum_state_entries)
            .map_err(map_final_store_error)?;
        if states != final_states {
            return Err(ArtifactSetInventoryError::ConcurrentModification);
        }
        ensure_not_cancelled(cancellation)?;
        Ok(builder.finish(
            u64::try_from(initial_entries.len())
                .map_err(|_| ArtifactSetInventoryError::StorageEntryLimitExceeded)?,
        ))
    }

    fn open_sets(&self) -> Result<Option<PinnedDirectory>, ArtifactSetInventoryError> {
        self.storage
            .root()
            .open_optional_child_directory(OsStr::new(SETS_DIRECTORY))
            .map_err(map_storage_open)
    }
}

#[cfg(test)]
mod tests;
