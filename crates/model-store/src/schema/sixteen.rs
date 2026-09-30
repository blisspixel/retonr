//! Schema 16 stores one inert repeatability terminal-result record.
//!
//! The table grants no qualification, activation, or live-use authority.
//! It accepts deterministic failure, judge failure, and passed only.
//! Schema 11 remains the only store for candidate-generation failure.
//! This cohort does not alter schemas 11 through 15.

use rusqlite::Connection;

use super::thirteen::{hex_column, primary_key};
use crate::StoreResult;

pub(super) fn migrate_schema_fifteen(connection: &Connection) -> StoreResult<()> {
    create(connection)?;
    connection.execute_batch("PRAGMA user_version = 16;")?;
    Ok(())
}

fn create(connection: &Connection) -> StoreResult<()> {
    let id = primary_key("generation_repeatability_result_id");
    let system = hex_column("generation_system_id");
    let plan = hex_column("generation_qualification_plan_id");
    let suite = hex_column("generation_suite_manifest_id");
    let repetition = hex_column("generation_repetition_id");
    let ledger = hex_column("generation_attempt_ledger_manifest_id");
    let receipt = hex_column("candidate_generation_receipt_set_id");
    let evaluation = hex_column("candidate_deterministic_evaluation_id");
    let judge_join = nullable_hex("candidate_judge_join_id");
    connection.execute_batch(&format!(
        "CREATE TABLE generation_repeatability_terminal_result_records (
             {id},
             {system},
             {plan},
             {suite},
             {repetition},
             {ledger},
             terminal_stage TEXT NOT NULL
                 CHECK(terminal_stage IN (
                     'deterministic_failed', 'judge_failed', 'passed'
                 )),
             {receipt},
             {evaluation},
             {judge_join},
             canonical_json BLOB NOT NULL CHECK(length(canonical_json) BETWEEN 1 AND 16384),
             CHECK(
                 (terminal_stage IN ('deterministic_failed', 'judge_failed')
                     AND candidate_judge_join_id IS NULL)
                 OR (terminal_stage = 'passed' AND candidate_judge_join_id IS NOT NULL)
             ),
             UNIQUE (generation_qualification_plan_id, generation_repetition_id),
             UNIQUE (generation_attempt_ledger_manifest_id, generation_repetition_id),
             FOREIGN KEY (
                 generation_attempt_ledger_manifest_id,
                 generation_qualification_plan_id,
                 generation_system_id,
                 generation_suite_manifest_id
             ) REFERENCES generation_attempt_ledger_manifests(
                 generation_attempt_ledger_manifest_id,
                 generation_qualification_plan_id,
                 generation_system_id,
                 generation_suite_manifest_id
             ) ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (
                 generation_qualification_plan_id,
                 generation_repetition_id
             ) REFERENCES generation_qualification_plan_repetitions(
                 generation_qualification_plan_id,
                 generation_repetition_id
             ) ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (candidate_generation_receipt_set_id)
                 REFERENCES candidate_generation_receipt_sets(candidate_generation_receipt_set_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (candidate_deterministic_evaluation_id)
                 REFERENCES candidate_deterministic_evaluation_records(
                     candidate_deterministic_evaluation_id
                 )
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (candidate_judge_join_id)
                 REFERENCES candidate_judge_join_records(candidate_judge_join_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT
         ) STRICT;"
    ))?;
    Ok(())
}

fn nullable_hex(name: &str) -> String {
    format!(
        "{name} TEXT
             CHECK({name} IS NULL OR (
                 length({name}) = 64
                 AND {name} = lower({name})
                 AND {name} NOT GLOB '*[^0-9a-f]*'
             ))"
    )
}
