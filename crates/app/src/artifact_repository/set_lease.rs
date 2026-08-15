use std::ffi::OsStr;

use rewrite_model::ArtifactSetId;
use rewrite_model_store::ArtifactStateStore;
use rewrite_types::CancellationToken;

use crate::{
    RuntimeArtifactSetLease, RuntimeArtifactSetLeaseLimits,
    runtime_artifact_set_lease::acquire_artifact_set,
};

use super::{
    ArtifactRepository, ArtifactRepositoryError, MANAGED_STORAGE_DIRECTORY, RepositoryLockMode,
    ensure_repository_not_cancelled, finish_operation,
};

impl ArtifactRepository {
    /// Acquires one shared lease over an exact registered managed artifact set.
    ///
    /// The lease holds the shared repository and storage lifecycle locks for its
    /// complete lifetime, so every exclusive operation fails while it is live.
    /// Acquisition recomputes the content-derived set-root name from the
    /// registered manifest, verifies every member's size, single link, and
    /// SHA-256 under a stable tree snapshot, and rereads durable state after
    /// verification.
    ///
    /// This operation creates no repository layout, applies no migration, writes
    /// no durable state, and grants no qualification, activation, or role
    /// authority.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactRepositoryError`] when the repository is absent, busy,
    /// or unsafe, or [`ArtifactRepositoryError::SetLease`] when the selected set
    /// is not installed, its ceilings are invalid, or its managed bytes and
    /// durable state disagree.
    pub fn lease_set(
        &self,
        artifact_set_id: &ArtifactSetId,
        limits: RuntimeArtifactSetLeaseLimits,
        cancellation: &CancellationToken,
    ) -> Result<RuntimeArtifactSetLease, ArtifactRepositoryError> {
        ensure_repository_not_cancelled(cancellation)?;
        self.require_data_directory()?;
        let mut guard = self.pin_data_directory(RepositoryLockMode::ExistingShared)?;
        guard.recheck()?;
        let acquired = (|| {
            guard.pin_state_database()?;
            guard.recheck()?;
            let store = ArtifactStateStore::open_existing_read_only(&self.state_database())?;
            guard.recheck()?;
            acquire_artifact_set(
                &guard.pinned,
                OsStr::new(MANAGED_STORAGE_DIRECTORY),
                &store,
                artifact_set_id,
                limits,
                cancellation,
            )
            .map_err(ArtifactRepositoryError::SetLease)
        })();
        let acquired = finish_operation(acquired, guard.recheck())?;
        Ok(RuntimeArtifactSetLease::from_parts(guard, acquired))
    }
}
