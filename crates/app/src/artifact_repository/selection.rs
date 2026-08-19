use std::fs;

use rewrite_model::{
    ActiveArtifactBinding, ArtifactRole, InstalledArtifact, QualificationId, QualificationRecord,
};
use rewrite_model_store::{ArtifactStateStore, StoreError};

use super::{
    ArtifactRepository, ArtifactRepositoryError, RepositoryLockMode, finish_operation, is_indirect,
};

impl ArtifactRepository {
    /// Returns the active generation binding after revalidating durable state.
    ///
    /// The call is read-only. It does not activate a role, start a runtime, or
    /// grant claim-extraction authority.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactRepositoryError`] when the repository is absent, busy,
    /// incompatible, or the persisted binding no longer validates.
    pub fn active_generation_binding(
        &self,
    ) -> Result<Option<ActiveArtifactBinding>, ArtifactRepositoryError> {
        self.require_data_directory()?;
        let mut guard = self.pin_data_directory(RepositoryLockMode::ExistingShared)?;
        guard.recheck()?;
        let result = (|| {
            guard.pin_state_database()?;
            guard.recheck()?;
            let store = ArtifactStateStore::open_existing_read_only(&self.state_database())?;
            guard.recheck()?;
            let storage = self.managed_storage();
            store
                .active_binding(ArtifactRole::Generation, |installed| {
                    installed_bytes_present(&storage, installed)
                })
                .map_err(ArtifactRepositoryError::State)
        })();
        finish_operation(result, guard.recheck())
    }

    /// Reloads one qualification record after revalidating durable identity.
    ///
    /// The call is read-only. It does not activate a role or attach a runtime.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactRepositoryError`] when the repository is absent, busy,
    /// incompatible, or the qualification cannot be reloaded.
    pub fn generation_qualification(
        &self,
        qualification_id: &QualificationId,
    ) -> Result<QualificationRecord, ArtifactRepositoryError> {
        self.require_data_directory()?;
        let mut guard = self.pin_data_directory(RepositoryLockMode::ExistingShared)?;
        guard.recheck()?;
        let result = (|| {
            guard.pin_state_database()?;
            guard.recheck()?;
            let store = ArtifactStateStore::open_existing_read_only(&self.state_database())?;
            guard.recheck()?;
            store
                .qualification(qualification_id)?
                .ok_or(ArtifactRepositoryError::State(StoreError::MissingRecord))
        })();
        finish_operation(result, guard.recheck())
    }
}

fn installed_bytes_present(storage: &std::path::Path, installed: &InstalledArtifact) -> bool {
    if installed.storage_key.contains('\0')
        || installed
            .storage_key
            .split(['/', '\\'])
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return false;
    }
    let path = storage.join(&installed.storage_key);
    match fs::symlink_metadata(&path) {
        Ok(metadata) => {
            metadata.is_file() && !is_indirect(&metadata) && metadata.len() == installed.byte_size
        }
        Err(_) => false,
    }
}
