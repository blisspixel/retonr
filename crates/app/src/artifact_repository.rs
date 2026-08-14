use std::{
    ffi::OsStr,
    fs::{self, File, TryLockError},
    io,
    path::{Path, PathBuf},
};

use rewrite_model::ArtifactManifest;
use rewrite_model_store::{ArtifactRemovalPhase, ArtifactStateStore};
use rewrite_types::CancellationToken;

#[cfg(unix)]
use crate::artifact_storage::set_private_directory_permissions;
use crate::{
    ArtifactImportLimits, ArtifactInventoryError, ArtifactInventoryLimits, ArtifactInventoryReport,
    ArtifactInventoryService, ArtifactOrphanReconciliationRequest,
    ArtifactOrphanReconciliationService, ArtifactReconciliationLimits, ArtifactRemovalDisposition,
    ArtifactRemovalLimits, ArtifactRemovalRequest, ArtifactRemovalService,
    OfflineArtifactImportRequest, OfflineArtifactImportService,
    artifact_storage::{PinnedDirectory, fingerprint_std_file, is_indirect, lock_shared},
};

const MANAGED_STORAGE_DIRECTORY: &str = "artifact-storage";
const STATE_DATABASE_FILE: &str = "artifact-state.sqlite3";
const REPOSITORY_LOCK_FILE: &str = ".artifact-repository.lock";

mod contract;
mod migration;

pub use contract::{
    ArtifactInstallationKey, ArtifactRepositoryBackupKey, ArtifactRepositoryError,
    ArtifactRepositoryErrorKind, ArtifactRepositoryImportDisposition,
    ArtifactRepositoryImportResult, ArtifactRepositoryMigrationDisposition,
    ArtifactRepositoryMigrationLimits, ArtifactRepositoryMigrationResult,
    ArtifactRepositoryPendingOperations, ArtifactRepositoryReconciliationResult,
    ArtifactRepositoryRemovalResult,
};

/// Application-owned entry point for administrative artifact lifecycle operations.
///
/// One caller-selected data directory derives every durable child path. Callers supply
/// exact installation keys, manifests, and import sources, but never state-database
/// paths, managed byte paths, storage keys, or persistence records.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactRepository {
    data_directory: PathBuf,
}

impl ArtifactRepository {
    /// Derives the fixed artifact repository layout without accessing the filesystem.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactRepositoryError::DataDirectoryIo`] when an absolute path
    /// cannot be formed from the supplied data directory.
    pub fn new(data_directory: impl AsRef<Path>) -> Result<Self, ArtifactRepositoryError> {
        let data_directory = std::path::absolute(data_directory.as_ref())
            .map_err(ArtifactRepositoryError::DataDirectoryIo)?;
        Ok(Self { data_directory })
    }

    /// Imports one caller-selected file into initialized or new managed storage.
    ///
    /// This is the only repository operation that initializes the fixed data layout.
    /// The returned state disposition includes the exact durable installation epoch.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactRepositoryError`] when the data directory, state database,
    /// import source, managed storage, or durable registration fails validation.
    pub fn import(
        &self,
        request: &OfflineArtifactImportRequest,
        limits: ArtifactImportLimits,
        cancellation: &CancellationToken,
    ) -> Result<ArtifactRepositoryImportResult, ArtifactRepositoryError> {
        let mut guard = self.initialize_and_lock_data_directory()?;
        guard.recheck()?;
        let result = (|| {
            guard.pin_or_create_state_database()?;
            guard.recheck()?;
            let mut store = self.open_import_store()?;
            guard.recheck()?;
            guard
                .pinned
                .sync()
                .map_err(map_data_directory_boundary_error)?;
            let mut service =
                OfflineArtifactImportService::open(self.managed_storage(), &mut store, limits)?;
            service
                .import(request, cancellation, |_| {})
                .map(ArtifactRepositoryImportResult::from)
                .map_err(ArtifactRepositoryError::Import)
        })();
        finish_operation(result, guard.recheck())
    }

    /// Inspects existing managed artifacts and exact-schema state without mutation.
    ///
    /// This operation creates no directory or database and applies no migration.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactRepositoryError`] when the repository is absent, migration
    /// is required, storage is unsafe or busy, or inventory cannot complete coherently.
    pub fn inventory(
        &self,
        limits: ArtifactInventoryLimits,
        cancellation: &CancellationToken,
    ) -> Result<ArtifactInventoryReport, ArtifactRepositoryError> {
        self.require_data_directory()?;
        let mut guard = self.pin_data_directory(RepositoryLockMode::ExistingShared)?;
        guard.recheck()?;
        let result = (|| {
            guard.pin_state_database()?;
            guard.recheck()?;
            let store = ArtifactStateStore::open_existing_read_only(&self.state_database())?;
            guard.recheck()?;
            let service = ArtifactInventoryService::open(self.managed_storage(), &store, limits)?;
            service
                .inventory(cancellation, |_| {})
                .map_err(ArtifactRepositoryError::Inventory)
        })();
        finish_operation(result, guard.recheck())
    }

    /// Inspects bounded durable state for operations requiring explicit recovery.
    ///
    /// This operation opens no managed artifact file, hashes no model bytes, creates
    /// no repository state, and applies no schema migration.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactRepositoryError`] when the repository is absent, busy,
    /// incompatible, corrupt, changed during inspection, cancelled, or exceeds the
    /// caller-owned state-entry ceiling.
    pub fn pending_operations(
        &self,
        maximum_state_entries: usize,
        cancellation: &CancellationToken,
    ) -> Result<ArtifactRepositoryPendingOperations, ArtifactRepositoryError> {
        if maximum_state_entries == 0
            || maximum_state_entries
                .checked_add(1)
                .and_then(|value| i64::try_from(value).ok())
                .is_none()
        {
            return Err(ArtifactRepositoryError::InvalidLimits);
        }
        ensure_repository_not_cancelled(cancellation)?;
        self.require_data_directory()?;
        let mut guard = self.pin_data_directory(RepositoryLockMode::ExistingShared)?;
        guard.recheck()?;
        let result = (|| {
            ensure_repository_not_cancelled(cancellation)?;
            guard.pin_state_database()?;
            guard.recheck()?;
            let store = ArtifactStateStore::open_existing_read_only(&self.state_database())?;
            guard.recheck()?;
            ensure_repository_not_cancelled(cancellation)?;
            let artifact_removals = store
                .pending_artifact_removals(maximum_state_entries)?
                .iter()
                .map(ArtifactInstallationKey::from_stored)
                .collect();
            ensure_repository_not_cancelled(cancellation)?;
            Ok(ArtifactRepositoryPendingOperations { artifact_removals })
        })();
        finish_operation(result, guard.recheck())
    }

    /// Reverifies and registers one exact existing orphan selected only by manifest.
    ///
    /// This operation creates no repository layout. The result retains the exact
    /// state-store-issued installation epoch.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactRepositoryError`] when existing state or storage cannot be
    /// opened safely, or the selected orphan cannot be verified and registered.
    pub fn reconcile(
        &self,
        manifest: ArtifactManifest,
        limits: ArtifactReconciliationLimits,
        cancellation: &CancellationToken,
    ) -> Result<ArtifactRepositoryReconciliationResult, ArtifactRepositoryError> {
        self.require_data_directory()?;
        let mut guard = self.pin_data_directory(RepositoryLockMode::ExistingExclusive)?;
        guard.recheck()?;
        let result = (|| {
            guard.pin_state_database()?;
            guard.recheck()?;
            let mut store =
                ArtifactStateStore::open_existing_writable_exact(&self.state_database())?;
            guard.recheck()?;
            let mut service = ArtifactOrphanReconciliationService::open_existing(
                self.managed_storage(),
                &mut store,
                limits,
            )?;
            service
                .reconcile(
                    &ArtifactOrphanReconciliationRequest { manifest },
                    cancellation,
                    |_| {},
                )
                .map(ArtifactRepositoryReconciliationResult::from)
                .map_err(ArtifactRepositoryError::Reconciliation)
        })();
        finish_operation(result, guard.recheck())
    }

    /// Removes one exact current inactive installation generation.
    ///
    /// A prepared removal is never resumed implicitly. Call [`Self::recover_removal`]
    /// to make the non-cancellable recovery boundary explicit.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactRepositoryError::RemovalRecoveryPending`] when an exact
    /// prepared journal exists, or another [`ArtifactRepositoryError`] on failure.
    pub fn remove(
        &self,
        key: &ArtifactInstallationKey,
        limits: ArtifactRemovalLimits,
        cancellation: &CancellationToken,
    ) -> Result<ArtifactRepositoryRemovalResult, ArtifactRepositoryError> {
        self.require_data_directory()?;
        key.validate()?;
        let mut guard = self.pin_data_directory(RepositoryLockMode::ExistingExclusive)?;
        guard.recheck()?;
        let result = (|| {
            guard.pin_state_database()?;
            guard.recheck()?;
            let mut store =
                ArtifactStateStore::open_existing_writable_exact(&self.state_database())?;
            guard.recheck()?;
            let (current, removal) = store.artifact_removal_state(key.artifact_id())?;
            if let Some(removal) = removal.as_ref()
                && ArtifactInstallationKey::from_stored(&removal.selection) == *key
            {
                if removal.phase == ArtifactRemovalPhase::Prepared {
                    return Err(ArtifactRepositoryError::RemovalRecoveryPending {
                        key: key.clone(),
                    });
                }
                return Ok(ArtifactRepositoryRemovalResult {
                    key: key.clone(),
                    disposition: ArtifactRemovalDisposition::AlreadyRemoved,
                });
            }
            let selection = current.ok_or(ArtifactRepositoryError::ArtifactNotInstalled)?;
            if ArtifactInstallationKey::from_stored(&selection) != *key {
                return Err(ArtifactRepositoryError::StaleInstallation);
            }
            let mut service =
                ArtifactRemovalService::open_existing(self.managed_storage(), &mut store, limits)?;
            service
                .remove(&ArtifactRemovalRequest { selection }, cancellation, |_| {})
                .map(ArtifactRepositoryRemovalResult::from)
                .map_err(|error| map_removal_error(key, error))
        })();
        finish_operation(result, guard.recheck())
    }

    /// Forward-completes one exact durably prepared removal generation.
    ///
    /// Recovery intentionally ignores cancellation and emits no progress after the
    /// prior durable preparation. The lower-level service enforces that invariant.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactRepositoryError::RemovalRecoveryNotPending`] unless the
    /// artifact has one exact Prepared journal and no current installation.
    pub fn recover_removal(
        &self,
        key: &ArtifactInstallationKey,
        limits: ArtifactRemovalLimits,
    ) -> Result<ArtifactRepositoryRemovalResult, ArtifactRepositoryError> {
        self.require_data_directory()?;
        key.validate()?;
        let mut guard = self.pin_data_directory(RepositoryLockMode::ExistingExclusive)?;
        guard.recheck()?;
        let result = (|| {
            guard.pin_state_database()?;
            guard.recheck()?;
            let mut store =
                ArtifactStateStore::open_existing_writable_exact(&self.state_database())?;
            guard.recheck()?;
            let (current, removal) = store.artifact_removal_state(key.artifact_id())?;
            let removal = removal.ok_or(ArtifactRepositoryError::RemovalRecoveryNotPending)?;
            if ArtifactInstallationKey::from_stored(&removal.selection) != *key {
                return Err(ArtifactRepositoryError::StaleInstallation);
            }
            if removal.phase == ArtifactRemovalPhase::Completed {
                return Ok(ArtifactRepositoryRemovalResult {
                    key: key.clone(),
                    disposition: ArtifactRemovalDisposition::AlreadyRemoved,
                });
            }
            if current.is_some() {
                return Err(ArtifactRepositoryError::RemovalRecoveryNotPending);
            }
            let mut service =
                ArtifactRemovalService::open_existing(self.managed_storage(), &mut store, limits)?;
            service
                .remove(
                    &ArtifactRemovalRequest {
                        selection: removal.selection,
                    },
                    &CancellationToken::new(),
                    |_| {},
                )
                .map(ArtifactRepositoryRemovalResult::from)
                .map_err(|error| map_removal_error(key, error))
        })();
        finish_operation(result, guard.recheck())
    }

    fn initialize_and_lock_data_directory(
        &self,
    ) -> Result<DataDirectoryGuard, ArtifactRepositoryError> {
        let mut created = false;
        match fs::symlink_metadata(&self.data_directory) {
            Ok(metadata) if metadata.is_dir() && !is_indirect(&metadata) => {}
            Ok(_) => return Err(ArtifactRepositoryError::UnsafeDataDirectory),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.create_data_directory()?;
                created = true;
            }
            Err(error) => return Err(ArtifactRepositoryError::DataDirectoryIo(error)),
        }
        if !created {
            let existing = PinnedDirectory::open_existing(&self.data_directory)
                .map_err(map_data_directory_boundary_error)?;
            let initialized =
                existing.has_direct_regular_child(OsStr::new(REPOSITORY_LOCK_FILE))?;
            if !initialized && !existing.is_empty()? {
                return Err(ArtifactRepositoryError::UnsafeDataDirectory);
            }
        }
        let guard = self.pin_data_directory(RepositoryLockMode::InitializeExclusive)?;
        #[cfg(unix)]
        if created {
            set_private_directory_permissions(&guard.pinned)
                .map_err(map_data_directory_boundary_error)?;
        }
        guard
            .pinned
            .sync()
            .map_err(map_data_directory_boundary_error)?;
        Ok(guard)
    }

    fn require_data_directory(&self) -> Result<(), ArtifactRepositoryError> {
        match fs::symlink_metadata(&self.data_directory) {
            Ok(metadata) if metadata.is_dir() && !is_indirect(&metadata) => Ok(()),
            Ok(_) => Err(ArtifactRepositoryError::UnsafeDataDirectory),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                Err(ArtifactRepositoryError::NotInitialized)
            }
            Err(error) => Err(ArtifactRepositoryError::DataDirectoryIo(error)),
        }
    }

    fn pin_data_directory(
        &self,
        mode: RepositoryLockMode,
    ) -> Result<DataDirectoryGuard, ArtifactRepositoryError> {
        let pinned = PinnedDirectory::open_existing(&self.data_directory)
            .map_err(map_data_directory_boundary_error)?;
        pinned
            .fingerprint()
            .map_err(map_data_directory_boundary_error)?;
        let (lock, lock_fingerprint) = match mode {
            RepositoryLockMode::InitializeExclusive => {
                pinned.open_or_create_lock_file(OsStr::new(REPOSITORY_LOCK_FILE))
            }
            RepositoryLockMode::ExistingShared | RepositoryLockMode::ExistingExclusive => {
                pinned.open_lock_file(OsStr::new(REPOSITORY_LOCK_FILE))
            }
        }
        .map_err(map_data_directory_boundary_error)?;
        match mode {
            RepositoryLockMode::ExistingShared => {
                lock_shared(&lock).map_err(map_data_directory_boundary_error)?;
            }
            RepositoryLockMode::InitializeExclusive | RepositoryLockMode::ExistingExclusive => {
                lock_exclusive(&lock)?;
            }
        }
        if mode == RepositoryLockMode::InitializeExclusive {
            pinned.sync().map_err(map_data_directory_boundary_error)?;
        }
        Ok(DataDirectoryGuard {
            path: self.data_directory.clone(),
            lock,
            lock_fingerprint,
            pinned,
            state_database: None,
        })
    }

    fn managed_storage(&self) -> PathBuf {
        self.data_directory.join(MANAGED_STORAGE_DIRECTORY)
    }

    fn state_database(&self) -> PathBuf {
        self.data_directory.join(STATE_DATABASE_FILE)
    }

    fn create_data_directory(&self) -> Result<(), ArtifactRepositoryError> {
        let parent = self
            .data_directory
            .parent()
            .ok_or(ArtifactRepositoryError::UnsafeDataDirectory)?;
        let name = self
            .data_directory
            .file_name()
            .ok_or(ArtifactRepositoryError::UnsafeDataDirectory)?;
        let parent =
            PinnedDirectory::open_existing(parent).map_err(map_data_directory_boundary_error)?;
        let created = parent
            .create_child_directory_exclusive(name)
            .map_err(map_data_directory_boundary_error)?;
        created.sync().map_err(map_data_directory_boundary_error)?;
        parent.sync().map_err(map_data_directory_boundary_error)
    }

    fn open_import_store(&self) -> Result<ArtifactStateStore, ArtifactRepositoryError> {
        ArtifactStateStore::open_existing_writable_exact(&self.state_database())
            .or_else(|error| match error {
                rewrite_model_store::StoreError::MigrationRequired { found: 0, .. } => {
                    ArtifactStateStore::open_existing_or_initialize_empty(&self.state_database())
                }
                other => Err(other),
            })
            .map_err(ArtifactRepositoryError::State)
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum RepositoryLockMode {
    InitializeExclusive,
    ExistingShared,
    ExistingExclusive,
}

struct DataDirectoryGuard {
    path: PathBuf,
    lock: File,
    lock_fingerprint: crate::artifact_storage::MetadataFingerprint,
    pinned: PinnedDirectory,
    state_database: Option<(File, crate::artifact_storage::MetadataFingerprint)>,
}

impl DataDirectoryGuard {
    fn pin_state_database(&mut self) -> Result<(), ArtifactRepositoryError> {
        let state_database = self
            .pinned
            .open_lock_file(OsStr::new(STATE_DATABASE_FILE))
            .map_err(map_data_directory_boundary_error)?;
        if !state_database.1.has_single_link() {
            return Err(ArtifactRepositoryError::UnsafeDataDirectory);
        }
        self.state_database = Some(state_database);
        Ok(())
    }

    fn pin_or_create_state_database(&mut self) -> Result<(), ArtifactRepositoryError> {
        let state_database = self
            .pinned
            .open_or_create_lock_file(OsStr::new(STATE_DATABASE_FILE))
            .map_err(map_data_directory_boundary_error)?;
        if !state_database.1.has_single_link() {
            return Err(ArtifactRepositoryError::UnsafeDataDirectory);
        }
        self.pinned
            .sync()
            .map_err(map_data_directory_boundary_error)?;
        self.state_database = Some(state_database);
        Ok(())
    }

    fn recheck(&self) -> Result<(), ArtifactRepositoryError> {
        let held_lock =
            fingerprint_std_file(&self.lock).map_err(map_data_directory_boundary_error)?;
        let current_lock = self
            .pinned
            .child_file_fingerprint(OsStr::new(REPOSITORY_LOCK_FILE))
            .map_err(map_data_directory_boundary_error)?;
        let pinned = self
            .pinned
            .fingerprint()
            .map_err(map_data_directory_boundary_error)?;
        let current = PinnedDirectory::fingerprint_path(&self.path)
            .map_err(map_data_directory_boundary_error)?;
        let state_matches = self
            .state_database
            .as_ref()
            .is_none_or(|(file, fingerprint)| {
                fingerprint_std_file(file)
                    .is_ok_and(|held| held.same_identity(fingerprint) && held.has_single_link())
                    && self
                        .pinned
                        .child_file_fingerprint(OsStr::new(STATE_DATABASE_FILE))
                        .is_ok_and(|current| {
                            current.same_identity(fingerprint) && current.has_single_link()
                        })
            });
        if held_lock == self.lock_fingerprint
            && current_lock == self.lock_fingerprint
            && pinned == current
            && state_matches
        {
            Ok(())
        } else {
            Err(ArtifactRepositoryError::UnsafeDataDirectory)
        }
    }
}

fn map_data_directory_boundary_error(error: ArtifactInventoryError) -> ArtifactRepositoryError {
    match error {
        ArtifactInventoryError::StorageInUse => ArtifactRepositoryError::RepositoryInUse,
        ArtifactInventoryError::StorageIo(error) => ArtifactRepositoryError::DataDirectoryIo(error),
        _ => ArtifactRepositoryError::UnsafeDataDirectory,
    }
}

fn lock_exclusive(file: &File) -> Result<(), ArtifactRepositoryError> {
    match file.try_lock() {
        Ok(()) => Ok(()),
        Err(TryLockError::WouldBlock) => Err(ArtifactRepositoryError::RepositoryInUse),
        Err(TryLockError::Error(error)) => Err(ArtifactRepositoryError::DataDirectoryIo(error)),
    }
}

fn map_removal_error(
    key: &ArtifactInstallationKey,
    error: crate::ArtifactRemovalError,
) -> ArtifactRepositoryError {
    if matches!(error, crate::ArtifactRemovalError::RecoveryRequired(_)) {
        ArtifactRepositoryError::RemovalRecoveryRequired {
            key: key.clone(),
            source: error,
        }
    } else {
        ArtifactRepositoryError::Removal(error)
    }
}

fn finish_operation<T>(
    result: Result<T, ArtifactRepositoryError>,
    boundary: Result<(), ArtifactRepositoryError>,
) -> Result<T, ArtifactRepositoryError> {
    match (result, boundary) {
        (Err(error @ ArtifactRepositoryError::RemovalRecoveryRequired { .. }), Err(_)) => {
            Err(error)
        }
        (_, Err(error)) => Err(error),
        (result, Ok(())) => result,
    }
}

fn ensure_repository_not_cancelled(
    cancellation: &CancellationToken,
) -> Result<(), ArtifactRepositoryError> {
    if cancellation.is_cancelled() {
        Err(ArtifactRepositoryError::Cancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
