use rewrite_model_store::{ArtifactRemovalPhase, ArtifactStateStore};
use rewrite_types::CancellationToken;

use crate::{
    ArtifactSetRemovalError, ArtifactSetRemovalLimits, ArtifactSetRemovalRequest,
    ArtifactSetRemovalService,
};

use super::{
    ArtifactRepository, ArtifactRepositoryError, ArtifactRepositoryErrorKind,
    ArtifactRepositorySetRemovalResult, ArtifactSetInstallationKey, RepositoryLockMode,
    finish_operation, store_error_kind,
};

impl ArtifactRepository {
    /// Removes one exact current inactive artifact-set installation generation.
    ///
    /// A prepared set removal is never resumed implicitly. Call
    /// [`Self::recover_set_removal`] to make the non-cancellable recovery
    /// boundary explicit. The operation does not qualify, activate, or lease
    /// the set.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactRepositoryError::SetRemovalRecoveryPending`] when an exact
    /// prepared journal exists, or another [`ArtifactRepositoryError`] on failure.
    pub fn remove_set(
        &self,
        key: &ArtifactSetInstallationKey,
        limits: ArtifactSetRemovalLimits,
        cancellation: &CancellationToken,
    ) -> Result<ArtifactRepositorySetRemovalResult, ArtifactRepositoryError> {
        self.require_data_directory()?;
        key.validate()?;
        let mut guard = self.pin_data_directory(RepositoryLockMode::ExistingExclusive)?;
        guard.recheck()?;
        let result = (|| {
            guard.pin_state_database()?;
            guard.recheck()?;
            let mut store =
                ArtifactStateStore::open_existing_writable_exact(&self.state_database())?;
            guard.recheck()?;
            let (current, removal) = store.artifact_set_removal_state(key.artifact_set_id())?;
            if let Some(removal) = removal.as_ref()
                && ArtifactSetInstallationKey::from_stored(&removal.selection) == *key
            {
                if removal.phase == ArtifactRemovalPhase::Prepared {
                    return Err(ArtifactRepositoryError::SetRemovalRecoveryPending {
                        key: key.clone(),
                    });
                }
                return Ok(ArtifactRepositorySetRemovalResult {
                    key: key.clone(),
                    disposition: crate::ArtifactRemovalDisposition::AlreadyRemoved,
                });
            }
            let selection = current.ok_or(ArtifactRepositoryError::ArtifactSetNotInstalled)?;
            if ArtifactSetInstallationKey::from_stored(&selection) != *key {
                return Err(ArtifactRepositoryError::StaleSetInstallation);
            }
            let mut service = ArtifactSetRemovalService::open_existing(
                self.managed_storage(),
                &mut store,
                limits,
            )?;
            service
                .remove(
                    &ArtifactSetRemovalRequest { selection },
                    cancellation,
                    |_| {},
                )
                .map(ArtifactRepositorySetRemovalResult::from)
                .map_err(|error| map_set_removal_error(key, error))
        })();
        finish_operation(result, guard.recheck())
    }

    /// Forward-completes one exact durably prepared artifact-set removal.
    ///
    /// Recovery intentionally ignores cancellation and emits no progress after the
    /// prior durable preparation.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactRepositoryError::SetRemovalRecoveryNotPending`] unless the
    /// set has one exact Prepared journal and no current installation.
    pub fn recover_set_removal(
        &self,
        key: &ArtifactSetInstallationKey,
        limits: ArtifactSetRemovalLimits,
    ) -> Result<ArtifactRepositorySetRemovalResult, ArtifactRepositoryError> {
        self.require_data_directory()?;
        key.validate()?;
        let mut guard = self.pin_data_directory(RepositoryLockMode::ExistingExclusive)?;
        guard.recheck()?;
        let result = (|| {
            guard.pin_state_database()?;
            guard.recheck()?;
            let mut store =
                ArtifactStateStore::open_existing_writable_exact(&self.state_database())?;
            guard.recheck()?;
            let (current, removal) = store.artifact_set_removal_state(key.artifact_set_id())?;
            let removal = removal.ok_or(ArtifactRepositoryError::SetRemovalRecoveryNotPending)?;
            if ArtifactSetInstallationKey::from_stored(&removal.selection) != *key {
                return Err(ArtifactRepositoryError::StaleSetInstallation);
            }
            if removal.phase == ArtifactRemovalPhase::Completed {
                return Ok(ArtifactRepositorySetRemovalResult {
                    key: key.clone(),
                    disposition: crate::ArtifactRemovalDisposition::AlreadyRemoved,
                });
            }
            if current.is_some() {
                return Err(ArtifactRepositoryError::SetRemovalRecoveryNotPending);
            }
            let mut service = ArtifactSetRemovalService::open_existing(
                self.managed_storage(),
                &mut store,
                limits,
            )?;
            service
                .remove(
                    &ArtifactSetRemovalRequest {
                        selection: removal.selection,
                    },
                    &CancellationToken::new(),
                    |_| {},
                )
                .map(ArtifactRepositorySetRemovalResult::from)
                .map_err(|error| map_set_removal_error(key, error))
        })();
        finish_operation(result, guard.recheck())
    }
}

pub(super) fn set_removal_error_kind(
    error: &ArtifactSetRemovalError,
) -> ArtifactRepositoryErrorKind {
    use ArtifactRepositoryErrorKind as Kind;
    use ArtifactSetRemovalError as Error;
    match error {
        Error::InvalidLimits
        | Error::InvalidSelection
        | Error::InvalidManifest(_)
        | Error::InvalidInstallation(_) => Kind::InvalidInput,
        Error::ArtifactSetTooLarge { .. } | Error::StorageEntryLimitExceeded => Kind::ResourceLimit,
        Error::StorageNotInitialized => Kind::NotInitialized,
        Error::StorageInUse => Kind::InUse,
        Error::StaleSelection => Kind::StaleSelection,
        Error::TreeMissing => Kind::NotFound,
        Error::StorageChanged => Kind::ConcurrentModification,
        Error::Cancelled => Kind::Cancelled,
        Error::UnsafeStorageLayout | Error::StorageConflict => Kind::Conflict,
        Error::StorageIo(_) | Error::State(_) => Kind::Operational,
        Error::StateCorrupt(error) => store_error_kind(error),
        Error::RecoveryRequired(_) => Kind::RecoveryRequired,
    }
}

fn map_set_removal_error(
    key: &ArtifactSetInstallationKey,
    error: ArtifactSetRemovalError,
) -> ArtifactRepositoryError {
    if matches!(error, ArtifactSetRemovalError::RecoveryRequired(_)) {
        ArtifactRepositoryError::SetRemovalRecoveryRequired {
            key: key.clone(),
            source: error,
        }
    } else {
        ArtifactRepositoryError::SetRemoval(error)
    }
}
