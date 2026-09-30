//! Schema 17 stores one inert generation-qualification record.
//!
//! The table grants no qualification, activation, or live-use authority.
//! A stored qualified status is a structural result only.
//! This cohort does not alter schemas 7 through 16.

use rusqlite::Connection;

use super::thirteen::{hex_column, primary_key};
use crate::StoreResult;

pub(super) fn migrate_schema_sixteen(connection: &Connection) -> StoreResult<()> {
    create(connection)?;
    connection.execute_batch("PRAGMA user_version = 17;")?;
    Ok(())
}

fn create(connection: &Connection) -> StoreResult<()> {
    let id = primary_key("generation_qualification_id");
    let target = hex_column("target_generation_system_id");
    let baseline = hex_column("baseline_generation_system_id");
    let policy = hex_column("operation_policy_id");
    let projection = hex_column("request_projection_id");
    let platform = hex_column("generation_qualification_platform_evidence_id");
    let license = hex_column("generation_qualification_license_evidence_id");
    let receipt = hex_column("generation_qualification_operation_receipt_id");
    connection.execute_batch(&format!(
        "CREATE TABLE generation_qualification_records (
             {id},
             {target},
             {baseline},
             {policy},
             {projection},
             {platform},
             {license},
             {receipt},
             status TEXT NOT NULL CHECK(status IN ('qualified', 'rejected')),
             canonical_json BLOB NOT NULL CHECK(length(canonical_json) BETWEEN 1 AND 16384),
             CHECK(target_generation_system_id <> baseline_generation_system_id),
             UNIQUE (generation_qualification_operation_receipt_id),
             FOREIGN KEY (target_generation_system_id)
                 REFERENCES generation_system_records(generation_system_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (baseline_generation_system_id)
                 REFERENCES generation_system_records(generation_system_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (operation_policy_id)
                 REFERENCES generation_qualification_operation_policies(operation_policy_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (request_projection_id)
                 REFERENCES generation_qualification_request_projections(request_projection_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (
                 generation_qualification_platform_evidence_id,
                 operation_policy_id,
                 request_projection_id,
                 target_generation_system_id
             ) REFERENCES generation_qualification_platform_evidence(
                 generation_qualification_platform_evidence_id,
                 operation_policy_id,
                 request_projection_id,
                 target_generation_system_id
             ) ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (
                 generation_qualification_license_evidence_id,
                 operation_policy_id,
                 request_projection_id,
                 target_generation_system_id
             ) REFERENCES generation_qualification_license_evidence(
                 generation_qualification_license_evidence_id,
                 operation_policy_id,
                 request_projection_id,
                 target_generation_system_id
             ) ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (generation_qualification_operation_receipt_id)
                 REFERENCES generation_qualification_operation_receipts(
                     generation_qualification_operation_receipt_id
                 )
                 ON UPDATE RESTRICT ON DELETE RESTRICT
         ) STRICT;"
    ))?;
    Ok(())
}
