use rewrite_model::{
    GenerationResourceAttemptResultRecordV1, MAX_GENERATION_RESOURCE_ATTEMPT_RESULT_JSON_BYTES,
};
use rusqlite::{Connection, params};

use super::parents::{self, CitedParents};
use super::read::{self, ResultRow};
use crate::{StoreError, StoreResult, WriteDisposition};

const TABLE: &str = "generation_resource_attempt_result_records";
const ID_COLUMN: &str = "generation_resource_attempt_result_id";

struct ResultBind<'a> {
    cited: CitedParents<'a>,
    id: &'a str,
    schema_version: i64,
    profile: &'static str,
    json: &'a [u8],
}

pub(super) fn write_one(
    connection: &Connection,
    record: &GenerationResourceAttemptResultRecordV1,
) -> StoreResult<WriteDisposition> {
    let json = canonical_json(record)?;
    let bind = bind_record(record, &json);
    parents::require(connection, &bind.cited)?;
    let disposition = insert(connection, &bind)?;
    confirm(connection, &bind)?;
    Ok(disposition)
}

fn bind_record<'a>(
    record: &'a GenerationResourceAttemptResultRecordV1,
    json: &'a [u8],
) -> ResultBind<'a> {
    ResultBind {
        cited: CitedParents::from_record(record),
        id: record.resource_attempt_result_id().digest().as_str(),
        schema_version: i64::from(record.schema_version()),
        profile: read::profile_literal(record.observation_profile()),
        json,
    }
}

fn canonical_json(record: &GenerationResourceAttemptResultRecordV1) -> StoreResult<Vec<u8>> {
    let bytes = serde_json::to_vec(record).map_err(StoreError::Serialization)?;
    if (1..=MAX_GENERATION_RESOURCE_ATTEMPT_RESULT_JSON_BYTES).contains(&bytes.len()) {
        Ok(bytes)
    } else {
        Err(StoreError::RecordTooLarge)
    }
}

fn insert(connection: &Connection, bind: &ResultBind<'_>) -> StoreResult<WriteDisposition> {
    match connection.execute(
        "INSERT OR IGNORE INTO generation_resource_attempt_result_records (
            generation_resource_attempt_result_id, schema_version, generation_system_id,
            generation_qualification_plan_id, generation_suite_manifest_id, generation_case_id,
            generation_repetition_id, planned_candidate_attempt_id,
            candidate_generation_attempt_record_id, candidate_generation_receipt_id,
            resource_policy_digest, observation_profile, canonical_json
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            bind.id,
            bind.schema_version,
            bind.cited.system_id,
            bind.cited.plan_id,
            bind.cited.suite_id,
            bind.cited.case_id,
            bind.cited.repetition_id,
            bind.cited.planned_attempt_id,
            bind.cited.attempt_record_id,
            bind.cited.receipt_id,
            bind.cited.policy_digest,
            bind.profile,
            bind.json,
        ],
    )? {
        1 => Ok(WriteDisposition::Inserted),
        0 => Ok(WriteDisposition::AlreadyPresent),
        _ => Err(StoreError::CorruptRecord),
    }
}

fn confirm(connection: &Connection, bind: &ResultBind<'_>) -> StoreResult<()> {
    let Some(row) = read::load_by_id(connection, TABLE, ID_COLUMN, bind.id)? else {
        return Err(StoreError::CorruptRecord);
    };
    if agrees(&row, bind) {
        Ok(())
    } else {
        Err(StoreError::ImmutableConflict)
    }
}

fn agrees(row: &ResultRow, bind: &ResultBind<'_>) -> bool {
    row.id == bind.id
        && row.schema_version == bind.schema_version
        && row.system_id == bind.cited.system_id
        && row.plan_id == bind.cited.plan_id
        && row.suite_id == bind.cited.suite_id
        && row.case_id == bind.cited.case_id
        && row.repetition_id == bind.cited.repetition_id
        && row.planned_attempt_id == bind.cited.planned_attempt_id
        && row.attempt_record_id == bind.cited.attempt_record_id
        && row.receipt_id == bind.cited.receipt_id
        && row.policy_digest == bind.cited.policy_digest
        && row.profile == bind.profile
        && row.bytes == bind.json
}
