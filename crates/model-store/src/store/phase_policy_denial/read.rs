use rewrite_model::MAX_GENERATION_PHASE_POLICY_DENIAL_RECORD_JSON_BYTES;
use rusqlite::{Connection, Row, params};

use super::parents;
use super::record::{DenialRecord, DenialScope, REASON};
use crate::{StoreError, StoreResult};

pub(super) struct DenialRow {
    pub(super) id: String,
    pub(super) schema_version: i64,
    pub(super) kind: String,
    pub(super) system_id: String,
    pub(super) plan_id: String,
    pub(super) suite_id: String,
    pub(super) policy_digest: String,
    pub(super) reason: String,
    pub(super) bytes: Vec<u8>,
}

pub(super) fn load<R: DenialRecord>(
    connection: &Connection,
    relations: R::Relations<'_>,
) -> StoreResult<Option<R>> {
    let scope = R::scope(relations);
    parents::require(connection, scope)?;
    let Some(row) = load_scope(connection, R::TABLE, R::ID_COLUMN, R::KIND, scope)? else {
        return Ok(None);
    };
    let record = R::decode(&row.bytes, relations).map_err(|_| StoreError::CorruptRecord)?;
    if row.kind == R::KIND
        && row.reason == REASON
        && record.identity() == row.id
        && i64::from(record.schema_version()) == row.schema_version
        && record.system_id() == row.system_id
        && record.plan_id() == row.plan_id
        && record.suite_id() == row.suite_id
        && record.policy_digest() == row.policy_digest
    {
        Ok(Some(record))
    } else {
        Err(StoreError::CorruptRecord)
    }
}

pub(super) fn load_by_id(
    connection: &Connection,
    table: &str,
    id_column: &str,
    id: &str,
) -> StoreResult<Option<DenialRow>> {
    let sql = format!(
        "SELECT {id_column}, schema_version, record_kind, generation_system_id,
                generation_qualification_plan_id, generation_suite_manifest_id,
                phase_policy_digest, reason, typeof(canonical_json), length(canonical_json),
                canonical_json
         FROM {table} WHERE {id_column} = ?1"
    );
    one_row(connection, &sql, params![id])
}

fn load_scope(
    connection: &Connection,
    table: &str,
    id_column: &str,
    kind: &str,
    scope: DenialScope<'_>,
) -> StoreResult<Option<DenialRow>> {
    let sql = format!(
        "SELECT {id_column}, schema_version, record_kind, generation_system_id,
                generation_qualification_plan_id, generation_suite_manifest_id,
                phase_policy_digest, reason, typeof(canonical_json), length(canonical_json),
                canonical_json
         FROM {table}
         WHERE generation_system_id = ?1
           AND generation_qualification_plan_id = ?2
           AND generation_suite_manifest_id = ?3
           AND phase_policy_digest = ?4
           AND record_kind = ?5
           AND reason = ?6"
    );
    one_row(
        connection,
        &sql,
        params![
            scope.system_id,
            scope.plan_id,
            scope.suite_id,
            scope.policy_digest,
            kind,
            REASON,
        ],
    )
}

fn one_row(
    connection: &Connection,
    sql: &str,
    parameters: impl rusqlite::Params,
) -> StoreResult<Option<DenialRow>> {
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

fn read_row(row: &Row<'_>) -> StoreResult<DenialRow> {
    let kind: String = row.get(8)?;
    let length: i64 = row.get(9)?;
    let bytes: Vec<u8> = row.get(10)?;
    Ok(DenialRow {
        id: row.get(0)?,
        schema_version: row.get(1)?,
        kind: row.get(2)?,
        system_id: row.get(3)?,
        plan_id: row.get(4)?,
        suite_id: row.get(5)?,
        policy_digest: row.get(6)?,
        reason: row.get(7)?,
        bytes: blob(&kind, length, bytes)?,
    })
}

fn blob(kind: &str, length: i64, bytes: Vec<u8>) -> StoreResult<Vec<u8>> {
    let bounded = usize::try_from(length).ok().is_some_and(|value| {
        (1..=MAX_GENERATION_PHASE_POLICY_DENIAL_RECORD_JSON_BYTES).contains(&value)
    });
    if kind == "blob" && bounded && i64::try_from(bytes.len()).ok() == Some(length) {
        Ok(bytes)
    } else {
        Err(StoreError::CorruptRecord)
    }
}
