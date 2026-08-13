use std::{ffi::OsStr, fs::File, io::Read as _};

use rewrite_types::{CancellationToken, Digest};
use sha2::{Digest as _, Sha256};

use super::{ArtifactInventoryError, MetadataFingerprint, PinnedDirectory};

const VERIFY_BUFFER_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy)]
pub(crate) enum ExactArtifactSync {
    Normal,
    #[cfg(test)]
    FailFile,
    #[cfg(test)]
    FailDirectory,
}

#[derive(Clone, Copy)]
pub(crate) struct ExactArtifactExpectation<'a> {
    pub(crate) byte_size: u64,
    pub(crate) digest: &'a Digest,
    pub(crate) maximum_entries: usize,
    pub(crate) sync: ExactArtifactSync,
}

pub(crate) enum ExactArtifactVerificationError {
    Boundary(ArtifactInventoryError),
    Missing,
    SizeMismatch,
    DigestMismatch,
    Aliased,
}

pub(crate) struct VerifiedManagedArtifact {
    _file: File,
}

pub(crate) fn verify_exact_artifact(
    artifacts: &PinnedDirectory,
    name: &OsStr,
    expectation: ExactArtifactExpectation<'_>,
    cancellation: &CancellationToken,
    mut progress: impl FnMut(u64),
) -> Result<VerifiedManagedArtifact, ExactArtifactVerificationError> {
    let managed = artifacts
        .open_managed_file_for_sync(name, expectation.maximum_entries, cancellation)
        .map_err(ExactArtifactVerificationError::Boundary)?
        .ok_or(ExactArtifactVerificationError::Missing)?;
    if managed.byte_size != expectation.byte_size {
        return Err(ExactArtifactVerificationError::SizeMismatch);
    }
    if !managed.fingerprint.has_single_link() {
        return Err(ExactArtifactVerificationError::Aliased);
    }
    let mut file = managed.file;
    let observed = hash_exact(
        &mut file,
        expectation.byte_size,
        cancellation,
        &mut progress,
    );
    let after_hash = MetadataFingerprint::from_file(&file)
        .map_err(ArtifactInventoryError::StorageIo)
        .map_err(ExactArtifactVerificationError::Boundary)?;
    if after_hash != managed.fingerprint || !after_hash.has_single_link() {
        return Err(ExactArtifactVerificationError::Boundary(
            ArtifactInventoryError::ConcurrentModification,
        ));
    }
    let observed = observed?;
    if &observed != expectation.digest {
        return Err(ExactArtifactVerificationError::DigestMismatch);
    }
    sync_file(&file, expectation.sync)?;
    artifacts
        .recheck_managed_file_for_lifecycle(
            name,
            &managed.fingerprint,
            expectation.maximum_entries,
            cancellation,
        )
        .map_err(ExactArtifactVerificationError::Boundary)?;
    sync_artifacts(artifacts, expectation.sync)?;
    Ok(VerifiedManagedArtifact { _file: file })
}

fn sync_file(file: &File, sync: ExactArtifactSync) -> Result<(), ExactArtifactVerificationError> {
    let _ = sync;
    #[cfg(test)]
    if matches!(sync, ExactArtifactSync::FailFile) {
        return Err(ExactArtifactVerificationError::Boundary(
            ArtifactInventoryError::StorageIo(std::io::Error::other(
                "injected artifact file synchronization failure",
            )),
        ));
    }
    file.sync_all()
        .map_err(ArtifactInventoryError::StorageIo)
        .map_err(ExactArtifactVerificationError::Boundary)
}

fn sync_artifacts(
    artifacts: &PinnedDirectory,
    sync: ExactArtifactSync,
) -> Result<(), ExactArtifactVerificationError> {
    let _ = sync;
    #[cfg(test)]
    if matches!(sync, ExactArtifactSync::FailDirectory) {
        return Err(ExactArtifactVerificationError::Boundary(
            ArtifactInventoryError::StorageIo(std::io::Error::other(
                "injected artifact directory synchronization failure",
            )),
        ));
    }
    artifacts
        .sync()
        .map_err(ExactArtifactVerificationError::Boundary)
}

fn hash_exact(
    file: &mut File,
    expected_size: u64,
    cancellation: &CancellationToken,
    progress: &mut impl FnMut(u64),
) -> Result<Digest, ExactArtifactVerificationError> {
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; VERIFY_BUFFER_BYTES];
    let mut observed_size = 0_u64;
    loop {
        ensure_not_cancelled(cancellation)?;
        let count = file
            .read(&mut buffer)
            .map_err(ArtifactInventoryError::StorageIo)
            .map_err(ExactArtifactVerificationError::Boundary)?;
        if count == 0 {
            break;
        }
        observed_size = observed_size
            .checked_add(
                u64::try_from(count).map_err(|_| ExactArtifactVerificationError::SizeMismatch)?,
            )
            .ok_or(ExactArtifactVerificationError::SizeMismatch)?;
        if observed_size > expected_size {
            return Err(ExactArtifactVerificationError::SizeMismatch);
        }
        hasher.update(&buffer[..count]);
        progress(observed_size);
        ensure_not_cancelled(cancellation)?;
    }
    if observed_size != expected_size {
        return Err(ExactArtifactVerificationError::SizeMismatch);
    }
    Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_| ExactArtifactVerificationError::DigestMismatch)
}

fn ensure_not_cancelled(
    cancellation: &CancellationToken,
) -> Result<(), ExactArtifactVerificationError> {
    if cancellation.is_cancelled() {
        Err(ExactArtifactVerificationError::Boundary(
            ArtifactInventoryError::Cancelled,
        ))
    } else {
        Ok(())
    }
}
