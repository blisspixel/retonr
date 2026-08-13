use std::{ffi::OsString, path::Path};

use rewrite_model::{ArtifactManifest, InstalledArtifact};
use rewrite_model_store::{ArtifactStateStore, StoreError, WriteDisposition};
use rewrite_types::CancellationToken;

mod contract;

use crate::{
    artifact_inventory::ArtifactInventoryError,
    artifact_storage::{
        ExactArtifactExpectation, ExactArtifactSync, ExactArtifactVerificationError,
        ExistingArtifactStorage, LifecycleLockMode, managed_storage_key, verify_exact_artifact,
    },
};
pub use contract::{
    ArtifactOrphanReconciliationProgress, ArtifactOrphanReconciliationRequest,
    ArtifactOrphanReconciliationResult, ArtifactOrphanReconciliationStage,
    ArtifactReconciliationDisposition, ArtifactReconciliationError, ArtifactReconciliationLimits,
};

/// Existing-only service for registering one independently reverified artifact orphan.
pub struct ArtifactOrphanReconciliationService<'a> {
    storage: ExistingArtifactStorage,
    limits: ArtifactReconciliationLimits,
    store: &'a mut ArtifactStateStore,
    sync: ExactArtifactSync,
}

impl<'a> ArtifactOrphanReconciliationService<'a> {
    /// Opens existing storage and takes its exclusive lifecycle lock.
    ///
    /// This operation creates nothing, performs no staging recovery, changes no
    /// managed bytes, and accesses no network.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactReconciliationError`] when limits are invalid, storage is
    /// absent or unsafe, or another lifecycle operation owns the lock.
    pub fn open_existing(
        root: impl AsRef<Path>,
        store: &'a mut ArtifactStateStore,
        limits: ArtifactReconciliationLimits,
    ) -> Result<Self, ArtifactReconciliationError> {
        validate_limits(limits)?;
        let storage = ExistingArtifactStorage::open(root, LifecycleLockMode::Exclusive)
            .map_err(map_open_error)?;
        Ok(Self {
            storage,
            limits,
            store,
            sync: ExactArtifactSync::Normal,
        })
    }

    /// Reverifies and atomically registers one exact canonical artifact.
    ///
    /// The complete manifest is the only selection authority. A prior inventory
    /// report, path, tag, or storage key is never accepted as current evidence.
    /// The operation never qualifies or activates the artifact.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactReconciliationError`] for an invalid manifest, missing or
    /// conflicting bytes, storage drift, cancellation, or durable-state failure.
    pub fn reconcile<F>(
        &mut self,
        request: &ArtifactOrphanReconciliationRequest,
        cancellation: &CancellationToken,
        mut progress: F,
    ) -> Result<ArtifactOrphanReconciliationResult, ArtifactReconciliationError>
    where
        F: FnMut(ArtifactOrphanReconciliationProgress),
    {
        ensure_not_cancelled(cancellation)?;
        validate_manifest(&request.manifest, self.limits)?;
        self.storage.validate_layout().map_err(map_storage_error)?;
        let total = request.manifest.byte_size;
        report_progress(
            &mut progress,
            ArtifactOrphanReconciliationStage::InspectingSelection,
            0,
            total,
        );
        ensure_not_cancelled(cancellation)?;
        let name = OsString::from(request.manifest.artifact_digest.as_str());
        report_progress(
            &mut progress,
            ArtifactOrphanReconciliationStage::VerifyingOrphan,
            0,
            total,
        );
        ensure_not_cancelled(cancellation)?;
        let verified = verify_exact_artifact(
            self.storage.artifacts(),
            &name,
            ExactArtifactExpectation {
                byte_size: total,
                digest: &request.manifest.artifact_digest,
                maximum_entries: self.limits.maximum_storage_entries,
                sync: self.sync,
            },
            cancellation,
            |completed| {
                report_progress(
                    &mut progress,
                    ArtifactOrphanReconciliationStage::VerifyingOrphan,
                    completed,
                    total,
                );
            },
        )
        .map_err(map_verification_error)?;
        self.storage.validate_layout().map_err(map_storage_error)?;
        ensure_not_cancelled(cancellation)?;
        let installed = installed_from(&request.manifest);
        let state = self
            .store
            .put_installation(&request.manifest, &installed)
            .map_err(map_store_error)?;
        drop(verified);
        let disposition = match state.installed {
            WriteDisposition::Inserted => ArtifactReconciliationDisposition::Registered,
            WriteDisposition::AlreadyPresent => {
                ArtifactReconciliationDisposition::AlreadyRegistered
            }
        };
        Ok(ArtifactOrphanReconciliationResult {
            installed,
            installation: state.installation,
            disposition,
        })
    }

    #[cfg(test)]
    fn inject_sync_failure(&mut self, sync: ExactArtifactSync) {
        self.sync = sync;
    }
}

fn validate_limits(
    limits: ArtifactReconciliationLimits,
) -> Result<(), ArtifactReconciliationError> {
    if limits.maximum_artifact_bytes == 0
        || limits.maximum_storage_entries == 0
        || limits.maximum_storage_entries.checked_add(1).is_none()
    {
        Err(ArtifactReconciliationError::InvalidLimits)
    } else {
        Ok(())
    }
}

fn validate_manifest(
    manifest: &ArtifactManifest,
    limits: ArtifactReconciliationLimits,
) -> Result<(), ArtifactReconciliationError> {
    manifest
        .validate()
        .map_err(ArtifactReconciliationError::InvalidManifest)?;
    if manifest.byte_size > limits.maximum_artifact_bytes {
        Err(ArtifactReconciliationError::ArtifactTooLarge {
            actual: manifest.byte_size,
            maximum: limits.maximum_artifact_bytes,
        })
    } else {
        Ok(())
    }
}

fn installed_from(manifest: &ArtifactManifest) -> InstalledArtifact {
    InstalledArtifact {
        artifact_id: manifest.artifact_id.clone(),
        artifact_digest: manifest.artifact_digest.clone(),
        byte_size: manifest.byte_size,
        storage_key: managed_storage_key(&manifest.artifact_digest),
    }
}

fn map_open_error(error: ArtifactInventoryError) -> ArtifactReconciliationError {
    match error {
        ArtifactInventoryError::StorageNotInitialized => {
            ArtifactReconciliationError::StorageNotInitialized
        }
        ArtifactInventoryError::UnsafeStorageLayout => {
            ArtifactReconciliationError::UnsafeStorageLayout
        }
        ArtifactInventoryError::StorageInUse => ArtifactReconciliationError::StorageInUse,
        ArtifactInventoryError::StorageIo(error) => ArtifactReconciliationError::StorageIo(error),
        ArtifactInventoryError::ConcurrentModification => {
            ArtifactReconciliationError::StorageChanged
        }
        ArtifactInventoryError::StorageEntryLimitExceeded => {
            ArtifactReconciliationError::StorageEntryLimitExceeded
        }
        ArtifactInventoryError::Cancelled => ArtifactReconciliationError::Cancelled,
        ArtifactInventoryError::InvalidLimits
        | ArtifactInventoryError::StateEntryLimitExceeded
        | ArtifactInventoryError::TotalVerificationLimitExceeded
        | ArtifactInventoryError::State(_) => ArtifactReconciliationError::UnsafeStorageLayout,
    }
}

fn map_storage_error(error: ArtifactInventoryError) -> ArtifactReconciliationError {
    match error {
        ArtifactInventoryError::UnsafeStorageLayout
        | ArtifactInventoryError::ConcurrentModification
        | ArtifactInventoryError::StorageNotInitialized => {
            ArtifactReconciliationError::StorageChanged
        }
        other => map_open_error(other),
    }
}

fn map_verification_error(error: ExactArtifactVerificationError) -> ArtifactReconciliationError {
    match error {
        ExactArtifactVerificationError::Boundary(error) => map_storage_error(error),
        ExactArtifactVerificationError::Missing => ArtifactReconciliationError::OrphanNotFound,
        ExactArtifactVerificationError::SizeMismatch
        | ExactArtifactVerificationError::DigestMismatch => {
            ArtifactReconciliationError::StorageConflict
        }
        ExactArtifactVerificationError::Aliased => ArtifactReconciliationError::StorageChanged,
    }
}

fn map_store_error(error: StoreError) -> ArtifactReconciliationError {
    match error {
        StoreError::ImmutableConflict => ArtifactReconciliationError::StateConflict,
        StoreError::RemovalPending => ArtifactReconciliationError::RemovalPending,
        error @ (StoreError::Serialization(_)
        | StoreError::InvalidManifest(_)
        | StoreError::InvalidInstallation(_)
        | StoreError::RecordTooLarge
        | StoreError::CorruptRecord
        | StoreError::MissingRecord
        | StoreError::InvalidActiveBinding) => ArtifactReconciliationError::StateCorrupt(error),
        other => ArtifactReconciliationError::State(other),
    }
}

fn ensure_not_cancelled(
    cancellation: &CancellationToken,
) -> Result<(), ArtifactReconciliationError> {
    if cancellation.is_cancelled() {
        Err(ArtifactReconciliationError::Cancelled)
    } else {
        Ok(())
    }
}

fn report_progress(
    progress: &mut impl FnMut(ArtifactOrphanReconciliationProgress),
    stage: ArtifactOrphanReconciliationStage,
    completed_bytes: u64,
    total_bytes: u64,
) {
    progress(ArtifactOrphanReconciliationProgress {
        stage,
        completed_bytes,
        total_bytes,
    });
}

#[cfg(test)]
mod tests;
