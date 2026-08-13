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
mod platform;

pub use contract::{
    ArtifactImportError, ArtifactImportLimits, ArtifactImportProgress, ArtifactImportResult,
    ArtifactImportStage, OfflineArtifactImportRequest,
};
#[cfg(unix)]
use platform::set_private_directory_permissions;
use platform::{is_indirect, open_lock_file, open_readonly_no_follow, sync_directory};

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
        reject_existing_non_regular_path(&lock_path)?;
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

        let installed_storage_key = storage_key(&request.manifest.artifact_digest);
        ensure_directory(&self.artifacts)?;
        let destination = self
            .artifacts
            .join(request.manifest.artifact_digest.as_str());
        let destination_exists = stored_file_exists(&destination)?;
        let mut staged = if destination_exists {
            None
        } else {
            ensure_directory(&self.staging)?;
            Some(
                TemporaryFileBuilder::new()
                    .prefix(STAGING_PREFIX)
                    .tempfile_in(&self.staging)
                    .map_err(ArtifactImportError::StorageIo)?,
            )
        };
        let observed = verify_source_bytes(
            &mut source,
            staged.as_mut(),
            &request.manifest,
            cancellation,
            &mut progress,
        )?;
        verify_source_after_read(&source, request.manifest.byte_size)?;
        if observed != request.manifest.artifact_digest {
            return Err(ArtifactImportError::DigestMismatch);
        }
        ensure_not_cancelled(cancellation)?;
        if let Some(mut staged) = staged {
            staged
                .as_file_mut()
                .sync_all()
                .map_err(ArtifactImportError::StorageIo)?;
            persist_or_verify(
                staged,
                &destination,
                &request.manifest,
                cancellation,
                &mut progress,
            )?;
            sync_directory(&self.artifacts)?;
        } else {
            verify_stored_file(&destination, &request.manifest, cancellation, &mut progress)?;
        }

        let installed = InstalledArtifact {
            artifact_id: request.manifest.artifact_id.clone(),
            artifact_digest: request.manifest.artifact_digest.clone(),
            byte_size: request.manifest.byte_size,
            storage_key: installed_storage_key,
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
    if is_indirect(&opened_metadata) {
        return Err(ArtifactImportError::IndirectSource);
    }
    if !opened_metadata.is_file() {
        return Err(ArtifactImportError::SourceNotRegular);
    }
    Ok((source, opened_metadata))
}

fn verify_source_bytes(
    source: &mut File,
    staged: Option<&mut NamedTempFile>,
    manifest: &ArtifactManifest,
    cancellation: &CancellationToken,
    progress: &mut impl FnMut(ArtifactImportProgress),
) -> Result<Digest, ArtifactImportError> {
    let stage = if staged.is_some() {
        ArtifactImportStage::StagingAndVerifying
    } else {
        ArtifactImportStage::VerifyingSource
    };
    report_progress(progress, stage, 0, manifest.byte_size);
    let mut source_progress = |completed| {
        report_progress(progress, stage, completed, manifest.byte_size);
    };
    match staged {
        Some(staged) => copy_and_hash(
            source,
            staged.as_file_mut(),
            manifest.byte_size,
            cancellation,
            &mut source_progress,
        ),
        None => copy_and_hash(
            source,
            &mut io::sink(),
            manifest.byte_size,
            cancellation,
            &mut source_progress,
        ),
    }
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
    reject_existing_non_regular_path(destination)?;
    if destination.exists() {
        return verify_stored_file(destination, manifest, cancellation, progress);
    }
    report_progress(
        progress,
        ArtifactImportStage::CommittingFile,
        manifest.byte_size,
        manifest.byte_size,
    );
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
        return Err(ArtifactImportError::UnsafeStorageLayout);
    }
    let file = open_readonly_no_follow(path).map_err(ArtifactImportError::StorageIo)?;
    let opened = file.metadata().map_err(ArtifactImportError::StorageIo)?;
    if !opened.is_file() || is_indirect(&opened) {
        return Err(ArtifactImportError::UnsafeStorageLayout);
    }
    Ok((file, opened))
}

fn stored_file_exists(path: &Path) -> Result<bool, ArtifactImportError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if is_indirect(&metadata) || !metadata.is_file() => {
            Err(ArtifactImportError::UnsafeStorageLayout)
        }
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(ArtifactImportError::StorageIo(error)),
    }
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
    match fs::create_dir_all(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(ArtifactImportError::StorageIo(error)),
    }
    let metadata = fs::symlink_metadata(path).map_err(ArtifactImportError::StorageIo)?;
    if !metadata.is_dir() || is_indirect(&metadata) {
        return Err(ArtifactImportError::UnsafeStorageLayout);
    }
    #[cfg(unix)]
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

fn reject_existing_non_regular_path(path: &Path) -> Result<(), ArtifactImportError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if is_indirect(&metadata) || !metadata.is_file() => {
            Err(ArtifactImportError::UnsafeStorageLayout)
        }
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

#[cfg(test)]
mod tests;
