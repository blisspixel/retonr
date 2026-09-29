//! Schema 15 stores one inert deterministic candidate-evaluation record.
//!
//! The table grants no qualification, activation, or live-use authority.
//! Pair identity, suite, repetition, systems, report bytes, digests, counts,
//! coverage, and status stay inside canonical JSON.
//! This cohort does not widen schema 11 repeatability or the schema 14 receipt-set table.

use rusqlite::Connection;

use super::thirteen::{hex_column, primary_key};
use crate::StoreResult;

pub(super) fn migrate_schema_fourteen(connection: &Connection) -> StoreResult<()> {
    create(connection)?;
    connection.execute_batch("PRAGMA user_version = 15;")?;
    Ok(())
}

fn create(connection: &Connection) -> StoreResult<()> {
    let id = primary_key("candidate_deterministic_evaluation_id");
    let receipt_a = hex_column("candidate_a_receipt_set_id");
    let receipt_b = hex_column("candidate_b_receipt_set_id");
    connection.execute_batch(&format!(
        "CREATE TABLE candidate_deterministic_evaluation_records (
             {id},
             schema_version INTEGER NOT NULL CHECK(schema_version = 1),
             {receipt_a},
             {receipt_b},
             canonical_json BLOB NOT NULL CHECK(length(canonical_json) BETWEEN 1 AND 16384),
             CHECK(candidate_a_receipt_set_id != candidate_b_receipt_set_id),
             FOREIGN KEY (candidate_a_receipt_set_id)
                 REFERENCES candidate_generation_receipt_sets(candidate_generation_receipt_set_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (candidate_b_receipt_set_id)
                 REFERENCES candidate_generation_receipt_sets(candidate_generation_receipt_set_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT
         ) STRICT;"
    ))?;
    Ok(())
}
