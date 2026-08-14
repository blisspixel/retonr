use std::{ffi::OsStr, fs::File};

use rewrite_model_store::{
    ArtifactStateStore, ExistingStoreMigration, StoreError, StoreMigrationDisposition,
};
use rewrite_types::CancellationToken;

use super::{
    ArtifactRepository, ArtifactRepositoryBackupKey, ArtifactRepositoryError,
    ArtifactRepositoryMigrationDisposition, ArtifactRepositoryMigrationLimits,
    ArtifactRepositoryMigrationResult, RepositoryLockMode, map_data_directory_boundary_error,
};
use crate::artifact_storage::{
    ExistingArtifactStorage, LifecycleLockMode, MetadataFingerprint, fingerprint_std_file,
};

const BACKUP_FILE_PREFIX: &str = ".artifact-state-backup-";

impl ArtifactRepository {
    /// Explicitly migrates one existing repository after retaining a verified backup.
    ///
    /// A current-schema repository is an idempotent no-op and creates no backup.
    /// Older supported schemas are backed up through `SQLite`, synced, verified, and
    /// then migrated in one lower-level transaction. Cancellation is honored until
    /// the final migration boundary and ignored after that boundary is crossed.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactRepositoryError`] when limits are invalid, the repository
    /// is absent or busy, source state is unsupported or corrupt, backup creation or
    /// verification fails, cancellation is observed before migration, or a boundary
    /// changes. A post-backup failure retains and reports the backup identity.
    pub fn migrate(
        &self,
        limits: ArtifactRepositoryMigrationLimits,
        cancellation: &CancellationToken,
    ) -> Result<ArtifactRepositoryMigrationResult, ArtifactRepositoryError> {
        validate_limits(limits)?;
        ensure_not_cancelled(cancellation)?;
        self.require_data_directory()?;
        let mut guard = self.pin_data_directory(RepositoryLockMode::ExistingExclusive)?;
        guard.pin_state_database()?;
        guard.recheck()?;
        validate_state_size(&guard, limits.maximum_state_bytes)?;
        let storage =
            ExistingArtifactStorage::open(self.managed_storage(), LifecycleLockMode::Exclusive)
                .map_err(map_data_directory_boundary_error)?;
        storage
            .validate_layout()
            .map_err(map_data_directory_boundary_error)?;
        let migration = ArtifactStateStore::begin_existing_migration(&self.state_database())?;
        let status = migration.schema_status();
        guard.recheck()?;
        ensure_not_cancelled(cancellation)?;
        if status.found == status.current {
            storage
                .validate_layout()
                .map_err(map_data_directory_boundary_error)?;
            guard.recheck()?;
            return Ok(ArtifactRepositoryMigrationResult {
                from_schema: status.found,
                to_schema: status.current,
                disposition: ArtifactRepositoryMigrationDisposition::AlreadyCurrent,
                backup_key: None,
            });
        }

        migrate_legacy_repository(
            self,
            &guard,
            &storage,
            migration,
            status,
            limits,
            cancellation,
        )
    }
}

fn migrate_legacy_repository(
    repository: &ArtifactRepository,
    guard: &super::DataDirectoryGuard,
    storage: &ExistingArtifactStorage,
    mut migration: ExistingStoreMigration,
    status: rewrite_model_store::StoreSchemaStatus,
    limits: ArtifactRepositoryMigrationLimits,
    cancellation: &CancellationToken,
) -> Result<ArtifactRepositoryMigrationResult, ArtifactRepositoryError> {
    let (backup_name, mut backup_file) = guard.pinned.create_staging_file(
        BACKUP_FILE_PREFIX,
        limits.maximum_repository_entries,
        cancellation,
    )?;
    let backup_file_name = backup_name
        .into_string()
        .map_err(|_| ArtifactRepositoryError::UnsafeDataDirectory)?;
    let backup_key = ArtifactRepositoryBackupKey::from_file_name(backup_file_name.clone());
    let backup_result = prepare_backup(
        &mut migration,
        &mut backup_file,
        limits.maximum_state_bytes,
        cancellation,
    );
    let backup_fingerprint = match backup_result {
        Ok(fingerprint) => fingerprint,
        Err(error) => {
            return cleanup_failed_backup(
                guard,
                OsStr::new(&backup_file_name),
                &backup_file,
                error,
            );
        }
    };
    let pre_migration = sync_and_recheck_backup(
        guard,
        storage,
        &backup_file_name,
        &backup_file,
        &backup_fingerprint,
        cancellation,
    );
    if let Err(error) = pre_migration {
        return cleanup_failed_backup(guard, OsStr::new(&backup_file_name), &backup_file, error);
    }
    commit_migration(
        repository,
        guard,
        storage,
        migration,
        status,
        PreparedBackup {
            file_name: backup_file_name,
            file: backup_file,
            fingerprint: backup_fingerprint,
            key: backup_key,
        },
    )
}

fn sync_and_recheck_backup(
    guard: &super::DataDirectoryGuard,
    storage: &ExistingArtifactStorage,
    backup_file_name: &str,
    backup_file: &File,
    backup_fingerprint: &MetadataFingerprint,
    cancellation: &CancellationToken,
) -> Result<(), ArtifactRepositoryError> {
    guard
        .pinned
        .sync()
        .map_err(map_data_directory_boundary_error)?;
    ensure_not_cancelled(cancellation)?;
    guard.recheck()?;
    storage
        .validate_layout()
        .map_err(map_data_directory_boundary_error)?;
    validate_backup_binding(
        guard,
        OsStr::new(backup_file_name),
        backup_file,
        backup_fingerprint,
    )
}

struct PreparedBackup {
    file_name: String,
    file: File,
    fingerprint: MetadataFingerprint,
    key: ArtifactRepositoryBackupKey,
}

fn commit_migration(
    repository: &ArtifactRepository,
    guard: &super::DataDirectoryGuard,
    storage: &ExistingArtifactStorage,
    migration: ExistingStoreMigration,
    status: rewrite_model_store::StoreSchemaStatus,
    backup: PreparedBackup,
) -> Result<ArtifactRepositoryMigrationResult, ArtifactRepositoryError> {
    let migration = migration
        .migrate()
        .map_err(|error| retain_backup(backup.key.clone(), error.into()))?;
    if migration.from_schema != status.found
        || migration.to_schema != status.current
        || migration.disposition != StoreMigrationDisposition::Migrated
    {
        return Err(retain_backup(
            backup.key,
            ArtifactRepositoryError::State(StoreError::CorruptRecord),
        ));
    }
    ArtifactStateStore::open_existing_read_only(&repository.state_database())
        .map_err(|error| retain_backup(backup.key.clone(), error.into()))?;
    guard
        .recheck()
        .map_err(|error| retain_backup(backup.key.clone(), error))?;
    storage
        .validate_layout()
        .map_err(map_data_directory_boundary_error)
        .map_err(|error| retain_backup(backup.key.clone(), error))?;
    validate_backup_binding(
        guard,
        OsStr::new(&backup.file_name),
        &backup.file,
        &backup.fingerprint,
    )
    .map_err(|error| retain_backup(backup.key.clone(), error))?;
    Ok(ArtifactRepositoryMigrationResult {
        from_schema: migration.from_schema,
        to_schema: migration.to_schema,
        disposition: ArtifactRepositoryMigrationDisposition::Migrated,
        backup_key: Some(backup.key),
    })
}

fn prepare_backup(
    migration: &mut ExistingStoreMigration,
    destination_file: &mut File,
    maximum_bytes: u64,
    cancellation: &CancellationToken,
) -> Result<MetadataFingerprint, ArtifactRepositoryError> {
    migration
        .backup_to(destination_file, maximum_bytes, || {
            cancellation.is_cancelled()
        })
        .map_err(|error| match error {
            StoreError::BackupCancelled => ArtifactRepositoryError::Cancelled,
            other => ArtifactRepositoryError::State(other),
        })?;
    let fingerprint =
        fingerprint_std_file(destination_file).map_err(map_data_directory_boundary_error)?;
    if !fingerprint.has_single_link() {
        return Err(ArtifactRepositoryError::UnsafeDataDirectory);
    }
    Ok(fingerprint)
}

fn cleanup_failed_backup<T>(
    guard: &super::DataDirectoryGuard,
    name: &OsStr,
    file: &File,
    original: ArtifactRepositoryError,
) -> Result<T, ArtifactRepositoryError> {
    guard
        .pinned
        .remove_file_if_same_identity(name, file)
        .map_err(map_data_directory_boundary_error)?;
    guard
        .pinned
        .sync()
        .map_err(map_data_directory_boundary_error)?;
    Err(original)
}

fn validate_backup_binding(
    guard: &super::DataDirectoryGuard,
    name: &OsStr,
    file: &File,
    expected: &MetadataFingerprint,
) -> Result<(), ArtifactRepositoryError> {
    let held = fingerprint_std_file(file).map_err(map_data_directory_boundary_error)?;
    let current = guard
        .pinned
        .child_file_fingerprint(name)
        .map_err(map_data_directory_boundary_error)?;
    if &held == expected && &current == expected && held.has_single_link() {
        Ok(())
    } else {
        Err(ArtifactRepositoryError::UnsafeDataDirectory)
    }
}

fn validate_state_size(
    guard: &super::DataDirectoryGuard,
    maximum_bytes: u64,
) -> Result<(), ArtifactRepositoryError> {
    let Some((file, _)) = guard.state_database.as_ref() else {
        return Err(ArtifactRepositoryError::UnsafeDataDirectory);
    };
    if file
        .metadata()
        .map_err(ArtifactRepositoryError::DataDirectoryIo)?
        .len()
        > maximum_bytes
    {
        Err(ArtifactRepositoryError::MigrationLimitExceeded)
    } else {
        Ok(())
    }
}

fn validate_limits(
    limits: ArtifactRepositoryMigrationLimits,
) -> Result<(), ArtifactRepositoryError> {
    if limits.maximum_state_bytes == 0 || limits.maximum_repository_entries == 0 {
        Err(ArtifactRepositoryError::InvalidLimits)
    } else {
        Ok(())
    }
}

fn ensure_not_cancelled(cancellation: &CancellationToken) -> Result<(), ArtifactRepositoryError> {
    if cancellation.is_cancelled() {
        Err(ArtifactRepositoryError::Cancelled)
    } else {
        Ok(())
    }
}

fn retain_backup(
    backup_key: ArtifactRepositoryBackupKey,
    source: ArtifactRepositoryError,
) -> ArtifactRepositoryError {
    ArtifactRepositoryError::MigrationFailed {
        backup_key,
        source: Box::new(source),
    }
}
