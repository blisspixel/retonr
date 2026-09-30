//! Schema 19 stores one inert generation-qualification selection.
//!
//! The table grants no qualification, activation, or live-use authority.
//! A stored selection does not change the qualification record it names.
//! This cohort does not alter schemas 7 through 18.

use rusqlite::Connection;

use super::thirteen::{hex_column, primary_key};
use crate::StoreResult;

pub(super) fn migrate_schema_eighteen(connection: &Connection) -> StoreResult<()> {
    create(connection)?;
    connection.execute_batch("PRAGMA user_version = 19;")?;
    Ok(())
}

fn create(connection: &Connection) -> StoreResult<()> {
    let id = primary_key("generation_qualification_selection_id");
    let qualification = hex_column("generation_qualification_id");
    connection.execute_batch(&format!(
        "CREATE TABLE generation_qualification_selections (
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
pub(crate) use super::eighteen::create_schema_seventeen_fixture;

#[cfg(test)]
pub(crate) fn create_schema_eighteen_fixture(connection: &Connection) -> StoreResult<()> {
    super::eighteen::create_schema_seventeen_fixture(connection)?;
    super::eighteen::migrate_schema_seventeen(connection)
}
