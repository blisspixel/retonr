use rusqlite::Connection;

use crate::StoreResult;

pub(super) fn migrate_schema_seven(connection: &Connection) -> StoreResult<()> {
    connection.execute_batch(
        "CREATE TABLE effective_package_evidence_v2 (
             effective_package_evidence_v2_id TEXT PRIMARY KEY NOT NULL
                 CHECK(length(effective_package_evidence_v2_id) = 64)
                 CHECK(effective_package_evidence_v2_id = lower(effective_package_evidence_v2_id))
                 CHECK(effective_package_evidence_v2_id NOT GLOB '*[^0-9a-f]*'),
             artifact_set_id TEXT NOT NULL
                 CHECK(length(artifact_set_id) = 64)
                 CHECK(artifact_set_id = lower(artifact_set_id))
                 CHECK(artifact_set_id NOT GLOB '*[^0-9a-f]*'),
             runtime_build_id TEXT NOT NULL
                 CHECK(length(runtime_build_id) = 64)
                 CHECK(runtime_build_id = lower(runtime_build_id))
                 CHECK(runtime_build_id NOT GLOB '*[^0-9a-f]*'),
             effective_runtime_state_id TEXT NOT NULL
                 CHECK(length(effective_runtime_state_id) = 64)
                 CHECK(effective_runtime_state_id = lower(effective_runtime_state_id))
                 CHECK(effective_runtime_state_id NOT GLOB '*[^0-9a-f]*'),
             member_count INTEGER NOT NULL CHECK(member_count BETWEEN 1 AND 1024),
             canonical_json BLOB NOT NULL
                 CHECK(length(canonical_json) BETWEEN 1 AND 2097152),
             UNIQUE (
                 effective_package_evidence_v2_id,
                 artifact_set_id,
                 runtime_build_id,
                 effective_runtime_state_id
             ),
             FOREIGN KEY (artifact_set_id) REFERENCES artifact_set_manifests(artifact_set_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (runtime_build_id) REFERENCES runtime_build_identities(runtime_build_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (effective_runtime_state_id)
                 REFERENCES effective_runtime_states(effective_runtime_state_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT
         ) STRICT;

         CREATE TABLE generation_system_records (
             generation_system_id TEXT PRIMARY KEY NOT NULL
                 CHECK(length(generation_system_id) = 64)
                 CHECK(generation_system_id = lower(generation_system_id))
                 CHECK(generation_system_id NOT GLOB '*[^0-9a-f]*'),
             runtime_package_manifest_id TEXT NOT NULL
                 CHECK(length(runtime_package_manifest_id) = 64)
                 CHECK(runtime_package_manifest_id = lower(runtime_package_manifest_id))
                 CHECK(runtime_package_manifest_id NOT GLOB '*[^0-9a-f]*'),
             runtime_build_id TEXT NOT NULL
                 CHECK(length(runtime_build_id) = 64)
                 CHECK(runtime_build_id = lower(runtime_build_id))
                 CHECK(runtime_build_id NOT GLOB '*[^0-9a-f]*'),
             effective_runtime_state_id TEXT NOT NULL
                 CHECK(length(effective_runtime_state_id) = 64)
                 CHECK(effective_runtime_state_id = lower(effective_runtime_state_id))
                 CHECK(effective_runtime_state_id NOT GLOB '*[^0-9a-f]*'),
             model_artifact_set_id TEXT NOT NULL
                 CHECK(length(model_artifact_set_id) = 64)
                 CHECK(model_artifact_set_id = lower(model_artifact_set_id))
                 CHECK(model_artifact_set_id NOT GLOB '*[^0-9a-f]*'),
             model_package_manifest_id TEXT NOT NULL
                 CHECK(length(model_package_manifest_id) = 64)
                 CHECK(model_package_manifest_id = lower(model_package_manifest_id))
                 CHECK(model_package_manifest_id NOT GLOB '*[^0-9a-f]*'),
             model_artifact_id TEXT NOT NULL
                 CHECK(length(model_artifact_id) = 64)
                 CHECK(model_artifact_id = lower(model_artifact_id))
                 CHECK(model_artifact_id NOT GLOB '*[^0-9a-f]*'),
             effective_package_evidence_v2_id TEXT NOT NULL
                 CHECK(length(effective_package_evidence_v2_id) = 64)
                 CHECK(effective_package_evidence_v2_id = lower(effective_package_evidence_v2_id))
                 CHECK(effective_package_evidence_v2_id NOT GLOB '*[^0-9a-f]*'),
             canonical_json BLOB NOT NULL CHECK(length(canonical_json) BETWEEN 1 AND 16384),
             FOREIGN KEY (runtime_package_manifest_id)
                 REFERENCES runtime_package_manifests(runtime_package_manifest_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (runtime_build_id) REFERENCES runtime_build_identities(runtime_build_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (effective_runtime_state_id)
                 REFERENCES effective_runtime_states(effective_runtime_state_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (model_artifact_set_id)
                 REFERENCES artifact_set_manifests(artifact_set_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (model_package_manifest_id)
                 REFERENCES model_package_manifests(model_package_manifest_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (
                 effective_package_evidence_v2_id,
                 model_artifact_set_id,
                 runtime_build_id,
                 effective_runtime_state_id
             ) REFERENCES effective_package_evidence_v2(
                 effective_package_evidence_v2_id,
                 artifact_set_id,
                 runtime_build_id,
                 effective_runtime_state_id
             ) ON UPDATE RESTRICT ON DELETE RESTRICT
         ) STRICT;

         PRAGMA user_version = 8;",
    )?;
    Ok(())
}
