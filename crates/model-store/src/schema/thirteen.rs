//! Schema 13 stores inert phase-evidence subordinate records.
//!
//! The tables grant no qualification, activation, or live-use authority.
//! Resource measurements and exceeded limits stay inside canonical JSON.
//! This cohort does not widen the schema 11 repeatability result table.

use rusqlite::Connection;

use crate::StoreResult;

mod denial;
mod resource;

pub(super) fn hex_column(name: &str) -> String {
    format!(
        "{name} TEXT NOT NULL
             CHECK(length({name}) = 64)
             CHECK({name} = lower({name}))
             CHECK({name} NOT GLOB '*[^0-9a-f]*')"
    )
}

pub(super) fn primary_key(name: &str) -> String {
    format!(
        "{name} TEXT PRIMARY KEY NOT NULL
             CHECK(length({name}) = 64)
             CHECK({name} = lower({name}))
             CHECK({name} NOT GLOB '*[^0-9a-f]*')"
    )
}

pub(super) fn migrate_schema_twelve(connection: &Connection) -> StoreResult<()> {
    resource::create(connection)?;
    denial::create(connection)?;
    connection.execute_batch("PRAGMA user_version = 13;")?;
    Ok(())
}
