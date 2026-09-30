use rewrite_model::{
    CandidateGenerationReceiptSetV1, MAX_CANDIDATE_GENERATION_RECEIPT_SET_JSON_BYTES,
};
use rusqlite::{Connection, params};

use super::parents::{self, Cited};
use super::read;
use crate::{StoreError, StoreResult, WriteDisposition};

pub(super) fn write_one(
    connection: &Connection,
    record: &CandidateGenerationReceiptSetV1,
) -> StoreResult<WriteDisposition> {
    let json = canonical_json(record)?;
    let cited = Cited::from_record(record);
    parents::require(connection, &cited)?;
    let disposition = insert(connection, &cited, &json)?;
    confirm(connection, &cited, &json)?;
    Ok(disposition)
}

fn canonical_json(record: &CandidateGenerationReceiptSetV1) -> StoreResult<Vec<u8>> {
    let bytes = serde_json::to_vec(record)?;
    if (1..=MAX_CANDIDATE_GENERATION_RECEIPT_SET_JSON_BYTES).contains(&bytes.len()) {
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
        "INSERT OR IGNORE INTO candidate_generation_receipt_sets (
            candidate_generation_receipt_set_id, schema_version,
            generation_qualification_plan_id, generation_suite_manifest_id,
            generation_repetition_id, generation_system_id, candidate_selection_policy_id,
            entry_count, canonical_json
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            cited.id,
            cited.schema_version,
            cited.plan_id,
            cited.suite_id,
            cited.repetition_id,
            cited.system_id,
            cited.policy_id,
            cited.entry_count,
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
