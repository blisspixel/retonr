use rewrite_model::{
    GenerationQualificationRecordV1, MAX_GENERATION_QUALIFICATION_RECORD_JSON_BYTES,
};
use rusqlite::{Connection, Row, params};

use super::parents::{self, Cited};
use crate::{StoreError, StoreResult};

#[cfg(test)]
mod tests;

const SELECT_COLUMNS: &str = "generation_qualification_id, target_generation_system_id,
    baseline_generation_system_id, operation_policy_id, request_projection_id,
    generation_qualification_platform_evidence_id, generation_qualification_license_evidence_id,
    generation_qualification_operation_receipt_id, status, typeof(canonical_json),
    length(canonical_json), substr(CAST(canonical_json AS BLOB), 1, 16385)";

pub(super) struct RecordRow {
    pub(super) id: String,
    target: String,
    baseline: String,
    policy: String,
    projection: String,
    platform: String,
    license: String,
    receipt: String,
    status: String,
    bytes: Vec<u8>,
}

pub(crate) fn load(
    connection: &Connection,
    record: &GenerationQualificationRecordV1,
) -> StoreResult<Option<GenerationQualificationRecordV1>> {
    let cited = Cited::from_record(record)?;
    parents::require(connection, &cited)?;
    let json = serde_json::to_vec(record)?;
    let by_receipt = load_receipt(connection, cited.receipt)?;
    let by_id = load_by_id(connection, cited.id)?;
    let owner = json_owner(connection, &json)?;
    match (by_receipt, by_id, owner) {
        (None, None, None) => Ok(None),
        (Some(receipt_row), Some(id_row), Some(owner_id))
            if receipt_row.id == id_row.id
                && receipt_row.id == owner_id
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
         FROM generation_qualification_records
         WHERE generation_qualification_id = ?1"
    );
    one_row(connection, &sql, params![id])
}

pub(super) fn json_owner(connection: &Connection, json: &[u8]) -> StoreResult<Option<String>> {
    let mut statement = connection.prepare(
        "SELECT generation_qualification_id
         FROM generation_qualification_records
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
    row.id == cited.id
        && row.target == cited.target
        && row.baseline == cited.baseline
        && row.policy == cited.policy
        && row.projection == cited.projection
        && row.platform == cited.platform
        && row.license == cited.license
        && row.receipt == cited.receipt
        && row.status == cited.status
        && row.bytes == json
}

fn load_receipt(connection: &Connection, receipt: &str) -> StoreResult<Option<RecordRow>> {
    let sql = format!(
        "SELECT {SELECT_COLUMNS}
         FROM generation_qualification_records
         WHERE generation_qualification_operation_receipt_id = ?1"
    );
    one_row(connection, &sql, params![receipt])
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
    let kind: String = row.get(9)?;
    let length: i64 = row.get(10)?;
    let bytes: Vec<u8> = row.get(11)?;
    Ok(RecordRow {
        id: row.get(0)?,
        target: row.get(1)?,
        baseline: row.get(2)?,
        policy: row.get(3)?,
        projection: row.get(4)?,
        platform: row.get(5)?,
        license: row.get(6)?,
        receipt: row.get(7)?,
        status: row.get(8)?,
        bytes: blob(&kind, length, bytes)?,
    })
}

fn blob(kind: &str, length: i64, bytes: Vec<u8>) -> StoreResult<Vec<u8>> {
    let bounded = usize::try_from(length)
        .ok()
        .is_some_and(|value| (1..=MAX_GENERATION_QUALIFICATION_RECORD_JSON_BYTES).contains(&value));
    if kind == "blob" && bounded && i64::try_from(bytes.len()).ok() == Some(length) {
        Ok(bytes)
    } else {
        Err(StoreError::CorruptRecord)
    }
}
