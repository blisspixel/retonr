use std::{fs::File, os::unix::fs::FileExt as _, time::Instant};

use rewrite_model::ArtifactId;
use rewrite_types::{CancellationToken, Digest};
use sha2::{Digest as _, Sha256};

use crate::managed_worker::ensure_worker_active;
use crate::{ManagedGenerationWorkerError, ManagedGenerationWorkerLimits};

const HASH_BUFFER_BYTES: usize = 1024 * 1024;

pub(super) fn hash_file(
    file: &File,
    expected_bytes: u64,
    remaining: &mut u64,
    limits: ManagedGenerationWorkerLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<ArtifactId, ManagedGenerationWorkerError> {
    *remaining = remaining
        .checked_sub(expected_bytes)
        .ok_or(ManagedGenerationWorkerError::ResourceLimit)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
    let buffer_bytes = u64::try_from(buffer.len())
        .map_err(|_error| ManagedGenerationWorkerError::ResourceLimit)?;
    let mut total = 0_u64;
    while total < expected_bytes {
        ensure_worker_active(cancellation, started, limits)?;
        let remaining_bytes = expected_bytes - total;
        let length = usize::try_from(remaining_bytes.min(buffer_bytes))
            .map_err(|_error| ManagedGenerationWorkerError::ResourceLimit)?;
        let read = file
            .read_at(&mut buffer[..length], total)
            .map_err(|_error| ManagedGenerationWorkerError::PlatformObservationFailed)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(
                u64::try_from(read)
                    .map_err(|_error| ManagedGenerationWorkerError::ResourceLimit)?,
            )
            .ok_or(ManagedGenerationWorkerError::ResourceLimit)?;
        if total > expected_bytes {
            return Err(ManagedGenerationWorkerError::ObservationChanged);
        }
        hasher.update(&buffer[..read]);
    }
    ensure_worker_active(cancellation, started, limits)?;
    if total != expected_bytes {
        return Err(ManagedGenerationWorkerError::ObservationChanged);
    }
    let digest = Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_error| ManagedGenerationWorkerError::PlatformObservationFailed)?;
    Ok(ArtifactId::from_digest(digest))
}

pub(super) fn hash_model_file(
    file: &File,
    expected_bytes: u64,
    limits: ManagedGenerationWorkerLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<ArtifactId, ManagedGenerationWorkerError> {
    let mut remaining = limits.maximum_model_weight_bytes;
    hash_file(
        file,
        expected_bytes,
        &mut remaining,
        limits,
        cancellation,
        started,
    )
}

#[cfg(test)]
mod tests;
