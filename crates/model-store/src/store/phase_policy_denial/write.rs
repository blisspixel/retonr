use rewrite_model::MAX_GENERATION_PHASE_POLICY_DENIAL_RECORD_JSON_BYTES;
use rusqlite::{Connection, params};

use super::parents;
use super::read::{self, DenialRow};
use super::record::{self, DenialRecord, REASON};
use crate::{StoreError, StoreResult, WriteDisposition};

pub(super) struct DenialBind<'a> {
    table: &'static str,
    id_column: &'static str,
    kind: &'static str,
    id: &'a str,
    schema_version: i64,
    system_id: &'a str,
    plan_id: &'a str,
    suite_id: &'a str,
    policy_digest: &'a str,
    json: &'a [u8],
}

pub(super) fn write_one<R: DenialRecord>(
    connection: &Connection,
    record: &R,
    relations: R::Relations<'_>,
) -> StoreResult<WriteDisposition> {
    record.validate(relations).map_err(record::map_caller)?;
    let json = canonical_json(record)?;
    parents::require(connection, R::scope(relations))?;
    let bind = DenialBind {
        table: R::TABLE,
        id_column: R::ID_COLUMN,
        kind: R::KIND,
        id: record.identity(),
        schema_version: i64::from(record.schema_version()),
        system_id: record.system_id(),
        plan_id: record.plan_id(),
        suite_id: record.suite_id(),
        policy_digest: record.policy_digest(),
        json: &json,
    };
    let disposition = insert(connection, &bind)?;
    confirm(connection, &bind)?;
    Ok(disposition)
}

fn canonical_json<R: DenialRecord>(record: &R) -> StoreResult<Vec<u8>> {
    let bytes = serde_json::to_vec(record).map_err(StoreError::Serialization)?;
    if (1..=MAX_GENERATION_PHASE_POLICY_DENIAL_RECORD_JSON_BYTES).contains(&bytes.len()) {
        Ok(bytes)
    } else {
        Err(StoreError::RecordTooLarge)
    }
}

fn insert(connection: &Connection, bind: &DenialBind<'_>) -> StoreResult<WriteDisposition> {
    let sql = format!(
        "INSERT OR IGNORE INTO {table} (
            {id_column}, schema_version, record_kind, generation_system_id,
            generation_qualification_plan_id, generation_suite_manifest_id,
            phase_policy_digest, reason, canonical_json
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        table = bind.table,
        id_column = bind.id_column,
    );
    match connection.execute(
        &sql,
        params![
            bind.id,
            bind.schema_version,
            bind.kind,
            bind.system_id,
            bind.plan_id,
            bind.suite_id,
            bind.policy_digest,
            REASON,
            bind.json,
        ],
    )? {
        1 => Ok(WriteDisposition::Inserted),
        0 => Ok(WriteDisposition::AlreadyPresent),
        _ => Err(StoreError::CorruptRecord),
    }
}

fn confirm(connection: &Connection, bind: &DenialBind<'_>) -> StoreResult<()> {
    let row = read::load_by_id(connection, bind.table, bind.id_column, bind.id)?
        .ok_or(StoreError::CorruptRecord)?;
    if agrees(&row, bind) {
        Ok(())
    } else {
        Err(StoreError::ImmutableConflict)
    }
}

fn agrees(row: &DenialRow, bind: &DenialBind<'_>) -> bool {
    row.id == bind.id
        && row.schema_version == bind.schema_version
        && row.kind == bind.kind
        && row.system_id == bind.system_id
        && row.plan_id == bind.plan_id
        && row.suite_id == bind.suite_id
        && row.policy_digest == bind.policy_digest
        && row.reason == REASON
        && row.bytes == bind.json
}
