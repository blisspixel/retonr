use rewrite_model::{
    CandidateDeterministicEvaluationRecordV1, MAX_CANDIDATE_DETERMINISTIC_EVALUATION_JSON_BYTES,
};
use rusqlite::{Connection, params};

use super::parents::{self, Cited};
use super::read;
use crate::{StoreError, StoreResult, WriteDisposition};

pub(super) fn write_one(
    connection: &Connection,
    record: &CandidateDeterministicEvaluationRecordV1,
) -> StoreResult<WriteDisposition> {
    let json = canonical_json(record)?;
    let cited = Cited::from_record(record);
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

fn canonical_json(record: &CandidateDeterministicEvaluationRecordV1) -> StoreResult<Vec<u8>> {
    let bytes = serde_json::to_vec(record)?;
    if (1..=MAX_CANDIDATE_DETERMINISTIC_EVALUATION_JSON_BYTES).contains(&bytes.len()) {
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
        "INSERT OR IGNORE INTO candidate_deterministic_evaluation_records (
            candidate_deterministic_evaluation_id, schema_version,
            candidate_a_receipt_set_id, candidate_b_receipt_set_id, canonical_json
         ) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            cited.id,
            cited.schema_version,
            cited.receipt_a,
            cited.receipt_b,
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
