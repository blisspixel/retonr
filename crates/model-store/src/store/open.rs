use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};

use super::ArtifactStateStore;
use crate::{StoreError, StoreResult, schema};

impl ArtifactStateStore {
    /// Compatibility alias for [`Self::open_or_create_and_migrate`].
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the database cannot be opened, configured, or
    /// migrated without losing existing state.
    pub fn open(path: &Path) -> StoreResult<Self> {
        Self::open_or_create_and_migrate(path)
    }

    /// Opens or creates an artifact state database and applies supported migrations.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the database cannot be opened, configured, or
    /// migrated without losing existing state.
    pub fn open_or_create_and_migrate(path: &Path) -> StoreResult<Self> {
        let path = sqlite_path(path);
        let mut connection = Connection::open_with_flags(&path, create_flags())?;
        schema::initialize(&mut connection)?;
        Ok(Self { connection })
    }

    /// Opens an existing writable artifact state database and applies migrations.
    ///
    /// This method never creates a missing database.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::NotInitialized`] when the path is absent, or another
    /// [`StoreError`] when the database cannot be opened, configured, or migrated.
    pub fn open_existing_and_migrate(path: &Path) -> StoreResult<Self> {
        let mut connection = open_existing(path, writable_flags())?;
        schema::initialize(&mut connection)?;
        Ok(Self { connection })
    }

    /// Opens an existing artifact state database read-only at the exact schema.
    ///
    /// This method never creates a database and never applies a migration.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::NotInitialized`] when the path is absent,
    /// [`StoreError::MigrationRequired`] for an older schema,
    /// [`StoreError::UnsupportedSchema`] for a newer schema, or another
    /// [`StoreError`] when the database cannot be opened or configured safely.
    pub fn open_existing_read_only(path: &Path) -> StoreResult<Self> {
        let connection = open_existing(path, read_only_flags())?;
        schema::validate_exact(&connection)?;
        Ok(Self { connection })
    }

    /// Opens an existing artifact state database writable at the exact schema.
    ///
    /// This method never creates a database and never applies a migration.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::NotInitialized`] when the path is absent,
    /// [`StoreError::MigrationRequired`] for an older schema,
    /// [`StoreError::UnsupportedSchema`] for a newer schema, or another
    /// [`StoreError`] when the database cannot be opened or configured safely.
    pub fn open_existing_writable_exact(path: &Path) -> StoreResult<Self> {
        let connection = open_existing(path, writable_flags())?;
        schema::validate_exact_writable(&connection)?;
        Ok(Self { connection })
    }

    /// Resumes first-time initialization of an existing empty version-zero database.
    ///
    /// This method never creates a database and refuses legacy or arbitrary
    /// unversioned state. It exists only to recover a process exit between creating
    /// the `SQLite` file and committing the initial schema.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] unless the existing database is current or contains
    /// no schema objects at version zero, or when initialization cannot commit.
    pub fn open_existing_or_initialize_empty(path: &Path) -> StoreResult<Self> {
        let mut connection = open_existing(path, writable_flags())?;
        schema::initialize_empty(&mut connection)?;
        Ok(Self { connection })
    }
}

fn create_flags() -> OpenFlags {
    writable_flags() | OpenFlags::SQLITE_OPEN_CREATE
}

fn writable_flags() -> OpenFlags {
    OpenFlags::SQLITE_OPEN_READ_WRITE
        | OpenFlags::SQLITE_OPEN_NO_MUTEX
        | OpenFlags::SQLITE_OPEN_NOFOLLOW
}

fn read_only_flags() -> OpenFlags {
    OpenFlags::SQLITE_OPEN_READ_ONLY
        | OpenFlags::SQLITE_OPEN_NO_MUTEX
        | OpenFlags::SQLITE_OPEN_NOFOLLOW
}

fn open_existing(path: &Path, flags: OpenFlags) -> StoreResult<Connection> {
    let sqlite_path = sqlite_path(path);
    match Connection::open_with_flags(&sqlite_path, flags) {
        Ok(connection) => Ok(connection),
        Err(error) => match path.symlink_metadata() {
            Err(metadata_error) if metadata_error.kind() == std::io::ErrorKind::NotFound => {
                Err(StoreError::NotInitialized)
            }
            _ => Err(StoreError::Database(error)),
        },
    }
}

#[cfg(unix)]
fn sqlite_path(path: &Path) -> PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let Some(parent) = absolute.parent() else {
        return absolute;
    };
    let Some(name) = absolute.file_name() else {
        return absolute;
    };
    parent
        .canonicalize()
        .map_or(absolute.clone(), |canonical| canonical.join(name))
}

#[cfg(not(unix))]
fn sqlite_path(path: &Path) -> PathBuf {
    path.to_path_buf()
}
