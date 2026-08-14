use rusqlite::Connection;

use crate::{StoreError, StoreResult};

pub(super) fn validate_database_integrity(connection: &Connection) -> StoreResult<()> {
    let quick_check: String =
        connection.pragma_query_value(None, "quick_check", |row| row.get(0))?;
    if quick_check != "ok" {
        return Err(StoreError::CorruptRecord);
    }
    let mut statement = connection.prepare("PRAGMA foreign_key_check")?;
    if statement.query([])?.next()?.is_some() {
        Err(StoreError::CorruptRecord)
    } else {
        Ok(())
    }
}
