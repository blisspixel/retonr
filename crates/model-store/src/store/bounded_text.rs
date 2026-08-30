use rusqlite::{Connection, OptionalExtension as _, params};

use crate::{StoreError, StoreResult};

const DIGEST_TEXT_BYTES: usize = 64;

pub(super) enum BoundedTextCell {
    MissingRow,
    Null,
    Text(String),
}

pub(super) fn read_required_bounded_text(
    connection: &Connection,
    table: &str,
    key_column: &str,
    key: &str,
    value_column: &str,
    maximum_bytes: usize,
) -> StoreResult<Option<String>> {
    match read_bounded_text_cell(
        connection,
        table,
        key_column,
        key,
        value_column,
        maximum_bytes,
    )? {
        BoundedTextCell::MissingRow => Ok(None),
        BoundedTextCell::Null => Err(StoreError::CorruptRecord),
        BoundedTextCell::Text(value) => Ok(Some(value)),
    }
}

pub(super) fn read_required_digest_text(
    connection: &Connection,
    table: &str,
    key_column: &str,
    key: &str,
    value_column: &str,
) -> StoreResult<Option<String>> {
    let value = read_required_bounded_text(
        connection,
        table,
        key_column,
        key,
        value_column,
        DIGEST_TEXT_BYTES,
    )?;
    value.map(require_digest_text).transpose()
}

pub(super) fn read_nullable_digest_text(
    connection: &Connection,
    table: &str,
    key_column: &str,
    key: &str,
    value_column: &str,
) -> StoreResult<BoundedTextCell> {
    let cell = read_bounded_text_cell(
        connection,
        table,
        key_column,
        key,
        value_column,
        DIGEST_TEXT_BYTES,
    )?;
    match cell {
        BoundedTextCell::Text(value) => Ok(BoundedTextCell::Text(require_digest_text(value)?)),
        other => Ok(other),
    }
}

fn read_bounded_text_cell(
    connection: &Connection,
    table: &str,
    key_column: &str,
    key: &str,
    value_column: &str,
    maximum_bytes: usize,
) -> StoreResult<BoundedTextCell> {
    let metadata_sql = format!(
        "SELECT typeof({value_column}), length(CAST({value_column} AS BLOB))
         FROM {table} WHERE {key_column} = ?1"
    );
    let metadata = connection
        .query_row(&metadata_sql, [key], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<i64>>(1)?))
        })
        .optional()?;
    let Some((kind, length)) = metadata else {
        return Ok(BoundedTextCell::MissingRow);
    };
    if kind == "null" && length.is_none() {
        return Ok(BoundedTextCell::Null);
    }
    let Some(length) = length else {
        return Err(StoreError::CorruptRecord);
    };
    if kind != "text"
        || length < 1
        || usize::try_from(length)
            .ok()
            .is_none_or(|value| value > maximum_bytes)
    {
        return Err(StoreError::CorruptRecord);
    }
    let fetch_sql = format!(
        "SELECT substr(CAST({value_column} AS BLOB), 1, ?2)
         FROM {table} WHERE {key_column} = ?1"
    );
    let bytes: Vec<u8> = connection.query_row(
        &fetch_sql,
        params![
            key,
            i64::try_from(maximum_bytes).map_err(|_| StoreError::RecordTooLarge)?
        ],
        |row| row.get(0),
    )?;
    if i64::try_from(bytes.len()).ok() != Some(length) {
        return Err(StoreError::CorruptRecord);
    }
    String::from_utf8(bytes)
        .map(BoundedTextCell::Text)
        .map_err(|_| StoreError::CorruptRecord)
}

fn require_digest_text(value: String) -> StoreResult<String> {
    if value.len() == DIGEST_TEXT_BYTES
        && value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        Ok(value)
    } else {
        Err(StoreError::CorruptRecord)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Connection {
        let connection = Connection::open_in_memory().expect("open memory database");
        connection
            .execute_batch(
                "CREATE TABLE bounded_values (
                     key TEXT PRIMARY KEY,
                     value ANY
                 ) STRICT;",
            )
            .expect("create bounded values");
        connection
    }

    fn insert(connection: &Connection, key: &str, expression: &str) {
        connection
            .execute_batch(&format!(
                "INSERT INTO bounded_values VALUES ('{key}', {expression});"
            ))
            .expect("insert bounded value");
    }

    #[test]
    fn bounded_text_distinguishes_absent_null_and_text() {
        let connection = fixture();
        insert(&connection, "null", "NULL");
        insert(&connection, "empty", "''");
        insert(&connection, "text", "'value'");
        assert!(matches!(
            read_bounded_text_cell(&connection, "bounded_values", "key", "missing", "value", 5)
                .expect("read missing"),
            BoundedTextCell::MissingRow
        ));
        assert!(matches!(
            read_bounded_text_cell(&connection, "bounded_values", "key", "null", "value", 5)
                .expect("read null"),
            BoundedTextCell::Null
        ));
        assert!(matches!(
            read_bounded_text_cell(&connection, "bounded_values", "key", "text", "value", 5)
                .expect("read text"),
            BoundedTextCell::Text(value) if value == "value"
        ));
        assert!(
            read_required_bounded_text(&connection, "bounded_values", "key", "missing", "value", 5)
                .expect("read absent required value")
                .is_none()
        );
        assert!(matches!(
            read_required_bounded_text(&connection, "bounded_values", "key", "null", "value", 5),
            Err(StoreError::CorruptRecord)
        ));
        assert!(matches!(
            read_required_bounded_text(&connection, "bounded_values", "key", "empty", "value", 5),
            Err(StoreError::CorruptRecord)
        ));
    }

    #[test]
    fn bounded_text_uses_bytes_and_accepts_the_exact_bound() {
        let connection = fixture();
        insert(&connection, "ascii", "'abcd'");
        insert(&connection, "unicode", "'éé'");
        assert!(matches!(
            read_bounded_text_cell(&connection, "bounded_values", "key", "ascii", "value", 4)
                .expect("read exact bound"),
            BoundedTextCell::Text(value) if value == "abcd"
        ));
        assert!(matches!(
            read_bounded_text_cell(&connection, "bounded_values", "key", "unicode", "value", 3),
            Err(StoreError::CorruptRecord)
        ));
        assert!(matches!(
            read_bounded_text_cell(&connection, "bounded_values", "key", "unicode", "value", 4)
                .expect("read exact unicode bytes"),
            BoundedTextCell::Text(value) if value == "éé"
        ));
    }

    #[test]
    fn bounded_text_rejects_bound_plus_one_large_blob_and_invalid_utf8() {
        let connection = fixture();
        insert(&connection, "plus-one", "'abcde'");
        insert(&connection, "large", "CAST(zeroblob(1048577) AS TEXT)");
        insert(&connection, "blob", "X'6162'");
        insert(&connection, "invalid-utf8", "CAST(X'FF' AS TEXT)");
        for key in ["plus-one", "large", "blob", "invalid-utf8"] {
            assert!(matches!(
                read_bounded_text_cell(&connection, "bounded_values", "key", key, "value", 4),
                Err(StoreError::CorruptRecord)
            ));
        }
    }

    #[test]
    fn digest_text_requires_exact_lowercase_hex() {
        let connection = fixture();
        insert(&connection, "valid", &format!("'{}'", "a".repeat(64)));
        insert(&connection, "short", &format!("'{}'", "a".repeat(63)));
        insert(&connection, "upper", &format!("'{}'", "A".repeat(64)));
        assert_eq!(
            read_required_digest_text(&connection, "bounded_values", "key", "valid", "value")
                .expect("read valid digest")
                .expect("digest present"),
            "a".repeat(64)
        );
        for key in ["short", "upper"] {
            assert!(matches!(
                read_required_digest_text(&connection, "bounded_values", "key", key, "value"),
                Err(StoreError::CorruptRecord)
            ));
        }
    }
}
