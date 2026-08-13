use std::ffi::OsStr;

use rewrite_model::ArtifactManifest;
use rewrite_types::CancellationToken;

use crate::artifact_storage::{
    ExactArtifactExpectation, ExactArtifactSync, ExactArtifactVerificationError, PinnedDirectory,
    VerifiedManagedArtifact, verify_exact_artifact,
};

use super::{ArtifactImportError, map_boundary_storage_error};

pub(super) fn verify_stored_file(
    directory: &PinnedDirectory,
    name: &OsStr,
    manifest: &ArtifactManifest,
    maximum_storage_entries: usize,
    cancellation: &CancellationToken,
) -> Result<VerifiedManagedArtifact, ArtifactImportError> {
    verify_exact_artifact(
        directory,
        name,
        ExactArtifactExpectation {
            byte_size: manifest.byte_size,
            digest: &manifest.artifact_digest,
            maximum_entries: maximum_storage_entries,
            sync: ExactArtifactSync::Normal,
        },
        cancellation,
        |_| {},
    )
    .map_err(map_verification_error)
}

fn map_verification_error(error: ExactArtifactVerificationError) -> ArtifactImportError {
    match error {
        ExactArtifactVerificationError::Boundary(error) => map_boundary_storage_error(error),
        ExactArtifactVerificationError::Missing | ExactArtifactVerificationError::Aliased => {
            ArtifactImportError::StorageChanged
        }
        ExactArtifactVerificationError::SizeMismatch
        | ExactArtifactVerificationError::DigestMismatch => ArtifactImportError::StorageConflict,
    }
}
