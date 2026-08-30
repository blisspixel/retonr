use rusqlite::{Connection, OptionalExtension as _};

use crate::store::generation_system_foundation::read;
use crate::{StoreError, StoreResult};

pub(in crate::store::generation_qualification_plan_foundation) fn load_record_blob(
    connection: &Connection,
    table: &str,
    key_column: &str,
    key: &str,
    maximum: usize,
) -> StoreResult<Option<Vec<u8>>> {
    let metadata = connection
        .query_row(
            &format!(
                "SELECT typeof(canonical_json), length(canonical_json)
                 FROM {table} WHERE {key_column} = ?1"
            ),
            [key],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()?;
    metadata
        .map(|(kind, length)| {
            read::read_bounded_blob(connection, table, key_column, key, &kind, length, maximum)
        })
        .transpose()
}

pub(super) fn load_ordered_ids(
    connection: &Connection,
    table: &str,
    owner_column: &str,
    owner: &str,
    ordinal_column: &str,
    id_column: &str,
    expected_count: usize,
) -> StoreResult<Vec<String>> {
    let limit = expected_count
        .checked_add(1)
        .ok_or(StoreError::CorruptRecord)?;
    let sql = format!(
        "SELECT {ordinal_column}, {id_column}, typeof({id_column})
         FROM {table} WHERE {owner_column} = ?1 ORDER BY {ordinal_column} LIMIT {limit}"
    );
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map([owner], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    let mut values = Vec::with_capacity(expected_count);
    for row in rows {
        let (ordinal, id, kind) = row?;
        if values.len() >= expected_count
            || usize::try_from(ordinal).ok() != Some(values.len())
            || kind != "text"
        {
            return Err(StoreError::CorruptRecord);
        }
        require_digest(&id)?;
        values.push(id);
    }
    if values.len() == expected_count {
        Ok(values)
    } else {
        Err(StoreError::CorruptRecord)
    }
}

pub(super) fn bounded_count(value: i64, maximum: usize) -> StoreResult<usize> {
    usize::try_from(value)
        .ok()
        .filter(|value| *value > 0 && *value <= maximum)
        .ok_or(StoreError::CorruptRecord)
}

pub(super) fn require_digest(value: &str) -> StoreResult<()> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}
