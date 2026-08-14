use std::{ffi::OsStr, fs::File};

use super::{
    ArtifactInventoryError, ManagedFile, MetadataFingerprint, PinnedDirectory, RetainedDirectory,
    RetainedFile, platform,
};
use crate::artifact_storage::{
    map_active_error,
    mutation::{create_directory_exclusive, create_new_file},
    validate_directory_handle, validate_regular_file,
};

pub(super) fn create_retained_directory(
    parent: &PinnedDirectory,
    name: &OsStr,
) -> Result<RetainedDirectory, ArtifactInventoryError> {
    create_retained_directory_with(parent, name, |_| Ok(()))
}

fn create_retained_directory_with(
    parent: &PinnedDirectory,
    name: &OsStr,
    finish: impl FnOnce(&RetainedDirectory) -> Result<(), ArtifactInventoryError>,
) -> Result<RetainedDirectory, ArtifactInventoryError> {
    let created = create_directory_exclusive(&parent.handle, name)?;
    let retained = establish_created_directory(parent, name, created)?;
    let completed = finish(&retained);
    match completed {
        Ok(()) => Ok(retained),
        Err(original) => Err(cleanup_created_directory(parent, name, retained)
            .err()
            .unwrap_or(original)),
    }
}

#[cfg(test)]
pub(in crate::artifact_storage::tree) fn create_retained_directory_with_failure(
    parent: &PinnedDirectory,
    name: &OsStr,
) -> Result<(), ArtifactInventoryError> {
    create_retained_directory_with(parent, name, |_| {
        Err(ArtifactInventoryError::StorageIo(std::io::Error::other(
            "injected post-create directory failure",
        )))
    })
    .map(drop)
}

fn establish_created_directory(
    parent: &PinnedDirectory,
    name: &OsStr,
    handle: File,
) -> Result<RetainedDirectory, ArtifactInventoryError> {
    validate_directory_handle(&handle, false)?;
    let created = PinnedDirectory { handle };
    let created_fingerprint = created.fingerprint()?;
    let shared_handle =
        platform::open_directory_for_publish(&parent.handle, name).map_err(map_active_error)?;
    validate_directory_handle(&shared_handle, false)?;
    let shared = PinnedDirectory {
        handle: shared_handle,
    };
    let shared_fingerprint = shared.fingerprint()?;
    if !shared_fingerprint.same_identity(&created_fingerprint) {
        return Err(ArtifactInventoryError::ConcurrentModification);
    }
    drop(created_fingerprint);
    drop(created);
    Ok(RetainedDirectory {
        directory: shared,
        fingerprint: shared_fingerprint,
    })
}

fn cleanup_created_directory(
    parent: &PinnedDirectory,
    name: &OsStr,
    retained: RetainedDirectory,
) -> Result<(), ArtifactInventoryError> {
    let named = parent.child_directory_fingerprint(name)?;
    if !named.same_identity(&retained.fingerprint) || !retained.directory.is_empty()? {
        return Err(ArtifactInventoryError::ConcurrentModification);
    }
    drop(named);
    let handle =
        platform::open_directory_for_cleanup(&parent.handle, name).map_err(map_active_error)?;
    validate_directory_handle(&handle, false)?;
    let cleanup_fingerprint =
        MetadataFingerprint::from_file(&handle).map_err(ArtifactInventoryError::StorageIo)?;
    if !cleanup_fingerprint.same_identity(&retained.fingerprint) {
        return Err(ArtifactInventoryError::ConcurrentModification);
    }
    drop(cleanup_fingerprint);
    drop(retained.fingerprint);
    drop(retained.directory);
    platform::remove_verified_directory(&parent.handle, name, handle).map_err(map_active_error)?;
    parent.sync()
}

pub(super) fn create_retained_file(
    parent: &PinnedDirectory,
    name: &OsStr,
    maximum_entries: usize,
) -> Result<(ManagedFile, RetainedFile), ArtifactInventoryError> {
    create_retained_file_with(parent, name, maximum_entries, inspect_created_file)
}

fn create_retained_file_with(
    parent: &PinnedDirectory,
    name: &OsStr,
    maximum_entries: usize,
    inspect: impl FnOnce(
        &PinnedDirectory,
        &OsStr,
        &File,
    ) -> Result<
        (MetadataFingerprint, File, MetadataFingerprint),
        ArtifactInventoryError,
    >,
) -> Result<(ManagedFile, RetainedFile), ArtifactInventoryError> {
    let file = create_new_file(&parent.handle, name).map_err(super::super::map_initial_error)?;
    let completed = inspect(parent, name, &file);
    let (fingerprint, retained_file, retained_fingerprint) = match completed {
        Ok(prepared) => prepared,
        Err(original) => {
            return Err(cleanup_created_file(parent, name, file, maximum_entries)
                .err()
                .unwrap_or(original));
        }
    };
    let byte_size = fingerprint.length;
    Ok((
        ManagedFile {
            file,
            fingerprint,
            byte_size,
        },
        RetainedFile {
            file: retained_file,
            fingerprint: retained_fingerprint,
            sealed: false,
        },
    ))
}

#[cfg(test)]
pub(in crate::artifact_storage::tree) fn create_retained_file_with_failure(
    parent: &PinnedDirectory,
    name: &OsStr,
    maximum_entries: usize,
) -> Result<(), ArtifactInventoryError> {
    create_retained_file_with(parent, name, maximum_entries, |_, _, _| {
        Err(ArtifactInventoryError::StorageIo(std::io::Error::other(
            "injected post-create file failure",
        )))
    })
    .map(drop)
}

fn inspect_created_file(
    parent: &PinnedDirectory,
    name: &OsStr,
    file: &File,
) -> Result<(MetadataFingerprint, File, MetadataFingerprint), ArtifactInventoryError> {
    validate_regular_file(file, true)?;
    let fingerprint =
        MetadataFingerprint::from_file(file).map_err(ArtifactInventoryError::StorageIo)?;
    if parent.child_file_fingerprint(name)? != fingerprint || !fingerprint.has_single_link() {
        return Err(ArtifactInventoryError::ConcurrentModification);
    }
    let retained = file
        .try_clone()
        .map_err(ArtifactInventoryError::StorageIo)?;
    let retained_fingerprint =
        MetadataFingerprint::from_file(&retained).map_err(ArtifactInventoryError::StorageIo)?;
    if retained_fingerprint != fingerprint {
        return Err(ArtifactInventoryError::ConcurrentModification);
    }
    Ok((fingerprint, retained, retained_fingerprint))
}

fn cleanup_created_file(
    parent: &PinnedDirectory,
    name: &OsStr,
    file: File,
    maximum_entries: usize,
) -> Result<(), ArtifactInventoryError> {
    let fingerprint =
        MetadataFingerprint::from_file(&file).map_err(ArtifactInventoryError::StorageIo)?;
    let removal = parent
        .open_managed_file_for_removal(
            name,
            maximum_entries,
            &rewrite_types::CancellationToken::new(),
        )?
        .ok_or(ArtifactInventoryError::ConcurrentModification)?;
    if !removal.fingerprint.same_identity(&fingerprint) {
        return Err(ArtifactInventoryError::ConcurrentModification);
    }
    drop(file);
    parent.remove_held_managed_file(name, removal.file, removal.fingerprint, maximum_entries)?;
    parent.sync()
}
