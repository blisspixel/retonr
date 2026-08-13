use std::{ffi::OsString, path::Path};

use rewrite_model_store::{
    ArtifactRemovalPhase, ArtifactStateStore, RemovalPreparationDisposition, StoreError,
};
use rewrite_types::CancellationToken;

mod contract;

use crate::{
    artifact_inventory::ArtifactInventoryError,
    artifact_storage::{
        ExactArtifactExpectation, ExactArtifactSync, ExactArtifactVerificationError,
        ExistingArtifactStorage, LifecycleLockMode, VerifiedManagedArtifact,
        verify_exact_artifact_for_removal,
    },
};
pub use contract::{
    ArtifactRemovalDisposition, ArtifactRemovalError, ArtifactRemovalLimits,
    ArtifactRemovalProgress, ArtifactRemovalRecoveryError, ArtifactRemovalRequest,
    ArtifactRemovalResult, ArtifactRemovalStage,
};

/// Existing-only service for removing one exact inactive managed artifact.
pub struct ArtifactRemovalService<'a> {
    storage: ExistingArtifactStorage,
    limits: ArtifactRemovalLimits,
    store: &'a mut ArtifactStateStore,
    #[cfg(test)]
    fault: ArtifactRemovalTestFault,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ArtifactRemovalTestFault {
    None,
    BeforeUnlink,
    BeforeDirectorySync,
    BeforeLayoutRecheck,
    BeforeCompletion,
}

impl<'a> ArtifactRemovalService<'a> {
    /// Opens existing storage and takes its exclusive lifecycle lock.
    ///
    /// Opening creates and removes nothing, performs no implicit recovery, and
    /// accesses no network.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactRemovalError`] when limits are invalid, storage is absent
    /// or unsafe, or another lifecycle operation owns the lock.
    pub fn open_existing(
        root: impl AsRef<Path>,
        store: &'a mut ArtifactStateStore,
        limits: ArtifactRemovalLimits,
    ) -> Result<Self, ArtifactRemovalError> {
        validate_limits(limits)?;
        let storage = ExistingArtifactStorage::open(root, LifecycleLockMode::Exclusive)
            .map_err(map_open_error)?;
        Ok(Self {
            storage,
            limits,
            store,
            #[cfg(test)]
            fault: ArtifactRemovalTestFault::None,
        })
    }

    /// Removes or resumes removal of one exact installed artifact generation.
    ///
    /// The request accepts no path, tag, storage key, or prior inventory report.
    /// No callback or cancellation point runs after durable preparation.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactRemovalError::RecoveryRequired`] after preparation when
    /// an exact retry is required to finish the operation.
    pub fn remove<F>(
        &mut self,
        request: &ArtifactRemovalRequest,
        cancellation: &CancellationToken,
        mut progress: F,
    ) -> Result<ArtifactRemovalResult, ArtifactRemovalError>
    where
        F: FnMut(ArtifactRemovalProgress),
    {
        validate_request(request)?;
        self.storage.validate_layout().map_err(map_storage_error)?;
        let total = request.selection.installed.byte_size;
        let (current, removal) = self
            .store
            .artifact_removal_state(&request.selection.installed.artifact_id)
            .map_err(map_store_error)?;
        if removal.as_ref().is_some_and(|value| {
            value.selection == request.selection && value.phase == ArtifactRemovalPhase::Completed
        }) {
            return Ok(result(request, ArtifactRemovalDisposition::AlreadyRemoved));
        }
        let recovering = removal.as_ref().is_some_and(|value| {
            value.selection == request.selection && value.phase == ArtifactRemovalPhase::Prepared
        });
        if request.selection.installed.byte_size > self.limits.maximum_artifact_bytes {
            if recovering {
                return Err(ArtifactRemovalError::RecoveryRequired(
                    ArtifactRemovalRecoveryError::Storage,
                ));
            }
            return Err(ArtifactRemovalError::ArtifactTooLarge {
                actual: request.selection.installed.byte_size,
                maximum: self.limits.maximum_artifact_bytes,
            });
        }
        if !recovering && current.as_ref() != Some(&request.selection) {
            return Err(ArtifactRemovalError::StaleSelection);
        }
        if !recovering {
            ensure_not_cancelled(cancellation)?;
            report_progress(
                &mut progress,
                ArtifactRemovalStage::InspectingSelection,
                0,
                total,
            );
            ensure_not_cancelled(cancellation)?;
        }

        let name = OsString::from(request.selection.installed.artifact_digest.as_str());
        let recovery_token = CancellationToken::new();
        let verification_token = if recovering {
            &recovery_token
        } else {
            cancellation
        };
        let verified = self
            .verify(
                &name,
                request,
                verification_token,
                &mut progress,
                recovering,
            )
            .map_err(|error| {
                if recovering {
                    map_prepared_error(error)
                } else {
                    error
                }
            })?;
        if !recovering {
            report_progress(
                &mut progress,
                ArtifactRemovalStage::PreparingRemoval,
                0,
                total,
            );
            ensure_not_cancelled(cancellation)?;
            self.storage.validate_layout().map_err(map_storage_error)?;
            if let Some(verified) = verified.as_ref() {
                verified
                    .recheck_for_removal(
                        self.storage.artifacts(),
                        &name,
                        self.limits.maximum_storage_entries,
                    )
                    .map_err(map_storage_error)?;
            }
        }
        let prepared = self
            .store
            .prepare_artifact_removal(&request.selection)
            .map_err(map_store_error)?;
        if prepared == RemovalPreparationDisposition::AlreadyCompleted {
            return Ok(result(request, ArtifactRemovalDisposition::AlreadyRemoved));
        }
        self.finish_prepared(&name, request, verified)?;
        Ok(result(
            request,
            if recovering {
                ArtifactRemovalDisposition::Recovered
            } else {
                ArtifactRemovalDisposition::Removed
            },
        ))
    }

    fn verify<F>(
        &self,
        name: &std::ffi::OsStr,
        request: &ArtifactRemovalRequest,
        cancellation: &CancellationToken,
        progress: &mut F,
        recovering: bool,
    ) -> Result<Option<VerifiedManagedArtifact>, ArtifactRemovalError>
    where
        F: FnMut(ArtifactRemovalProgress),
    {
        let total = request.selection.installed.byte_size;
        if !recovering {
            report_progress(
                progress,
                ArtifactRemovalStage::VerifyingInactiveBytes,
                0,
                total,
            );
        }
        match verify_exact_artifact_for_removal(
            self.storage.artifacts(),
            name,
            ExactArtifactExpectation {
                byte_size: total,
                digest: &request.selection.installed.artifact_digest,
                maximum_entries: self.limits.maximum_storage_entries,
                sync: ExactArtifactSync::Normal,
            },
            cancellation,
            |completed| {
                if !recovering {
                    report_progress(
                        progress,
                        ArtifactRemovalStage::VerifyingInactiveBytes,
                        completed,
                        total,
                    );
                }
            },
        ) {
            Ok(verified) => Ok(Some(verified)),
            Err(ExactArtifactVerificationError::Missing) if recovering => Ok(None),
            Err(error) => Err(map_verification_error(error)),
        }
    }

    fn finish_prepared(
        &mut self,
        name: &std::ffi::OsStr,
        request: &ArtifactRemovalRequest,
        verified: Option<VerifiedManagedArtifact>,
    ) -> Result<(), ArtifactRemovalError> {
        let result = (|| {
            #[cfg(test)]
            self.inject_recovery_fault(ArtifactRemovalTestFault::BeforeUnlink)?;
            if let Some(verified) = verified {
                verified
                    .recheck_and_remove(
                        self.storage.artifacts(),
                        name,
                        self.limits.maximum_storage_entries,
                    )
                    .map_err(|error| annotate_storage_error("unlink", error))?;
            } else {
                self.storage
                    .artifacts()
                    .confirm_managed_file_absent(name, self.limits.maximum_storage_entries)?;
            }
            #[cfg(test)]
            self.inject_recovery_fault(ArtifactRemovalTestFault::BeforeDirectorySync)?;
            self.storage
                .artifacts()
                .sync()
                .map_err(|error| annotate_storage_error("directory sync", error))?;
            #[cfg(test)]
            self.inject_recovery_fault(ArtifactRemovalTestFault::BeforeLayoutRecheck)?;
            self.storage
                .validate_layout()
                .map_err(|error| annotate_storage_error("layout recheck", error))?;
            Ok::<(), ArtifactInventoryError>(())
        })();
        result.map_err(|error| {
            ArtifactRemovalError::RecoveryRequired(map_recovery_storage_error(error))
        })?;
        #[cfg(test)]
        if self.fault == ArtifactRemovalTestFault::BeforeCompletion {
            return Err(ArtifactRemovalError::RecoveryRequired(
                ArtifactRemovalRecoveryError::State(StoreError::CorruptRecord),
            ));
        }
        self.store
            .complete_artifact_removal(&request.selection)
            .map_err(|error| {
                ArtifactRemovalError::RecoveryRequired(ArtifactRemovalRecoveryError::State(error))
            })?;
        Ok(())
    }

    #[cfg(test)]
    fn inject_recovery_fault(
        &self,
        expected: ArtifactRemovalTestFault,
    ) -> Result<(), ArtifactInventoryError> {
        if self.fault == expected {
            Err(ArtifactInventoryError::StorageIo(std::io::Error::other(
                "injected post-preparation failure",
            )))
        } else {
            Ok(())
        }
    }
}

fn annotate_storage_error(
    operation: &'static str,
    error: ArtifactInventoryError,
) -> ArtifactInventoryError {
    match error {
        ArtifactInventoryError::StorageIo(source) => ArtifactInventoryError::StorageIo(
            std::io::Error::new(source.kind(), format!("{operation}: {source}")),
        ),
        other => other,
    }
}

fn validate_limits(limits: ArtifactRemovalLimits) -> Result<(), ArtifactRemovalError> {
    if limits.maximum_artifact_bytes == 0
        || limits.maximum_storage_entries == 0
        || limits.maximum_storage_entries.checked_add(1).is_none()
    {
        Err(ArtifactRemovalError::InvalidLimits)
    } else {
        Ok(())
    }
}

fn validate_request(request: &ArtifactRemovalRequest) -> Result<(), ArtifactRemovalError> {
    request
        .selection
        .installed
        .validate()
        .map_err(|_| ArtifactRemovalError::InvalidSelection)?;
    if request.selection.epoch.get() == 0 {
        return Err(ArtifactRemovalError::InvalidSelection);
    }
    let expected = format!(
        "artifacts/{}",
        request.selection.installed.artifact_digest.as_str()
    );
    if request.selection.installed.storage_key != expected {
        return Err(ArtifactRemovalError::InvalidSelection);
    }
    Ok(())
}

fn result(
    request: &ArtifactRemovalRequest,
    disposition: ArtifactRemovalDisposition,
) -> ArtifactRemovalResult {
    ArtifactRemovalResult {
        selection: request.selection.clone(),
        disposition,
    }
}

fn report_progress(
    progress: &mut impl FnMut(ArtifactRemovalProgress),
    stage: ArtifactRemovalStage,
    completed_bytes: u64,
    total_bytes: u64,
) {
    progress(ArtifactRemovalProgress {
        stage,
        completed_bytes,
        total_bytes,
    });
}

fn ensure_not_cancelled(cancellation: &CancellationToken) -> Result<(), ArtifactRemovalError> {
    if cancellation.is_cancelled() {
        Err(ArtifactRemovalError::Cancelled)
    } else {
        Ok(())
    }
}

fn map_open_error(error: ArtifactInventoryError) -> ArtifactRemovalError {
    match error {
        ArtifactInventoryError::StorageNotInitialized => {
            ArtifactRemovalError::StorageNotInitialized
        }
        ArtifactInventoryError::UnsafeStorageLayout
        | ArtifactInventoryError::InvalidLimits
        | ArtifactInventoryError::StateEntryLimitExceeded
        | ArtifactInventoryError::TotalVerificationLimitExceeded
        | ArtifactInventoryError::State(_) => ArtifactRemovalError::UnsafeStorageLayout,
        ArtifactInventoryError::StorageInUse => ArtifactRemovalError::StorageInUse,
        ArtifactInventoryError::StorageEntryLimitExceeded => {
            ArtifactRemovalError::StorageEntryLimitExceeded
        }
        ArtifactInventoryError::Cancelled => ArtifactRemovalError::Cancelled,
        ArtifactInventoryError::StorageIo(error) => ArtifactRemovalError::StorageIo(error),
        ArtifactInventoryError::ConcurrentModification => ArtifactRemovalError::StorageChanged,
    }
}

fn map_storage_error(error: ArtifactInventoryError) -> ArtifactRemovalError {
    match error {
        ArtifactInventoryError::ConcurrentModification
        | ArtifactInventoryError::StorageNotInitialized
        | ArtifactInventoryError::UnsafeStorageLayout => ArtifactRemovalError::StorageChanged,
        other => map_open_error(other),
    }
}

fn map_verification_error(error: ExactArtifactVerificationError) -> ArtifactRemovalError {
    match error {
        ExactArtifactVerificationError::Boundary(error) => map_storage_error(error),
        ExactArtifactVerificationError::Missing => ArtifactRemovalError::BytesMissing,
        ExactArtifactVerificationError::SizeMismatch
        | ExactArtifactVerificationError::DigestMismatch => ArtifactRemovalError::StorageConflict,
        ExactArtifactVerificationError::Aliased => ArtifactRemovalError::StorageChanged,
    }
}

fn map_store_error(error: StoreError) -> ArtifactRemovalError {
    match error {
        StoreError::ActiveArtifact => ArtifactRemovalError::ActiveArtifact,
        StoreError::StaleInstallation | StoreError::MissingRecord => {
            ArtifactRemovalError::StaleSelection
        }
        error @ (StoreError::Serialization(_)
        | StoreError::InvalidManifest(_)
        | StoreError::InvalidInstallation(_)
        | StoreError::RecordTooLarge
        | StoreError::CorruptRecord
        | StoreError::InvalidActiveBinding) => ArtifactRemovalError::StateCorrupt(error),
        other => ArtifactRemovalError::State(other),
    }
}

fn map_recovery_storage_error(error: ArtifactInventoryError) -> ArtifactRemovalRecoveryError {
    match error {
        ArtifactInventoryError::StorageIo(error) => ArtifactRemovalRecoveryError::StorageIo(error),
        _ => ArtifactRemovalRecoveryError::Storage,
    }
}

fn map_prepared_error(error: ArtifactRemovalError) -> ArtifactRemovalError {
    match error {
        ArtifactRemovalError::StorageIo(source) => {
            ArtifactRemovalError::RecoveryRequired(ArtifactRemovalRecoveryError::StorageIo(source))
        }
        _ => ArtifactRemovalError::RecoveryRequired(ArtifactRemovalRecoveryError::Storage),
    }
}

#[cfg(test)]
mod tests;
