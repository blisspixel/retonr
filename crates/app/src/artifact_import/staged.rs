use std::{
    ffi::{OsStr, OsString},
    fs::File,
};

use crate::artifact_storage::{MetadataFingerprint, PinnedDirectory, fingerprint_std_file};
use rewrite_types::CancellationToken;

use super::{
    ArtifactImportError, MAX_RECOVERY_ENTRIES, map_boundary_recovery_error,
    map_boundary_storage_error,
};

pub(super) struct StagedArtifact<'a> {
    directory: &'a PinnedDirectory,
    name: OsString,
    file: Option<File>,
    frozen: Option<MetadataFingerprint>,
    cleaned: bool,
}

impl<'a> StagedArtifact<'a> {
    pub(super) fn new(directory: &'a PinnedDirectory, name: OsString, file: File) -> Self {
        Self {
            directory,
            name,
            file: Some(file),
            frozen: None,
            cleaned: false,
        }
    }

    pub(super) fn name(&self) -> &OsStr {
        &self.name
    }

    pub(super) fn file_mut(&mut self) -> Result<&mut File, ArtifactImportError> {
        self.file
            .as_mut()
            .ok_or(ArtifactImportError::StorageChanged)
    }

    pub(super) fn freeze_single_link(&mut self) -> Result<(), ArtifactImportError> {
        let fingerprint = fingerprint_std_file(
            self.file
                .as_ref()
                .ok_or(ArtifactImportError::StorageChanged)?,
        )
        .map_err(map_boundary_storage_error)?;
        if !fingerprint.has_single_link() {
            return Err(ArtifactImportError::StorageChanged);
        }
        self.frozen = Some(fingerprint);
        Ok(())
    }

    pub(super) fn recheck_frozen_single_link(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), ArtifactImportError> {
        let frozen = self
            .frozen
            .as_ref()
            .ok_or(ArtifactImportError::StorageChanged)?;
        self.directory
            .recheck_managed_file_for_lifecycle(
                &self.name,
                frozen,
                MAX_RECOVERY_ENTRIES,
                cancellation,
            )
            .map_err(map_boundary_recovery_error)?;
        if frozen.has_single_link() {
            Ok(())
        } else {
            Err(ArtifactImportError::StorageChanged)
        }
    }

    pub(super) fn cleanup(mut self) -> Result<(), ArtifactImportError> {
        self.frozen.take();
        let file = self
            .file
            .as_ref()
            .ok_or(ArtifactImportError::StorageChanged)?;
        self.directory
            .remove_file_if_same_identity(&self.name, file)
            .map_err(map_boundary_storage_error)?;
        self.file.take();
        self.directory.sync().map_err(map_boundary_storage_error)?;
        self.cleaned = true;
        Ok(())
    }
}

impl Drop for StagedArtifact<'_> {
    fn drop(&mut self) {
        self.frozen.take();
        if !self.cleaned
            && let Some(file) = self.file.as_ref()
            && self
                .directory
                .remove_file_if_same_identity(&self.name, file)
                .is_ok()
        {
            self.file.take();
            let _ = self.directory.sync();
        }
    }
}
