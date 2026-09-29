//! Schema 14 stores one inert candidate-generation receipt set.
//!
//! The table grants no qualification, activation, or live-use authority.
//! Receipt-set entries stay inside canonical JSON.
//! This cohort does not widen the schema 11 repeatability result table.

use rusqlite::Connection;

use super::thirteen::{hex_column, primary_key};
use crate::StoreResult;

pub(super) fn migrate_schema_thirteen(connection: &Connection) -> StoreResult<()> {
    create(connection)?;
    connection.execute_batch("PRAGMA user_version = 14;")?;
    Ok(())
}

fn create(connection: &Connection) -> StoreResult<()> {
    let id = primary_key("candidate_generation_receipt_set_id");
    let plan = hex_column("generation_qualification_plan_id");
    let suite = hex_column("generation_suite_manifest_id");
    let repetition = hex_column("generation_repetition_id");
    let system = hex_column("generation_system_id");
    let policy = hex_column("candidate_selection_policy_id");
    connection.execute_batch(&format!(
        "CREATE TABLE candidate_generation_receipt_sets (
             {id},
             schema_version INTEGER NOT NULL CHECK(schema_version = 1),
             {plan},
             {suite},
             {repetition},
             {system},
             {policy},
             entry_count INTEGER NOT NULL CHECK(entry_count BETWEEN 1 AND 256),
             canonical_json BLOB NOT NULL CHECK(length(canonical_json) BETWEEN 1 AND 4194304),
             UNIQUE (
                 generation_qualification_plan_id,
                 generation_repetition_id,
                 generation_system_id
             ),
             FOREIGN KEY (generation_qualification_plan_id, generation_system_id)
                 REFERENCES generation_qualification_plan_systems(
                     generation_qualification_plan_id,
                     generation_system_id
                 ) ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (generation_qualification_plan_id, generation_repetition_id)
                 REFERENCES generation_qualification_plan_repetitions(
                     generation_qualification_plan_id,
                     generation_repetition_id
                 ) ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (generation_qualification_plan_id)
                 REFERENCES generation_qualification_plans(generation_qualification_plan_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (generation_suite_manifest_id)
                 REFERENCES generation_suite_manifests(generation_suite_manifest_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (generation_system_id)
                 REFERENCES generation_system_records(generation_system_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (generation_repetition_id)
                 REFERENCES generation_repetition_records(generation_repetition_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (candidate_selection_policy_id)
                 REFERENCES candidate_selection_policies(candidate_selection_policy_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT
         ) STRICT;"
    ))?;
    Ok(())
}
