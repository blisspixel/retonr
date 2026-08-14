use std::{ffi::OsStr, ffi::OsString};

use rewrite_model::{ArtifactSetRelativePath, MAX_ARTIFACT_SET_RELATIVE_PATH_BYTES};
use rewrite_types::CancellationToken;

use super::{
    ArtifactInventoryError, FileShare, ManagedFile, ManagedTreeEntry, ManagedTreeEntryKind,
    ManagedTreeLimits, MetadataFingerprint, PinnedDirectory, ensure_not_cancelled,
    map_active_error, validate_regular_file,
};
use crate::artifact_storage::RawDirectoryEntry;

pub(super) struct HeldDirectory {
    directory: PinnedDirectory,
    fingerprint: MetadataFingerprint,
    name_from_parent: Option<OsString>,
}

pub(super) struct HeldDirectoryChain {
    held: Vec<HeldDirectory>,
}

impl HeldDirectoryChain {
    pub(super) fn new(root: &PinnedDirectory) -> Result<Self, ArtifactInventoryError> {
        let directory = root.duplicate()?;
        let fingerprint = directory.fingerprint()?;
        Ok(Self {
            held: vec![HeldDirectory {
                directory,
                fingerprint,
                name_from_parent: None,
            }],
        })
    }

    pub(super) fn descend(&mut self, name: &OsStr) -> Result<(), ArtifactInventoryError> {
        let parent = &self.held.last().expect("root is always held").directory;
        let directory = parent.open_direct_child_directory(name)?;
        let fingerprint = directory.fingerprint()?;
        self.held.push(HeldDirectory {
            directory,
            fingerprint,
            name_from_parent: Some(name.to_owned()),
        });
        Ok(())
    }

    pub(super) fn open_regular_file(
        self,
        name: &OsStr,
        share: FileShare,
    ) -> Result<ManagedFile, ArtifactInventoryError> {
        let parent = &self.held.last().expect("root is always held").directory;
        let file = parent.open_file(name, share).map_err(map_active_error)?;
        validate_regular_file(&file, false)?;
        let fingerprint =
            MetadataFingerprint::from_file(&file).map_err(ArtifactInventoryError::StorageIo)?;
        if parent.child_file_fingerprint(name)? != fingerprint {
            return Err(ArtifactInventoryError::ConcurrentModification);
        }
        self.recheck()?;
        Ok(ManagedFile {
            byte_size: fingerprint.length,
            file,
            fingerprint,
        })
    }

    pub(super) fn finish_directory(self) -> Result<PinnedDirectory, ArtifactInventoryError> {
        self.recheck()?;
        Ok(self
            .held
            .into_iter()
            .last()
            .expect("root is always held")
            .directory)
    }

    fn recheck(&self) -> Result<(), ArtifactInventoryError> {
        for held in &self.held {
            if held.directory.fingerprint()? != held.fingerprint {
                return Err(ArtifactInventoryError::ConcurrentModification);
            }
        }
        for pair in self.held.windows(2) {
            let name = pair[1]
                .name_from_parent
                .as_deref()
                .expect("non-root directory has a parent name");
            if pair[0].directory.child_directory_fingerprint(name)? != pair[1].fingerprint {
                return Err(ArtifactInventoryError::ConcurrentModification);
            }
        }
        Ok(())
    }
}

pub(super) struct TreeWalker<'a> {
    limits: ManagedTreeLimits,
    cancellation: &'a CancellationToken,
    pub(super) entries: Vec<ManagedTreeEntry>,
}

impl<'a> TreeWalker<'a> {
    pub(super) fn new(limits: ManagedTreeLimits, cancellation: &'a CancellationToken) -> Self {
        Self {
            limits,
            cancellation,
            entries: Vec::new(),
        }
    }

    pub(super) fn walk(
        &mut self,
        directory: &PinnedDirectory,
        prefix: Option<&str>,
        depth: usize,
    ) -> Result<(), ArtifactInventoryError> {
        ensure_not_cancelled(self.cancellation)?;
        let initial = directory.fingerprint()?;
        let remaining = self.limits.remaining(self.entries.len())?;
        let mut raw = directory.raw_entries(remaining, self.cancellation)?;
        raw.sort_unstable_by(|left, right| left.name.cmp(&right.name));
        for entry in raw {
            self.inspect_entry(directory, prefix, depth, &entry)?;
        }
        if directory.fingerprint()? != initial {
            return Err(ArtifactInventoryError::ConcurrentModification);
        }
        Ok(())
    }

    fn inspect_entry(
        &mut self,
        directory: &PinnedDirectory,
        prefix: Option<&str>,
        depth: usize,
        raw: &RawDirectoryEntry,
    ) -> Result<(), ArtifactInventoryError> {
        ensure_not_cancelled(self.cancellation)?;
        let name = raw
            .name
            .to_str()
            .ok_or(ArtifactInventoryError::UnsafeStorageLayout)?;
        let path = match prefix {
            Some(prefix) => format!("{prefix}/{name}"),
            None => name.to_owned(),
        };
        let relative_path = ArtifactSetRelativePath::new(path.clone())
            .map_err(|_| ArtifactInventoryError::UnsafeStorageLayout)?;
        if self.entries.len() == self.limits.maximum_entries {
            return Err(ArtifactInventoryError::StorageEntryLimitExceeded);
        }
        if raw.indirect {
            return Err(ArtifactInventoryError::UnsafeStorageLayout);
        }
        if raw.direct_regular_file {
            return self.inspect_file(directory, raw, relative_path);
        }
        if depth >= maximum_tree_depth() {
            return Err(ArtifactInventoryError::StorageEntryLimitExceeded);
        }
        let child = directory
            .open_direct_child_directory(&raw.name)
            .map_err(map_non_directory)?;
        let fingerprint = child.fingerprint()?;
        let directory_index = self.entries.len();
        self.entries.push(ManagedTreeEntry {
            relative_path,
            kind: ManagedTreeEntryKind::Directory,
            byte_size: 0,
            fingerprint,
        });
        self.walk(&child, Some(&path), depth + 1)?;
        let expected = self.entries[directory_index].fingerprint();
        if directory.child_directory_fingerprint(&raw.name)? != *expected {
            return Err(ArtifactInventoryError::ConcurrentModification);
        }
        Ok(())
    }

    fn inspect_file(
        &mut self,
        directory: &PinnedDirectory,
        raw: &RawDirectoryEntry,
        relative_path: ArtifactSetRelativePath,
    ) -> Result<(), ArtifactInventoryError> {
        let file = directory
            .open_file(&raw.name, FileShare::Verification)
            .map_err(map_active_error)?;
        validate_regular_file(&file, false)?;
        let fingerprint =
            MetadataFingerprint::from_file(&file).map_err(ArtifactInventoryError::StorageIo)?;
        if fingerprint.length != raw.byte_size
            || directory.child_file_fingerprint(&raw.name)? != fingerprint
        {
            return Err(ArtifactInventoryError::ConcurrentModification);
        }
        self.entries.push(ManagedTreeEntry {
            relative_path,
            kind: ManagedTreeEntryKind::RegularFile,
            byte_size: fingerprint.length,
            fingerprint,
        });
        Ok(())
    }
}

fn maximum_tree_depth() -> usize {
    MAX_ARTIFACT_SET_RELATIVE_PATH_BYTES.div_ceil(2)
}

fn map_non_directory(error: ArtifactInventoryError) -> ArtifactInventoryError {
    if matches!(error, ArtifactInventoryError::ConcurrentModification) {
        ArtifactInventoryError::UnsafeStorageLayout
    } else {
        error
    }
}
