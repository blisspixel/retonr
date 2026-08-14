use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::artifact_storage::{OwnedStagingTree, PinnedDirectory, is_indirect};

use super::{ArtifactSetImportError, ArtifactSetImportLimits, boundary::map_managed_tree};

pub(super) struct PinnedSourceTree {
    path: PathBuf,
    pub(super) directory: PinnedDirectory,
}

impl PinnedSourceTree {
    pub(super) fn open(path: &Path) -> Result<Self, ArtifactSetImportError> {
        let path = std::path::absolute(path).map_err(ArtifactSetImportError::SourceIo)?;
        let metadata = fs::symlink_metadata(&path).map_err(ArtifactSetImportError::SourceIo)?;
        if is_indirect(&metadata) {
            return Err(ArtifactSetImportError::IndirectSource);
        }
        if !metadata.is_dir() {
            return Err(ArtifactSetImportError::SourceNotDirectory);
        }
        let directory = PinnedDirectory::open_existing(&path).map_err(|error| match error {
            crate::ArtifactInventoryError::StorageIo(error) => {
                ArtifactSetImportError::SourceIo(error)
            }
            _ => ArtifactSetImportError::UnsafeSourceTree,
        })?;
        let source = Self { path, directory };
        source.recheck()?;
        Ok(source)
    }

    pub(super) fn recheck(&self) -> Result<(), ArtifactSetImportError> {
        let held = self.directory.fingerprint().map_err(map_source_boundary)?;
        let current = PinnedDirectory::fingerprint_path(&self.path).map_err(map_source_boundary)?;
        if held == current {
            Ok(())
        } else {
            Err(ArtifactSetImportError::StorageChanged)
        }
    }
}

pub(super) fn fail_with_cleanup<T>(
    staging: OwnedStagingTree,
    original: ArtifactSetImportError,
) -> Result<T, ArtifactSetImportError> {
    match staging.cleanup() {
        Ok(()) => Err(original),
        Err(error) => Err(map_managed_tree(error)),
    }
}

pub(super) fn validate_limit_shape(
    limits: ArtifactSetImportLimits,
) -> Result<(), ArtifactSetImportError> {
    if limits.maximum_members == 0
        || limits.maximum_member_bytes == 0
        || limits.maximum_total_bytes == 0
        || limits.maximum_tree_entries == 0
        || limits.maximum_storage_entries == 0
        || limits.maximum_staging_entries == 0
    {
        Err(ArtifactSetImportError::InvalidLimits)
    } else {
        Ok(())
    }
}

fn map_source_boundary(error: crate::ArtifactInventoryError) -> ArtifactSetImportError {
    match error {
        crate::ArtifactInventoryError::StorageIo(error) => ArtifactSetImportError::SourceIo(error),
        crate::ArtifactInventoryError::ConcurrentModification => {
            ArtifactSetImportError::StorageChanged
        }
        _ => ArtifactSetImportError::UnsafeSourceTree,
    }
}
