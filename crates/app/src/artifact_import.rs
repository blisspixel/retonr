use std::{
    fs::{self, File, Metadata, TryLockError},
    io::{self, Read as _},
    path::{Path, PathBuf},
};

use rewrite_model::{ArtifactManifest, InstalledArtifact};
use rewrite_model_store::ArtifactStateStore;
use rewrite_types::{CancellationToken, Digest};
use sha2::{Digest as _, Sha256};
use tempfile::{Builder as TemporaryFileBuilder, NamedTempFile};

mod contract;

pub use contract::{
    ArtifactImportError, ArtifactImportLimits, ArtifactImportProgress, ArtifactImportResult,
    ArtifactImportStage, OfflineArtifactImportRequest,
};

const COPY_BUFFER_BYTES: usize = 1024 * 1024;
const STAGING_PREFIX: &str = ".import-";
const LOCK_FILE: &str = ".artifact-import.lock";

/// Application service for non-destructive, offline artifact-file import.
pub struct OfflineArtifactImportService<'a> {
    artifacts: PathBuf,
    staging: PathBuf,
    limits: ArtifactImportLimits,
    store: &'a mut ArtifactStateStore,
    _lock: File,
}

impl<'a> OfflineArtifactImportService<'a> {
    /// Opens application-owned storage and exclusively locks its import lifecycle.
    ///
    /// Stale staging files from an interrupted prior process are removed only after
    /// the exclusive lock is held. No source file or final artifact is removed.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactImportError`] when the storage root cannot be created,
    /// contains an indirect path at a managed boundary, is already in use, or stale
    /// staging state cannot be recovered safely.
    pub fn open(
        root: impl AsRef<Path>,
        store: &'a mut ArtifactStateStore,
        limits: ArtifactImportLimits,
    ) -> Result<Self, ArtifactImportError> {
        if limits.maximum_artifact_bytes == 0 {
            return Err(ArtifactImportError::InvalidLimits);
        }
        let root = root.as_ref().to_path_buf();
        ensure_directory(&root)?;
        sync_parent_directory(&root)?;
        let lock_path = root.join(LOCK_FILE);
        reject_existing_indirect_path(&lock_path)?;
        let lock = open_lock_file(&lock_path).map_err(ArtifactImportError::StorageIo)?;
        let lock_metadata = lock.metadata().map_err(ArtifactImportError::StorageIo)?;
        if !lock_metadata.is_file() || is_indirect(&lock_metadata) {
            return Err(ArtifactImportError::UnsafeStorageLayout);
        }
        match lock.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => return Err(ArtifactImportError::StorageInUse),
            Err(TryLockError::Error(error)) => {
                return Err(ArtifactImportError::StorageIo(error));
            }
        }
        let staging = root.join(".staging");
        let artifacts = root.join("artifacts");
        ensure_directory(&staging)?;
        ensure_directory(&artifacts)?;
        sync_directory(&root)?;
        recover_staging(&staging)?;
        Ok(Self {
            artifacts,
            staging,
            limits,
            store,
            _lock: lock,
        })
    }

    /// Copies, verifies, atomically persists, and registers one artifact file.
    ///
    /// The caller-selected source is never modified. Exact existing final bytes and
    /// state make the operation idempotent. The operation never activates an
    /// artifact and performs no network access.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactImportError`] for an invalid manifest or source, content
    /// mismatch, unsafe storage, persistence failure, or state-store failure.
    pub fn import<F>(
        &mut self,
        request: &OfflineArtifactImportRequest,
        cancellation: &CancellationToken,
        mut progress: F,
    ) -> Result<ArtifactImportResult, ArtifactImportError>
    where
        F: FnMut(ArtifactImportProgress),
    {
        ensure_not_cancelled(cancellation)?;
        request
            .manifest
            .validate()
            .map_err(ArtifactImportError::InvalidManifest)?;
        if request.manifest.byte_size > self.limits.maximum_artifact_bytes {
            return Err(ArtifactImportError::ArtifactTooLarge {
                actual: request.manifest.byte_size,
                maximum: self.limits.maximum_artifact_bytes,
            });
        }
        report_progress(
            &mut progress,
            ArtifactImportStage::InspectingSource,
            0,
            request.manifest.byte_size,
        );
        let (mut source, initial_metadata) = open_source(&request.source)?;
        if initial_metadata.len() != request.manifest.byte_size {
            return Err(ArtifactImportError::SizeMismatch);
        }

        let storage_key = storage_key(&request.manifest.artifact_digest);
        ensure_directory(&self.artifacts)?;
        let destination = self
            .artifacts
            .join(request.manifest.artifact_digest.as_str());
        let mut staged = TemporaryFileBuilder::new()
            .prefix(STAGING_PREFIX)
            .tempfile_in(&self.staging)
            .map_err(ArtifactImportError::StorageIo)?;
        report_progress(
            &mut progress,
            ArtifactImportStage::StagingAndVerifying,
            0,
            request.manifest.byte_size,
        );
        let observed = copy_and_hash(
            &mut source,
            staged.as_file_mut(),
            request.manifest.byte_size,
            cancellation,
            &mut |completed| {
                report_progress(
                    &mut progress,
                    ArtifactImportStage::StagingAndVerifying,
                    completed,
                    request.manifest.byte_size,
                );
            },
        )?;
        verify_source_after_read(&source, request.manifest.byte_size)?;
        if observed != request.manifest.artifact_digest {
            return Err(ArtifactImportError::DigestMismatch);
        }
        staged
            .as_file_mut()
            .sync_all()
            .map_err(ArtifactImportError::StorageIo)?;
        ensure_not_cancelled(cancellation)?;
        report_progress(
            &mut progress,
            ArtifactImportStage::CommittingFile,
            request.manifest.byte_size,
            request.manifest.byte_size,
        );
        persist_or_verify(
            staged,
            &destination,
            &request.manifest,
            cancellation,
            &mut progress,
        )?;
        sync_directory(&self.artifacts)?;

        let installed = InstalledArtifact {
            artifact_id: request.manifest.artifact_id.clone(),
            artifact_digest: request.manifest.artifact_digest.clone(),
            byte_size: request.manifest.byte_size,
            storage_key,
        };
        report_progress(
            &mut progress,
            ArtifactImportStage::RegisteringState,
            request.manifest.byte_size,
            request.manifest.byte_size,
        );
        let state = self
            .store
            .put_installation(&request.manifest, &installed)
            .map_err(ArtifactImportError::State)?;
        report_progress(
            &mut progress,
            ArtifactImportStage::Complete,
            request.manifest.byte_size,
            request.manifest.byte_size,
        );
        Ok(ArtifactImportResult { installed, state })
    }
}

fn open_source(path: &Path) -> Result<(File, Metadata), ArtifactImportError> {
    let path_metadata = fs::symlink_metadata(path).map_err(ArtifactImportError::SourceIo)?;
    if is_indirect(&path_metadata) {
        return Err(ArtifactImportError::IndirectSource);
    }
    if !path_metadata.is_file() {
        return Err(ArtifactImportError::SourceNotRegular);
    }
    let source = open_readonly_no_follow(path).map_err(ArtifactImportError::SourceIo)?;
    let opened_metadata = source.metadata().map_err(ArtifactImportError::SourceIo)?;
    if !opened_metadata.is_file() {
        return Err(ArtifactImportError::SourceNotRegular);
    }
    Ok((source, opened_metadata))
}

fn copy_and_hash<W: io::Write, F: FnMut(u64)>(
    source: &mut File,
    destination: &mut W,
    expected_size: u64,
    cancellation: &CancellationToken,
    progress: &mut F,
) -> Result<Digest, ArtifactImportError> {
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    let mut observed_size = 0_u64;
    loop {
        ensure_not_cancelled(cancellation)?;
        let count = source
            .read(&mut buffer)
            .map_err(ArtifactImportError::SourceIo)?;
        if count == 0 {
            break;
        }
        observed_size = observed_size
            .checked_add(u64::try_from(count).map_err(|_| ArtifactImportError::SizeMismatch)?)
            .ok_or(ArtifactImportError::SizeMismatch)?;
        if observed_size > expected_size {
            return Err(ArtifactImportError::SizeMismatch);
        }
        destination
            .write_all(&buffer[..count])
            .map_err(ArtifactImportError::StorageIo)?;
        hasher.update(&buffer[..count]);
        progress(observed_size);
    }
    if observed_size != expected_size {
        return Err(ArtifactImportError::SizeMismatch);
    }
    Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_| ArtifactImportError::DigestMismatch)
}

fn verify_source_after_read(source: &File, expected_size: u64) -> Result<(), ArtifactImportError> {
    let final_metadata = source.metadata().map_err(ArtifactImportError::SourceIo)?;
    if final_metadata.len() != expected_size {
        return Err(ArtifactImportError::SizeMismatch);
    }
    Ok(())
}

fn persist_or_verify(
    staged: NamedTempFile,
    destination: &Path,
    manifest: &ArtifactManifest,
    cancellation: &CancellationToken,
    progress: &mut impl FnMut(ArtifactImportProgress),
) -> Result<(), ArtifactImportError> {
    reject_existing_indirect_path(destination)?;
    if destination.exists() {
        return verify_stored_file(destination, manifest, cancellation, progress);
    }
    match staged.persist_noclobber(destination) {
        Ok(file) => file.sync_all().map_err(ArtifactImportError::StorageIo),
        Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => {
            drop(error.file);
            verify_stored_file(destination, manifest, cancellation, progress)
        }
        Err(error) => Err(ArtifactImportError::StorageIo(error.error)),
    }
}

fn verify_stored_file(
    path: &Path,
    manifest: &ArtifactManifest,
    cancellation: &CancellationToken,
    progress: &mut impl FnMut(ArtifactImportProgress),
) -> Result<(), ArtifactImportError> {
    let (mut file, metadata) = open_stored_file(path)?;
    if metadata.len() != manifest.byte_size {
        return Err(ArtifactImportError::StorageConflict);
    }
    report_progress(
        progress,
        ArtifactImportStage::VerifyingExistingFile,
        0,
        manifest.byte_size,
    );
    let digest = copy_and_hash(
        &mut file,
        &mut io::sink(),
        manifest.byte_size,
        cancellation,
        &mut |completed| {
            report_progress(
                progress,
                ArtifactImportStage::VerifyingExistingFile,
                completed,
                manifest.byte_size,
            );
        },
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
    Ok(())
}

fn open_stored_file(path: &Path) -> Result<(File, Metadata), ArtifactImportError> {
    let metadata = fs::symlink_metadata(path).map_err(ArtifactImportError::StorageIo)?;
    if is_indirect(&metadata) || !metadata.is_file() {
        return Err(ArtifactImportError::StorageConflict);
    }
    let file = open_readonly_no_follow(path).map_err(ArtifactImportError::StorageIo)?;
    let opened = file.metadata().map_err(ArtifactImportError::StorageIo)?;
    if !opened.is_file() || is_indirect(&opened) {
        return Err(ArtifactImportError::StorageConflict);
    }
    Ok((file, opened))
}

fn storage_key(digest: &Digest) -> String {
    format!("artifacts/{}", digest.as_str())
}

fn ensure_not_cancelled(cancellation: &CancellationToken) -> Result<(), ArtifactImportError> {
    if cancellation.is_cancelled() {
        Err(ArtifactImportError::Cancelled)
    } else {
        Ok(())
    }
}

fn report_progress(
    progress: &mut impl FnMut(ArtifactImportProgress),
    stage: ArtifactImportStage,
    completed_bytes: u64,
    total_bytes: u64,
) {
    progress(ArtifactImportProgress {
        stage,
        completed_bytes,
        total_bytes,
    });
}

fn ensure_directory(path: &Path) -> Result<(), ArtifactImportError> {
    fs::create_dir_all(path).map_err(ArtifactImportError::StorageIo)?;
    let metadata = fs::symlink_metadata(path).map_err(ArtifactImportError::StorageIo)?;
    if !metadata.is_dir() || is_indirect(&metadata) {
        return Err(ArtifactImportError::UnsafeStorageLayout);
    }
    set_private_directory_permissions(path)?;
    Ok(())
}

fn sync_parent_directory(path: &Path) -> Result<(), ArtifactImportError> {
    let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    else {
        return Ok(());
    };
    sync_directory(parent)
}

fn reject_existing_indirect_path(path: &Path) -> Result<(), ArtifactImportError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if is_indirect(&metadata) => Err(ArtifactImportError::UnsafeStorageLayout),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(ArtifactImportError::StorageIo(error)),
    }
}

fn recover_staging(staging: &Path) -> Result<(), ArtifactImportError> {
    for entry in fs::read_dir(staging).map_err(ArtifactImportError::StorageIo)? {
        let entry = entry.map_err(ArtifactImportError::StorageIo)?;
        let name = entry.file_name();
        if !name.to_string_lossy().starts_with(STAGING_PREFIX) {
            continue;
        }
        let metadata =
            fs::symlink_metadata(entry.path()).map_err(ArtifactImportError::StorageIo)?;
        if !metadata.is_file() || is_indirect(&metadata) {
            return Err(ArtifactImportError::UnsafeStorageLayout);
        }
        fs::remove_file(entry.path()).map_err(ArtifactImportError::StorageIo)?;
    }
    sync_directory(staging)
}

#[cfg(unix)]
fn set_private_directory_permissions(path: &Path) -> Result<(), ArtifactImportError> {
    use std::os::unix::fs::PermissionsExt as _;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(ArtifactImportError::StorageIo)
}

#[cfg(windows)]
fn set_private_directory_permissions(path: &Path) -> Result<(), ArtifactImportError> {
    let metadata = fs::symlink_metadata(path).map_err(ArtifactImportError::StorageIo)?;
    if metadata.is_dir() && !is_indirect(&metadata) {
        Ok(())
    } else {
        Err(ArtifactImportError::UnsafeStorageLayout)
    }
}

fn sync_directory(path: &Path) -> Result<(), ArtifactImportError> {
    let metadata = fs::symlink_metadata(path).map_err(ArtifactImportError::StorageIo)?;
    if !metadata.is_dir() || is_indirect(&metadata) {
        return Err(ArtifactImportError::UnsafeStorageLayout);
    }
    #[cfg(unix)]
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(ArtifactImportError::StorageIo)?;
    Ok(())
}

#[cfg(unix)]
fn open_readonly_no_follow(path: &Path) -> io::Result<File> {
    use rustix::fs::{Mode, OFlags};

    rustix::fs::open(
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(io::Error::from)
}

#[cfg(windows)]
fn open_readonly_no_follow(path: &Path) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt as _;

    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    fs::OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
}

#[cfg(unix)]
fn open_lock_file(path: &Path) -> io::Result<File> {
    use rustix::fs::{Mode, OFlags};

    rustix::fs::open(
        path,
        OFlags::RDWR | OFlags::CREATE | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::RUSR | Mode::WUSR,
    )
    .map(File::from)
    .map_err(io::Error::from)
}

#[cfg(windows)]
fn open_lock_file(path: &Path) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt as _;

    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
}

fn is_indirect(metadata: &Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;

        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(unix)]
    {
        false
    }
}

#[cfg(test)]
mod tests;
