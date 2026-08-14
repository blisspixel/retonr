use std::{
    fs::{File, Metadata},
    io::{Seek as _, SeekFrom, Write as _},
    path::Path,
};

use rusqlite::{
    Connection, MAIN_DB,
    backup::{Backup, StepResult},
};

use crate::{ArtifactStateStore, StoreError, StoreResult, schema};

const BACKUP_WRITE_CHUNK_BYTES: usize = 64 * 1024;
const BACKUP_PAGES_PER_STEP: i32 = 128;
const MAXIMUM_TRANSIENT_BACKUP_STEPS: u64 = 1_024;

/// Supported schema version observed in one integrity-validated state database.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StoreSchemaStatus {
    /// Exact supported schema found in the existing database.
    pub found: u32,
    /// Current schema required by this adapter.
    pub current: u32,
}

impl StoreSchemaStatus {
    /// Returns whether the supported existing schema requires migration.
    #[must_use]
    pub const fn migration_required(self) -> bool {
        self.found != self.current
    }
}

/// Outcome of one explicit existing-state schema migration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreMigrationDisposition {
    /// The database already used the current exact schema.
    AlreadyCurrent,
    /// A supported older schema was migrated atomically.
    Migrated,
}

/// Content-free result of one explicit existing-state schema migration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StoreMigrationResult {
    /// Supported schema validated when the migration session began.
    pub from_schema: u32,
    /// Exact schema validated after the migration transaction.
    pub to_schema: u32,
    /// Whether the transaction changed the schema.
    pub disposition: StoreMigrationDisposition,
}

/// Exclusive, transaction-bound authority for one existing state migration.
///
/// Construction opens the exact existing database writable without following its
/// final path entry, starts `BEGIN IMMEDIATE`, and validates its supported schema.
/// The session retains that write reservation until [`Self::migrate`] commits or
/// dropping the session rolls the transaction back. It cannot be cloned.
#[must_use = "dropping a migration session rolls its transaction back"]
pub struct ExistingStoreMigration {
    connection: Connection,
    backup_source: Connection,
    status: StoreSchemaStatus,
    backup_complete: bool,
    transaction_active: bool,
}

impl ArtifactStateStore {
    /// Inspects and validates one existing supported state schema without mutation.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::NotInitialized`] when the path is absent,
    /// [`StoreError::MigrationRequired`] for version zero, or another
    /// [`StoreError`] when the schema is future, corrupt, or cannot be read.
    pub fn inspect_existing_schema(path: &Path) -> StoreResult<StoreSchemaStatus> {
        let connection = super::store::open_existing(path, super::store::read_only_flags())?;
        inspect_connection(&connection)
    }

    /// Starts one exclusive, transaction-bound migration session.
    ///
    /// This method never creates state or changes its schema. The returned session
    /// retains a `SQLite` write reservation and rolls back if dropped before commit.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::NotInitialized`] for an absent database,
    /// [`StoreError::MigrationRequired`] for version zero, or another
    /// [`StoreError`] when the schema is future, corrupt, busy, or cannot be read.
    pub fn begin_existing_migration(path: &Path) -> StoreResult<ExistingStoreMigration> {
        let connection = super::store::open_existing(path, super::store::writable_flags())?;
        schema::configure(&connection, true)?;
        connection.execute_batch("BEGIN IMMEDIATE")?;
        let session_state = (|| {
            let status = inspect_connection(&connection)?;
            let backup_source = super::store::open_existing(path, super::store::read_only_flags())?;
            if inspect_connection(&backup_source)? != status {
                return Err(StoreError::CorruptRecord);
            }
            Ok((status, backup_source))
        })();
        match session_state {
            Ok((status, backup_source)) => Ok(ExistingStoreMigration {
                connection,
                backup_source,
                status,
                backup_complete: false,
                transaction_active: true,
            }),
            Err(error) => {
                let _rollback_result = connection.execute_batch("ROLLBACK");
                Err(error)
            }
        }
    }
}

impl ExistingStoreMigration {
    /// Returns the exact supported schema bound inside this session's transaction.
    #[must_use]
    pub const fn schema_status(&self) -> StoreSchemaStatus {
        self.status
    }

    /// Copies and serializes locked logical state into the caller-held empty file.
    ///
    /// A bounded online backup first captures the complete logical source, including
    /// committed WAL state, in a temporary non-WAL in-memory database. The destination
    /// must be a regular, single-link, empty file opened for read and write. Normalized
    /// rollback-mode bytes are written to that handle in fixed chunks, synchronized,
    /// then read back and deserialized from the same handle for schema and integrity
    /// validation. No destination pathname is accepted or reopened.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the destination is invalid, serialization or file
    /// I/O fails, the logical snapshot exceeds `maximum_bytes`, cancellation is
    /// requested between bounded page or write steps, or read-back validation fails.
    pub fn backup_to(
        &mut self,
        destination: &mut File,
        maximum_bytes: u64,
        mut cancel_check: impl FnMut() -> bool,
    ) -> StoreResult<u64> {
        self.backup_complete = false;
        if maximum_bytes == 0 {
            return Err(StoreError::InvalidLimit);
        }
        require_destination(destination, 0)?;
        if cancel_check() {
            return Err(StoreError::BackupCancelled);
        }
        let (snapshot, snapshot_bytes) =
            snapshot_locked_source(&self.backup_source, maximum_bytes, &mut cancel_check)?;
        let serialized = snapshot.serialize(MAIN_DB)?;
        let serialized_bytes =
            u64::try_from(serialized.len()).map_err(|_| StoreError::BackupTooLarge)?;
        require_within_backup_limit(serialized_bytes, maximum_bytes)?;
        if serialized_bytes != snapshot_bytes {
            return Err(StoreError::CorruptRecord);
        }
        destination
            .seek(SeekFrom::Start(0))
            .map_err(StoreError::BackupIo)?;
        write_rollback_image(destination, &serialized, &mut cancel_check)?;
        drop(serialized);
        destination.sync_all().map_err(StoreError::BackupIo)?;
        require_destination(destination, serialized_bytes)?;
        if cancel_check() {
            return Err(StoreError::BackupCancelled);
        }
        let actual = inspect_serialized_backup(destination, serialized_bytes)?;
        if actual != self.status {
            return Err(StoreError::CorruptRecord);
        }
        require_destination(destination, serialized_bytes)?;
        self.backup_complete = true;
        Ok(serialized_bytes)
    }

    /// Applies and commits the supported migration bound to this session.
    ///
    /// The migration runs inside the same transaction that established the write
    /// reservation before backup. Current shape and database integrity are validated
    /// before `COMMIT`. Consuming a session without a completed verified backup fails
    /// and rolls its transaction back.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::BackupRequired`] until [`Self::backup_to`] succeeds, or
    /// another [`StoreError`] if migration validation or commit fails.
    pub fn migrate(mut self) -> StoreResult<StoreMigrationResult> {
        if !self.backup_complete {
            return Err(StoreError::BackupRequired);
        }
        schema::migrate_existing_transaction(&self.connection, i64::from(self.status.found))?;
        self.connection.execute_batch("COMMIT")?;
        self.transaction_active = false;
        Ok(StoreMigrationResult {
            from_schema: self.status.found,
            to_schema: self.status.current,
            disposition: if self.status.migration_required() {
                StoreMigrationDisposition::Migrated
            } else {
                StoreMigrationDisposition::AlreadyCurrent
            },
        })
    }
}

impl Drop for ExistingStoreMigration {
    fn drop(&mut self) {
        if self.transaction_active {
            let _rollback_result = self.connection.execute_batch("ROLLBACK");
        }
    }
}

fn inspect_connection(connection: &Connection) -> StoreResult<StoreSchemaStatus> {
    schema::configure(connection, false)?;
    crate::integrity::validate_database_integrity(connection)?;
    let found: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    match found {
        1 => schema::validate_schema_one(connection)?,
        2 => schema::validate_schema_two(connection)?,
        schema::STORE_SCHEMA_VERSION => schema::validate_schema_shape(connection)?,
        0 => {
            return Err(StoreError::MigrationRequired {
                found,
                current: schema::STORE_SCHEMA_VERSION,
            });
        }
        value if !(0..=schema::STORE_SCHEMA_VERSION).contains(&value) => {
            return Err(StoreError::UnsupportedSchema(value));
        }
        value => {
            return Err(StoreError::MigrationRequired {
                found: value,
                current: schema::STORE_SCHEMA_VERSION,
            });
        }
    }
    Ok(StoreSchemaStatus {
        found: u32::try_from(found).map_err(|_| StoreError::CorruptRecord)?,
        current: u32::try_from(schema::STORE_SCHEMA_VERSION)
            .map_err(|_| StoreError::CorruptRecord)?,
    })
}

fn inspect_serialized_backup(
    destination: &mut File,
    serialized_bytes: u64,
) -> StoreResult<StoreSchemaStatus> {
    let serialized_bytes =
        usize::try_from(serialized_bytes).map_err(|_| StoreError::BackupTooLarge)?;
    destination
        .seek(SeekFrom::Start(0))
        .map_err(StoreError::BackupIo)?;
    let mut connection = Connection::open_in_memory()?;
    connection.deserialize_read_exact(MAIN_DB, destination, serialized_bytes, true)?;
    inspect_connection(&connection)
}

fn snapshot_locked_source(
    source: &Connection,
    maximum_bytes: u64,
    cancel_check: &mut impl FnMut() -> bool,
) -> StoreResult<(Connection, u64)> {
    let page_size = database_page_size(source)?;
    require_within_backup_limit(database_extent(source, page_size)?, maximum_bytes)?;
    if cancel_check() {
        return Err(StoreError::BackupCancelled);
    }

    let mut snapshot = Connection::open_in_memory()?;
    schema::configure(&snapshot, true)?;
    let journal_mode: String =
        snapshot.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
    if journal_mode != "memory" {
        return Err(StoreError::CorruptRecord);
    }
    let completed_bytes;
    {
        let backup = Backup::new(source, &mut snapshot)?;
        let maximum_steps = maximum_backup_steps(maximum_bytes, page_size);
        let mut steps = 0u64;
        loop {
            if cancel_check() {
                return Err(StoreError::BackupCancelled);
            }
            if steps >= maximum_steps {
                return Err(StoreError::BackupIncomplete);
            }
            steps = steps.saturating_add(1);
            let result = backup.step(BACKUP_PAGES_PER_STEP)?;
            let progress = backup.progress();
            let observed_bytes = progress_extent(i64::from(progress.pagecount), page_size)?;
            require_within_backup_limit(observed_bytes, maximum_bytes)?;
            match result {
                StepResult::Done => {
                    if progress.remaining != 0 {
                        return Err(StoreError::CorruptRecord);
                    }
                    completed_bytes = observed_bytes;
                    break;
                }
                StepResult::More => {}
                StepResult::Busy | StepResult::Locked => std::thread::yield_now(),
                _ => return Err(StoreError::BackupIncomplete),
            }
        }
    }
    Ok((snapshot, completed_bytes))
}

fn write_rollback_image(
    destination: &mut File,
    serialized: &[u8],
    cancel_check: &mut impl FnMut() -> bool,
) -> StoreResult<()> {
    const SQLITE_HEADER: &[u8; 16] = b"SQLite format 3\0";
    const WRITE_VERSION_OFFSET: usize = 18;
    const BODY_OFFSET: usize = 20;
    if serialized.len() < BODY_OFFSET || &serialized[..SQLITE_HEADER.len()] != SQLITE_HEADER {
        return Err(StoreError::CorruptRecord);
    }
    write_backup_chunk(
        destination,
        &serialized[..WRITE_VERSION_OFFSET],
        cancel_check,
    )?;
    write_backup_chunk(destination, &[1, 1], cancel_check)?;
    for chunk in serialized[BODY_OFFSET..].chunks(BACKUP_WRITE_CHUNK_BYTES) {
        write_backup_chunk(destination, chunk, cancel_check)?;
    }
    Ok(())
}

fn write_backup_chunk(
    destination: &mut File,
    chunk: &[u8],
    cancel_check: &mut impl FnMut() -> bool,
) -> StoreResult<()> {
    if cancel_check() {
        return Err(StoreError::BackupCancelled);
    }
    destination.write_all(chunk).map_err(StoreError::BackupIo)
}

fn database_page_size(connection: &Connection) -> StoreResult<u64> {
    let value: i64 = connection.pragma_query_value(None, "page_size", |row| row.get(0))?;
    u64::try_from(value)
        .ok()
        .filter(|size| *size != 0)
        .ok_or(StoreError::CorruptRecord)
}

fn database_extent(connection: &Connection, page_size: u64) -> StoreResult<u64> {
    let page_count: i64 = connection.pragma_query_value(None, "page_count", |row| row.get(0))?;
    progress_extent(page_count, page_size)
}

fn progress_extent(page_count: i64, page_size: u64) -> StoreResult<u64> {
    u64::try_from(page_count)
        .map_err(|_| StoreError::CorruptRecord)?
        .checked_mul(page_size)
        .ok_or(StoreError::BackupTooLarge)
}

fn maximum_backup_steps(maximum_bytes: u64, page_size: u64) -> u64 {
    let maximum_pages = maximum_bytes.saturating_add(page_size - 1) / page_size;
    let pages_per_step = u64::try_from(BACKUP_PAGES_PER_STEP).unwrap_or(1);
    maximum_pages
        .saturating_add(pages_per_step - 1)
        .saturating_div(pages_per_step)
        .saturating_add(MAXIMUM_TRANSIENT_BACKUP_STEPS)
}

fn require_within_backup_limit(actual: u64, maximum: u64) -> StoreResult<()> {
    if actual <= maximum {
        Ok(())
    } else {
        Err(StoreError::BackupTooLarge)
    }
}

fn require_destination(file: &File, expected_bytes: u64) -> StoreResult<()> {
    let metadata = file.metadata().map_err(StoreError::BackupIo)?;
    if !metadata.is_file() || metadata.len() != expected_bytes {
        return Err(StoreError::InvalidBackupDestination);
    }
    require_single_link(file, &metadata)
}

#[cfg(unix)]
fn require_single_link(_file: &File, metadata: &Metadata) -> StoreResult<()> {
    if std::os::unix::fs::MetadataExt::nlink(metadata) == 1 {
        Ok(())
    } else {
        Err(StoreError::InvalidBackupDestination)
    }
}

#[cfg(windows)]
fn require_single_link(file: &File, _metadata: &Metadata) -> StoreResult<()> {
    let information = winx::winapi_util::file::information(file).map_err(StoreError::BackupIo)?;
    if information.number_of_links() == 1 {
        Ok(())
    } else {
        Err(StoreError::InvalidBackupDestination)
    }
}

#[cfg(not(any(unix, windows)))]
const fn require_single_link(_file: &File, _metadata: &Metadata) -> StoreResult<()> {
    Err(StoreError::InvalidBackupDestination)
}

#[cfg(test)]
#[path = "migration/tests.rs"]
mod tests;
