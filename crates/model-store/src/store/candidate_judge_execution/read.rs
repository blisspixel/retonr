use rusqlite::Connection;

use super::codec::{self, CohortRecords};
use super::derive;
use super::rows::{
    self, BatchRow, JoinRow, PlanRow, ReceiptRow, RequestRow, ResponseRow, ScheduleRow,
};
use super::{CandidateJudgeExecutionV1ReadInput, StoredCandidateJudgeExecutionV1};
use crate::{StoreError, StoreResult};

mod check;
mod decode;

struct Present {
    plan: PlanRow,
    schedule: ScheduleRow,
    request: RequestRow,
    response: ResponseRow,
    batch: BatchRow,
    receipt: ReceiptRow,
    join: JoinRow,
}

pub(super) fn load(
    connection: &Connection,
    input: &CandidateJudgeExecutionV1ReadInput<'_>,
) -> StoreResult<Option<StoredCandidateJudgeExecutionV1>> {
    let before = connection.total_changes();
    let stored = load_cohort(connection, input)?;
    if connection.total_changes() == before {
        Ok(stored)
    } else {
        Err(StoreError::CorruptRecord)
    }
}

pub(super) fn confirm_snapshot(
    connection: &Connection,
    expected: &StoredCandidateJudgeExecutionV1,
) -> StoreResult<()> {
    let plan_id = expected.plan().candidate_judge_plan_id().digest().as_str();
    if !rows::counts(connection, plan_id)?.all_one() {
        return Err(StoreError::CorruptRecord);
    }
    let stored = present(connection, plan_id)?;
    let closed = derive::close_expected(expected)?;
    check::snapshot_columns(&stored, expected, &closed)?;
    if stored.plan.bytes != serde_json::to_vec(expected.plan())?
        || stored.schedule.bytes != serde_json::to_vec(expected.schedule())?
        || stored.request.bytes != serde_json::to_vec(expected.request_aggregate())?
        || stored.response.bytes != serde_json::to_vec(expected.response_aggregate())?
        || stored.batch.bytes != serde_json::to_vec(expected.observation_batch())?
        || stored.receipt.bytes != serde_json::to_vec(expected.managed_receipt())?
        || stored.join.bytes != serde_json::to_vec(expected.join())?
    {
        return Err(StoreError::CorruptRecord);
    }
    Ok(())
}

fn load_cohort(
    connection: &Connection,
    input: &CandidateJudgeExecutionV1ReadInput<'_>,
) -> StoreResult<Option<StoredCandidateJudgeExecutionV1>> {
    let facts = codec::facts_from_read(input);
    let (records, loaded) = codec::reconstruct(connection, &facts, None)?;
    let plan_id = records.plan.candidate_judge_plan_id().digest().as_str();
    let counts = rows::counts(connection, plan_id)?;
    if counts.all_zero() {
        return Ok(None);
    }
    if !counts.all_one() {
        return Err(StoreError::CorruptRecord);
    }
    let present = present(connection, plan_id)?;
    let decoded = decode::cohort(&present, &records, &loaded, &facts)?;
    let closed = derive::close(&decoded)?;
    check::columns(&present, &decoded, &closed)?;
    if !same_cohort(&decoded, &records) {
        return Err(StoreError::CorruptRecord);
    }
    Ok(Some(StoredCandidateJudgeExecutionV1::from_cohort(decoded)))
}

fn present(connection: &Connection, plan_id: &str) -> StoreResult<Present> {
    Ok(Present {
        plan: rows::load_plan(connection, plan_id)?.ok_or(StoreError::CorruptRecord)?,
        schedule: rows::load_schedule(connection, plan_id)?.ok_or(StoreError::CorruptRecord)?,
        request: rows::load_request(connection, plan_id)?.ok_or(StoreError::CorruptRecord)?,
        response: rows::load_response(connection, plan_id)?.ok_or(StoreError::CorruptRecord)?,
        batch: rows::load_batch(connection, plan_id)?.ok_or(StoreError::CorruptRecord)?,
        receipt: rows::load_receipt(connection, plan_id)?.ok_or(StoreError::CorruptRecord)?,
        join: rows::load_join(connection, plan_id)?.ok_or(StoreError::CorruptRecord)?,
    })
}

fn same_cohort(decoded: &CohortRecords, reconstructed: &CohortRecords) -> bool {
    decoded.plan == reconstructed.plan
        && decoded.schedule == reconstructed.schedule
        && decoded.requests == reconstructed.requests
        && decoded.responses == reconstructed.responses
        && decoded.observations == reconstructed.observations
        && decoded.receipt == reconstructed.receipt
        && decoded.join == reconstructed.join
}
