use rusqlite::{Connection, OptionalExtension as _, params};

use crate::{StoreError, StoreResult};

#[expect(
    clippy::too_many_arguments,
    reason = "generic exact ordered association"
)]
pub(super) fn insert_association(
    connection: &Connection,
    table: &str,
    owner_column: &str,
    owner: &str,
    ordinal_column: &str,
    ordinal: usize,
    id_column: &str,
    id: &str,
) -> StoreResult<()> {
    let ordinal = i64::try_from(ordinal).map_err(|_| StoreError::RecordTooLarge)?;
    let sql = format!(
        "INSERT INTO {table} ({owner_column}, {ordinal_column}, {id_column})
         VALUES (?1, ?2, ?3) ON CONFLICT DO NOTHING"
    );
    let changed = connection.execute(&sql, params![owner, ordinal, id])?;
    if changed == 1 {
        return Ok(());
    }
    let sql = format!(
        "SELECT {id_column} FROM {table}
         WHERE {owner_column} = ?1 AND {ordinal_column} = ?2"
    );
    let actual = connection
        .query_row(&sql, params![owner, ordinal], |row| row.get::<_, String>(0))
        .optional()?
        .ok_or(StoreError::ImmutableConflict)?;
    if actual == id {
        Ok(())
    } else {
        Err(StoreError::ImmutableConflict)
    }
}

pub(super) fn require_record_result(
    connection: &Connection,
    changed: usize,
    table: &str,
    key_column: &str,
    key: &str,
    bytes: &[u8],
    maximum: usize,
) -> StoreResult<()> {
    if changed == 1 {
        return Ok(());
    }
    let actual = existing_blob(connection, table, key_column, key, maximum)?;
    if actual == bytes {
        Ok(())
    } else {
        Err(StoreError::ImmutableConflict)
    }
}

pub(super) fn existing_blob(
    connection: &Connection,
    table: &str,
    key_column: &str,
    key: &str,
    maximum: usize,
) -> StoreResult<Vec<u8>> {
    super::super::read::bounds::load_record_blob(connection, table, key_column, key, maximum)?
        .ok_or(StoreError::CorruptRecord)
}
