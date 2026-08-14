use std::{
    ffi::{OsStr, OsString},
    io,
};

use rewrite_model::ArtifactSetRelativePath;
use rewrite_types::CancellationToken;

use super::mutation::ManagedFile;
use super::{
    ArtifactInventoryError, FileShare, MetadataFingerprint, PinnedDirectory, ensure_not_cancelled,
    map_active_error, map_initial_error, validate_directory_handle, validate_regular_file,
};

mod platform;
mod staging;
mod traversal;

pub(crate) use staging::OwnedStagingTree;

#[cfg(test)]
mod tests;

/// Caller-owned tree entry ceiling combined with fixed portable path ceilings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ManagedTreeLimits {
    maximum_entries: usize,
}

impl ManagedTreeLimits {
    /// Creates a nonzero tree ceiling.
    pub(crate) fn new(maximum_entries: usize) -> Result<Self, ArtifactInventoryError> {
        if maximum_entries == 0 {
            Err(ArtifactInventoryError::InvalidLimits)
        } else {
            Ok(Self { maximum_entries })
        }
    }

    fn remaining(self, observed: usize) -> Result<usize, ArtifactInventoryError> {
        self.maximum_entries
            .checked_sub(observed)
            .ok_or(ArtifactInventoryError::StorageEntryLimitExceeded)
    }
}

/// Validated type of one direct managed-tree entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ManagedTreeEntryKind {
    /// Direct directory.
    Directory,
    /// Direct regular file.
    RegularFile,
}

/// Content-free metadata for one validated managed-tree entry.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ManagedTreeEntry {
    relative_path: ArtifactSetRelativePath,
    kind: ManagedTreeEntryKind,
    byte_size: u64,
    fingerprint: MetadataFingerprint,
}

impl ManagedTreeEntry {
    /// Returns the canonical portable relative path.
    pub(crate) const fn relative_path(&self) -> &ArtifactSetRelativePath {
        &self.relative_path
    }

    /// Returns the validated direct entry type.
    pub(crate) const fn kind(&self) -> ManagedTreeEntryKind {
        self.kind
    }

    /// Returns the exact size observed for a regular file, or zero for a directory.
    pub(crate) const fn byte_size(&self) -> u64 {
        self.byte_size
    }

    /// Reports whether this entry has exactly one filesystem link.
    pub(crate) fn has_single_link(&self) -> bool {
        self.fingerprint.has_single_link()
    }

    fn fingerprint(&self) -> &MetadataFingerprint {
        &self.fingerprint
    }
}

/// Stable, sorted, content-free snapshot of one exact tree.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ManagedTreeSnapshot {
    root_fingerprint: MetadataFingerprint,
    entries: Vec<ManagedTreeEntry>,
}

impl ManagedTreeSnapshot {
    /// Returns entries in canonical relative-path order.
    pub(crate) fn entries(&self) -> &[ManagedTreeEntry] {
        &self.entries
    }
}

impl PinnedDirectory {
    /// Enumerates and rechecks a direct regular-file and directory tree.
    pub(crate) fn enumerate_tree(
        &self,
        limits: ManagedTreeLimits,
        cancellation: &CancellationToken,
    ) -> Result<ManagedTreeSnapshot, ArtifactInventoryError> {
        ensure_not_cancelled(cancellation)?;
        let root_fingerprint = self.fingerprint()?;
        let mut walker = traversal::TreeWalker::new(limits, cancellation);
        walker.walk(self, None, 0)?;
        if self.fingerprint()? != root_fingerprint {
            return Err(ArtifactInventoryError::ConcurrentModification);
        }
        walker
            .entries
            .sort_unstable_by(|left, right| left.relative_path.cmp(&right.relative_path));
        let entries = std::mem::take(&mut walker.entries);
        drop(walker);
        Ok(ManagedTreeSnapshot {
            root_fingerprint,
            entries,
        })
    }

    /// Opens one validated relative regular file without following any path link.
    pub(crate) fn open_relative_regular_file(
        &self,
        relative_path: &ArtifactSetRelativePath,
    ) -> Result<ManagedFile, ArtifactInventoryError> {
        self.open_relative_regular_file_with_share(relative_path, FileShare::Verification)
    }

    /// Reopens one relative file and requires the exact prior metadata fingerprint.
    pub(crate) fn recheck_relative_regular_file(
        &self,
        relative_path: &ArtifactSetRelativePath,
        expected: &MetadataFingerprint,
    ) -> Result<(), ArtifactInventoryError> {
        let current = self.open_relative_regular_file(relative_path)?;
        if &current.fingerprint == expected {
            Ok(())
        } else {
            Err(ArtifactInventoryError::ConcurrentModification)
        }
    }

    pub(crate) fn duplicate(&self) -> Result<Self, ArtifactInventoryError> {
        let handle = self
            .handle
            .try_clone()
            .map_err(ArtifactInventoryError::StorageIo)?;
        validate_directory_handle(&handle, false)?;
        Ok(Self { handle })
    }

    fn open_direct_child_directory(&self, name: &OsStr) -> Result<Self, ArtifactInventoryError> {
        let handle = self.open_directory(name).map_err(map_active_error)?;
        validate_directory_handle(&handle, false)?;
        Ok(Self { handle })
    }

    fn open_relative_directory(
        &self,
        relative_path: &ArtifactSetRelativePath,
    ) -> Result<Self, ArtifactInventoryError> {
        let mut chain = traversal::HeldDirectoryChain::new(self)?;
        for component in relative_path.as_str().split('/') {
            chain.descend(OsStr::new(component))?;
        }
        chain.finish_directory()
    }

    fn open_relative_regular_file_with_share(
        &self,
        relative_path: &ArtifactSetRelativePath,
        share: FileShare,
    ) -> Result<ManagedFile, ArtifactInventoryError> {
        let mut components = relative_path.as_str().split('/').peekable();
        let mut chain = traversal::HeldDirectoryChain::new(self)?;
        let mut file_name = None;
        while let Some(component) = components.next() {
            if components.peek().is_some() {
                chain.descend(OsStr::new(component))?;
            } else {
                file_name = Some(OsString::from(component));
            }
        }
        let file_name = file_name.ok_or(ArtifactInventoryError::UnsafeStorageLayout)?;
        chain.open_regular_file(&file_name, share)
    }
}

fn validate_single_component(name: &OsStr) -> Result<(), ArtifactInventoryError> {
    let name = name
        .to_str()
        .ok_or(ArtifactInventoryError::UnsafeStorageLayout)?;
    let path = ArtifactSetRelativePath::new(name.to_owned())
        .map_err(|_| ArtifactInventoryError::UnsafeStorageLayout)?;
    if path.as_str().contains('/') {
        Err(ArtifactInventoryError::UnsafeStorageLayout)
    } else {
        Ok(())
    }
}

fn map_publish_error(error: io::Error) -> ArtifactInventoryError {
    if error.kind() == io::ErrorKind::AlreadyExists {
        ArtifactInventoryError::ConcurrentModification
    } else {
        map_active_error(error)
    }
}
