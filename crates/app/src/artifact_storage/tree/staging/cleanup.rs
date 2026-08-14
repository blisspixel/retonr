use std::{collections::BTreeMap, ffi::OsStr};

use rewrite_model::ArtifactSetRelativePath;
use rewrite_types::CancellationToken;

use super::{
    ArtifactInventoryError, ManagedTreeEntryKind, ManagedTreeSnapshot, MetadataFingerprint,
    OwnedStagingTree, StableMetadataFingerprint, SyncedStagingTree, directory_for_parent, platform,
    split_parent,
};

pub(super) struct PublicationLedger {
    root: StableMetadataFingerprint,
    entries: BTreeMap<ArtifactSetRelativePath, EntryProof>,
}

#[derive(Clone, Copy)]
struct EntryProof {
    kind: ManagedTreeEntryKind,
    fingerprint: StableMetadataFingerprint,
}

impl PublicationLedger {
    pub(super) fn from_snapshot(snapshot: &ManagedTreeSnapshot) -> Self {
        Self {
            root: snapshot.root_fingerprint.stable(),
            entries: snapshot
                .entries()
                .iter()
                .map(|entry| {
                    (
                        entry.relative_path().clone(),
                        EntryProof {
                            kind: entry.kind(),
                            fingerprint: entry.fingerprint().stable(),
                        },
                    )
                })
                .collect(),
        }
    }

    fn matches(&self, snapshot: &ManagedTreeSnapshot) -> bool {
        snapshot.root_fingerprint.stable() == self.root
            && snapshot.entries().len() == self.entries.len()
            && snapshot.entries().iter().all(|entry| {
                self.entries
                    .get(entry.relative_path())
                    .is_some_and(|expected| {
                        expected.kind == entry.kind()
                            && expected.fingerprint == entry.fingerprint().stable()
                    })
            })
    }
}

impl OwnedStagingTree {
    /// Deletes only exact, exclusively created ledger entries and their root.
    pub(crate) fn cleanup(self) -> Result<(), ArtifactInventoryError> {
        self.seal_files(&CancellationToken::new())?;
        let snapshot = self.enumerate(&CancellationToken::new())?;
        if snapshot.entries().iter().any(|entry| {
            entry.kind() == ManagedTreeEntryKind::RegularFile && !entry.has_single_link()
        }) {
            return Err(ArtifactInventoryError::UnsafeStorageLayout);
        }
        drop(snapshot);
        self.remove_exact_ledger()
    }

    fn remove_exact_ledger(self) -> Result<(), ArtifactInventoryError> {
        self.verify_directory_bindings()?;
        let OwnedStagingTree {
            parent,
            name,
            root,
            root_fingerprint,
            mut directories,
            files,
            limits,
            synced_snapshot,
        } = self;
        drop(synced_snapshot);
        for (path, retained) in files.into_inner() {
            let (parent_path, file_name) = split_parent(&path);
            let file_parent = directory_for_parent(&root, &directories, parent_path.as_ref())?;
            let current = file_parent.child_file_fingerprint(OsStr::new(file_name))?;
            let named_exact =
                current.same_identity(&retained.fingerprint) && current.has_single_link();
            drop(current);
            if !named_exact {
                return Err(ArtifactInventoryError::ConcurrentModification);
            }
            let removal = file_parent
                .open_managed_file_for_removal(
                    OsStr::new(file_name),
                    limits.maximum_entries,
                    &CancellationToken::new(),
                )?
                .ok_or(ArtifactInventoryError::ConcurrentModification)?;
            let exact = removal.fingerprint.same_identity(&retained.fingerprint)
                && removal.fingerprint.has_single_link();
            drop(retained);
            if !exact {
                return Err(ArtifactInventoryError::ConcurrentModification);
            }
            file_parent.remove_held_managed_file(
                OsStr::new(file_name),
                removal.file,
                removal.fingerprint,
                limits.maximum_entries,
            )?;
        }
        let paths = directories.keys().rev().cloned().collect::<Vec<_>>();
        for path in paths {
            let retained = directories
                .remove(&path)
                .expect("directory ledger key remains present");
            let (parent_path, directory_name) = split_parent(&path);
            let directory_parent = directory_for_parent(&root, &directories, parent_path.as_ref())?;
            let named = directory_parent.child_directory_fingerprint(OsStr::new(directory_name))?;
            let named_exact = named.same_identity(&retained.fingerprint);
            drop(named);
            if !named_exact || !retained.directory.is_empty()? {
                return Err(ArtifactInventoryError::ConcurrentModification);
            }
            let cleanup = platform::open_directory_for_cleanup(
                &directory_parent.handle,
                OsStr::new(directory_name),
            )
            .map_err(super::super::map_active_error)?;
            super::super::validate_directory_handle(&cleanup, false)?;
            let cleanup_fingerprint = MetadataFingerprint::from_file(&cleanup)
                .map_err(ArtifactInventoryError::StorageIo)?;
            let exact = cleanup_fingerprint.same_identity(&retained.fingerprint);
            if !exact {
                return Err(ArtifactInventoryError::ConcurrentModification);
            }
            drop(cleanup_fingerprint);
            drop(retained.fingerprint);
            drop(retained.directory);
            platform::remove_verified_directory(
                &directory_parent.handle,
                OsStr::new(directory_name),
                cleanup,
            )
            .map_err(super::super::map_active_error)?;
        }
        let named = parent.child_directory_fingerprint(&name)?;
        let named_exact = named.same_identity(&root_fingerprint);
        drop(named);
        if !named_exact || !root.is_empty()? {
            return Err(ArtifactInventoryError::ConcurrentModification);
        }
        let cleanup = platform::open_directory_for_cleanup(&parent.handle, &name)
            .map_err(super::super::map_active_error)?;
        super::super::validate_directory_handle(&cleanup, false)?;
        let cleanup_fingerprint =
            MetadataFingerprint::from_file(&cleanup).map_err(ArtifactInventoryError::StorageIo)?;
        let exact = cleanup_fingerprint.same_identity(&root_fingerprint);
        if !exact {
            return Err(ArtifactInventoryError::ConcurrentModification);
        }
        drop(cleanup_fingerprint);
        drop(root_fingerprint);
        drop(root);
        platform::remove_verified_directory(&parent.handle, &name, cleanup)
            .map_err(super::super::map_active_error)?;
        parent.sync()
    }

    pub(super) fn close_descendant_handles(&mut self) {
        self.directories.clear();
        self.files.borrow_mut().clear();
    }

    pub(super) fn cleanup_closed_ledger(
        self,
        ledger: &PublicationLedger,
    ) -> Result<(), ArtifactInventoryError> {
        let snapshot = self
            .root
            .enumerate_tree(self.limits, &CancellationToken::new())?;
        if !ledger.matches(&snapshot)
            || snapshot.entries().iter().any(|entry| {
                entry.kind() == ManagedTreeEntryKind::RegularFile && !entry.has_single_link()
            })
        {
            return Err(ArtifactInventoryError::ConcurrentModification);
        }
        drop(snapshot);
        self.remove_closed_entries(ledger)
    }

    fn remove_closed_entries(
        self,
        ledger: &PublicationLedger,
    ) -> Result<(), ArtifactInventoryError> {
        let OwnedStagingTree {
            parent,
            name,
            root,
            root_fingerprint,
            directories: _,
            files: _,
            limits,
            synced_snapshot,
        } = self;
        drop(synced_snapshot);
        for (path, proof) in ledger
            .entries
            .iter()
            .filter(|(_, proof)| proof.kind == ManagedTreeEntryKind::RegularFile)
        {
            let (parent_path, file_name) = split_parent(path);
            let file_parent = open_relative_parent(&root, parent_path.as_ref())?;
            let removal = file_parent
                .open_managed_file_for_removal(
                    OsStr::new(file_name),
                    limits.maximum_entries,
                    &CancellationToken::new(),
                )?
                .ok_or(ArtifactInventoryError::ConcurrentModification)?;
            if removal.fingerprint.stable() != proof.fingerprint
                || !removal.fingerprint.has_single_link()
            {
                return Err(ArtifactInventoryError::ConcurrentModification);
            }
            file_parent.remove_held_managed_file(
                OsStr::new(file_name),
                removal.file,
                removal.fingerprint,
                limits.maximum_entries,
            )?;
        }
        let directories = ledger
            .entries
            .iter()
            .filter(|(_, proof)| proof.kind == ManagedTreeEntryKind::Directory)
            .rev();
        for (path, proof) in directories {
            let (parent_path, directory_name) = split_parent(path);
            let directory_parent = open_relative_parent(&root, parent_path.as_ref())?;
            remove_closed_directory(&directory_parent, directory_name, proof.fingerprint)?;
        }
        remove_root(&parent, &name, root, root_fingerprint, ledger.root)
    }
}

impl SyncedStagingTree {
    /// Deletes this synchronized tree through its exact creation ledger.
    pub(crate) fn cleanup(self) -> Result<(), ArtifactInventoryError> {
        let SyncedStagingTree { tree, snapshot } = self;
        let current = tree.enumerate(&CancellationToken::new())?;
        if snapshot.as_ref() != Some(&current) {
            return Err(ArtifactInventoryError::ConcurrentModification);
        }
        if current.entries().iter().any(|entry| {
            entry.kind() == ManagedTreeEntryKind::RegularFile && !entry.has_single_link()
        }) {
            return Err(ArtifactInventoryError::UnsafeStorageLayout);
        }
        drop(current);
        drop(snapshot);
        tree.remove_exact_ledger()
    }

    #[cfg(test)]
    pub(in crate::artifact_storage::tree) fn cleanup_after_closed_publication_failure(
        mut self,
    ) -> Result<(), ArtifactInventoryError> {
        let ledger = PublicationLedger::from_snapshot(
            self.snapshot
                .as_ref()
                .expect("synchronized staging retains its snapshot"),
        );
        drop(self.snapshot.take());
        self.tree.close_descendant_handles();
        self.tree.cleanup_closed_ledger(&ledger)
    }
}

pub(super) fn cleanup_prepublication_failure(
    staging: SyncedStagingTree,
    original: ArtifactInventoryError,
) -> ArtifactInventoryError {
    match staging.cleanup() {
        Ok(()) => original,
        Err(cleanup) => cleanup,
    }
}

pub(super) fn cleanup_closed_publication_failure(
    staging: OwnedStagingTree,
    ledger: &PublicationLedger,
    original: ArtifactInventoryError,
) -> ArtifactInventoryError {
    match staging.cleanup_closed_ledger(ledger) {
        Ok(()) => original,
        Err(cleanup) => cleanup,
    }
}

fn open_relative_parent(
    root: &super::PinnedDirectory,
    parent: Option<&ArtifactSetRelativePath>,
) -> Result<super::PinnedDirectory, ArtifactInventoryError> {
    match parent {
        Some(parent) => root.open_relative_directory(parent),
        None => root.duplicate(),
    }
}

fn remove_closed_directory(
    parent: &super::PinnedDirectory,
    name: &str,
    expected: StableMetadataFingerprint,
) -> Result<(), ArtifactInventoryError> {
    let cleanup = platform::open_directory_for_cleanup(&parent.handle, OsStr::new(name))
        .map_err(super::super::map_active_error)?;
    super::super::validate_directory_handle(&cleanup, false)?;
    let fingerprint =
        MetadataFingerprint::from_file(&cleanup).map_err(ArtifactInventoryError::StorageIo)?;
    if !expected.same_identity(&fingerprint) {
        return Err(ArtifactInventoryError::ConcurrentModification);
    }
    drop(fingerprint);
    let directory = super::PinnedDirectory { handle: cleanup };
    if !directory.is_empty()? {
        return Err(ArtifactInventoryError::ConcurrentModification);
    }
    platform::remove_verified_directory(&parent.handle, OsStr::new(name), directory.handle)
        .map_err(super::super::map_active_error)
}

fn remove_root(
    parent: &super::PinnedDirectory,
    name: &std::ffi::OsStr,
    root: super::PinnedDirectory,
    root_fingerprint: MetadataFingerprint,
    expected: StableMetadataFingerprint,
) -> Result<(), ArtifactInventoryError> {
    if !expected.same_identity(&root.fingerprint()?) {
        return Err(ArtifactInventoryError::ConcurrentModification);
    }
    drop(root_fingerprint);
    drop(root);
    remove_closed_directory(
        parent,
        name.to_str()
            .ok_or(ArtifactInventoryError::UnsafeStorageLayout)?,
        expected,
    )?;
    parent.sync()
}
