use rusqlite::Connection;

use crate::{StoreError, StoreResult};

use super::{
    create_current_schema, create_schema_one, create_schema_two, migrate_schema_five,
    migrate_schema_four, migrate_schema_one, migrate_schema_three, migrate_schema_two,
};

pub(crate) fn validate_schema_shape(connection: &Connection) -> StoreResult<()> {
    let actual = schema_objects(connection)?;
    let current = canonical_current_objects()?;
    if actual == current || actual == canonical_migrated_current_objects()? {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}

pub(crate) fn validate_schema_four(connection: &Connection) -> StoreResult<()> {
    let actual = schema_objects(connection)?;
    if actual == canonical_schema_four_objects()?
        || actual == canonical_migrated_schema_four_objects()?
    {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}

pub(crate) fn validate_schema_five(connection: &Connection) -> StoreResult<()> {
    let actual = schema_objects(connection)?;
    if actual == canonical_schema_five_objects()?
        || actual == canonical_migrated_schema_five_objects()?
    {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}

pub(crate) fn validate_schema_three(connection: &Connection) -> StoreResult<()> {
    let actual = schema_objects(connection)?;
    if actual == canonical_schema_three_objects()?
        || actual == canonical_migrated_schema_three_objects()?
    {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}

pub(crate) fn validate_schema_two(connection: &Connection) -> StoreResult<()> {
    let actual = schema_objects(connection)?;
    if actual == canonical_schema_two_objects()?
        || actual == canonical_migrated_schema_two_objects()?
    {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}

pub(crate) fn validate_schema_one(connection: &Connection) -> StoreResult<()> {
    if schema_objects(connection)? == canonical_schema_one_objects()? {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}

#[derive(Debug, Eq, PartialEq)]
struct SchemaObject {
    kind: String,
    name: String,
    table: String,
    sql: Option<String>,
}

fn schema_objects(connection: &Connection) -> StoreResult<Vec<SchemaObject>> {
    let mut statement = connection.prepare(
        "SELECT type, name, tbl_name, sql
         FROM sqlite_schema
         ORDER BY type, name",
    )?;
    statement
        .query_map([], |row| {
            Ok(SchemaObject {
                kind: row.get(0)?,
                name: row.get(1)?,
                table: row.get(2)?,
                sql: row
                    .get::<_, Option<String>>(3)?
                    .map(|sql| normalize_sql(&sql)),
            })
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::Database)
}

fn canonical_current_objects() -> StoreResult<Vec<SchemaObject>> {
    let connection = Connection::open_in_memory()?;
    create_current_schema(&connection)?;
    schema_objects(&connection)
}

fn canonical_schema_four_objects() -> StoreResult<Vec<SchemaObject>> {
    let connection = Connection::open_in_memory()?;
    create_schema_two(&connection)?;
    migrate_schema_two(&connection)?;
    migrate_schema_three(&connection)?;
    schema_objects(&connection)
}

fn canonical_schema_five_objects() -> StoreResult<Vec<SchemaObject>> {
    let connection = Connection::open_in_memory()?;
    create_schema_two(&connection)?;
    migrate_schema_two(&connection)?;
    migrate_schema_three(&connection)?;
    migrate_schema_four(&connection)?;
    schema_objects(&connection)
}

fn canonical_schema_three_objects() -> StoreResult<Vec<SchemaObject>> {
    let connection = Connection::open_in_memory()?;
    create_schema_two(&connection)?;
    migrate_schema_two(&connection)?;
    schema_objects(&connection)
}

fn canonical_schema_two_objects() -> StoreResult<Vec<SchemaObject>> {
    let connection = Connection::open_in_memory()?;
    create_schema_two(&connection)?;
    schema_objects(&connection)
}

fn canonical_schema_one_objects() -> StoreResult<Vec<SchemaObject>> {
    let connection = Connection::open_in_memory()?;
    create_schema_one(&connection)?;
    schema_objects(&connection)
}

fn canonical_migrated_schema_two_objects() -> StoreResult<Vec<SchemaObject>> {
    let connection = Connection::open_in_memory()?;
    create_schema_one(&connection)?;
    migrate_schema_one(&connection)?;
    schema_objects(&connection)
}

fn canonical_migrated_schema_three_objects() -> StoreResult<Vec<SchemaObject>> {
    let connection = Connection::open_in_memory()?;
    create_schema_one(&connection)?;
    migrate_schema_one(&connection)?;
    migrate_schema_two(&connection)?;
    schema_objects(&connection)
}

fn canonical_migrated_schema_four_objects() -> StoreResult<Vec<SchemaObject>> {
    let connection = Connection::open_in_memory()?;
    create_schema_one(&connection)?;
    migrate_schema_one(&connection)?;
    migrate_schema_two(&connection)?;
    migrate_schema_three(&connection)?;
    schema_objects(&connection)
}

fn canonical_migrated_current_objects() -> StoreResult<Vec<SchemaObject>> {
    let connection = Connection::open_in_memory()?;
    create_schema_one(&connection)?;
    migrate_schema_one(&connection)?;
    migrate_schema_two(&connection)?;
    migrate_schema_three(&connection)?;
    migrate_schema_four(&connection)?;
    migrate_schema_five(&connection)?;
    schema_objects(&connection)
}

fn canonical_migrated_schema_five_objects() -> StoreResult<Vec<SchemaObject>> {
    let connection = Connection::open_in_memory()?;
    create_schema_one(&connection)?;
    migrate_schema_one(&connection)?;
    migrate_schema_two(&connection)?;
    migrate_schema_three(&connection)?;
    migrate_schema_four(&connection)?;
    schema_objects(&connection)
}

fn normalize_sql(sql: &str) -> String {
    let mut normalized = String::with_capacity(sql.len());
    let mut characters = sql.chars().peekable();
    let mut quote = None;
    let mut pending_space = false;
    while let Some(character) = characters.next() {
        if let Some(terminator) = quote {
            normalized.push(character);
            if character == terminator {
                if terminator != ']' && characters.peek() == Some(&terminator) {
                    if let Some(escaped) = characters.next() {
                        normalized.push(escaped);
                    }
                } else {
                    quote = None;
                }
            }
        } else if character.is_ascii_whitespace() {
            pending_space = !normalized.is_empty();
        } else {
            if pending_space {
                normalized.push(' ');
                pending_space = false;
            }
            normalized.push(character.to_ascii_lowercase());
            if matches!(character, '\'' | '"' | '`' | '[') {
                quote = Some(if character == '[' { ']' } else { character });
            }
        }
    }
    normalized
}
