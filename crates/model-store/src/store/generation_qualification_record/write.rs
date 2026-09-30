use rewrite_model::{
    GenerationQualificationRecordV1, MAX_GENERATION_QUALIFICATION_RECORD_JSON_BYTES,
};
use rusqlite::{Connection, params};

use super::parents::{self, Cited};
use super::read;
use crate::{StoreError, StoreResult, WriteDisposition};

pub(super) fn write_one(
    connection: &Connection,
    record: &GenerationQualificationRecordV1,
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

fn canonical_json(record: &GenerationQualificationRecordV1) -> StoreResult<Vec<u8>> {
    let bytes = serde_json::to_vec(record)?;
    if (1..=MAX_GENERATION_QUALIFICATION_RECORD_JSON_BYTES).contains(&bytes.len()) {
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
        "INSERT OR IGNORE INTO generation_qualification_records (
            generation_qualification_id, target_generation_system_id,
            baseline_generation_system_id, operation_policy_id, request_projection_id,
            generation_qualification_platform_evidence_id,
            generation_qualification_license_evidence_id,
            generation_qualification_operation_receipt_id, status, canonical_json
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            cited.id,
            cited.target,
            cited.baseline,
            cited.policy,
            cited.projection,
            cited.platform,
            cited.license,
            cited.receipt,
            cited.status,
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
