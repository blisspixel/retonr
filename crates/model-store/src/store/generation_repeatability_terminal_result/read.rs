use rewrite_model::{
    GenerationRepeatabilityResultRecordV1, MAX_GENERATION_REPEATABILITY_RESULT_JSON_BYTES,
};
use rusqlite::{Connection, Row, params};

use super::parents::{self, Cited};
use crate::{StoreError, StoreResult};

const SELECT_COLUMNS: &str = "generation_repeatability_result_id, generation_system_id,
    generation_qualification_plan_id, generation_suite_manifest_id, generation_repetition_id,
    generation_attempt_ledger_manifest_id, terminal_stage, candidate_generation_receipt_set_id,
    candidate_deterministic_evaluation_id, candidate_judge_join_id,
    typeof(canonical_json), length(canonical_json), canonical_json";

pub(super) struct ResultRow {
    pub(super) id: String,
    system_id: String,
    plan_id: String,
    suite_id: String,
    repetition_id: String,
    ledger_id: String,
    stage: String,
    receipt_id: String,
    evaluation_id: String,
    judge_id: Option<String>,
    bytes: Vec<u8>,
}

pub(super) fn load(
    connection: &Connection,
    record: &GenerationRepeatabilityResultRecordV1,
) -> StoreResult<Option<GenerationRepeatabilityResultRecordV1>> {
    let cited = Cited::from_record(record)?;
    parents::require(connection, &cited)?;
    let json = serde_json::to_vec(record)?;
    let by_plan = load_pair(
        connection,
        "generation_qualification_plan_id",
        cited.plan_id,
        cited.repetition_id,
    )?;
    let by_ledger = load_pair(
        connection,
        "generation_attempt_ledger_manifest_id",
        cited.ledger_id,
        cited.repetition_id,
    )?;
    let by_id = load_by_id(connection, cited.id)?;
    let owner = json_owner(connection, &json)?;
    match (by_plan, by_ledger, by_id, owner) {
        (None, None, None, None) => Ok(None),
        (Some(plan_row), Some(ledger_row), Some(id_row), Some(owner_id))
            if plan_row.id == ledger_row.id
                && plan_row.id == id_row.id
                && plan_row.id == owner_id
                && matches(&id_row, &cited, &json) =>
        {
            Ok(Some(record.clone()))
        }
        _ => Err(StoreError::CorruptRecord),
    }
}

pub(super) fn load_by_id(connection: &Connection, id: &str) -> StoreResult<Option<ResultRow>> {
    let sql = format!(
        "SELECT {SELECT_COLUMNS}
         FROM generation_repeatability_terminal_result_records
         WHERE generation_repeatability_result_id = ?1"
    );
    one_row(connection, &sql, params![id])
}

pub(super) fn json_owner(connection: &Connection, json: &[u8]) -> StoreResult<Option<String>> {
    let mut statement = connection.prepare(
        "SELECT generation_repeatability_result_id
         FROM generation_repeatability_terminal_result_records
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

pub(super) fn matches(row: &ResultRow, cited: &Cited<'_>, json: &[u8]) -> bool {
    row.id == cited.id
        && row.system_id == cited.system_id
        && row.plan_id == cited.plan_id
        && row.suite_id == cited.suite_id
        && row.repetition_id == cited.repetition_id
        && row.ledger_id == cited.ledger_id
        && row.stage == cited.stage
        && row.receipt_id == cited.receipt_id
        && row.evaluation_id == cited.evaluation_id
        && row.judge_id.as_deref() == cited.judge_id
        && row.bytes == json
}

fn load_pair(
    connection: &Connection,
    column: &str,
    value: &str,
    repetition_id: &str,
) -> StoreResult<Option<ResultRow>> {
    let sql = format!(
        "SELECT {SELECT_COLUMNS}
         FROM generation_repeatability_terminal_result_records
         WHERE {column} = ?1 AND generation_repetition_id = ?2"
    );
    one_row(connection, &sql, params![value, repetition_id])
}

fn one_row(
    connection: &Connection,
    sql: &str,
    parameters: impl rusqlite::Params,
) -> StoreResult<Option<ResultRow>> {
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

fn read_row(row: &Row<'_>) -> StoreResult<ResultRow> {
    let kind: String = row.get(10)?;
    let length: i64 = row.get(11)?;
    let bytes: Vec<u8> = row.get(12)?;
    Ok(ResultRow {
        id: row.get(0)?,
        system_id: row.get(1)?,
        plan_id: row.get(2)?,
        suite_id: row.get(3)?,
        repetition_id: row.get(4)?,
        ledger_id: row.get(5)?,
        stage: row.get(6)?,
        receipt_id: row.get(7)?,
        evaluation_id: row.get(8)?,
        judge_id: row.get(9)?,
        bytes: blob(&kind, length, bytes)?,
    })
}

fn blob(kind: &str, length: i64, bytes: Vec<u8>) -> StoreResult<Vec<u8>> {
    let bounded = usize::try_from(length)
        .ok()
        .is_some_and(|value| (1..=MAX_GENERATION_REPEATABILITY_RESULT_JSON_BYTES).contains(&value));
    if kind == "blob" && bounded && i64::try_from(bytes.len()).ok() == Some(length) {
        Ok(bytes)
    } else {
        Err(StoreError::CorruptRecord)
    }
}
