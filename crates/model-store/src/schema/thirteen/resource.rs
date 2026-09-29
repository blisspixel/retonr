use rusqlite::Connection;

use super::{hex_column, primary_key};
use crate::StoreResult;

pub(super) fn create(connection: &Connection) -> StoreResult<()> {
    let id = primary_key("generation_resource_attempt_result_id");
    let system = hex_column("generation_system_id");
    let plan = hex_column("generation_qualification_plan_id");
    let suite = hex_column("generation_suite_manifest_id");
    let case_id = hex_column("generation_case_id");
    let repetition = hex_column("generation_repetition_id");
    let attempt = hex_column("planned_candidate_attempt_id");
    let attempt_record = hex_column("candidate_generation_attempt_record_id");
    let receipt = hex_column("candidate_generation_receipt_id");
    let policy = hex_column("resource_policy_digest");
    connection.execute_batch(&format!(
        "CREATE TABLE generation_resource_attempt_result_records (
             {id},
             schema_version INTEGER NOT NULL CHECK(schema_version = 1),
             {system},
             {plan},
             {suite},
             {case_id},
             {repetition},
             {attempt},
             {attempt_record},
             {receipt},
             {policy},
             observation_profile TEXT NOT NULL
                 CHECK(observation_profile = 'managed_ollama_v0_32_15_linux_v1'),
             canonical_json BLOB NOT NULL CHECK(length(canonical_json) BETWEEN 1 AND 16384),
             UNIQUE (generation_qualification_plan_id, planned_candidate_attempt_id),
             UNIQUE (candidate_generation_attempt_record_id),
             UNIQUE (candidate_generation_receipt_id),
             FOREIGN KEY (generation_qualification_plan_id, generation_system_id)
                 REFERENCES generation_qualification_plan_systems(
                     generation_qualification_plan_id,
                     generation_system_id
                 ) ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (generation_qualification_plan_id, planned_candidate_attempt_id)
                 REFERENCES generation_qualification_plan_attempts(
                     generation_qualification_plan_id,
                     planned_candidate_attempt_id
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
             FOREIGN KEY (generation_case_id)
                 REFERENCES generation_case_manifests(generation_case_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (generation_repetition_id)
                 REFERENCES generation_repetition_records(generation_repetition_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (planned_candidate_attempt_id)
                 REFERENCES planned_candidate_attempts(planned_candidate_attempt_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (candidate_generation_attempt_record_id)
                 REFERENCES candidate_generation_attempt_records(
                     candidate_generation_attempt_record_id
                 ) ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (candidate_generation_receipt_id)
                 REFERENCES candidate_generation_receipts(candidate_generation_receipt_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT
         ) STRICT;"
    ))?;
    Ok(())
}
