use std::{ffi::OsStr, io};

use rewrite_model::ArtifactManifest;
use rewrite_types::CancellationToken;

use crate::artifact_storage::{ManagedFile, PinnedDirectory};

use super::{ArtifactImportError, copy_and_hash, map_boundary_storage_error};

pub(super) fn verify_stored_file(
    directory: &PinnedDirectory,
    name: &OsStr,
    manifest: &ArtifactManifest,
    maximum_storage_entries: usize,
    cancellation: &CancellationToken,
) -> Result<(), ArtifactImportError> {
    let ManagedFile {
        mut file,
        fingerprint,
        byte_size,
    } = directory
        .open_managed_file_for_sync(name, maximum_storage_entries, cancellation)
        .map_err(|error| match error {
            crate::artifact_inventory::ArtifactInventoryError::StorageIo(source) => {
                ArtifactImportError::StorageIo(io::Error::new(
                    source.kind(),
                    "could not open the final artifact for synchronization",
                ))
            }
            other => map_boundary_storage_error(other),
        })?
        .ok_or(ArtifactImportError::StorageChanged)?;
    if byte_size != manifest.byte_size {
        return Err(ArtifactImportError::StorageConflict);
    }
    if !fingerprint.has_single_link() {
        return Err(ArtifactImportError::StorageChanged);
    }
    let digest = copy_and_hash(
        &mut file,
        &mut io::sink(),
        manifest.byte_size,
        cancellation,
        &mut |_| {},
    )
    .map_err(|error| match error {
        ArtifactImportError::SizeMismatch | ArtifactImportError::DigestMismatch => {
            ArtifactImportError::StorageConflict
        }
        ArtifactImportError::SourceIo(source) => ArtifactImportError::StorageIo(source),
        other => other,
    })?;
    if digest != manifest.artifact_digest {
        return Err(ArtifactImportError::StorageConflict);
    }
    file.sync_all().map_err(|error| {
        ArtifactImportError::StorageIo(io::Error::new(
            error.kind(),
            format!("could not synchronize the final artifact: {error}"),
        ))
    })?;
    drop(file);
    directory
        .recheck_managed_file_for_lifecycle(
            name,
            &fingerprint,
            maximum_storage_entries,
            cancellation,
        )
        .map_err(|error| match error {
            crate::artifact_inventory::ArtifactInventoryError::StorageIo(source) => {
                ArtifactImportError::StorageIo(io::Error::new(
                    source.kind(),
                    format!("could not recheck the final artifact: {source}"),
                ))
            }
            other => map_boundary_storage_error(other),
        })?;
    directory.sync().map_err(|error| match error {
        crate::artifact_inventory::ArtifactInventoryError::StorageIo(source) => {
            ArtifactImportError::StorageIo(io::Error::new(
                source.kind(),
                format!("could not synchronize the artifact directory: {source}"),
            ))
        }
        other => map_boundary_storage_error(other),
    })
}
