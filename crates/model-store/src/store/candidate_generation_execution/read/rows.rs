use rusqlite::{Connection, OptionalExtension as _, params};

use crate::store::bounded_text::{
    BoundedTextCell, read_nullable_digest_text, read_required_bounded_text,
    read_required_digest_text,
};
use crate::store::generation_system_foundation::read::read_bounded_blob;
use crate::{StoreError, StoreResult};

pub(super) struct JsonRow {
    pub(super) id: String,
    pub(super) indexes: Vec<String>,
    pub(super) canonical_json: Vec<u8>,
}

pub(super) struct AttemptRow {
    pub(super) value: JsonRow,
    pub(super) outcome: String,
    pub(super) precursor_id: Option<String>,
    pub(super) receipt_id: Option<String>,
}

pub(super) struct StorageRow {
    pub(super) indexes: Vec<String>,
    pub(super) relative_reference: String,
    pub(super) maximum_tree_entries: i64,
    pub(super) maximum_tree_depth: i64,
    pub(super) maximum_aggregate_bytes: i64,
}

pub(super) fn load_attempt(
    connection: &Connection,
    planned_attempt_id: &str,
    maximum_json_bytes: usize,
) -> StoreResult<Option<AttemptRow>> {
    let table = "candidate_generation_attempt_records";
    let key_column = "planned_candidate_attempt_id";
    let Some(id) = read_required_digest_text(
        connection,
        table,
        key_column,
        planned_attempt_id,
        "candidate_generation_attempt_record_id",
    )?
    else {
        return Ok(None);
    };
    let value = load_json_row(
        connection,
        table,
        "candidate_generation_attempt_record_id",
        &id,
        &[
            "generation_qualification_plan_id",
            "planned_candidate_attempt_id",
        ],
        maximum_json_bytes,
    )?
    .ok_or(StoreError::CorruptRecord)?;
    let outcome = read_required_bounded_text(
        connection,
        table,
        "candidate_generation_attempt_record_id",
        &id,
        "outcome",
        9,
    )?
    .ok_or(StoreError::CorruptRecord)?;
    let precursor_id = nullable_digest(
        connection,
        table,
        &id,
        "candidate_generation_attempt_precursor_id",
    )?;
    let receipt_id = nullable_digest(connection, table, &id, "candidate_generation_receipt_id")?;
    Ok(Some(AttemptRow {
        value,
        outcome,
        precursor_id,
        receipt_id,
    }))
}

pub(super) fn load_json_row(
    connection: &Connection,
    table: &str,
    key_column: &str,
    key: &str,
    index_columns: &[&str],
    maximum_json_bytes: usize,
) -> StoreResult<Option<JsonRow>> {
    let Some(id) = read_required_digest_text(connection, table, key_column, key, key_column)?
    else {
        return Ok(None);
    };
    let mut indexes = Vec::with_capacity(index_columns.len());
    for column in index_columns {
        indexes.push(
            read_required_digest_text(connection, table, key_column, key, column)?
                .ok_or(StoreError::CorruptRecord)?,
        );
    }
    let metadata_sql = format!(
        "SELECT typeof(canonical_json), length(canonical_json)
         FROM {table} WHERE {key_column} = ?1"
    );
    let (kind, length) = connection
        .query_row(&metadata_sql, [key], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .optional()?
        .ok_or(StoreError::CorruptRecord)?;
    let canonical_json = read_bounded_blob(
        connection,
        table,
        key_column,
        key,
        &kind,
        length,
        maximum_json_bytes,
    )?;
    Ok(Some(JsonRow {
        id,
        indexes,
        canonical_json,
    }))
}

pub(super) fn load_storage(
    connection: &Connection,
    bundle_id: &str,
) -> StoreResult<Option<StorageRow>> {
    let table = "generation_evidence_bundle_storage";
    let key = "candidate_generation_evidence_bundle_id";
    let Some(id) = read_required_digest_text(connection, table, key, bundle_id, key)? else {
        return Ok(None);
    };
    let mut indexes = vec![id];
    for column in [
        "generation_qualification_plan_id",
        "planned_candidate_attempt_id",
        "storage_root_id",
    ] {
        indexes.push(
            read_required_digest_text(connection, table, key, bundle_id, column)?
                .ok_or(StoreError::CorruptRecord)?,
        );
    }
    let relative_reference =
        read_required_bounded_text(connection, table, key, bundle_id, "relative_reference", 512)?
            .ok_or(StoreError::CorruptRecord)?;
    let maximum_tree_entries =
        read_integer(connection, table, key, bundle_id, "maximum_tree_entries")?;
    let maximum_tree_depth = read_integer(connection, table, key, bundle_id, "maximum_tree_depth")?;
    let maximum_aggregate_bytes =
        read_integer(connection, table, key, bundle_id, "maximum_aggregate_bytes")?;
    Ok(Some(StorageRow {
        indexes,
        relative_reference,
        maximum_tree_entries,
        maximum_tree_depth,
        maximum_aggregate_bytes,
    }))
}

fn nullable_digest(
    connection: &Connection,
    table: &str,
    key: &str,
    column: &str,
) -> StoreResult<Option<String>> {
    match read_nullable_digest_text(
        connection,
        table,
        "candidate_generation_attempt_record_id",
        key,
        column,
    )? {
        BoundedTextCell::Null => Ok(None),
        BoundedTextCell::Text(value) => Ok(Some(value)),
        BoundedTextCell::MissingRow => Err(StoreError::CorruptRecord),
    }
}

fn read_integer(
    connection: &Connection,
    table: &str,
    key_column: &str,
    key: &str,
    value_column: &str,
) -> StoreResult<i64> {
    let sql = format!(
        "SELECT typeof({value_column}), {value_column} FROM {table} WHERE {key_column} = ?1"
    );
    let value = connection
        .query_row(&sql, params![key], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .optional()?;
    match value {
        Some((kind, value)) if kind == "integer" => Ok(value),
        _ => Err(StoreError::CorruptRecord),
    }
}
