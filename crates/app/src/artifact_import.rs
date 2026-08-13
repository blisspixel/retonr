use std::{
    ffi::{OsStr, OsString},
    fs::{self, File, Metadata, TryLockError},
    io::{self, Read as _},
    path::{Path, PathBuf},
};

use rewrite_model::{ArtifactManifest, InstalledArtifact};
use rewrite_model_store::ArtifactStateStore;
use rewrite_types::{CancellationToken, Digest};
use sha2::{Digest as _, Sha256};

mod boundary;
mod contract;
pub(crate) mod platform;
mod staged;
mod verify;

use crate::artifact_storage::{
    ExactEntryCapacity, LIFECYCLE_LOCK_FILE, PinnedDirectory, fingerprint_std_file, is_indirect,
};
use boundary::{
    map_open_error as map_boundary_open_error, map_recovery_error as map_boundary_recovery_error,
    map_storage_error as map_boundary_storage_error,
};
pub use contract::{
    ArtifactImportError, ArtifactImportLimits, ArtifactImportProgress, ArtifactImportResult,
    ArtifactImportStage, OfflineArtifactImportRequest,
};
#[cfg(unix)]
use platform::set_private_directory_permissions;
use platform::{open_readonly_no_follow, sync_directory};
use staged::StagedArtifact;
use verify::verify_stored_file;

const COPY_BUFFER_BYTES: usize = 1024 * 1024;
const STAGING_PREFIX: &str = ".import-";
const MAX_RECOVERY_ENTRIES: usize = 1_024;

/// Application service for non-destructive, offline artifact-file import.
pub struct OfflineArtifactImportService<'a> {
    root_path: PathBuf,
    root: PinnedDirectory,
    artifacts: PinnedDirectory,
    staging: PinnedDirectory,
    limits: ArtifactImportLimits,
    store: &'a mut ArtifactStateStore,
    lock: File,
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
        if limits.maximum_artifact_bytes == 0
            || limits.maximum_storage_entries == 0
            || limits.maximum_storage_entries.checked_add(1).is_none()
        {
            return Err(ArtifactImportError::InvalidLimits);
        }
        let root_path =
            std::path::absolute(root.as_ref()).map_err(ArtifactImportError::StorageIo)?;
        ensure_root_directory(&root_path)?;
        sync_parent_directory(&root_path)?;
        let root = PinnedDirectory::open_existing(&root_path).map_err(map_boundary_open_error)?;
        let (lock, _) = root
            .open_or_create_lock_file(OsStr::new(LIFECYCLE_LOCK_FILE))
            .map_err(map_boundary_open_error)?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => return Err(ArtifactImportError::StorageInUse),
            Err(TryLockError::Error(error)) => {
                return Err(ArtifactImportError::StorageIo(error));
            }
        }
        let staging = root
            .ensure_child_directory(OsStr::new(".staging"))
            .map_err(map_boundary_open_error)?;
        let artifacts = root
            .ensure_child_directory(OsStr::new("artifacts"))
            .map_err(map_boundary_open_error)?;
        root.sync().map_err(map_boundary_open_error)?;
        staging
            .recover_owned_staging(STAGING_PREFIX, MAX_RECOVERY_ENTRIES)
            .map_err(map_boundary_recovery_error)?;
        let service = Self {
            root_path,
            root,
            artifacts,
            staging,
            limits,
            store,
            lock,
        };
        service.validate_storage_layout()?;
        Ok(service)
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
        self.validate_storage_layout()?;
        self.validate_request(request)?;
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
        let destination_name = OsString::from(request.manifest.artifact_digest.as_str());
        let destination_exists = self
            .artifacts
            .open_managed_file(
                &destination_name,
                self.limits.maximum_storage_entries,
                cancellation,
            )
            .map_err(map_boundary_storage_error)?
            .is_some();
        let mut staged = if destination_exists {
            None
        } else {
            let (name, file) = self
                .staging
                .create_staging_file(STAGING_PREFIX, MAX_RECOVERY_ENTRIES, cancellation)
                .map_err(map_boundary_recovery_error)?;
            Some(StagedArtifact::new(&self.staging, name, file))
        };
        let staged_file = match staged.as_mut() {
            Some(staged) => Some(staged.file_mut()?),
            None => None,
        };
        let observed = verify_source_bytes(
            &mut source,
            staged_file,
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
            staged.freeze_single_link()?;
            self.commit_staged(
                staged,
                &destination_name,
                request.manifest.byte_size,
                cancellation,
                &mut progress,
            )?;
        }

        let installed = InstalledArtifact {
            artifact_id: request.manifest.artifact_id.clone(),
            artifact_digest: request.manifest.artifact_digest.clone(),
            byte_size: request.manifest.byte_size,
            storage_key: installed_storage_key,
        };
        report_progress(
            &mut progress,
            ArtifactImportStage::Finalizing,
            0,
            request.manifest.byte_size,
        );
        ensure_not_cancelled(cancellation)?;
        let _verified = verify_stored_file(
            &self.artifacts,
            &destination_name,
            &request.manifest,
            self.limits.maximum_storage_entries,
            cancellation,
        )?;
        self.validate_storage_layout()?;
        ensure_not_cancelled(cancellation)?;
        let state = self
            .store
            .put_installation(&request.manifest, &installed)
            .map_err(ArtifactImportError::State)?;
        Ok(ArtifactImportResult { installed, state })
    }

    fn commit_staged<F>(
        &self,
        mut staged: StagedArtifact<'_>,
        destination_name: &OsStr,
        total_bytes: u64,
        cancellation: &CancellationToken,
        progress: &mut F,
    ) -> Result<(), ArtifactImportError>
    where
        F: FnMut(ArtifactImportProgress),
    {
        let commit = (|| {
            report_progress(
                progress,
                ArtifactImportStage::CommittingFile,
                total_bytes,
                total_bytes,
            );
            ensure_not_cancelled(cancellation)?;
            staged.recheck_frozen_single_link(cancellation)?;
            match self.artifacts.exact_entry_capacity(
                destination_name,
                self.limits.maximum_storage_entries,
                cancellation,
            ) {
                Ok(ExactEntryCapacity::Available) => {}
                Ok(ExactEntryCapacity::Present) => {
                    return Err(ArtifactImportError::StorageChanged);
                }
                Ok(ExactEntryCapacity::Full) => {
                    return Err(ArtifactImportError::StorageEntryLimitExceeded);
                }
                Err(error) => return Err(map_boundary_storage_error(error)),
            }
            staged
                .file_mut()?
                .sync_all()
                .map_err(ArtifactImportError::StorageIo)?;
            match self
                .staging
                .hard_link_to(staged.name(), &self.artifacts, destination_name)
            {
                Ok(()) => self.artifacts.sync().map_err(map_boundary_storage_error),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    Err(ArtifactImportError::StorageChanged)
                }
                Err(error) => Err(ArtifactImportError::StorageIo(error)),
            }
        })();
        let cleanup = staged.cleanup();
        commit?;
        cleanup
    }

    fn validate_storage_layout(&self) -> Result<(), ArtifactImportError> {
        let root = self
            .root
            .fingerprint()
            .map_err(map_boundary_storage_error)?;
        let path_root = PinnedDirectory::fingerprint_path(&self.root_path)
            .map_err(map_boundary_storage_error)?;
        if root != path_root {
            return Err(ArtifactImportError::StorageChanged);
        }
        let lock_path = self
            .root
            .child_file_fingerprint(OsStr::new(LIFECYCLE_LOCK_FILE))
            .map_err(map_boundary_storage_error)?;
        let lock_handle = fingerprint_std_file(&self.lock).map_err(map_boundary_storage_error)?;
        if lock_path != lock_handle {
            return Err(ArtifactImportError::StorageChanged);
        }
        let artifacts = self
            .artifacts
            .fingerprint()
            .map_err(map_boundary_storage_error)?;
        if artifacts
            != self
                .root
                .child_directory_fingerprint(OsStr::new("artifacts"))
                .map_err(map_boundary_storage_error)?
        {
            return Err(ArtifactImportError::StorageChanged);
        }
        let staging = self
            .staging
            .fingerprint()
            .map_err(map_boundary_storage_error)?;
        if staging
            != self
                .root
                .child_directory_fingerprint(OsStr::new(".staging"))
                .map_err(map_boundary_storage_error)?
        {
            return Err(ArtifactImportError::StorageChanged);
        }
        Ok(())
    }

    fn validate_request(
        &self,
        request: &OfflineArtifactImportRequest,
    ) -> Result<(), ArtifactImportError> {
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
        Ok(())
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
    staged: Option<&mut File>,
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
            staged,
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

fn ensure_root_directory(path: &Path) -> Result<(), ArtifactImportError> {
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

#[cfg(test)]
mod tests;
