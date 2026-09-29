use rusqlite::Connection;
use serde_json::Value;

use super::super::codec::PreparedCohort;
use super::super::rows;
use super::super::text;
use crate::{StoreError, StoreResult};

pub(super) fn stored(connection: &Connection, prepared: &PreparedCohort<'_>) -> StoreResult<()> {
    confirm(
        connection,
        "candidate_judge_plans",
        "candidate_judge_plan_id",
        text::digest_text(prepared.records.plan.candidate_judge_plan_id().digest()),
        &prepared.encoded.plan,
    )?;
    confirm(
        connection,
        "candidate_judge_schedules",
        "candidate_judge_schedule_id",
        text::digest_text(
            prepared
                .records
                .schedule
                .candidate_judge_schedule_id()
                .digest(),
        ),
        &prepared.encoded.schedule,
    )?;
    confirm(
        connection,
        "candidate_judge_request_aggregates",
        "candidate_judge_request_aggregate_id",
        text::digest_text(prepared.records.requests.request_aggregate_id().digest()),
        &prepared.encoded.requests,
    )?;
    confirm(
        connection,
        "candidate_judge_response_aggregates",
        "candidate_judge_response_aggregate_id",
        text::digest_text(prepared.records.responses.response_aggregate_id().digest()),
        &prepared.encoded.responses,
    )?;
    confirm(
        connection,
        "candidate_judge_observation_batches",
        "candidate_judge_observation_batch_id",
        text::digest_text(
            prepared
                .records
                .observations
                .observation_batch_id()
                .digest(),
        ),
        &prepared.encoded.observations,
    )?;
    confirm(
        connection,
        "managed_local_judge_receipts",
        "managed_local_judge_receipt_id",
        text::digest_text(
            prepared
                .records
                .receipt
                .managed_local_judge_receipt_id()
                .digest(),
        ),
        &prepared.encoded.receipt,
    )?;
    confirm(
        connection,
        "candidate_judge_join_records",
        "candidate_judge_join_id",
        text::digest_text(prepared.records.join.candidate_judge_join_id().digest()),
        &prepared.encoded.join,
    )
}

fn confirm(
    connection: &Connection,
    table: &str,
    id_column: &str,
    id: &str,
    expected: &[u8],
) -> StoreResult<()> {
    let stored =
        rows::load_canonical(connection, table, id_column, id)?.ok_or(StoreError::CorruptRecord)?;
    if stored == expected {
        Ok(())
    } else if serde_json::from_slice::<Value>(&stored).is_ok() {
        Err(StoreError::ImmutableConflict)
    } else {
        Err(StoreError::CorruptRecord)
    }
}
