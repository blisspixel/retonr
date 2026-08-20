use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    time::Instant,
};

use rewrite_types::{CancellationToken, Digest};
use sha2::{Digest as _, Sha256};

use crate::{AttachedProcessWitnessError, AttachedProcessWitnessLimits, ensure_active};

const HASH_BUFFER_BYTES: usize = 1024 * 1024;

pub(super) fn hash_opened_file(
    file: &mut File,
    expected_bytes: u64,
    limits: AttachedProcessWitnessLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<Digest, AttachedProcessWitnessError> {
    if expected_bytes == 0 {
        return Err(AttachedProcessWitnessError::EntrypointNotRegular);
    }
    if expected_bytes > limits.maximum_entrypoint_bytes {
        return Err(AttachedProcessWitnessError::EntrypointTooLarge);
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|_error| AttachedProcessWitnessError::EntrypointUnavailable)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
    let mut total = 0_u64;
    loop {
        ensure_active(cancellation, started, limits)?;
        let read = file
            .read(&mut buffer)
            .map_err(|_error| AttachedProcessWitnessError::EntrypointUnavailable)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(
                u64::try_from(read)
                    .map_err(|_error| AttachedProcessWitnessError::EntrypointTooLarge)?,
            )
            .ok_or(AttachedProcessWitnessError::EntrypointTooLarge)?;
        if total > limits.maximum_entrypoint_bytes {
            return Err(AttachedProcessWitnessError::EntrypointTooLarge);
        }
        hasher.update(&buffer[..read]);
    }
    if total != expected_bytes {
        return Err(AttachedProcessWitnessError::EntrypointChanged);
    }
    Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_error| AttachedProcessWitnessError::EntrypointChanged)
}
