use rusqlite::{Connection, OptionalExtension as _, params};

mod row_decode;

use super::context::Anchor;
use crate::{StoreError, StoreResult};
use row_decode::{
    INTERRUPTION_SQL, LICENSE_SQL, PLATFORM_SQL, RECEIPT_SQL, RESULT_LIST_SQL, RESULT_SQL,
    finish_interruption, finish_license, finish_phase_manifest, finish_platform, finish_receipt,
    finish_result, map_interruption, map_license, map_phase_manifest, map_platform, map_receipt,
    map_result,
};

const MAX_JSON_BYTES: usize = 16 * 1024;

pub(super) struct PhaseManifestRow {
    pub(super) id: String,
    pub(super) system_id: String,
    pub(super) plan_id: String,
    pub(super) suite_id: String,
    pub(super) evidence_item_count: i64,
    pub(super) status: String,
    pub(super) bytes: Vec<u8>,
}

pub(super) struct PlatformRow {
    pub(super) id: String,
    pub(super) policy_id: String,
    pub(super) projection_id: String,
    pub(super) target_id: String,
    pub(super) status: String,
    pub(super) reason: String,
    pub(super) bytes: Vec<u8>,
}

pub(super) struct LicenseRow {
    pub(super) id: String,
    pub(super) policy_id: String,
    pub(super) projection_id: String,
    pub(super) target_id: String,
    pub(super) permission: String,
    pub(super) decision: String,
    pub(super) reason: String,
    pub(super) bytes: Vec<u8>,
}

pub(super) struct ResultRow {
    pub(super) id: String,
    pub(super) system_id: String,
    pub(super) plan_id: String,
    pub(super) suite_id: String,
    pub(super) repetition_id: String,
    pub(super) ledger_id: String,
    pub(super) terminal_stage: String,
    pub(super) receipt_set_id: Option<String>,
    pub(super) deterministic_id: Option<String>,
    pub(super) judge_join_id: Option<String>,
    pub(super) bytes: Vec<u8>,
}

pub(super) struct ReceiptRow {
    pub(super) id: String,
    pub(super) policy_id: String,
    pub(super) projection_id: String,
    pub(super) plan_id: String,
    pub(super) suite_id: String,
    pub(super) target_id: String,
    pub(super) baseline_id: String,
    pub(super) platform_id: String,
    pub(super) license_id: String,
    pub(super) ledger_id: String,
    pub(super) repeatability_id: String,
    pub(super) resource_id: String,
    pub(super) human_id: String,
    pub(super) elapsed_nanoseconds: i64,
    pub(super) peak_concurrent_attempts: i64,
    pub(super) terminal_status: String,
    pub(super) finalization_status: String,
    pub(super) bytes: Vec<u8>,
}

pub(super) struct InterruptionRow {
    pub(super) id: String,
    pub(super) receipt_id: String,
    pub(super) policy_id: String,
    pub(super) target_id: String,
    pub(super) plan_id: String,
    pub(super) suite_id: String,
    pub(super) phase: String,
    pub(super) checkpoint: String,
    pub(super) planned_attempt_id: Option<String>,
    pub(super) reason: String,
    pub(super) terminal_status: String,
    pub(super) bytes: Vec<u8>,
}

pub(super) fn cohort_absent(connection: &Connection, anchor: &Anchor<'_>) -> StoreResult<bool> {
    let policy = anchor
        .preregistration
        .operation_policy()
        .operation_policy_id()
        .digest()
        .as_str();
    let plan = anchor
        .preregistration
        .plan_foundation()
        .plan()
        .qualification_plan_id()
        .digest()
        .as_str();
    let system = anchor
        .preregistration
        .plan_foundation()
        .generation_systems()
        .get(anchor.target_index)
        .ok_or(StoreError::CorruptRecord)?
        .generation_system_id()
        .digest()
        .as_str();
    let policy_tables = [
        "generation_qualification_platform_evidence",
        "generation_qualification_license_evidence",
        "generation_qualification_operation_receipts",
        "generation_qualification_phase_interruption_records",
    ];
    for table in policy_tables {
        if count_where(
            connection,
            &format!("SELECT COUNT(*) FROM {table} WHERE operation_policy_id = ?1"),
            params![policy],
        )? != 0
        {
            return Ok(false);
        }
    }
    let phase_tables = [
        "generation_attempt_ledger_manifests",
        "generation_repeatability_result_records",
        "generation_repeatability_evidence_manifests",
        "generation_resource_evidence_manifests",
        "generation_human_adjudication_evidence_manifests",
    ];
    for table in phase_tables {
        if count_where(
            connection,
            &format!(
                "SELECT COUNT(*) FROM {table}
                 WHERE generation_qualification_plan_id = ?1 AND generation_system_id = ?2"
            ),
            params![plan, system],
        )? != 0
        {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn load_platform(
    connection: &Connection,
    policy_id: &str,
) -> StoreResult<Option<PlatformRow>> {
    let raw = connection
        .query_row(PLATFORM_SQL, params![policy_id], map_platform)
        .optional()?;
    raw.map(finish_platform).transpose()
}

pub(super) fn load_license(
    connection: &Connection,
    policy_id: &str,
) -> StoreResult<Option<LicenseRow>> {
    let raw = connection
        .query_row(LICENSE_SQL, params![policy_id], map_license)
        .optional()?;
    raw.map(finish_license).transpose()
}

pub(super) fn load_phase_manifest(
    connection: &Connection,
    table: &str,
    identifier: &str,
    plan_id: &str,
    system_id: &str,
) -> StoreResult<Option<PhaseManifestRow>> {
    let sql = format!(
        "SELECT {identifier}, generation_system_id, generation_qualification_plan_id,
                generation_suite_manifest_id, evidence_item_count, status,
                typeof(canonical_json), length(canonical_json),
                CAST(substr(canonical_json, 1, {MAX_JSON_BYTES}) AS BLOB)
         FROM {table}
         WHERE generation_qualification_plan_id = ?1 AND generation_system_id = ?2"
    );
    let raw = connection
        .query_row(&sql, params![plan_id, system_id], map_phase_manifest)
        .optional()?;
    raw.map(finish_phase_manifest).transpose()
}

pub(super) fn load_results(
    connection: &Connection,
    plan_id: &str,
    system_id: &str,
) -> StoreResult<Vec<ResultRow>> {
    let mut statement = connection.prepare(RESULT_LIST_SQL)?;
    let rows = statement.query_map(params![plan_id, system_id], map_result)?;
    let mut values = Vec::new();
    for row in rows {
        values.push(finish_result(row?)?);
    }
    Ok(values)
}

pub(super) fn load_result(
    connection: &Connection,
    plan_id: &str,
    repetition_id: &str,
) -> StoreResult<Option<ResultRow>> {
    let raw = connection
        .query_row(RESULT_SQL, params![plan_id, repetition_id], map_result)
        .optional()?;
    raw.map(finish_result).transpose()
}

pub(super) fn load_receipt(
    connection: &Connection,
    policy_id: &str,
) -> StoreResult<Option<ReceiptRow>> {
    let raw = connection
        .query_row(RECEIPT_SQL, params![policy_id], map_receipt)
        .optional()?;
    raw.map(finish_receipt).transpose()
}

pub(super) fn load_interruption(
    connection: &Connection,
    policy_id: &str,
) -> StoreResult<Option<InterruptionRow>> {
    let raw = connection
        .query_row(INTERRUPTION_SQL, params![policy_id], map_interruption)
        .optional()?;
    raw.map(finish_interruption).transpose()
}

fn count_where(
    connection: &Connection,
    sql: &str,
    parameters: impl rusqlite::Params,
) -> StoreResult<i64> {
    Ok(connection.query_row(sql, parameters, |row| row.get(0))?)
}
