use std::{
    fs::File,
    io::{Read as _, Seek as _, SeekFrom, Write as _},
    os::unix::fs::PermissionsExt as _,
    path::Path,
    time::{Duration, Instant},
};

use rewrite_types::{CancellationToken, Digest as DomainDigest};
use rustix::fs::{
    MemfdFlags, Mode, OFlags, SealFlags, fcntl_add_seals, fcntl_get_seals, memfd_create, open,
};
use sha2::{Digest as _, Sha256};

use crate::{IsolationError, IsolationResult, error::native};

use super::linux_executable::has_elf_magic;
use super::linux_validation::native_errno;

const MAXIMUM_HELPER_BYTES: u64 = 128 * 1024 * 1024;

pub(super) fn open_executable(path: &Path, helper: bool) -> IsolationResult<File> {
    let file = open(
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|_error| invalid_executable(helper, "unavailable"))?;
    validate_executable(&file, helper)?;
    Ok(file)
}

pub(super) fn validate_executable(file: &File, helper: bool) -> IsolationResult<()> {
    let metadata = file
        .metadata()
        .map_err(|_error| invalid_executable(helper, "metadata"))?;
    if !metadata.is_file()
        || metadata.permissions().mode() & 0o111 == 0
        || !has_elf_magic(file).map_err(|_error| invalid_executable(helper, "format"))?
    {
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

pub(super) fn snapshot_helper(
    mut helper: File,
    expected_digest: &DomainDigest,
    expected_bytes: u64,
    timeout: Duration,
    cancellation: &CancellationToken,
    started: Instant,
) -> IsolationResult<File> {
    require_snapshot_active(timeout, cancellation, started)?;
    let bytes = helper
        .metadata()
        .map_err(|error| native("read-helper-metadata", &error))?
        .len();
    require_snapshot_active(timeout, cancellation, started)?;
    if expected_bytes == 0 || expected_bytes > MAXIMUM_HELPER_BYTES || bytes != expected_bytes {
        return Err(IsolationError::InvalidHelper);
    }
    helper
        .seek(SeekFrom::Start(0))
        .map_err(|error| native("seek-helper", &error))?;
    require_snapshot_active(timeout, cancellation, started)?;
    let (mut snapshot, supports_exec_seal) = create_executable_memfd()?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    let mut observed = 0_u64;
    while observed < expected_bytes {
        require_snapshot_active(timeout, cancellation, started)?;
        let remaining = usize::try_from(expected_bytes - observed)
            .unwrap_or(usize::MAX)
            .min(buffer.len());
        let read = helper
            .read(&mut buffer[..remaining])
            .map_err(|error| native("hash-helper", &error))?;
        if read == 0 {
            return Err(IsolationError::InvalidHelper);
        }
        require_snapshot_active(timeout, cancellation, started)?;
        snapshot
            .write_all(&buffer[..read])
            .map_err(|error| native("write-helper-snapshot", &error))?;
        observed = observed
            .checked_add(u64::try_from(read).map_err(|_error| IsolationError::InvalidHelper)?)
            .ok_or(IsolationError::InvalidHelper)?;
        hasher.update(&buffer[..read]);
    }
    require_snapshot_active(timeout, cancellation, started)?;
    let mut trailing = [0_u8; 1];
    if helper
        .read(&mut trailing)
        .map_err(|error| native("check-helper-trailing-byte", &error))?
        != 0
        || snapshot
            .metadata()
            .map_err(|error| native("read-helper-snapshot-metadata", &error))?
            .len()
            != expected_bytes
    {
        return Err(IsolationError::InvalidHelper);
    }
    let digest = DomainDigest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_error| IsolationError::InvalidHelper)?;
    if &digest != expected_digest {
        return Err(IsolationError::InvalidHelper);
    }
    snapshot
        .set_permissions(std::fs::Permissions::from_mode(0o555))
        .map_err(|error| native("make-helper-snapshot-executable", &error))?;
    let mut required = SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK;
    if supports_exec_seal {
        required |= SealFlags::EXEC;
    }
    required |= SealFlags::SEAL;
    fcntl_add_seals(&snapshot, required)
        .map_err(|error| native_errno("seal-helper-snapshot", error))?;
    if !fcntl_get_seals(&snapshot)
        .map_err(|error| native_errno("read-helper-snapshot-seals", error))?
        .contains(required)
    {
        return Err(IsolationError::InvalidHelper);
    }
    snapshot
        .seek(SeekFrom::Start(0))
        .map_err(|error| native("seek-helper-snapshot", &error))?;
    validate_executable(&snapshot, true)?;
    Ok(snapshot)
}

fn create_executable_memfd() -> IsolationResult<(File, bool)> {
    let common = MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING;
    match memfd_create("retonr-isolation-helper", common | MemfdFlags::EXEC) {
        Ok(descriptor) => Ok((File::from(descriptor), true)),
        Err(rustix::io::Errno::INVAL) => memfd_create("retonr-isolation-helper", common)
            .map(|descriptor| (File::from(descriptor), false))
            .map_err(|error| native_errno("create-helper-snapshot", error)),
        Err(error) => Err(native_errno("create-helper-snapshot", error)),
    }
}

fn require_snapshot_active(
    timeout: Duration,
    cancellation: &CancellationToken,
    started: Instant,
) -> IsolationResult<()> {
    ensure_active(cancellation)?;
    if started.elapsed() >= timeout {
        Err(IsolationError::StartupTimeout)
    } else {
        Ok(())
    }
}

fn ensure_active(cancellation: &CancellationToken) -> IsolationResult<()> {
    if cancellation.is_cancelled() {
        Err(IsolationError::Cancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs::{self, File},
        io::{Read as _, Seek as _, SeekFrom, Write as _},
        os::unix::fs::{PermissionsExt as _, symlink},
        time::{Duration, Instant},
    };

    use rewrite_types::{CancellationToken, Digest};
    use rustix::fs::{CWD, Mode, SealFlags, fcntl_get_seals, mkfifoat};
    use tempfile::{NamedTempFile, tempdir};

    use super::{create_executable_memfd, open_executable, snapshot_helper};
    use crate::IsolationError;

    const ELF_FIXTURE_BYTES: &[u8] = b"\x7fELF exact helper bytes";

    #[test]
    fn helper_snapshot_is_expected_sealed_and_independent_of_source_mutation() {
        let bytes = ELF_FIXTURE_BYTES;
        let mut temporary = NamedTempFile::new().expect("temporary helper");
        temporary.write_all(bytes).expect("write helper");
        temporary
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o755))
            .expect("make helper executable");
        let digest = Digest::sha256(bytes);
        let mut snapshot = snapshot_helper(
            File::open(temporary.path()).expect("open helper"),
            &digest,
            u64::try_from(bytes.len()).expect("helper length"),
            Duration::from_secs(1),
            &CancellationToken::new(),
            Instant::now(),
        )
        .expect("snapshot expected helper");

        fs::write(temporary.path(), b"changed helper byt").expect("mutate original helper");
        let mut observed = Vec::new();
        snapshot
            .read_to_end(&mut observed)
            .expect("read sealed snapshot");
        assert_eq!(observed, bytes);
        let required = SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK | SealFlags::SEAL;
        assert!(
            fcntl_get_seals(&snapshot)
                .expect("read helper seals")
                .contains(required)
        );
        let (_probe, supports_exec_seal) =
            create_executable_memfd().expect("probe executable memfd support");
        if supports_exec_seal {
            assert!(
                fcntl_get_seals(&snapshot)
                    .expect("read executable helper seals")
                    .contains(SealFlags::EXEC)
            );
            assert!(
                snapshot
                    .set_permissions(fs::Permissions::from_mode(0o444))
                    .is_err()
            );
        }
        snapshot.seek(SeekFrom::Start(0)).expect("rewind snapshot");
        assert!(snapshot.write_all(b"x").is_err());
    }

    #[test]
    fn helper_path_open_is_nonblocking_and_rejects_indirection() {
        let temporary = tempdir().expect("temporary root");
        let direct = temporary.path().join("direct-helper");
        fs::write(&direct, ELF_FIXTURE_BYTES).expect("write direct helper");
        fs::set_permissions(&direct, fs::Permissions::from_mode(0o755))
            .expect("make direct helper executable");
        assert!(open_executable(&direct, true).is_ok());

        let script = temporary.path().join("script-helper");
        fs::write(&script, b"#!/bin/sh\nexit 0\n").expect("write script helper");
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755))
            .expect("make script executable");
        assert_eq!(
            open_executable(&script, true).expect_err("script helper must fail"),
            IsolationError::InvalidHelper
        );

        let indirect = temporary.path().join("indirect-helper");
        symlink(&direct, &indirect).expect("create helper symlink");
        assert_eq!(
            open_executable(&indirect, true).expect_err("helper symlink must fail"),
            IsolationError::InvalidHelper
        );

        let fifo = temporary.path().join("helper-fifo");
        mkfifoat(CWD, &fifo, Mode::RUSR | Mode::WUSR).expect("create helper FIFO");
        let started = Instant::now();
        assert_eq!(
            open_executable(&fifo, true).expect_err("helper FIFO must fail"),
            IsolationError::InvalidHelper
        );
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn helper_snapshot_rejects_an_unexpected_identity() {
        let bytes = ELF_FIXTURE_BYTES;
        let mut temporary = NamedTempFile::new().expect("temporary helper");
        temporary.write_all(bytes).expect("write helper");
        temporary
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o755))
            .expect("make helper executable");
        assert_eq!(
            snapshot_helper(
                File::open(temporary.path()).expect("open helper"),
                &Digest::sha256(b"different helper"),
                u64::try_from(bytes.len()).expect("helper length"),
                Duration::from_secs(1),
                &CancellationToken::new(),
                Instant::now(),
            )
            .expect_err("unexpected helper must fail"),
            IsolationError::InvalidHelper
        );
    }

    #[test]
    fn helper_snapshot_rejects_an_expired_deadline_before_reading() {
        let bytes = ELF_FIXTURE_BYTES;
        let mut temporary = NamedTempFile::new().expect("temporary helper");
        temporary.write_all(bytes).expect("write helper");
        temporary
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o755))
            .expect("make helper executable");
        assert_eq!(
            snapshot_helper(
                File::open(temporary.path()).expect("open helper"),
                &Digest::sha256(bytes),
                u64::try_from(bytes.len()).expect("helper length"),
                Duration::from_secs(1),
                &CancellationToken::new(),
                Instant::now()
                    .checked_sub(Duration::from_secs(2))
                    .expect("expired start"),
            )
            .expect_err("expired helper snapshot must fail"),
            IsolationError::StartupTimeout
        );
    }
}
