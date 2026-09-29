use rusqlite::Connection;

use super::{hex_column, primary_key};
use crate::StoreResult;

pub(super) fn create(connection: &Connection) -> StoreResult<()> {
    create_denial(
        connection,
        "generation_resource_policy_denial_records",
        "generation_resource_policy_denial_record_id",
        "resource_policy_denial",
    )?;
    create_denial(
        connection,
        "generation_human_adjudication_policy_denial_records",
        "generation_human_adjudication_policy_denial_record_id",
        "human_adjudication_policy_denial",
    )
}

fn create_denial(
    connection: &Connection,
    table: &str,
    id_column: &str,
    kind: &str,
) -> StoreResult<()> {
    let id = primary_key(id_column);
    let system = hex_column("generation_system_id");
    let plan = hex_column("generation_qualification_plan_id");
    let suite = hex_column("generation_suite_manifest_id");
    let policy = hex_column("phase_policy_digest");
    connection.execute_batch(&format!(
        "CREATE TABLE {table} (
             {id},
             schema_version INTEGER NOT NULL CHECK(schema_version = 1),
             record_kind TEXT NOT NULL CHECK(record_kind = '{kind}'),
             {system},
             {plan},
             {suite},
             {policy},
             reason TEXT NOT NULL CHECK(reason = 'policy_source_denied'),
             canonical_json BLOB NOT NULL CHECK(length(canonical_json) BETWEEN 1 AND 4096),
             FOREIGN KEY (generation_qualification_plan_id, generation_system_id)
                 REFERENCES generation_qualification_plan_systems(
                     generation_qualification_plan_id,
                     generation_system_id
                 ) ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (generation_qualification_plan_id)
                 REFERENCES generation_qualification_plans(generation_qualification_plan_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (generation_suite_manifest_id)
                 REFERENCES generation_suite_manifests(generation_suite_manifest_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (generation_system_id)
                 REFERENCES generation_system_records(generation_system_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT
         ) STRICT;"
    ))?;
    Ok(())
}
