use rewrite_model::{
    GenerationRepeatabilityResultRecordV1, GenerationRepeatabilityTerminalStageV1,
};
use rusqlite::{Connection, OptionalExtension as _};

use crate::{StoreError, StoreResult};

pub(super) struct Cited<'a> {
    pub(super) id: &'a str,
    pub(super) system_id: &'a str,
    pub(super) plan_id: &'a str,
    pub(super) suite_id: &'a str,
    pub(super) repetition_id: &'a str,
    pub(super) ledger_id: &'a str,
    pub(super) stage: &'a str,
    pub(super) receipt_id: &'a str,
    pub(super) evaluation_id: &'a str,
    pub(super) judge_id: Option<&'a str>,
}

impl<'a> Cited<'a> {
    pub(super) fn from_record(
        record: &'a GenerationRepeatabilityResultRecordV1,
    ) -> StoreResult<Self> {
        let stage = stage_label(record.terminal_stage())?;
        let Some(receipt) = record.candidate_generation_receipt_set_id() else {
            return Err(StoreError::CorruptRecord);
        };
        let Some(evaluation) = record.candidate_deterministic_evaluation_id() else {
            return Err(StoreError::CorruptRecord);
        };
        let judge_id = record
            .candidate_judge_join_id()
            .map(|value| value.digest().as_str());
        if judge_agrees(stage, judge_id.is_some()) {
            Ok(Self {
                id: record.repeatability_result_id().digest().as_str(),
                system_id: record.generation_system_id().digest().as_str(),
                plan_id: record.generation_qualification_plan_id().digest().as_str(),
                suite_id: record.suite_manifest_id().digest().as_str(),
                repetition_id: record.repetition_id().digest().as_str(),
                ledger_id: record.attempt_ledger_manifest_id().digest().as_str(),
                stage,
                receipt_id: receipt.digest().as_str(),
                evaluation_id: evaluation.digest().as_str(),
                judge_id,
            })
        } else {
            Err(StoreError::CorruptRecord)
        }
    }
}

pub(super) fn require(connection: &Connection, cited: &Cited<'_>) -> StoreResult<()> {
    require_ledger(connection, cited)?;
    require_repetition(connection, cited)?;
    require_receipt(connection, cited)?;
    require_evaluation(connection, cited)?;
    require_judge(connection, cited)
}

fn stage_label(stage: GenerationRepeatabilityTerminalStageV1) -> StoreResult<&'static str> {
    match stage {
        GenerationRepeatabilityTerminalStageV1::DeterministicFailed => Ok("deterministic_failed"),
        GenerationRepeatabilityTerminalStageV1::JudgeFailed => Ok("judge_failed"),
        GenerationRepeatabilityTerminalStageV1::Passed => Ok("passed"),
        GenerationRepeatabilityTerminalStageV1::CandidateGenerationFailed => {
            Err(StoreError::CorruptRecord)
        }
    }
}

fn judge_agrees(stage: &str, present: bool) -> bool {
    match stage {
        "passed" => present,
        "deterministic_failed" | "judge_failed" => !present,
        _ => false,
    }
}

fn require_ledger(connection: &Connection, cited: &Cited<'_>) -> StoreResult<()> {
    let Some(row) = ledger_row(connection, cited.ledger_id)? else {
        return Err(StoreError::MissingRecord);
    };
    if row.system_id == cited.system_id
        && row.plan_id == cited.plan_id
        && row.suite_id == cited.suite_id
        && row.status != "skipped"
    {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn require_repetition(connection: &Connection, cited: &Cited<'_>) -> StoreResult<()> {
    let Some(suite) = optional_text(
        connection,
        "SELECT generation_suite_manifest_id FROM generation_repetition_records
         WHERE generation_repetition_id = ?1",
        cited.repetition_id,
    )?
    else {
        return Err(StoreError::MissingRecord);
    };
    if suite == cited.suite_id && pair_exists(connection, cited)? {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn require_receipt(connection: &Connection, cited: &Cited<'_>) -> StoreResult<()> {
    let Some(row) = receipt_row(connection, cited.receipt_id)? else {
        return Err(StoreError::MissingRecord);
    };
    if row.plan == cited.plan_id
        && row.suite == cited.suite_id
        && row.repetition == cited.repetition_id
        && row.system == cited.system_id
    {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn require_evaluation(connection: &Connection, cited: &Cited<'_>) -> StoreResult<()> {
    let Some(row) = evaluation_row(connection, cited.evaluation_id)? else {
        return Err(StoreError::MissingRecord);
    };
    if row.receipt_a == cited.receipt_id || row.receipt_b == cited.receipt_id {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn require_judge(connection: &Connection, cited: &Cited<'_>) -> StoreResult<()> {
    let Some(judge_id) = cited.judge_id else {
        return Ok(());
    };
    let Some(row) = judge_row(connection, judge_id)? else {
        return Err(StoreError::MissingRecord);
    };
    if row.evaluation_id == cited.evaluation_id
        && (row.receipt_a == cited.receipt_id || row.receipt_b == cited.receipt_id)
    {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}

struct LedgerRow {
    system_id: String,
    plan_id: String,
    suite_id: String,
    status: String,
}

struct ReceiptRow {
    plan: String,
    suite: String,
    repetition: String,
    system: String,
}

struct EvaluationRow {
    receipt_a: String,
    receipt_b: String,
}

struct JudgeRow {
    receipt_a: String,
    receipt_b: String,
    evaluation_id: String,
}

fn ledger_row(connection: &Connection, id: &str) -> StoreResult<Option<LedgerRow>> {
    connection
        .query_row(
            "SELECT generation_system_id, generation_qualification_plan_id,
                generation_suite_manifest_id, status
             FROM generation_attempt_ledger_manifests
             WHERE generation_attempt_ledger_manifest_id = ?1",
            [id],
            |row| {
                Ok(LedgerRow {
                    system_id: row.get(0)?,
                    plan_id: row.get(1)?,
                    suite_id: row.get(2)?,
                    status: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(StoreError::from)
}

fn receipt_row(connection: &Connection, id: &str) -> StoreResult<Option<ReceiptRow>> {
    connection
        .query_row(
            "SELECT generation_qualification_plan_id, generation_suite_manifest_id,
                generation_repetition_id, generation_system_id
             FROM candidate_generation_receipt_sets
             WHERE candidate_generation_receipt_set_id = ?1",
            [id],
            |row| {
                Ok(ReceiptRow {
                    plan: row.get(0)?,
                    suite: row.get(1)?,
                    repetition: row.get(2)?,
                    system: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(StoreError::from)
}

fn evaluation_row(connection: &Connection, id: &str) -> StoreResult<Option<EvaluationRow>> {
    connection
        .query_row(
            "SELECT candidate_a_receipt_set_id, candidate_b_receipt_set_id
             FROM candidate_deterministic_evaluation_records
             WHERE candidate_deterministic_evaluation_id = ?1",
            [id],
            |row| {
                Ok(EvaluationRow {
                    receipt_a: row.get(0)?,
                    receipt_b: row.get(1)?,
                })
            },
        )
        .optional()
        .map_err(StoreError::from)
}

fn judge_row(connection: &Connection, id: &str) -> StoreResult<Option<JudgeRow>> {
    connection
        .query_row(
            "SELECT candidate_a_receipt_set_id, candidate_b_receipt_set_id,
                deterministic_evaluation_id
             FROM candidate_judge_join_records
             WHERE candidate_judge_join_id = ?1",
            [id],
            |row| {
                Ok(JudgeRow {
                    receipt_a: row.get(0)?,
                    receipt_b: row.get(1)?,
                    evaluation_id: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(StoreError::from)
}

fn pair_exists(connection: &Connection, cited: &Cited<'_>) -> StoreResult<bool> {
    let count: i64 = connection.query_row(
        "SELECT count(*) FROM generation_qualification_plan_repetitions
         WHERE generation_qualification_plan_id = ?1 AND generation_repetition_id = ?2",
        [cited.plan_id, cited.repetition_id],
        |row| row.get(0),
    )?;
    Ok(count == 1)
}

fn optional_text(connection: &Connection, sql: &str, id: &str) -> StoreResult<Option<String>> {
    connection
        .query_row(sql, [id], |row| row.get(0))
        .optional()
        .map_err(StoreError::from)
}
