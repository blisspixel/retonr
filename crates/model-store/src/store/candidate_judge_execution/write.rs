use rusqlite::Connection;

use super::CandidateJudgeExecutionV1WriteDisposition;
use super::codec::PreparedCohort;
use crate::{StoreError, StoreResult, WriteDisposition};

mod aggregates;
mod closure;
mod confirm;
mod plan_schedule;

pub(super) fn insert_cohort(
    connection: &Connection,
    prepared: &PreparedCohort<'_>,
) -> StoreResult<CandidateJudgeExecutionV1WriteDisposition> {
    let written = [
        plan_schedule::insert_plan(connection, prepared)?,
        plan_schedule::insert_schedule(connection, prepared)?,
        aggregates::insert_requests(connection, prepared)?,
        aggregates::insert_responses(connection, prepared)?,
        aggregates::insert_observations(connection, prepared)?,
        closure::insert_receipt(connection, prepared)?,
        closure::insert_join(connection, prepared)?,
    ];
    let disposition = require_uniform(&written)?;
    confirm::stored(connection, prepared)?;
    Ok(CandidateJudgeExecutionV1WriteDisposition {
        plan: disposition,
        schedule: disposition,
        request_aggregate: disposition,
        response_aggregate: disposition,
        observation_batch: disposition,
        managed_receipt: disposition,
        join: disposition,
    })
}

pub(super) fn insert(
    connection: &Connection,
    sql: &str,
    parameters: impl rusqlite::Params,
) -> StoreResult<WriteDisposition> {
    match connection.execute(sql, parameters)? {
        1 => Ok(WriteDisposition::Inserted),
        0 => Ok(WriteDisposition::AlreadyPresent),
        _ => Err(StoreError::CorruptRecord),
    }
}

fn require_uniform(values: &[WriteDisposition]) -> StoreResult<WriteDisposition> {
    let first = values.first().copied().ok_or(StoreError::CorruptRecord)?;
    if values.iter().all(|value| *value == first) {
        Ok(first)
    } else {
        Err(StoreError::ImmutableConflict)
    }
}
