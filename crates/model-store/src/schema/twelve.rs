//! Schema 12 stores one inert judge-execution cohort.
//!
//! The tables grant no qualification, activation, or live-use authority.
//! Nested judge responses and observations stay inside their aggregate JSON.
//! This cohort does not widen the schema 11 repeatability result table.

use rusqlite::Connection;

use crate::StoreResult;

mod aggregates;
mod batch;
mod join;
mod plan;
mod receipt;
mod schedule;

pub(super) fn migrate_schema_eleven(connection: &Connection) -> StoreResult<()> {
    plan::create(connection)?;
    schedule::create(connection)?;
    aggregates::create(connection)?;
    batch::create(connection)?;
    receipt::create(connection)?;
    join::create(connection)?;
    connection.execute_batch("PRAGMA user_version = 12;")?;
    Ok(())
}
