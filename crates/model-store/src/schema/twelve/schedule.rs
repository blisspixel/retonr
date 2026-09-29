use rusqlite::Connection;

use crate::StoreResult;

const SCHEDULE_SQL: &str = "CREATE TABLE candidate_judge_schedules (
    candidate_judge_schedule_id TEXT PRIMARY KEY NOT NULL
        CHECK(length(candidate_judge_schedule_id) = 64)
        CHECK(candidate_judge_schedule_id = lower(candidate_judge_schedule_id))
        CHECK(candidate_judge_schedule_id NOT GLOB '*[^0-9a-f]*'),
    schema_version INTEGER NOT NULL CHECK(schema_version = 1),
    candidate_judge_plan_id TEXT NOT NULL
        CHECK(length(candidate_judge_plan_id) = 64)
        CHECK(candidate_judge_plan_id = lower(candidate_judge_plan_id))
        CHECK(candidate_judge_plan_id NOT GLOB '*[^0-9a-f]*'),
    candidate_receipt_pair_set_id TEXT NOT NULL
        CHECK(length(candidate_receipt_pair_set_id) = 64)
        CHECK(candidate_receipt_pair_set_id = lower(candidate_receipt_pair_set_id))
        CHECK(candidate_receipt_pair_set_id NOT GLOB '*[^0-9a-f]*'),
    case_count INTEGER NOT NULL CHECK(case_count BETWEEN 1 AND 256),
    presentation_seed TEXT NOT NULL
        CHECK(length(presentation_seed) BETWEEN 1 AND 20)
        CHECK(presentation_seed NOT GLOB '*[^0-9]*')
        CHECK(presentation_seed = '0' OR substr(presentation_seed, 1, 1) <> '0'),
    entry_count INTEGER NOT NULL
        CHECK(entry_count BETWEEN 2 AND 512 AND entry_count = case_count * 2),
    canonical_json BLOB NOT NULL CHECK(length(canonical_json) BETWEEN 1 AND 262144),
    UNIQUE (candidate_judge_plan_id, candidate_receipt_pair_set_id),
    UNIQUE (candidate_judge_plan_id, candidate_judge_schedule_id, entry_count),
    FOREIGN KEY (candidate_judge_plan_id, case_count, presentation_seed)
        REFERENCES candidate_judge_plans(
            candidate_judge_plan_id,
            case_count,
            presentation_seed
        ) ON UPDATE RESTRICT ON DELETE RESTRICT
) STRICT;";

pub(super) fn create(connection: &Connection) -> StoreResult<()> {
    connection.execute_batch(SCHEDULE_SQL)?;
    Ok(())
}
