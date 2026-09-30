use rewrite_model::CandidateGenerationReceiptSetV1;
use rusqlite::{Connection, OptionalExtension as _};

use crate::{StoreError, StoreResult};

pub(super) struct Cited<'a> {
    pub(super) id: &'a str,
    pub(super) plan_id: &'a str,
    pub(super) suite_id: &'a str,
    pub(super) repetition_id: &'a str,
    pub(super) system_id: &'a str,
    pub(super) policy_id: &'a str,
    pub(super) schema_version: i64,
    pub(super) entry_count: i64,
    pub(super) entries: &'a [rewrite_model::CandidateGenerationReceiptSetEntryV1],
}

impl<'a> Cited<'a> {
    pub(super) fn from_record(record: &'a CandidateGenerationReceiptSetV1) -> Self {
        Self {
            id: record.receipt_set_id().digest().as_str(),
            plan_id: record.qualification_plan_id().digest().as_str(),
            suite_id: record.suite_manifest_id().digest().as_str(),
            repetition_id: record.repetition_id().digest().as_str(),
            system_id: record.generation_system_id().digest().as_str(),
            policy_id: record.selection_policy_id().digest().as_str(),
            schema_version: i64::from(record.schema_version()),
            entry_count: i64::from(record.entry_count()),
            entries: record.entries(),
        }
    }
}

pub(super) fn require(connection: &Connection, cited: &Cited<'_>) -> StoreResult<()> {
    require_scope(connection, cited)?;
    require_entries(connection, cited)
}

fn require_scope(connection: &Connection, cited: &Cited<'_>) -> StoreResult<()> {
    require_plan(connection, cited)?;
    require_suite(connection, cited)?;
    require_policy(connection, cited)?;
    require_repetition(connection, cited)?;
    require_system(connection, cited)?;
    require_case_order(connection, cited)
}

fn require_plan(connection: &Connection, cited: &Cited<'_>) -> StoreResult<()> {
    let row = plan_row(connection, cited.plan_id)?;
    if row.suite == cited.suite_id && row.policy == cited.policy_id {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn require_suite(connection: &Connection, cited: &Cited<'_>) -> StoreResult<()> {
    let case_count = required_i64(
        connection,
        "SELECT case_count FROM generation_suite_manifests
         WHERE generation_suite_manifest_id = ?1",
        cited.suite_id,
    )?;
    if case_count == cited.entry_count {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn require_policy(connection: &Connection, cited: &Cited<'_>) -> StoreResult<()> {
    let row = policy_row(connection, cited.policy_id)?;
    if row.suite == cited.suite_id && row.entry_count == cited.entry_count {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn require_repetition(connection: &Connection, cited: &Cited<'_>) -> StoreResult<()> {
    let suite = required_text(
        connection,
        "SELECT generation_suite_manifest_id FROM generation_repetition_records
         WHERE generation_repetition_id = ?1",
        cited.repetition_id,
    )?;
    if suite == cited.suite_id
        && pair_exists(
            connection,
            "generation_qualification_plan_repetitions",
            "generation_qualification_plan_id",
            cited.plan_id,
            "generation_repetition_id",
            cited.repetition_id,
        )?
    {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn require_system(connection: &Connection, cited: &Cited<'_>) -> StoreResult<()> {
    if !row_exists(
        connection,
        "generation_system_records",
        "generation_system_id",
        cited.system_id,
    )? {
        return Err(StoreError::MissingRecord);
    }
    if pair_exists(
        connection,
        "generation_qualification_plan_systems",
        "generation_qualification_plan_id",
        cited.plan_id,
        "generation_system_id",
        cited.system_id,
    )? {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn require_case_order(connection: &Connection, cited: &Cited<'_>) -> StoreResult<()> {
    let cases = suite_cases(connection, cited.suite_id)?;
    let stored = i64::try_from(cases.len()).map_err(|_| StoreError::CorruptRecord)?;
    let cited_entries =
        i64::try_from(cited.entries.len()).map_err(|_| StoreError::CorruptRecord)?;
    if stored != cited.entry_count || cited_entries != cited.entry_count {
        return Err(StoreError::CorruptRecord);
    }
    if cases
        .iter()
        .zip(cited.entries)
        .all(|(case, entry)| case == entry.case_id().digest().as_str())
    {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn require_entries(connection: &Connection, cited: &Cited<'_>) -> StoreResult<()> {
    for entry in cited.entries {
        require_entry(connection, cited, entry)?;
    }
    Ok(())
}

fn require_entry(
    connection: &Connection,
    cited: &Cited<'_>,
    entry: &rewrite_model::CandidateGenerationReceiptSetEntryV1,
) -> StoreResult<()> {
    let planned = entry.planned_attempt_id().digest().as_str();
    let attempt = entry.attempt_record_id().digest().as_str();
    let receipt = entry.receipt_id().digest().as_str();
    require_planned(connection, cited, entry, planned)?;
    require_attempt(connection, cited, planned, attempt, receipt)?;
    require_receipt(connection, cited, planned, receipt)
}

fn require_planned(
    connection: &Connection,
    cited: &Cited<'_>,
    entry: &rewrite_model::CandidateGenerationReceiptSetEntryV1,
    planned_id: &str,
) -> StoreResult<()> {
    let Some(row) = planned_attempt(connection, planned_id)? else {
        return Err(StoreError::MissingRecord);
    };
    let matches = row.suite == cited.suite_id
        && row.case == entry.case_id().digest().as_str()
        && row.repetition == cited.repetition_id
        && row.system == cited.system_id;
    let on_plan = pair_exists(
        connection,
        "generation_qualification_plan_attempts",
        "generation_qualification_plan_id",
        cited.plan_id,
        "planned_candidate_attempt_id",
        planned_id,
    )?;
    let count = planned_count(connection, cited, entry.case_id().digest().as_str())?;
    if matches && on_plan && count == 1 {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn require_attempt(
    connection: &Connection,
    cited: &Cited<'_>,
    planned_id: &str,
    attempt_id: &str,
    receipt_id: &str,
) -> StoreResult<()> {
    let row = attempt_record(connection, attempt_id)?;
    if row.plan == cited.plan_id
        && row.planned_attempt == planned_id
        && row.outcome == "completed"
        && row.receipt.as_deref() == Some(receipt_id)
    {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn require_receipt(
    connection: &Connection,
    cited: &Cited<'_>,
    planned_id: &str,
    receipt_id: &str,
) -> StoreResult<()> {
    let row = receipt_row(connection, receipt_id)?;
    if row.plan == cited.plan_id && row.planned_attempt == planned_id {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}

struct PlanScope {
    suite: String,
    policy: String,
}

struct PolicyScope {
    suite: String,
    entry_count: i64,
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

fn plan_row(connection: &Connection, plan_id: &str) -> StoreResult<PlanScope> {
    connection
        .query_row(
            "SELECT generation_suite_manifest_id, candidate_selection_policy_id
             FROM generation_qualification_plans
             WHERE generation_qualification_plan_id = ?1",
            [plan_id],
            |row| {
                Ok(PlanScope {
                    suite: row.get(0)?,
                    policy: row.get(1)?,
                })
            },
        )
        .optional()?
        .ok_or(StoreError::MissingRecord)
}

fn policy_row(connection: &Connection, policy_id: &str) -> StoreResult<PolicyScope> {
    connection
        .query_row(
            "SELECT generation_suite_manifest_id, entry_count
             FROM candidate_selection_policies WHERE candidate_selection_policy_id = ?1",
            [policy_id],
            |row| {
                Ok(PolicyScope {
                    suite: row.get(0)?,
                    entry_count: row.get(1)?,
                })
            },
        )
        .optional()?
        .ok_or(StoreError::MissingRecord)
}

fn planned_attempt(
    connection: &Connection,
    planned_id: &str,
) -> StoreResult<Option<PlannedAttempt>> {
    connection
        .query_row(
            "SELECT generation_suite_manifest_id, generation_case_id,
                    generation_repetition_id, generation_system_id
             FROM planned_candidate_attempts WHERE planned_candidate_attempt_id = ?1",
            [planned_id],
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

fn planned_count(connection: &Connection, cited: &Cited<'_>, case_id: &str) -> StoreResult<i64> {
    connection
        .query_row(
            "SELECT COUNT(*) FROM planned_candidate_attempts
             WHERE generation_suite_manifest_id = ?1 AND generation_case_id = ?2
               AND generation_repetition_id = ?3 AND generation_system_id = ?4",
            [
                cited.suite_id,
                case_id,
                cited.repetition_id,
                cited.system_id,
            ],
            |row| row.get(0),
        )
        .map_err(StoreError::from)
}

fn attempt_record(connection: &Connection, attempt_id: &str) -> StoreResult<AttemptRecord> {
    connection
        .query_row(
            "SELECT generation_qualification_plan_id, planned_candidate_attempt_id, outcome,
                    candidate_generation_receipt_id
             FROM candidate_generation_attempt_records
             WHERE candidate_generation_attempt_record_id = ?1",
            [attempt_id],
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

fn suite_cases(connection: &Connection, suite_id: &str) -> StoreResult<Vec<String>> {
    let mut statement = connection.prepare(
        "SELECT generation_case_id FROM generation_suite_cases
         WHERE generation_suite_manifest_id = ?1 ORDER BY semantic_ordinal",
    )?;
    let mut rows = statement.query([suite_id])?;
    let mut cases = Vec::new();
    while let Some(row) = rows.next()? {
        cases.push(row.get(0)?);
    }
    Ok(cases)
}

fn required_text(connection: &Connection, sql: &str, id: &str) -> StoreResult<String> {
    connection
        .query_row(sql, [id], |row| row.get(0))
        .optional()?
        .ok_or(StoreError::MissingRecord)
}

fn required_i64(connection: &Connection, sql: &str, id: &str) -> StoreResult<i64> {
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
