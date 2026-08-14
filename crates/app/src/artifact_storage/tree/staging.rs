use std::{
    cell::RefCell,
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    fs::File,
    io,
};

use rewrite_model::ArtifactSetRelativePath;
use rewrite_types::CancellationToken;

use super::{
    ArtifactInventoryError, FileShare, ManagedFile, ManagedTreeEntryKind, ManagedTreeLimits,
    ManagedTreeSnapshot, MetadataFingerprint, PinnedDirectory, ensure_not_cancelled,
    map_publish_error, platform, validate_single_component,
};
use crate::artifact_storage::{StableMetadataFingerprint, mutation::random_staging_name};

mod cleanup;
mod creation;
use cleanup::{
    PublicationLedger, cleanup_closed_publication_failure, cleanup_prepublication_failure,
};
use creation::{create_retained_directory, create_retained_file};
#[cfg(test)]
pub(super) use creation::{
    create_retained_directory_with_failure, create_retained_file_with_failure,
};

const STAGING_NAME_ATTEMPTS: usize = 1_024;
const STAGING_PREFIX: &str = ".set-import-";

struct RetainedDirectory {
    directory: PinnedDirectory,
    fingerprint: MetadataFingerprint,
}

struct RetainedFile {
    file: File,
    fingerprint: MetadataFingerprint,
    sealed: bool,
}

/// Exclusive capability for a freshly created application-owned staging tree.
pub(crate) struct OwnedStagingTree {
    parent: PinnedDirectory,
    name: OsString,
    root: PinnedDirectory,
    root_fingerprint: MetadataFingerprint,
    directories: BTreeMap<ArtifactSetRelativePath, RetainedDirectory>,
    files: RefCell<BTreeMap<ArtifactSetRelativePath, RetainedFile>>,
    limits: ManagedTreeLimits,
    synced_snapshot: Option<ManagedTreeSnapshot>,
}

/// Proof that a staging tree completed whole-tree synchronization and recheck.
pub(crate) struct SyncedStagingTree {
    tree: OwnedStagingTree,
    snapshot: Option<ManagedTreeSnapshot>,
}

impl OwnedStagingTree {
    /// Creates and pins a fresh random staging root beneath the supplied parent.
    pub(crate) fn create(
        parent: &PinnedDirectory,
        limits: ManagedTreeLimits,
        maximum_staging_roots: usize,
        cancellation: &CancellationToken,
    ) -> Result<Self, ArtifactInventoryError> {
        if maximum_staging_roots == 0 {
            return Err(ArtifactInventoryError::InvalidLimits);
        }
        let entries = parent.raw_entries(maximum_staging_roots, cancellation)?;
        if entries.len() == maximum_staging_roots {
            return Err(ArtifactInventoryError::StorageEntryLimitExceeded);
        }
        Self::create_with(parent, limits, || random_staging_name(STAGING_PREFIX))
    }

    pub(super) fn create_with(
        parent: &PinnedDirectory,
        limits: ManagedTreeLimits,
        mut next_name: impl FnMut() -> Result<OsString, ArtifactInventoryError>,
    ) -> Result<Self, ArtifactInventoryError> {
        let retained_parent = parent.duplicate()?;
        for _ in 0..STAGING_NAME_ATTEMPTS {
            let name = next_name()?;
            validate_single_component(&name)?;
            match create_retained_directory(parent, &name) {
                Ok(retained) => {
                    let RetainedDirectory {
                        directory: root,
                        fingerprint: root_fingerprint,
                    } = retained;
                    return Ok(Self {
                        parent: retained_parent,
                        name,
                        root,
                        root_fingerprint,
                        directories: BTreeMap::new(),
                        files: RefCell::new(BTreeMap::new()),
                        limits,
                        synced_snapshot: None,
                    });
                }
                Err(ArtifactInventoryError::StorageIo(ref error))
                    if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(ArtifactInventoryError::StorageIo(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not reserve a unique staging directory",
        )))
    }

    /// Exclusively creates and retains every missing component of one directory.
    pub(crate) fn ensure_directory(
        &mut self,
        relative_path: &ArtifactSetRelativePath,
    ) -> Result<(), ArtifactInventoryError> {
        self.require_mutable()?;
        self.verify_directory_bindings()?;
        let mut current_path = String::new();
        for component in relative_path.as_str().split('/') {
            if !current_path.is_empty() {
                current_path.push('/');
            }
            current_path.push_str(component);
            let path = ArtifactSetRelativePath::new(current_path.clone())
                .map_err(|_| ArtifactInventoryError::UnsafeStorageLayout)?;
            if self.directories.contains_key(&path) {
                continue;
            }
            let parent = parent_for_path(&self.root, &self.directories, &path)?;
            let retained = create_retained_directory(parent, OsStr::new(component))?;
            self.directories.insert(path, retained);
        }
        self.verify_directory_bindings()
    }

    /// Creates and retains one new regular file without replacing an entry.
    pub(crate) fn create_file(
        &self,
        relative_path: &ArtifactSetRelativePath,
    ) -> Result<ManagedFile, ArtifactInventoryError> {
        self.require_mutable()?;
        self.verify_directory_bindings()?;
        if self.files.borrow().contains_key(relative_path) {
            return Err(ArtifactInventoryError::ConcurrentModification);
        }
        let (parent_path, file_name) = split_parent(relative_path);
        let parent = match parent_path {
            Some(path) => {
                &self
                    .directories
                    .get(&path)
                    .ok_or(ArtifactInventoryError::ConcurrentModification)?
                    .directory
            }
            None => &self.root,
        };
        let (created, retained) =
            create_retained_file(parent, OsStr::new(file_name), self.limits.maximum_entries)?;
        let previous = self
            .files
            .borrow_mut()
            .insert(relative_path.clone(), retained);
        debug_assert!(
            previous.is_none(),
            "file ledger was checked before creation"
        );
        Ok(created)
    }

    /// Returns one coherent snapshot matching the exact creation ledger.
    pub(crate) fn enumerate(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<ManagedTreeSnapshot, ArtifactInventoryError> {
        self.verify_directory_bindings()?;
        let snapshot = self.root.enumerate_tree(self.limits, cancellation)?;
        self.verify_snapshot_entries(&snapshot)?;
        Ok(snapshot)
    }

    /// Syncs all tree bytes and freezes later staging mutation.
    pub(crate) fn sync_bottom_up(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<(), ArtifactInventoryError> {
        self.require_mutable()?;
        self.seal_files(cancellation)?;
        let sealed = self.enumerate(cancellation)?;
        if sealed.entries().iter().any(|entry| {
            entry.kind() == ManagedTreeEntryKind::RegularFile && !entry.has_single_link()
        }) {
            return Err(ArtifactInventoryError::UnsafeStorageLayout);
        }
        drop(sealed);
        for retained in self.directories.values().rev() {
            ensure_not_cancelled(cancellation)?;
            if !retained
                .directory
                .fingerprint()?
                .same_identity(&retained.fingerprint)
            {
                return Err(ArtifactInventoryError::ConcurrentModification);
            }
            retained.directory.sync()?;
        }
        self.root.sync()?;
        let after = self.enumerate(cancellation)?;
        self.synced_snapshot = Some(after);
        Ok(())
    }

    /// Consumes a successfully synchronized tree into a publication proof.
    pub(crate) fn into_synced(mut self) -> Result<SyncedStagingTree, ArtifactInventoryError> {
        let Some(snapshot) = self.synced_snapshot.take() else {
            let original = ArtifactInventoryError::ConcurrentModification;
            return Err(match self.cleanup() {
                Ok(()) => original,
                Err(cleanup) => cleanup,
            });
        };
        Ok(SyncedStagingTree {
            tree: self,
            snapshot: Some(snapshot),
        })
    }

    fn require_mutable(&self) -> Result<(), ArtifactInventoryError> {
        if self.synced_snapshot.is_none() {
            Ok(())
        } else {
            Err(ArtifactInventoryError::ConcurrentModification)
        }
    }

    fn seal_files(&self, cancellation: &CancellationToken) -> Result<(), ArtifactInventoryError> {
        let mut files = self.files.borrow_mut();
        for (path, retained) in files.iter_mut() {
            if retained.sealed {
                continue;
            }
            ensure_not_cancelled(cancellation)?;
            retained
                .file
                .sync_all()
                .map_err(ArtifactInventoryError::StorageIo)?;
            let sealed = self
                .root
                .open_relative_regular_file_with_share(path, FileShare::Sealed)?;
            let current = MetadataFingerprint::from_file(&retained.file)
                .map_err(ArtifactInventoryError::StorageIo)?;
            if !current.same_identity(&sealed.fingerprint) || !sealed.fingerprint.has_single_link()
            {
                return Err(ArtifactInventoryError::ConcurrentModification);
            }
            retained.file = sealed.file;
            retained.fingerprint = sealed.fingerprint;
            retained.sealed = true;
        }
        Ok(())
    }

    fn verify_root_binding(&self) -> Result<(), ArtifactInventoryError> {
        let held = self.root.fingerprint()?;
        let named = self.parent.child_directory_fingerprint(&self.name)?;
        if held.same_identity(&self.root_fingerprint) && named.same_identity(&self.root_fingerprint)
        {
            Ok(())
        } else {
            Err(ArtifactInventoryError::ConcurrentModification)
        }
    }

    fn verify_directory_bindings(&self) -> Result<(), ArtifactInventoryError> {
        self.verify_root_binding()?;
        for (path, retained) in &self.directories {
            if !retained
                .directory
                .fingerprint()?
                .same_identity(&retained.fingerprint)
            {
                return Err(ArtifactInventoryError::ConcurrentModification);
            }
            let (parent_path, name) = split_parent(path);
            let parent = match parent_path {
                Some(parent_path) => {
                    &self
                        .directories
                        .get(&parent_path)
                        .ok_or(ArtifactInventoryError::ConcurrentModification)?
                        .directory
                }
                None => &self.root,
            };
            if !parent
                .child_directory_fingerprint(OsStr::new(name))?
                .same_identity(&retained.fingerprint)
            {
                return Err(ArtifactInventoryError::ConcurrentModification);
            }
        }
        Ok(())
    }

    fn verify_snapshot_entries(
        &self,
        snapshot: &ManagedTreeSnapshot,
    ) -> Result<(), ArtifactInventoryError> {
        let files = self.files.borrow();
        if snapshot.entries().len() != self.directories.len() + files.len() {
            return Err(ArtifactInventoryError::ConcurrentModification);
        }
        for entry in snapshot.entries() {
            let matches = match entry.kind() {
                ManagedTreeEntryKind::Directory => self
                    .directories
                    .get(entry.relative_path())
                    .is_some_and(|value| entry.fingerprint().same_identity(&value.fingerprint)),
                ManagedTreeEntryKind::RegularFile => files
                    .get(entry.relative_path())
                    .is_some_and(|value| entry.fingerprint().same_identity(&value.fingerprint)),
            };
            if !matches {
                return Err(ArtifactInventoryError::ConcurrentModification);
            }
        }
        Ok(())
    }
}

impl SyncedStagingTree {
    /// Returns the pinned synchronized root for exact domain verification.
    pub(crate) const fn root(&self) -> &PinnedDirectory {
        &self.tree.root
    }

    /// Publishes after one final snapshot and cancellation check, without replacement.
    pub(crate) fn publish_no_replace(
        mut self,
        destination_parent: &PinnedDirectory,
        destination_name: &OsStr,
        maximum_destination_entries: usize,
        cancellation: &CancellationToken,
    ) -> Result<PinnedDirectory, ArtifactInventoryError> {
        let preflight = self.preflight_publish(
            destination_parent,
            destination_name,
            maximum_destination_entries,
        );
        if let Err(error) = preflight {
            return Err(cleanup_prepublication_failure(self, error));
        }
        if let Err(error) = ensure_not_cancelled(cancellation) {
            return Err(cleanup_prepublication_failure(self, error));
        }
        let ledger = PublicationLedger::from_snapshot(
            self.snapshot
                .as_ref()
                .expect("synchronized staging retains its snapshot"),
        );
        drop(self.snapshot.take());
        self.tree.close_descendant_handles();
        if let Err(error) = platform::rename_directory_no_replace(
            &self.tree.parent.handle,
            &self.tree.name,
            &destination_parent.handle,
            destination_name,
        ) {
            return Err(cleanup_closed_publication_failure(
                self.tree,
                &ledger,
                map_publish_error(error),
            ));
        }
        let SyncedStagingTree { tree, snapshot: _ } = self;
        let OwnedStagingTree {
            parent,
            name: _,
            root,
            root_fingerprint,
            directories,
            files,
            limits: _,
            synced_snapshot: _,
        } = tree;
        drop(directories);
        drop(files);
        let published = destination_parent.open_direct_child_directory(destination_name)?;
        if !published.fingerprint()?.same_identity(&root_fingerprint) {
            return Err(ArtifactInventoryError::ConcurrentModification);
        }
        drop(root);
        parent.sync()?;
        destination_parent.sync()?;
        Ok(published)
    }

    fn preflight_publish(
        &self,
        destination_parent: &PinnedDirectory,
        destination_name: &OsStr,
        maximum_destination_entries: usize,
    ) -> Result<(), ArtifactInventoryError> {
        validate_single_component(destination_name)?;
        if maximum_destination_entries == 0 {
            return Err(ArtifactInventoryError::InvalidLimits);
        }
        self.tree.verify_directory_bindings()?;
        let current = self.tree.enumerate(&CancellationToken::new())?;
        if self.snapshot.as_ref() != Some(&current) {
            return Err(ArtifactInventoryError::ConcurrentModification);
        }
        match destination_parent.exact_entry_capacity(
            destination_name,
            maximum_destination_entries,
            &CancellationToken::new(),
        )? {
            crate::artifact_storage::ExactEntryCapacity::Available => Ok(()),
            crate::artifact_storage::ExactEntryCapacity::Present => {
                Err(ArtifactInventoryError::ConcurrentModification)
            }
            crate::artifact_storage::ExactEntryCapacity::Full => {
                Err(ArtifactInventoryError::StorageEntryLimitExceeded)
            }
        }
    }
}

fn parent_for_path<'a>(
    root: &'a PinnedDirectory,
    directories: &'a BTreeMap<ArtifactSetRelativePath, RetainedDirectory>,
    path: &ArtifactSetRelativePath,
) -> Result<&'a PinnedDirectory, ArtifactInventoryError> {
    let (parent, _) = split_parent(path);
    directory_for_parent(root, directories, parent.as_ref())
}

fn directory_for_parent<'a>(
    root: &'a PinnedDirectory,
    directories: &'a BTreeMap<ArtifactSetRelativePath, RetainedDirectory>,
    parent: Option<&ArtifactSetRelativePath>,
) -> Result<&'a PinnedDirectory, ArtifactInventoryError> {
    match parent {
        Some(parent) => directories
            .get(parent)
            .map(|retained| &retained.directory)
            .ok_or(ArtifactInventoryError::ConcurrentModification),
        None => Ok(root),
    }
}

fn split_parent(path: &ArtifactSetRelativePath) -> (Option<ArtifactSetRelativePath>, &str) {
    match path.as_str().rsplit_once('/') {
        Some((parent, name)) => (
            Some(ArtifactSetRelativePath::new(parent.to_owned()).expect("validated path prefix")),
            name,
        ),
        None => (None, path.as_str()),
    }
}
