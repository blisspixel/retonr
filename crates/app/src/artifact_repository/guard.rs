use std::{
    ffi::OsStr,
    fs::{File, TryLockError},
    path::PathBuf,
};

use rewrite_types::CancellationToken;

use super::{
    ArtifactInstallationKey, ArtifactInventoryError, ArtifactRepositoryError, REPOSITORY_LOCK_FILE,
    STATE_DATABASE_FILE,
};
use crate::artifact_storage::{MetadataFingerprint, PinnedDirectory, fingerprint_std_file};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum RepositoryLockMode {
    InitializeExclusive,
    ExistingShared,
    ExistingExclusive,
}

pub(crate) struct DataDirectoryGuard {
    pub(crate) path: PathBuf,
    pub(crate) lock: File,
    pub(crate) lock_fingerprint: MetadataFingerprint,
    pub(crate) pinned: PinnedDirectory,
    pub(crate) state_database: Option<(File, MetadataFingerprint)>,
}

impl DataDirectoryGuard {
    pub(crate) fn pin_state_database(&mut self) -> Result<(), ArtifactRepositoryError> {
        let state_database = self
            .pinned
            .open_lock_file(OsStr::new(STATE_DATABASE_FILE))
            .map_err(map_data_directory_boundary_error)?;
        if !state_database.1.has_single_link() {
            return Err(ArtifactRepositoryError::UnsafeDataDirectory);
        }
        self.state_database = Some(state_database);
        Ok(())
    }

    pub(crate) fn pin_or_create_state_database(&mut self) -> Result<(), ArtifactRepositoryError> {
        let state_database = self
            .pinned
            .open_or_create_lock_file(OsStr::new(STATE_DATABASE_FILE))
            .map_err(map_data_directory_boundary_error)?;
        if !state_database.1.has_single_link() {
            return Err(ArtifactRepositoryError::UnsafeDataDirectory);
        }
        self.pinned
            .sync()
            .map_err(map_data_directory_boundary_error)?;
        self.state_database = Some(state_database);
        Ok(())
    }

    pub(crate) fn recheck(&self) -> Result<(), ArtifactRepositoryError> {
        let held_lock =
            fingerprint_std_file(&self.lock).map_err(map_data_directory_boundary_error)?;
        let current_lock = self
            .pinned
            .child_file_fingerprint(OsStr::new(REPOSITORY_LOCK_FILE))
            .map_err(map_data_directory_boundary_error)?;
        let pinned = self
            .pinned
            .fingerprint()
            .map_err(map_data_directory_boundary_error)?;
        let current = PinnedDirectory::fingerprint_path(&self.path)
            .map_err(map_data_directory_boundary_error)?;
        let state_matches = self
            .state_database
            .as_ref()
            .is_none_or(|(file, fingerprint)| {
                fingerprint_std_file(file)
                    .is_ok_and(|held| held.same_identity(fingerprint) && held.has_single_link())
                    && self
                        .pinned
                        .child_file_fingerprint(OsStr::new(STATE_DATABASE_FILE))
                        .is_ok_and(|current| {
                            current.same_identity(fingerprint) && current.has_single_link()
                        })
            });
        if held_lock == self.lock_fingerprint
            && current_lock == self.lock_fingerprint
            && pinned == current
            && state_matches
        {
            Ok(())
        } else {
            Err(ArtifactRepositoryError::UnsafeDataDirectory)
        }
    }
}

pub(crate) fn map_data_directory_boundary_error(
    error: ArtifactInventoryError,
) -> ArtifactRepositoryError {
    match error {
        ArtifactInventoryError::StorageInUse => ArtifactRepositoryError::RepositoryInUse,
        ArtifactInventoryError::StorageIo(error) => ArtifactRepositoryError::DataDirectoryIo(error),
        _ => ArtifactRepositoryError::UnsafeDataDirectory,
    }
}

pub(crate) fn lock_exclusive(file: &File) -> Result<(), ArtifactRepositoryError> {
    match file.try_lock() {
        Ok(()) => Ok(()),
        Err(TryLockError::WouldBlock) => Err(ArtifactRepositoryError::RepositoryInUse),
        Err(TryLockError::Error(error)) => Err(ArtifactRepositoryError::DataDirectoryIo(error)),
    }
}

pub(crate) fn map_removal_error(
    key: &ArtifactInstallationKey,
    error: crate::ArtifactRemovalError,
) -> ArtifactRepositoryError {
    if matches!(error, crate::ArtifactRemovalError::RecoveryRequired(_)) {
        ArtifactRepositoryError::RemovalRecoveryRequired {
            key: key.clone(),
            source: error,
        }
    } else {
        ArtifactRepositoryError::Removal(error)
    }
}

pub(crate) fn finish_operation<T>(
    result: Result<T, ArtifactRepositoryError>,
    boundary: Result<(), ArtifactRepositoryError>,
) -> Result<T, ArtifactRepositoryError> {
    match (result, boundary) {
        (
            Err(
                error @ (ArtifactRepositoryError::RemovalRecoveryRequired { .. }
                | ArtifactRepositoryError::SetRemovalRecoveryRequired { .. }),
            ),
            Err(_),
        )
        | (_, Err(error)) => Err(error),
        (result, Ok(())) => result,
    }
}

pub(crate) fn ensure_repository_not_cancelled(
    cancellation: &CancellationToken,
) -> Result<(), ArtifactRepositoryError> {
    if cancellation.is_cancelled() {
        Err(ArtifactRepositoryError::Cancelled)
    } else {
        Ok(())
    }
}
