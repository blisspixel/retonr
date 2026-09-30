use rewrite_model::{
    GenerationRepeatabilityResultRecordV1, MAX_GENERATION_REPEATABILITY_RESULT_JSON_BYTES,
};
use rusqlite::{Connection, params};

use super::parents::{self, Cited};
use super::read;
use crate::{StoreError, StoreResult, WriteDisposition};

pub(super) fn write_one(
    connection: &Connection,
    record: &GenerationRepeatabilityResultRecordV1,
) -> StoreResult<WriteDisposition> {
    let json = canonical_json(record)?;
    let cited = Cited::from_record(record)?;
    parents::require(connection, &cited)?;
    if let Some(owner) = read::json_owner(connection, &json)?
        && owner != cited.id
    {
        return Err(StoreError::CorruptRecord);
    }
    let disposition = insert(connection, &cited, &json)?;
    confirm(connection, &cited, &json)?;
    Ok(disposition)
}

fn canonical_json(record: &GenerationRepeatabilityResultRecordV1) -> StoreResult<Vec<u8>> {
    let bytes = serde_json::to_vec(record)?;
    if (1..=MAX_GENERATION_REPEATABILITY_RESULT_JSON_BYTES).contains(&bytes.len()) {
        Ok(bytes)
    } else {
        Err(StoreError::RecordTooLarge)
    }
}

fn insert(
    connection: &Connection,
    cited: &Cited<'_>,
    json: &[u8],
) -> StoreResult<WriteDisposition> {
    match connection.execute(
        "INSERT OR IGNORE INTO generation_repeatability_terminal_result_records (
            generation_repeatability_result_id, generation_system_id,
            generation_qualification_plan_id, generation_suite_manifest_id,
            generation_repetition_id, generation_attempt_ledger_manifest_id, terminal_stage,
            candidate_generation_receipt_set_id, candidate_deterministic_evaluation_id,
            candidate_judge_join_id, canonical_json
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            cited.id,
            cited.system_id,
            cited.plan_id,
            cited.suite_id,
            cited.repetition_id,
            cited.ledger_id,
            cited.stage,
            cited.receipt_id,
            cited.evaluation_id,
            cited.judge_id,
            json,
        ],
    )? {
        1 => Ok(WriteDisposition::Inserted),
        0 => Ok(WriteDisposition::AlreadyPresent),
        _ => Err(StoreError::CorruptRecord),
    }
}

fn confirm(connection: &Connection, cited: &Cited<'_>, json: &[u8]) -> StoreResult<()> {
    let Some(row) = read::load_by_id(connection, cited.id)? else {
        return Err(StoreError::CorruptRecord);
    };
    if read::matches(&row, cited, json) {
        Ok(())
    } else {
        Err(StoreError::ImmutableConflict)
    }
}
