use rewrite_model::{
    CandidateDeterministicEvaluationRecordV1, MAX_CANDIDATE_DETERMINISTIC_EVALUATION_JSON_BYTES,
};
use rusqlite::{Connection, Row, params};

use super::parents::{self, Cited};
use crate::{StoreError, StoreResult};

const SELECT_COLUMNS: &str = "candidate_deterministic_evaluation_id, schema_version,
    candidate_a_receipt_set_id, candidate_b_receipt_set_id,
    typeof(canonical_json), length(canonical_json), canonical_json";

pub(super) struct EvaluationRow {
    pub(super) id: String,
    schema_version: i64,
    receipt_a: String,
    receipt_b: String,
    bytes: Vec<u8>,
}

pub(super) fn load(
    connection: &Connection,
    record: &CandidateDeterministicEvaluationRecordV1,
) -> StoreResult<Option<CandidateDeterministicEvaluationRecordV1>> {
    let cited = Cited::from_record(record);
    parents::require(connection, &cited)?;
    let json = serde_json::to_vec(record)?;
    let by_id = load_by_id(connection, cited.id)?;
    let owner = json_owner(connection, &json)?;
    match (by_id, owner) {
        (None, None) => Ok(None),
        (Some(row), Some(owner_id)) if row.id == owner_id && matches(&row, &cited, &json) => {
            Ok(Some(record.clone()))
        }
        _ => Err(StoreError::CorruptRecord),
    }
}

pub(super) fn load_by_id(connection: &Connection, id: &str) -> StoreResult<Option<EvaluationRow>> {
    let sql = format!(
        "SELECT {SELECT_COLUMNS} FROM candidate_deterministic_evaluation_records
         WHERE candidate_deterministic_evaluation_id = ?1"
    );
    one_row(connection, &sql, params![id])
}

pub(super) fn json_owner(connection: &Connection, json: &[u8]) -> StoreResult<Option<String>> {
    let mut statement = connection.prepare(
        "SELECT candidate_deterministic_evaluation_id
         FROM candidate_deterministic_evaluation_records
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

pub(super) fn matches(row: &EvaluationRow, cited: &Cited<'_>, json: &[u8]) -> bool {
    row.id == cited.id
        && row.schema_version == cited.schema_version
        && row.receipt_a == cited.receipt_a
        && row.receipt_b == cited.receipt_b
        && row.bytes == json
}

fn one_row(
    connection: &Connection,
    sql: &str,
    parameters: impl rusqlite::Params,
) -> StoreResult<Option<EvaluationRow>> {
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

fn read_row(row: &Row<'_>) -> StoreResult<EvaluationRow> {
    let kind: String = row.get(4)?;
    let length: i64 = row.get(5)?;
    let bytes: Vec<u8> = row.get(6)?;
    Ok(EvaluationRow {
        id: row.get(0)?,
        schema_version: row.get(1)?,
        receipt_a: row.get(2)?,
        receipt_b: row.get(3)?,
        bytes: blob(&kind, length, bytes)?,
    })
}

fn blob(kind: &str, length: i64, bytes: Vec<u8>) -> StoreResult<Vec<u8>> {
    let bounded = usize::try_from(length).ok().is_some_and(|value| {
        (1..=MAX_CANDIDATE_DETERMINISTIC_EVALUATION_JSON_BYTES).contains(&value)
    });
    if kind == "blob" && bounded && i64::try_from(bytes.len()).ok() == Some(length) {
        Ok(bytes)
    } else {
        Err(StoreError::CorruptRecord)
    }
}
