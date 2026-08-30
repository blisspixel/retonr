use rusqlite::Connection;

use crate::StoreResult;

pub(super) fn migrate_schema_six(connection: &Connection) -> StoreResult<()> {
    connection.execute_batch(
        "CREATE TABLE generation_qualification_operation_policies (
             operation_policy_id TEXT PRIMARY KEY NOT NULL
                 CHECK(length(operation_policy_id) = 64
                     AND operation_policy_id NOT GLOB '*[^0-9a-f]*'),
             generation_qualification_plan_id TEXT NOT NULL
                 CHECK(length(generation_qualification_plan_id) = 64
                     AND generation_qualification_plan_id NOT GLOB '*[^0-9a-f]*'),
             suite_manifest_id TEXT NOT NULL
                 CHECK(length(suite_manifest_id) = 64
                     AND suite_manifest_id NOT GLOB '*[^0-9a-f]*'),
             target_generation_system_id TEXT NOT NULL
                 CHECK(length(target_generation_system_id) = 64
                     AND target_generation_system_id NOT GLOB '*[^0-9a-f]*'),
             baseline_generation_system_id TEXT NOT NULL
                 CHECK(length(baseline_generation_system_id) = 64
                     AND baseline_generation_system_id NOT GLOB '*[^0-9a-f]*'),
             canonical_json BLOB NOT NULL
                 CHECK(length(canonical_json) BETWEEN 1 AND 16384),
             CHECK(target_generation_system_id <> baseline_generation_system_id),
             UNIQUE(
                 operation_policy_id,
                 generation_qualification_plan_id,
                 suite_manifest_id,
                 target_generation_system_id,
                 baseline_generation_system_id
             )
         ) STRICT;

         CREATE TABLE generation_qualification_request_projections (
             request_projection_id TEXT PRIMARY KEY NOT NULL
                 CHECK(length(request_projection_id) = 64
                     AND request_projection_id NOT GLOB '*[^0-9a-f]*'),
             operation_policy_id TEXT NOT NULL
                 CHECK(length(operation_policy_id) = 64
                     AND operation_policy_id NOT GLOB '*[^0-9a-f]*'),
             generation_qualification_plan_id TEXT NOT NULL
                 CHECK(length(generation_qualification_plan_id) = 64
                     AND generation_qualification_plan_id NOT GLOB '*[^0-9a-f]*'),
             suite_manifest_id TEXT NOT NULL
                 CHECK(length(suite_manifest_id) = 64
                     AND suite_manifest_id NOT GLOB '*[^0-9a-f]*'),
             target_generation_system_id TEXT NOT NULL
                 CHECK(length(target_generation_system_id) = 64
                     AND target_generation_system_id NOT GLOB '*[^0-9a-f]*'),
             baseline_generation_system_id TEXT NOT NULL
                 CHECK(length(baseline_generation_system_id) = 64
                     AND baseline_generation_system_id NOT GLOB '*[^0-9a-f]*'),
             entry_count INTEGER NOT NULL CHECK(entry_count BETWEEN 1 AND 1024),
             canonical_json BLOB NOT NULL
                 CHECK(length(canonical_json) BETWEEN 1 AND 4194304),
             CHECK(target_generation_system_id <> baseline_generation_system_id),
             UNIQUE(operation_policy_id),
             FOREIGN KEY(
                 operation_policy_id,
                 generation_qualification_plan_id,
                 suite_manifest_id,
                 target_generation_system_id,
                 baseline_generation_system_id
             ) REFERENCES generation_qualification_operation_policies(
                 operation_policy_id,
                 generation_qualification_plan_id,
                 suite_manifest_id,
                 target_generation_system_id,
                 baseline_generation_system_id
             ) ON UPDATE RESTRICT ON DELETE RESTRICT
         ) STRICT;

         PRAGMA user_version = 7;",
    )?;
    Ok(())
}
