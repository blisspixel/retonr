use std::{
    fs::{File, Permissions},
    io::{Read as _, Seek as _, SeekFrom, Write as _},
    os::unix::fs::PermissionsExt as _,
};

use rewrite_model::ArtifactId;
use rewrite_types::CancellationToken;
use rustix::fs::{MemfdFlags, SealFlags, fcntl_add_seals, fcntl_get_seals, memfd_create};
use sha2::{Digest as _, Sha256};

use crate::artifact_storage::MetadataFingerprint;

use super::{
    HASH_BUFFER_BYTES, PackageAttestationError, ensure_not_cancelled, require_stable_handle,
};

pub(super) fn snapshot(
    retained: &File,
    fingerprint: &MetadataFingerprint,
    artifact_id: &ArtifactId,
    byte_size: u64,
    cancellation: &CancellationToken,
) -> Result<File, PackageAttestationError> {
    ensure_not_cancelled(cancellation)?;
    require_stable_handle(retained, fingerprint)?;
    let mut source = retained
        .try_clone()
        .map_err(PackageAttestationError::MemberIo)?;
    source
        .seek(SeekFrom::Start(0))
        .map_err(PackageAttestationError::MemberIo)?;
    let (mut snapshot, supports_exec_seal) = create_executable_memfd()?;
    let mut hasher = Sha256::new();
    let mut observed = 0_u64;
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
    while observed < byte_size {
        ensure_not_cancelled(cancellation)?;
        let remaining = usize::try_from(byte_size - observed)
            .unwrap_or(usize::MAX)
            .min(buffer.len());
        let read = source
            .read(&mut buffer[..remaining])
            .map_err(PackageAttestationError::MemberIo)?;
        if read == 0 {
            return Err(PackageAttestationError::MemberBytesConflict);
        }
        snapshot
            .write_all(&buffer[..read])
            .map_err(PackageAttestationError::MemberIo)?;
        hasher.update(&buffer[..read]);
        observed = observed
            .checked_add(
                u64::try_from(read).map_err(|_error| PackageAttestationError::InvalidLimits)?,
            )
            .ok_or(PackageAttestationError::InvalidLimits)?;
    }
    let mut trailing = [0_u8; 1];
    if source
        .read(&mut trailing)
        .map_err(PackageAttestationError::MemberIo)?
        != 0
        || snapshot
            .metadata()
            .map_err(PackageAttestationError::MemberIo)?
            .len()
            != byte_size
        || ArtifactId::from_digest(
            rewrite_types::Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
                .map_err(|_error| PackageAttestationError::MemberBytesConflict)?,
        )
        .digest()
            != artifact_id.digest()
    {
        return Err(PackageAttestationError::MemberBytesConflict);
    }
    snapshot
        .set_permissions(Permissions::from_mode(0o555))
        .map_err(PackageAttestationError::MemberIo)?;
    let mut required = SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK;
    if supports_exec_seal {
        required |= SealFlags::EXEC;
    }
    required |= SealFlags::SEAL;
    fcntl_add_seals(&snapshot, required).map_err(native_io)?;
    if !fcntl_get_seals(&snapshot)
        .map_err(native_io)?
        .contains(required)
    {
        return Err(PackageAttestationError::MemberIdentityChanged);
    }
    snapshot
        .seek(SeekFrom::Start(0))
        .map_err(PackageAttestationError::MemberIo)?;
    require_stable_handle(retained, fingerprint)?;
    Ok(snapshot)
}

fn create_executable_memfd() -> Result<(File, bool), PackageAttestationError> {
    let common = MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING;
    match memfd_create("retonr-runtime-entrypoint", common | MemfdFlags::EXEC) {
        Ok(descriptor) => Ok((File::from(descriptor), true)),
        Err(rustix::io::Errno::INVAL) => memfd_create("retonr-runtime-entrypoint", common)
            .map(|descriptor| (File::from(descriptor), false))
            .map_err(native_io),
        Err(error) => Err(native_io(error)),
    }
}

fn native_io(error: rustix::io::Errno) -> PackageAttestationError {
    PackageAttestationError::MemberIo(std::io::Error::from(error))
}
