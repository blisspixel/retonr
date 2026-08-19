use rewrite_model::ArtifactSetManifest;
use rewrite_model_store::ArtifactStateStore;
use rewrite_types::CancellationToken;

use crate::{
    ArtifactSetReconciliationError, ArtifactSetReconciliationLimits,
    ArtifactSetReconciliationRequest, ArtifactSetReconciliationResult,
    ArtifactSetReconciliationService,
};

use super::{
    ArtifactRepository, ArtifactRepositoryError, ArtifactRepositoryErrorKind,
    ArtifactSetInstallationKey, RepositoryLockMode, finish_operation, store_error_kind,
};

/// Successful repository-level selected set-root reconciliation result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactRepositorySetReconciliationResult {
    /// Exact store-issued artifact-set installation key.
    pub key: ArtifactSetInstallationKey,
    /// Whether this call registered or confirmed exact prior state.
    pub disposition: crate::ArtifactReconciliationDisposition,
}

impl From<ArtifactSetReconciliationResult> for ArtifactRepositorySetReconciliationResult {
    fn from(value: ArtifactSetReconciliationResult) -> Self {
        Self {
            key: ArtifactSetInstallationKey::from_stored(&value.installation),
            disposition: value.disposition,
        }
    }
}

impl ArtifactRepository {
    /// Reverifies and registers one exact existing set root selected only by manifest.
    ///
    /// This operation creates no repository layout and never copies, replaces,
    /// qualifies, or activates the set. The result retains the exact
    /// state-store-issued installation epoch.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactRepositoryError`] when existing state or storage cannot be
    /// opened safely, or the selected set root cannot be verified and registered.
    pub fn reconcile_set(
        &self,
        manifest: ArtifactSetManifest,
        limits: ArtifactSetReconciliationLimits,
        cancellation: &CancellationToken,
    ) -> Result<ArtifactRepositorySetReconciliationResult, ArtifactRepositoryError> {
        self.require_data_directory()?;
        let mut guard = self.pin_data_directory(RepositoryLockMode::ExistingExclusive)?;
        guard.recheck()?;
        let result = (|| {
            guard.pin_state_database()?;
            guard.recheck()?;
            let mut store =
                ArtifactStateStore::open_existing_writable_exact(&self.state_database())?;
            guard.recheck()?;
            let mut service = ArtifactSetReconciliationService::open_existing(
                self.managed_storage(),
                &mut store,
                limits,
            )?;
            service
                .reconcile(
                    &ArtifactSetReconciliationRequest { manifest },
                    cancellation,
                    |_| {},
                )
                .map(ArtifactRepositorySetReconciliationResult::from)
                .map_err(ArtifactRepositoryError::SetReconciliation)
        })();
        finish_operation(result, guard.recheck())
    }
}

pub(super) fn set_reconciliation_error_kind(
    error: &ArtifactSetReconciliationError,
) -> ArtifactRepositoryErrorKind {
    use ArtifactRepositoryErrorKind as Kind;
    use ArtifactSetReconciliationError as Error;
    match error {
        Error::InvalidLimits | Error::InvalidManifest(_) | Error::InvalidInstallation(_) => {
            Kind::InvalidInput
        }
        Error::ArtifactSetTooLarge { .. } | Error::StorageEntryLimitExceeded => Kind::ResourceLimit,
        Error::StorageNotInitialized => Kind::NotInitialized,
        Error::StorageInUse => Kind::InUse,
        Error::OrphanNotFound => Kind::NotFound,
        Error::StorageChanged => Kind::ConcurrentModification,
        Error::Cancelled => Kind::Cancelled,
        Error::UnsafeStorageLayout | Error::StorageConflict | Error::StateConflict => {
            Kind::Conflict
        }
        Error::StorageIo(_) | Error::State(_) => Kind::Operational,
        Error::StateCorrupt(error) => store_error_kind(error),
    }
}
