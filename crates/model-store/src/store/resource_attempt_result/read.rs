use rewrite_model::{
    GenerationResourceAttemptResultRecordV1, GenerationResourceObservationProfileV1,
    MAX_GENERATION_RESOURCE_ATTEMPT_RESULT_JSON_BYTES,
};
use rusqlite::{Connection, Row, params};

use super::parents::{self, CitedParents};
use crate::{StoreError, StoreResult};

const TABLE: &str = "generation_resource_attempt_result_records";
const ID_COLUMN: &str = "generation_resource_attempt_result_id";

pub(super) struct ResultRow {
    pub(super) id: String,
    pub(super) schema_version: i64,
    pub(super) system_id: String,
    pub(super) plan_id: String,
    pub(super) suite_id: String,
    pub(super) case_id: String,
    pub(super) repetition_id: String,
    pub(super) planned_attempt_id: String,
    pub(super) attempt_record_id: String,
    pub(super) receipt_id: String,
    pub(super) policy_digest: String,
    pub(super) profile: String,
    pub(super) bytes: Vec<u8>,
}

pub(super) const fn profile_literal(
    profile: GenerationResourceObservationProfileV1,
) -> &'static str {
    match profile {
        GenerationResourceObservationProfileV1::ManagedOllamaV0_32_15LinuxV1 => {
            "managed_ollama_v0_32_15_linux_v1"
        }
    }
}

pub(super) fn load(
    connection: &Connection,
    record: &GenerationResourceAttemptResultRecordV1,
) -> StoreResult<Option<GenerationResourceAttemptResultRecordV1>> {
    let cited = CitedParents::from_record(record);
    parents::require(connection, &cited)?;
    let slot = load_slot(connection, cited.plan_id, cited.planned_attempt_id)?;
    let by_id = load_by_id(connection, TABLE, ID_COLUMN, record_id(record))?;
    match (slot, by_id) {
        (None, None) => Ok(None),
        (Some(slot_row), Some(id_row))
            if slot_row.id == id_row.id && agrees(&slot_row, record, &cited) =>
        {
            Ok(Some(record.clone()))
        }
        _ => Err(StoreError::CorruptRecord),
    }
}

pub(super) fn load_by_id(
    connection: &Connection,
    table: &str,
    id_column: &str,
    id: &str,
) -> StoreResult<Option<ResultRow>> {
    let sql = format!(
        "SELECT {id_column}, schema_version, generation_system_id,
                generation_qualification_plan_id, generation_suite_manifest_id,
                generation_case_id, generation_repetition_id, planned_candidate_attempt_id,
                candidate_generation_attempt_record_id, candidate_generation_receipt_id,
                resource_policy_digest, observation_profile, typeof(canonical_json),
                length(canonical_json), canonical_json
         FROM {table} WHERE {id_column} = ?1"
    );
    one_row(connection, &sql, params![id])
}

fn load_slot(
    connection: &Connection,
    plan_id: &str,
    planned_attempt_id: &str,
) -> StoreResult<Option<ResultRow>> {
    one_row(
        connection,
        "SELECT generation_resource_attempt_result_id, schema_version, generation_system_id,
                generation_qualification_plan_id, generation_suite_manifest_id,
                generation_case_id, generation_repetition_id, planned_candidate_attempt_id,
                candidate_generation_attempt_record_id, candidate_generation_receipt_id,
                resource_policy_digest, observation_profile, typeof(canonical_json),
                length(canonical_json), canonical_json
         FROM generation_resource_attempt_result_records
         WHERE generation_qualification_plan_id = ?1 AND planned_candidate_attempt_id = ?2",
        params![plan_id, planned_attempt_id],
    )
}

fn agrees(
    row: &ResultRow,
    record: &GenerationResourceAttemptResultRecordV1,
    cited: &CitedParents<'_>,
) -> bool {
    let Ok(json) = serde_json::to_vec(record) else {
        return false;
    };
    row.id == record_id(record)
        && row.schema_version == i64::from(record.schema_version())
        && row.system_id == cited.system_id
        && row.plan_id == cited.plan_id
        && row.suite_id == cited.suite_id
        && row.case_id == cited.case_id
        && row.repetition_id == cited.repetition_id
        && row.planned_attempt_id == cited.planned_attempt_id
        && row.attempt_record_id == cited.attempt_record_id
        && row.receipt_id == cited.receipt_id
        && row.policy_digest == cited.policy_digest
        && row.profile == profile_literal(record.observation_profile())
        && row.bytes == json
}

fn record_id(record: &GenerationResourceAttemptResultRecordV1) -> &str {
    record.resource_attempt_result_id().digest().as_str()
}

fn one_row(
    connection: &Connection,
    sql: &str,
    parameters: impl rusqlite::Params,
) -> StoreResult<Option<ResultRow>> {
    let mut statement = connection.prepare(sql)?;
    let mut rows = statement.query(parameters)?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };
    let stored = read_row(row)?;
    if rows.next()?.is_some() {
        Err(StoreError::CorruptRecord)
    } else {
        Ok(Some(stored))
    }
}

fn read_row(row: &Row<'_>) -> StoreResult<ResultRow> {
    let kind: String = row.get(12)?;
    let length: i64 = row.get(13)?;
    let bytes: Vec<u8> = row.get(14)?;
    Ok(ResultRow {
        id: row.get(0)?,
        schema_version: row.get(1)?,
        system_id: row.get(2)?,
        plan_id: row.get(3)?,
        suite_id: row.get(4)?,
        case_id: row.get(5)?,
        repetition_id: row.get(6)?,
        planned_attempt_id: row.get(7)?,
        attempt_record_id: row.get(8)?,
        receipt_id: row.get(9)?,
        policy_digest: row.get(10)?,
        profile: row.get(11)?,
        bytes: blob(&kind, length, bytes)?,
    })
}

fn blob(kind: &str, length: i64, bytes: Vec<u8>) -> StoreResult<Vec<u8>> {
    let bounded = usize::try_from(length).ok().is_some_and(|value| {
        (1..=MAX_GENERATION_RESOURCE_ATTEMPT_RESULT_JSON_BYTES).contains(&value)
    });
    if kind == "blob" && bounded && i64::try_from(bytes.len()).ok() == Some(length) {
        Ok(bytes)
    } else {
        Err(StoreError::CorruptRecord)
    }
}
