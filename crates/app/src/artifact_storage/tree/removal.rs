use std::ffi::OsStr;

use rewrite_model::ArtifactSetRelativePath;
use rewrite_types::CancellationToken;

use super::{
    ArtifactInventoryError, ManagedTreeEntryKind, ManagedTreeLimits, MetadataFingerprint,
    PinnedDirectory, map_active_error, platform, validate_directory_handle,
};

/// Deletes one previously verified published set tree and its empty root.
///
/// Files are removed first, then empty directories, then the held root. The
/// caller must have already verified the tree and journaled durable preparation.
pub(crate) fn remove_verified_managed_tree(
    parent: &PinnedDirectory,
    name: &OsStr,
    root: PinnedDirectory,
    limits: ManagedTreeLimits,
    maximum_parent_entries: usize,
) -> Result<(), ArtifactInventoryError> {
    let snapshot = root.enumerate_tree(limits, &CancellationToken::new())?;
    let entries = snapshot
        .entries()
        .iter()
        .rev()
        .map(|entry| (entry.kind(), entry.relative_path().clone()))
        .collect::<Vec<_>>();
    // Windows identity fingerprints retain open handles. Release them before
    // unlink so the snapshot cannot block deletion of its own entries.
    drop(snapshot);
    for (kind, path) in entries {
        match kind {
            ManagedTreeEntryKind::RegularFile => {
                remove_tree_file(&root, &path, limits.maximum_entries())?;
            }
            ManagedTreeEntryKind::Directory => {
                remove_tree_directory(&root, &path)?;
            }
        }
    }
    if !root.is_empty()? {
        return Err(ArtifactInventoryError::ConcurrentModification);
    }
    remove_empty_child_directory(parent, name, root)?;
    parent.confirm_managed_file_absent(name, maximum_parent_entries)
}

fn remove_tree_file(
    root: &PinnedDirectory,
    path: &ArtifactSetRelativePath,
    maximum_entries: usize,
) -> Result<(), ArtifactInventoryError> {
    let (parent, name) = split_parent(path);
    let parent = open_parent(root, parent.as_ref())?;
    let removal = parent
        .open_managed_file_for_removal(
            OsStr::new(name),
            maximum_entries,
            &CancellationToken::new(),
        )?
        .ok_or(ArtifactInventoryError::ConcurrentModification)?;
    if !removal.fingerprint.has_single_link() {
        return Err(ArtifactInventoryError::UnsafeStorageLayout);
    }
    parent.remove_held_managed_file(
        OsStr::new(name),
        removal.file,
        removal.fingerprint,
        maximum_entries,
    )
}

fn remove_tree_directory(
    root: &PinnedDirectory,
    path: &ArtifactSetRelativePath,
) -> Result<(), ArtifactInventoryError> {
    let (parent, name) = split_parent(path);
    let parent = open_parent(root, parent.as_ref())?;
    let directory = parent.open_direct_child_directory(OsStr::new(name))?;
    if !directory.is_empty()? {
        return Err(ArtifactInventoryError::ConcurrentModification);
    }
    remove_empty_child_directory(&parent, OsStr::new(name), directory)
}

fn remove_empty_child_directory(
    parent: &PinnedDirectory,
    name: &OsStr,
    held: PinnedDirectory,
) -> Result<(), ArtifactInventoryError> {
    if !held.is_empty()? {
        return Err(ArtifactInventoryError::ConcurrentModification);
    }
    let named = parent.child_directory_fingerprint(name)?;
    let held_fingerprint = held.fingerprint()?;
    let named_exact = named.same_identity(&held_fingerprint);
    named.release();
    if !named_exact {
        return Err(ArtifactInventoryError::ConcurrentModification);
    }
    let cleanup =
        platform::open_directory_for_cleanup(&parent.handle, name).map_err(map_active_error)?;
    validate_directory_handle(&cleanup, false)?;
    let cleanup_fingerprint =
        MetadataFingerprint::from_file(&cleanup).map_err(ArtifactInventoryError::StorageIo)?;
    let exact = cleanup_fingerprint.same_identity(&held_fingerprint);
    cleanup_fingerprint.release();
    held_fingerprint.release();
    drop(held);
    if !exact {
        return Err(ArtifactInventoryError::ConcurrentModification);
    }
    platform::remove_verified_directory(&parent.handle, name, cleanup).map_err(map_active_error)
}

fn open_parent(
    root: &PinnedDirectory,
    parent: Option<&ArtifactSetRelativePath>,
) -> Result<PinnedDirectory, ArtifactInventoryError> {
    match parent {
        Some(parent) => root.open_relative_directory(parent),
        None => root.duplicate(),
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
