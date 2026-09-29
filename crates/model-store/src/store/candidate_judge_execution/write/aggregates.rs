use rusqlite::{Connection, params};

use super::super::codec::PreparedCohort;
use super::super::text;
use super::insert;
use crate::{StoreResult, WriteDisposition};

pub(super) fn insert_requests(
    connection: &Connection,
    prepared: &PreparedCohort<'_>,
) -> StoreResult<WriteDisposition> {
    let value = &prepared.records.requests;
    insert(
        connection,
        "INSERT OR IGNORE INTO candidate_judge_request_aggregates (
            candidate_judge_request_aggregate_id, schema_version, candidate_judge_plan_id,
            candidate_judge_schedule_id, entry_count, canonical_json
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            text::digest_text(value.request_aggregate_id().digest()),
            i64::from(value.schema_version()),
            text::digest_text(value.candidate_judge_plan_id().digest()),
            text::digest_text(value.candidate_judge_schedule_id().digest()),
            prepared.closed.entry_count,
            &prepared.encoded.requests,
        ],
    )
}

pub(super) fn insert_responses(
    connection: &Connection,
    prepared: &PreparedCohort<'_>,
) -> StoreResult<WriteDisposition> {
    let value = &prepared.records.responses;
    insert(
        connection,
        "INSERT OR IGNORE INTO candidate_judge_response_aggregates (
            candidate_judge_response_aggregate_id, schema_version, candidate_judge_plan_id,
            candidate_judge_schedule_id, candidate_judge_request_aggregate_id, entry_count,
            canonical_json
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            text::digest_text(value.response_aggregate_id().digest()),
            i64::from(value.schema_version()),
            text::digest_text(value.candidate_judge_plan_id().digest()),
            text::digest_text(value.candidate_judge_schedule_id().digest()),
            text::digest_text(value.candidate_judge_request_aggregate_id().digest()),
            prepared.closed.entry_count,
            &prepared.encoded.responses,
        ],
    )
}

pub(super) fn insert_observations(
    connection: &Connection,
    prepared: &PreparedCohort<'_>,
) -> StoreResult<WriteDisposition> {
    let value = &prepared.records.observations;
    insert(
        connection,
        "INSERT OR IGNORE INTO candidate_judge_observation_batches (
            candidate_judge_observation_batch_id, schema_version, candidate_judge_plan_id,
            candidate_judge_schedule_id, candidate_judge_request_aggregate_id, entry_count,
            canonical_json
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            text::digest_text(value.observation_batch_id().digest()),
            1_i64,
            text::digest_text(value.candidate_judge_plan_id().digest()),
            text::digest_text(value.candidate_judge_schedule_id().digest()),
            text::digest_text(value.candidate_judge_request_aggregate_id().digest()),
            prepared.closed.entry_count,
            &prepared.encoded.observations,
        ],
    )
}
