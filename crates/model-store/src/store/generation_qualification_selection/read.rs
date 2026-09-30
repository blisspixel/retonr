use rewrite_model::{
    GenerationQualificationSelectionV1, MAX_GENERATION_QUALIFICATION_SELECTION_JSON_BYTES,
};
use rusqlite::{Connection, Row, params};

use super::parents::{self, Cited};
use crate::{StoreError, StoreResult};

const SELECT_COLUMNS: &str = "generation_qualification_selection_id, generation_qualification_id,
    typeof(canonical_json), length(canonical_json), canonical_json";

pub(super) struct RecordRow {
    pub(super) id: String,
    qualification: String,
    bytes: Vec<u8>,
}

pub(super) fn load(
    connection: &Connection,
    record: &GenerationQualificationSelectionV1,
) -> StoreResult<Option<GenerationQualificationSelectionV1>> {
    let cited = Cited::from_record(record);
    parents::require(connection, &cited)?;
    let json = serde_json::to_vec(record)?;
    let by_qualification = load_qualification(connection, cited.qualification)?;
    let by_id = load_by_id(connection, cited.id)?;
    let owner = json_owner(connection, &json)?;
    match (by_qualification, by_id, owner) {
        (None, None, None) => Ok(None),
        (Some(qualification_row), Some(id_row), Some(owner_id))
            if qualification_row.id == id_row.id
                && qualification_row.id == owner_id
                && matches(&id_row, &cited, &json) =>
        {
            Ok(Some(record.clone()))
        }
        _ => Err(StoreError::CorruptRecord),
    }
}

pub(super) fn load_by_id(connection: &Connection, id: &str) -> StoreResult<Option<RecordRow>> {
    let sql = format!(
        "SELECT {SELECT_COLUMNS}
         FROM generation_qualification_selections
         WHERE generation_qualification_selection_id = ?1"
    );
    one_row(connection, &sql, params![id])
}

pub(super) fn json_owner(connection: &Connection, json: &[u8]) -> StoreResult<Option<String>> {
    let mut statement = connection.prepare(
        "SELECT generation_qualification_selection_id
         FROM generation_qualification_selections
         WHERE canonical_json = ?1",
    )?;
    let mut rows = statement.query(params![json])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };
    let id: String = row.get(0)?;
    if rows.next()?.is_some() {
        Err(StoreError::CorruptRecord)
    } else {
        Ok(Some(id))
    }
}

pub(super) fn matches(row: &RecordRow, cited: &Cited<'_>, json: &[u8]) -> bool {
    row.id == cited.id && row.qualification == cited.qualification && row.bytes == json
}

fn load_qualification(
    connection: &Connection,
    qualification: &str,
) -> StoreResult<Option<RecordRow>> {
    let sql = format!(
        "SELECT {SELECT_COLUMNS}
         FROM generation_qualification_selections
         WHERE generation_qualification_id = ?1"
    );
    one_row(connection, &sql, params![qualification])
}

fn one_row(
    connection: &Connection,
    sql: &str,
    parameters: impl rusqlite::Params,
) -> StoreResult<Option<RecordRow>> {
    let mut statement = connection.prepare(sql)?;
    let mut rows = statement.query(parameters)?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };
    let stored = read_row(row)?;
    if rows.next()?.is_some() {
        Err(StoreError::CorruptRecord)
    } else {
        Ok(Some(stored))
    }
}

fn read_row(row: &Row<'_>) -> StoreResult<RecordRow> {
    let kind: String = row.get(2)?;
    let length: i64 = row.get(3)?;
    let bytes: Vec<u8> = row.get(4)?;
    Ok(RecordRow {
        id: row.get(0)?,
        qualification: row.get(1)?,
        bytes: blob(&kind, length, bytes)?,
    })
}

fn blob(kind: &str, length: i64, bytes: Vec<u8>) -> StoreResult<Vec<u8>> {
    let bounded = usize::try_from(length).ok().is_some_and(|value| {
        (1..=MAX_GENERATION_QUALIFICATION_SELECTION_JSON_BYTES).contains(&value)
    });
    if kind == "blob" && bounded && i64::try_from(bytes.len()).ok() == Some(length) {
        Ok(bytes)
    } else {
        Err(StoreError::CorruptRecord)
    }
}
