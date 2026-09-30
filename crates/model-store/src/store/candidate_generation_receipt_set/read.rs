use rewrite_model::{
    CandidateGenerationReceiptSetV1, MAX_CANDIDATE_GENERATION_RECEIPT_SET_JSON_BYTES,
};
use rusqlite::{Connection, Row, params};

use super::parents::{self, Cited};
use crate::{StoreError, StoreResult};

const SELECT_COLUMNS: &str = "candidate_generation_receipt_set_id, schema_version,
    generation_qualification_plan_id, generation_suite_manifest_id,
    generation_repetition_id, generation_system_id, candidate_selection_policy_id,
    entry_count, typeof(canonical_json), length(canonical_json), canonical_json";

pub(super) struct ReceiptSetRow {
    pub(super) id: String,
    schema_version: i64,
    plan_id: String,
    suite_id: String,
    repetition_id: String,
    system_id: String,
    policy_id: String,
    entry_count: i64,
    bytes: Vec<u8>,
}

pub(super) fn load(
    connection: &Connection,
    record: &CandidateGenerationReceiptSetV1,
) -> StoreResult<Option<CandidateGenerationReceiptSetV1>> {
    let cited = Cited::from_record(record);
    parents::require(connection, &cited)?;
    let slot = load_slot(connection, &cited)?;
    let by_id = load_by_id(connection, cited.id)?;
    match (slot, by_id) {
        (None, None) => Ok(None),
        (Some(slot_row), Some(id_row))
            if slot_row.id == id_row.id && row_matches(&slot_row, record) =>
        {
            Ok(Some(record.clone()))
        }
        _ => Err(StoreError::CorruptRecord),
    }
}

pub(super) fn load_by_id(connection: &Connection, id: &str) -> StoreResult<Option<ReceiptSetRow>> {
    let sql = format!(
        "SELECT {SELECT_COLUMNS} FROM candidate_generation_receipt_sets
         WHERE candidate_generation_receipt_set_id = ?1"
    );
    one_row(connection, &sql, params![id])
}

pub(super) fn matches(row: &ReceiptSetRow, cited: &Cited<'_>, json: &[u8]) -> bool {
    row.id == cited.id
        && row.schema_version == cited.schema_version
        && row.plan_id == cited.plan_id
        && row.suite_id == cited.suite_id
        && row.repetition_id == cited.repetition_id
        && row.system_id == cited.system_id
        && row.policy_id == cited.policy_id
        && row.entry_count == cited.entry_count
        && row.bytes == json
}

fn load_slot(connection: &Connection, cited: &Cited<'_>) -> StoreResult<Option<ReceiptSetRow>> {
    let sql = format!(
        "SELECT {SELECT_COLUMNS} FROM candidate_generation_receipt_sets
         WHERE generation_qualification_plan_id = ?1 AND generation_repetition_id = ?2
           AND generation_system_id = ?3"
    );
    one_row(
        connection,
        &sql,
        params![cited.plan_id, cited.repetition_id, cited.system_id],
    )
}

fn row_matches(row: &ReceiptSetRow, record: &CandidateGenerationReceiptSetV1) -> bool {
    let Ok(json) = serde_json::to_vec(record) else {
        return false;
    };
    matches(row, &Cited::from_record(record), &json)
}

fn one_row(
    connection: &Connection,
    sql: &str,
    parameters: impl rusqlite::Params,
) -> StoreResult<Option<ReceiptSetRow>> {
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

fn read_row(row: &Row<'_>) -> StoreResult<ReceiptSetRow> {
    let kind: String = row.get(8)?;
    let length: i64 = row.get(9)?;
    let bytes: Vec<u8> = row.get(10)?;
    Ok(ReceiptSetRow {
        id: row.get(0)?,
        schema_version: row.get(1)?,
        plan_id: row.get(2)?,
        suite_id: row.get(3)?,
        repetition_id: row.get(4)?,
        system_id: row.get(5)?,
        policy_id: row.get(6)?,
        entry_count: row.get(7)?,
        bytes: blob(&kind, length, bytes)?,
    })
}

fn blob(kind: &str, length: i64, bytes: Vec<u8>) -> StoreResult<Vec<u8>> {
    let bounded = usize::try_from(length).ok().is_some_and(|value| {
        (1..=MAX_CANDIDATE_GENERATION_RECEIPT_SET_JSON_BYTES).contains(&value)
    });
    if kind == "blob" && bounded && i64::try_from(bytes.len()).ok() == Some(length) {
        Ok(bytes)
    } else {
        Err(StoreError::CorruptRecord)
    }
}
