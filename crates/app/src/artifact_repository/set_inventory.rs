use rewrite_model_store::ArtifactStateStore;
use rewrite_types::CancellationToken;

use crate::{
    ArtifactSetInventoryError, ArtifactSetInventoryLimits, ArtifactSetInventoryReport,
    ArtifactSetInventoryService,
};

use super::{
    ArtifactRepository, ArtifactRepositoryError, ArtifactRepositoryErrorKind, RepositoryLockMode,
    finish_operation,
};

impl ArtifactRepository {
    /// Inspects existing managed artifact sets and exact-schema state without mutation.
    ///
    /// This operation creates no directory or database, applies no migration, and
    /// does not inspect or mutate single-file artifacts. The report is
    /// point-in-time evidence only and does not grant a lease, qualify a package,
    /// or authorize a role.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactRepositoryError`] when the repository is absent, migration
    /// is required, storage is unsafe or busy, or inventory cannot complete
    /// coherently.
    pub fn inventory_set(
        &self,
        limits: ArtifactSetInventoryLimits,
        cancellation: &CancellationToken,
    ) -> Result<ArtifactSetInventoryReport, ArtifactRepositoryError> {
        self.require_data_directory()?;
        let mut guard = self.pin_data_directory(RepositoryLockMode::ExistingShared)?;
        guard.recheck()?;
        let result = (|| {
            guard.pin_state_database()?;
            guard.recheck()?;
            let store = ArtifactStateStore::open_existing_read_only(&self.state_database())?;
            guard.recheck()?;
            let service =
                ArtifactSetInventoryService::open(self.managed_storage(), &store, limits)?;
            service
                .inventory(cancellation, |_| {})
                .map_err(ArtifactRepositoryError::SetInventory)
        })();
        finish_operation(result, guard.recheck())
    }
}

pub(super) fn set_inventory_error_kind(
    error: &ArtifactSetInventoryError,
) -> ArtifactRepositoryErrorKind {
    use ArtifactRepositoryErrorKind as Kind;
    use ArtifactSetInventoryError as Error;
    match error {
        Error::InvalidLimits => Kind::InvalidInput,
        Error::StorageNotInitialized => Kind::NotInitialized,
        Error::StorageInUse => Kind::InUse,
        Error::StateEntryLimitExceeded
        | Error::StorageEntryLimitExceeded
        | Error::TotalVerificationLimitExceeded => Kind::ResourceLimit,
        Error::ConcurrentModification => Kind::ConcurrentModification,
        Error::Cancelled => Kind::Cancelled,
        Error::UnsafeStorageLayout => Kind::Conflict,
        Error::StorageIo(_) => Kind::Operational,
        Error::State(error) => super::store_error_kind(error),
    }
}
