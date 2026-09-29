use rusqlite::{Connection, OptionalExtension as _};

use super::record::DenialScope;
use crate::{StoreError, StoreResult};

pub(super) fn require(connection: &Connection, scope: DenialScope<'_>) -> StoreResult<()> {
    if !row_exists(
        connection,
        "generation_system_records",
        "generation_system_id",
        scope.system_id,
    )? {
        return Err(StoreError::MissingRecord);
    }
    let plan_suite = stored_plan_suite(connection, scope.plan_id)?;
    if !row_exists(
        connection,
        "generation_suite_manifests",
        "generation_suite_manifest_id",
        scope.suite_id,
    )? {
        return Err(StoreError::MissingRecord);
    }
    // The denial foreign key requires the suite row. It does not require that
    // suite to be the suite stored on the qualification plan.
    if plan_suite != scope.suite_id {
        return Err(StoreError::CorruptRecord);
    }
    if pair_exists(connection, scope)? {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn stored_plan_suite(connection: &Connection, plan_id: &str) -> StoreResult<String> {
    connection
        .query_row(
            "SELECT generation_suite_manifest_id FROM generation_qualification_plans
             WHERE generation_qualification_plan_id = ?1",
            [plan_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(StoreError::MissingRecord)
}

fn row_exists(connection: &Connection, table: &str, column: &str, id: &str) -> StoreResult<bool> {
    let sql = format!("SELECT 1 FROM {table} WHERE {column} = ?1");
    let present: Option<i64> = connection
        .query_row(&sql, [id], |row| row.get(0))
        .optional()?;
    Ok(present.is_some())
}

fn pair_exists(connection: &Connection, scope: DenialScope<'_>) -> StoreResult<bool> {
    let present: Option<i64> = connection
        .query_row(
            "SELECT 1 FROM generation_qualification_plan_systems
             WHERE generation_qualification_plan_id = ?1 AND generation_system_id = ?2",
            [scope.plan_id, scope.system_id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(present.is_some())
}
