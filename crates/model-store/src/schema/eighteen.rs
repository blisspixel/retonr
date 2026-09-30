//! Schema 18 stores one inert generation-qualification invalidation.
//!
//! The table grants no qualification, activation, or live-use authority.
//! A stored invalidation does not change the qualification record it names.
//! This cohort does not alter schemas 7 through 17.

use rusqlite::Connection;

use super::thirteen::{hex_column, primary_key};
use crate::StoreResult;

pub(super) fn migrate_schema_seventeen(connection: &Connection) -> StoreResult<()> {
    create(connection)?;
    connection.execute_batch("PRAGMA user_version = 18;")?;
    Ok(())
}

fn create(connection: &Connection) -> StoreResult<()> {
    let id = primary_key("generation_qualification_invalidation_id");
    let qualification = hex_column("generation_qualification_id");
    connection.execute_batch(&format!(
        "CREATE TABLE generation_qualification_invalidations (
             {id},
             {qualification},
             canonical_json BLOB NOT NULL CHECK(length(canonical_json) BETWEEN 1 AND 16384),
             UNIQUE (generation_qualification_id),
             FOREIGN KEY (generation_qualification_id)
                 REFERENCES generation_qualification_records(generation_qualification_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT
         ) STRICT;"
    ))?;
    Ok(())
}

#[cfg(test)]
pub(crate) fn create_schema_seventeen_fixture(connection: &Connection) -> StoreResult<()> {
    super::create_schema_sixteen_fixture(connection)?;
    super::migrate_schema_sixteen(connection)
}
