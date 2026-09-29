use rusqlite::Connection;

use crate::StoreResult;

const REQUEST_SQL: &str = "CREATE TABLE candidate_judge_request_aggregates (
    candidate_judge_request_aggregate_id TEXT PRIMARY KEY NOT NULL
        CHECK(length(candidate_judge_request_aggregate_id) = 64)
        CHECK(candidate_judge_request_aggregate_id = lower(candidate_judge_request_aggregate_id))
        CHECK(candidate_judge_request_aggregate_id NOT GLOB '*[^0-9a-f]*'),
    schema_version INTEGER NOT NULL CHECK(schema_version = 1),
    candidate_judge_plan_id TEXT NOT NULL
        CHECK(length(candidate_judge_plan_id) = 64)
        CHECK(candidate_judge_plan_id = lower(candidate_judge_plan_id))
        CHECK(candidate_judge_plan_id NOT GLOB '*[^0-9a-f]*'),
    candidate_judge_schedule_id TEXT NOT NULL
        CHECK(length(candidate_judge_schedule_id) = 64)
        CHECK(candidate_judge_schedule_id = lower(candidate_judge_schedule_id))
        CHECK(candidate_judge_schedule_id NOT GLOB '*[^0-9a-f]*'),
    entry_count INTEGER NOT NULL CHECK(entry_count BETWEEN 2 AND 512),
    canonical_json BLOB NOT NULL CHECK(length(canonical_json) BETWEEN 1 AND 262144),
    UNIQUE (
        candidate_judge_request_aggregate_id,
        candidate_judge_plan_id,
        candidate_judge_schedule_id,
        entry_count
    ),
    FOREIGN KEY (candidate_judge_plan_id, candidate_judge_schedule_id, entry_count)
        REFERENCES candidate_judge_schedules(
            candidate_judge_plan_id,
            candidate_judge_schedule_id,
            entry_count
        ) ON UPDATE RESTRICT ON DELETE RESTRICT
) STRICT;";

const RESPONSE_SQL: &str = "CREATE TABLE candidate_judge_response_aggregates (
    candidate_judge_response_aggregate_id TEXT PRIMARY KEY NOT NULL
        CHECK(length(candidate_judge_response_aggregate_id) = 64)
        CHECK(candidate_judge_response_aggregate_id =
            lower(candidate_judge_response_aggregate_id))
        CHECK(candidate_judge_response_aggregate_id NOT GLOB '*[^0-9a-f]*'),
    schema_version INTEGER NOT NULL CHECK(schema_version = 1),
    candidate_judge_plan_id TEXT NOT NULL
        CHECK(length(candidate_judge_plan_id) = 64)
        CHECK(candidate_judge_plan_id = lower(candidate_judge_plan_id))
        CHECK(candidate_judge_plan_id NOT GLOB '*[^0-9a-f]*'),
    candidate_judge_schedule_id TEXT NOT NULL
        CHECK(length(candidate_judge_schedule_id) = 64)
        CHECK(candidate_judge_schedule_id = lower(candidate_judge_schedule_id))
        CHECK(candidate_judge_schedule_id NOT GLOB '*[^0-9a-f]*'),
    candidate_judge_request_aggregate_id TEXT NOT NULL
        CHECK(length(candidate_judge_request_aggregate_id) = 64)
        CHECK(candidate_judge_request_aggregate_id =
            lower(candidate_judge_request_aggregate_id))
        CHECK(candidate_judge_request_aggregate_id NOT GLOB '*[^0-9a-f]*'),
    entry_count INTEGER NOT NULL CHECK(entry_count BETWEEN 2 AND 512),
    canonical_json BLOB NOT NULL CHECK(length(canonical_json) BETWEEN 1 AND 262144),
    UNIQUE (
        candidate_judge_response_aggregate_id,
        candidate_judge_request_aggregate_id,
        candidate_judge_plan_id,
        candidate_judge_schedule_id,
        entry_count
    ),
    FOREIGN KEY (
        candidate_judge_request_aggregate_id,
        candidate_judge_plan_id,
        candidate_judge_schedule_id,
        entry_count
    ) REFERENCES candidate_judge_request_aggregates(
        candidate_judge_request_aggregate_id,
        candidate_judge_plan_id,
        candidate_judge_schedule_id,
        entry_count
    ) ON UPDATE RESTRICT ON DELETE RESTRICT
) STRICT;";

pub(super) fn create(connection: &Connection) -> StoreResult<()> {
    connection.execute_batch(REQUEST_SQL)?;
    connection.execute_batch(RESPONSE_SQL)?;
    Ok(())
}
