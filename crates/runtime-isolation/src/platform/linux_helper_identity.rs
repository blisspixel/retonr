use std::{
    fs::File,
    io::{Read as _, Seek as _, SeekFrom},
    os::unix::fs::PermissionsExt as _,
    path::Path,
    time::{Duration, Instant},
};

use rewrite_types::{CancellationToken, Digest as DomainDigest};
use sha2::{Digest as _, Sha256};

use crate::{IsolationError, IsolationResult, error::native};

const MAXIMUM_HELPER_BYTES: u64 = 128 * 1024 * 1024;

pub(super) fn open_executable(path: &Path, helper: bool) -> IsolationResult<File> {
    let file = File::open(path).map_err(|_error| invalid_executable(helper, "unavailable"))?;
    validate_executable(&file, helper)?;
    Ok(file)
}

pub(super) fn validate_executable(file: &File, helper: bool) -> IsolationResult<()> {
    let metadata = file
        .metadata()
        .map_err(|_error| invalid_executable(helper, "metadata"))?;
    if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
        return Err(invalid_executable(helper, "object"));
    }
    Ok(())
}

fn invalid_executable(helper: bool, target_reason: &'static str) -> IsolationError {
    if helper {
        IsolationError::InvalidHelper
    } else {
        IsolationError::InvalidLaunch(target_reason)
    }
}

pub(super) fn hash_helper(
    helper: &mut File,
    timeout: Duration,
    cancellation: &CancellationToken,
    started: Instant,
) -> IsolationResult<(DomainDigest, u64)> {
    let bytes = helper
        .metadata()
        .map_err(|error| native("read-helper-metadata", &error))?
        .len();
    if bytes == 0 || bytes > MAXIMUM_HELPER_BYTES {
        return Err(IsolationError::InvalidHelper);
    }
    helper
        .seek(SeekFrom::Start(0))
        .map_err(|error| native("seek-helper", &error))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    let mut observed = 0_u64;
    loop {
        ensure_active(cancellation)?;
        if started.elapsed() >= timeout {
            return Err(IsolationError::StartupTimeout);
        }
        let read = helper
            .read(&mut buffer)
            .map_err(|error| native("hash-helper", &error))?;
        if read == 0 {
            break;
        }
        observed = observed
            .saturating_add(u64::try_from(read).map_err(|_error| IsolationError::InvalidHelper)?);
        hasher.update(&buffer[..read]);
    }
    if observed != bytes {
        return Err(IsolationError::InvalidHelper);
    }
    let digest = DomainDigest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_error| IsolationError::InvalidHelper)?;
    Ok((digest, bytes))
}

fn ensure_active(cancellation: &CancellationToken) -> IsolationResult<()> {
    if cancellation.is_cancelled() {
        Err(IsolationError::Cancelled)
    } else {
        Ok(())
    }
}
