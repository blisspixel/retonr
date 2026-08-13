use std::{
    ffi::OsStr,
    fs::File,
    path::{Path, PathBuf},
};

use rewrite_model_store::ExclusiveArtifactLifecycleLock;

use super::{
    ArtifactInventoryError, MetadataFingerprint, PinnedDirectory, fingerprint_std_file, lock_shared,
};

pub(crate) const LIFECYCLE_LOCK_FILE: &str = ".artifact-import.lock";

#[derive(Clone, Copy)]
pub(crate) enum LifecycleLockMode {
    Shared,
    Exclusive,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ArtifactStorageLayoutFingerprint {
    root: MetadataFingerprint,
    lock_path: MetadataFingerprint,
    artifacts: MetadataFingerprint,
}

pub(crate) struct ExistingArtifactStorage {
    root_path: PathBuf,
    root: PinnedDirectory,
    artifacts: PinnedDirectory,
    lock: File,
    exclusive_lock: Option<ExclusiveArtifactLifecycleLock>,
}

impl ExistingArtifactStorage {
    pub(crate) fn open(
        root: impl AsRef<Path>,
        lock_mode: LifecycleLockMode,
    ) -> Result<Self, ArtifactInventoryError> {
        let root_path =
            std::path::absolute(root.as_ref()).map_err(ArtifactInventoryError::StorageIo)?;
        let root = PinnedDirectory::open_existing(&root_path)?;
        let (lock, _) = root.open_lock_file(OsStr::new(LIFECYCLE_LOCK_FILE))?;
        let exclusive_lock = match lock_mode {
            LifecycleLockMode::Shared => {
                lock_shared(&lock)?;
                None
            }
            LifecycleLockMode::Exclusive => Some(
                ExclusiveArtifactLifecycleLock::try_acquire(
                    lock.try_clone()
                        .map_err(ArtifactInventoryError::StorageIo)?,
                )
                .map_err(map_exclusive_lock_error)?,
            ),
        };
        let artifacts = root.open_child_directory(OsStr::new("artifacts"))?;
        let storage = Self {
            root_path,
            root,
            artifacts,
            lock,
            exclusive_lock,
        };
        storage.validate_layout()?;
        Ok(storage)
    }

    pub(crate) fn artifacts(&self) -> &PinnedDirectory {
        &self.artifacts
    }

    pub(crate) fn exclusive_lifecycle_lock(
        &self,
    ) -> Result<&ExclusiveArtifactLifecycleLock, ArtifactInventoryError> {
        self.exclusive_lock
            .as_ref()
            .ok_or(ArtifactInventoryError::UnsafeStorageLayout)
    }

    pub(crate) fn validate_layout(
        &self,
    ) -> Result<ArtifactStorageLayoutFingerprint, ArtifactInventoryError> {
        let root = self.root.fingerprint()?;
        if root != PinnedDirectory::fingerprint_path(&self.root_path)? {
            return Err(ArtifactInventoryError::ConcurrentModification);
        }
        let lock_path = self
            .root
            .child_file_fingerprint(OsStr::new(LIFECYCLE_LOCK_FILE))?;
        if lock_path != fingerprint_std_file(&self.lock)? {
            return Err(ArtifactInventoryError::ConcurrentModification);
        }
        let artifacts = self.artifacts.fingerprint()?;
        if artifacts
            != self
                .root
                .child_directory_fingerprint(OsStr::new("artifacts"))?
        {
            return Err(ArtifactInventoryError::ConcurrentModification);
        }
        Ok(ArtifactStorageLayoutFingerprint {
            root,
            lock_path,
            artifacts,
        })
    }
}

fn map_exclusive_lock_error(error: std::fs::TryLockError) -> ArtifactInventoryError {
    match error {
        std::fs::TryLockError::WouldBlock => ArtifactInventoryError::StorageInUse,
        std::fs::TryLockError::Error(error) => ArtifactInventoryError::StorageIo(error),
    }
}
