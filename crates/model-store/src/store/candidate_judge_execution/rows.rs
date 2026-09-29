use rusqlite::{Connection, OptionalExtension as _, params};

mod row_decode;

use crate::{StoreError, StoreResult};
pub(super) use row_decode::{
    BatchRow, JoinRow, PlanRow, ReceiptRow, RequestRow, ResponseRow, ScheduleRow,
};
use row_decode::{
    map_batch, map_join, map_plan, map_receipt, map_request, map_response, map_schedule,
};

const TABLES: [&str; 7] = [
    "candidate_judge_plans",
    "candidate_judge_schedules",
    "candidate_judge_request_aggregates",
    "candidate_judge_response_aggregates",
    "candidate_judge_observation_batches",
    "managed_local_judge_receipts",
    "candidate_judge_join_records",
];

pub(super) struct Counts {
    values: [i64; 7],
}

impl Counts {
    pub(super) fn all_zero(&self) -> bool {
        self.values.iter().all(|value| *value == 0)
    }

    pub(super) fn all_one(&self) -> bool {
        self.values.iter().all(|value| *value == 1)
    }
}

pub(super) fn counts(connection: &Connection, plan_id: &str) -> StoreResult<Counts> {
    let mut values = [0_i64; 7];
    for (index, table) in TABLES.iter().enumerate() {
        let sql = format!("SELECT count(*) FROM {table} WHERE candidate_judge_plan_id = ?1");
        values[index] = connection.query_row(&sql, params![plan_id], |row| row.get(0))?;
    }
    Ok(Counts { values })
}

pub(super) fn load_canonical(
    connection: &Connection,
    table: &str,
    id_column: &str,
    id: &str,
) -> StoreResult<Option<Vec<u8>>> {
    let sql = format!(
        "SELECT typeof(canonical_json), length(canonical_json), canonical_json FROM {table} WHERE {id_column} = ?1"
    );
    let mut statement = connection.prepare(&sql)?;
    let mut rows = statement.query(params![id])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };
    let kind: String = row.get(0)?;
    let bytes = row_decode::owned_blob(&kind, row.get(1)?, row.get(2)?)?;
    if rows.next()?.is_some() {
        return Err(StoreError::CorruptRecord);
    }
    Ok(Some(bytes))
}

pub(super) fn load_plan(connection: &Connection, plan_id: &str) -> StoreResult<Option<PlanRow>> {
    one_row(connection, row_decode::PLAN_SQL, plan_id, map_plan)
}

pub(super) fn load_schedule(
    connection: &Connection,
    plan_id: &str,
) -> StoreResult<Option<ScheduleRow>> {
    one_row(connection, row_decode::SCHEDULE_SQL, plan_id, map_schedule)
}

pub(super) fn load_request(
    connection: &Connection,
    plan_id: &str,
) -> StoreResult<Option<RequestRow>> {
    one_row(connection, row_decode::REQUEST_SQL, plan_id, map_request)
}

pub(super) fn load_response(
    connection: &Connection,
    plan_id: &str,
) -> StoreResult<Option<ResponseRow>> {
    one_row(connection, row_decode::RESPONSE_SQL, plan_id, map_response)
}

pub(super) fn load_batch(connection: &Connection, plan_id: &str) -> StoreResult<Option<BatchRow>> {
    one_row(connection, row_decode::BATCH_SQL, plan_id, map_batch)
}

pub(super) fn load_receipt(
    connection: &Connection,
    plan_id: &str,
) -> StoreResult<Option<ReceiptRow>> {
    one_row(connection, row_decode::RECEIPT_SQL, plan_id, map_receipt)
}

pub(super) fn load_join(connection: &Connection, plan_id: &str) -> StoreResult<Option<JoinRow>> {
    one_row(connection, row_decode::JOIN_SQL, plan_id, map_join)
}

fn one_row<T>(
    connection: &Connection,
    sql: &str,
    plan_id: &str,
    map: impl Fn(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
) -> StoreResult<Option<T>> {
    let mut statement = connection.prepare(sql)?;
    match statement.query_row(params![plan_id], map).optional() {
        Ok(value) => Ok(value),
        Err(error) => Err(row_error(error)),
    }
}

fn row_error(error: rusqlite::Error) -> StoreError {
    match error {
        rusqlite::Error::ToSqlConversionFailure(source) => match source.downcast::<StoreError>() {
            Ok(value) => *value,
            Err(source) => StoreError::Database(rusqlite::Error::ToSqlConversionFailure(source)),
        },
        other => StoreError::Database(other),
    }
}
