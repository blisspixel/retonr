//! Schema 11 stores one inert operation-level terminal-evidence cohort.
//!
//! The tables grant no qualification, activation, or live-use authority.
//! A repeatability result in this cohort is only a candidate-generation
//! failure. Later judge stages need their own tables and a later schema,
//! because this table rejects every other stage and every judge identity.

use rusqlite::Connection;

use crate::StoreResult;

mod closure;
mod evidence;
mod manifests;

pub(super) fn migrate_schema_ten(connection: &Connection) -> StoreResult<()> {
    evidence::create(connection)?;
    manifests::create(connection)?;
    closure::create(connection)?;
    connection.execute_batch("PRAGMA user_version = 11;")?;
    Ok(())
}
