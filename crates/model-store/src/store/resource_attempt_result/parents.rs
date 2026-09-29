use rewrite_model::GenerationResourceAttemptResultRecordV1;
use rusqlite::{Connection, OptionalExtension as _};

use crate::{StoreError, StoreResult};

pub(super) struct CitedParents<'a> {
    pub(super) system_id: &'a str,
    pub(super) plan_id: &'a str,
    pub(super) suite_id: &'a str,
    pub(super) case_id: &'a str,
    pub(super) repetition_id: &'a str,
    pub(super) planned_attempt_id: &'a str,
    pub(super) attempt_record_id: &'a str,
    pub(super) receipt_id: &'a str,
    pub(super) policy_digest: &'a str,
}

impl<'a> CitedParents<'a> {
    pub(super) fn from_record(record: &'a GenerationResourceAttemptResultRecordV1) -> Self {
        Self {
            system_id: record.generation_system_id().digest().as_str(),
            plan_id: record.generation_qualification_plan_id().digest().as_str(),
            suite_id: record.suite_manifest_id().digest().as_str(),
            case_id: record.case_id().digest().as_str(),
            repetition_id: record.repetition_id().digest().as_str(),
            planned_attempt_id: record.planned_attempt_id().digest().as_str(),
            attempt_record_id: record.attempt_record_id().digest().as_str(),
            receipt_id: record.candidate_generation_receipt_id().digest().as_str(),
            policy_digest: record.resource_policy_digest().as_str(),
        }
    }
}

pub(super) fn require(connection: &Connection, cited: &CitedParents<'_>) -> StoreResult<()> {
    require_rows(connection, cited)?;
    require_agreement(connection, cited)?;
    require_execution(connection, cited)
}

fn require_rows(connection: &Connection, cited: &CitedParents<'_>) -> StoreResult<()> {
    for (table, column, id) in [
        (
            "generation_system_records",
            "generation_system_id",
            cited.system_id,
        ),
        (
            "generation_suite_manifests",
            "generation_suite_manifest_id",
            cited.suite_id,
        ),
        (
            "generation_case_manifests",
            "generation_case_id",
            cited.case_id,
        ),
    ] {
        if !row_exists(connection, table, column, id)? {
            return Err(StoreError::MissingRecord);
        }
    }
    stored_plan_suite(connection, cited.plan_id)?;
    stored_repetition_suite(connection, cited.repetition_id)?;
    if planned_attempt(connection, cited.planned_attempt_id)?.is_none() {
        return Err(StoreError::MissingRecord);
    }
    Ok(())
}

fn require_agreement(connection: &Connection, cited: &CitedParents<'_>) -> StoreResult<()> {
    // Foreign keys require each parent row. They do not require the plan suite,
    // repetition suite, planned-attempt scope, or suite membership to agree.
    if stored_plan_suite(connection, cited.plan_id)? != cited.suite_id
        || stored_repetition_suite(connection, cited.repetition_id)? != cited.suite_id
        || planned_attempt(connection, cited.planned_attempt_id)?
            .is_none_or(|row| !planned_agrees(&row, cited))
        || !pair_exists(
            connection,
            "generation_suite_cases",
            "generation_suite_manifest_id",
            cited.suite_id,
            "generation_case_id",
            cited.case_id,
        )?
        || !pair_exists(
            connection,
            "generation_qualification_plan_systems",
            "generation_qualification_plan_id",
            cited.plan_id,
            "generation_system_id",
            cited.system_id,
        )?
        || !pair_exists(
            connection,
            "generation_qualification_plan_attempts",
            "generation_qualification_plan_id",
            cited.plan_id,
            "planned_candidate_attempt_id",
            cited.planned_attempt_id,
        )?
    {
        Err(StoreError::CorruptRecord)
    } else {
        Ok(())
    }
}

fn require_execution(connection: &Connection, cited: &CitedParents<'_>) -> StoreResult<()> {
    let attempt = attempt_record(connection, cited.attempt_record_id)?;
    let receipt = receipt_row(connection, cited.receipt_id)?;
    if attempt_agrees(&attempt, cited) && receipt_agrees(&receipt, cited) {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}

struct PlannedAttempt {
    suite: String,
    case: String,
    repetition: String,
    system: String,
}

struct AttemptRecord {
    plan: String,
    planned_attempt: String,
    outcome: String,
    receipt: Option<String>,
}

struct ReceiptRow {
    plan: String,
    planned_attempt: String,
}

fn planned_agrees(row: &PlannedAttempt, cited: &CitedParents<'_>) -> bool {
    row.suite == cited.suite_id
        && row.case == cited.case_id
        && row.repetition == cited.repetition_id
        && row.system == cited.system_id
}

fn attempt_agrees(row: &AttemptRecord, cited: &CitedParents<'_>) -> bool {
    row.plan == cited.plan_id
        && row.planned_attempt == cited.planned_attempt_id
        && row.outcome == "completed"
        && row.receipt.as_deref() == Some(cited.receipt_id)
}

fn receipt_agrees(row: &ReceiptRow, cited: &CitedParents<'_>) -> bool {
    row.plan == cited.plan_id && row.planned_attempt == cited.planned_attempt_id
}

fn stored_plan_suite(connection: &Connection, plan_id: &str) -> StoreResult<String> {
    required_text(
        connection,
        "SELECT generation_suite_manifest_id FROM generation_qualification_plans
         WHERE generation_qualification_plan_id = ?1",
        plan_id,
    )
}

fn stored_repetition_suite(connection: &Connection, repetition_id: &str) -> StoreResult<String> {
    required_text(
        connection,
        "SELECT generation_suite_manifest_id FROM generation_repetition_records
         WHERE generation_repetition_id = ?1",
        repetition_id,
    )
}

fn planned_attempt(
    connection: &Connection,
    planned_attempt_id: &str,
) -> StoreResult<Option<PlannedAttempt>> {
    connection
        .query_row(
            "SELECT generation_suite_manifest_id, generation_case_id,
                    generation_repetition_id, generation_system_id
             FROM planned_candidate_attempts WHERE planned_candidate_attempt_id = ?1",
            [planned_attempt_id],
            |row| {
                Ok(PlannedAttempt {
                    suite: row.get(0)?,
                    case: row.get(1)?,
                    repetition: row.get(2)?,
                    system: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(StoreError::from)
}

fn attempt_record(connection: &Connection, attempt_record_id: &str) -> StoreResult<AttemptRecord> {
    connection
        .query_row(
            "SELECT generation_qualification_plan_id, planned_candidate_attempt_id, outcome,
                    candidate_generation_receipt_id
             FROM candidate_generation_attempt_records
             WHERE candidate_generation_attempt_record_id = ?1",
            [attempt_record_id],
            |row| {
                Ok(AttemptRecord {
                    plan: row.get(0)?,
                    planned_attempt: row.get(1)?,
                    outcome: row.get(2)?,
                    receipt: row.get(3)?,
                })
            },
        )
        .optional()?
        .ok_or(StoreError::MissingRecord)
}

fn receipt_row(connection: &Connection, receipt_id: &str) -> StoreResult<ReceiptRow> {
    connection
        .query_row(
            "SELECT generation_qualification_plan_id, planned_candidate_attempt_id
             FROM candidate_generation_receipts WHERE candidate_generation_receipt_id = ?1",
            [receipt_id],
            |row| {
                Ok(ReceiptRow {
                    plan: row.get(0)?,
                    planned_attempt: row.get(1)?,
                })
            },
        )
        .optional()?
        .ok_or(StoreError::MissingRecord)
}

fn required_text(connection: &Connection, sql: &str, id: &str) -> StoreResult<String> {
    connection
        .query_row(sql, [id], |row| row.get(0))
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

fn pair_exists(
    connection: &Connection,
    table: &str,
    left_column: &str,
    left_id: &str,
    right_column: &str,
    right_id: &str,
) -> StoreResult<bool> {
    let sql = format!("SELECT 1 FROM {table} WHERE {left_column} = ?1 AND {right_column} = ?2");
    let present: Option<i64> = connection
        .query_row(&sql, [left_id, right_id], |row| row.get(0))
        .optional()?;
    Ok(present.is_some())
}
