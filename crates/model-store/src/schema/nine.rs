use rusqlite::Connection;

use crate::StoreResult;

#[expect(clippy::too_many_lines, reason = "one atomic schema cohort")]
pub(super) fn migrate_schema_eight(connection: &Connection) -> StoreResult<()> {
    connection.execute_batch(
        "CREATE TABLE generation_cluster_records (
             generation_cluster_id TEXT PRIMARY KEY NOT NULL
                 CHECK(length(generation_cluster_id) = 64)
                 CHECK(generation_cluster_id = lower(generation_cluster_id))
                 CHECK(generation_cluster_id NOT GLOB '*[^0-9a-f]*'),
             canonical_json BLOB NOT NULL CHECK(length(canonical_json) BETWEEN 1 AND 16384)
         ) STRICT;

         CREATE TABLE generation_case_manifests (
             generation_case_id TEXT PRIMARY KEY NOT NULL
                 CHECK(length(generation_case_id) = 64)
                 CHECK(generation_case_id = lower(generation_case_id))
                 CHECK(generation_case_id NOT GLOB '*[^0-9a-f]*'),
             generation_cluster_id TEXT NOT NULL
                 CHECK(length(generation_cluster_id) = 64)
                 CHECK(generation_cluster_id = lower(generation_cluster_id))
                 CHECK(generation_cluster_id NOT GLOB '*[^0-9a-f]*'),
             deterministic_case_contract_digest TEXT NOT NULL
                 CHECK(length(deterministic_case_contract_digest) = 64)
                 CHECK(deterministic_case_contract_digest =
                       lower(deterministic_case_contract_digest))
                 CHECK(deterministic_case_contract_digest NOT GLOB '*[^0-9a-f]*'),
             canonical_json BLOB NOT NULL CHECK(length(canonical_json) BETWEEN 1 AND 16384),
             FOREIGN KEY (generation_cluster_id)
                 REFERENCES generation_cluster_records(generation_cluster_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (deterministic_case_contract_digest)
                 REFERENCES generation_deterministic_case_contracts(contract_digest)
                 ON UPDATE RESTRICT ON DELETE RESTRICT
         ) STRICT;

         CREATE TABLE generation_deterministic_case_contracts (
             contract_digest TEXT PRIMARY KEY NOT NULL
                 CHECK(length(contract_digest) = 64)
                 CHECK(contract_digest = lower(contract_digest))
                 CHECK(contract_digest NOT GLOB '*[^0-9a-f]*'),
             canonical_json BLOB NOT NULL CHECK(length(canonical_json) BETWEEN 1 AND 1048576)
         ) STRICT;

         CREATE TABLE generation_suite_manifests (
             generation_suite_manifest_id TEXT PRIMARY KEY NOT NULL
                 CHECK(length(generation_suite_manifest_id) = 64)
                 CHECK(generation_suite_manifest_id = lower(generation_suite_manifest_id))
                 CHECK(generation_suite_manifest_id NOT GLOB '*[^0-9a-f]*'),
             case_count INTEGER NOT NULL CHECK(case_count BETWEEN 1 AND 256),
             canonical_json BLOB NOT NULL CHECK(length(canonical_json) BETWEEN 1 AND 4194304)
         ) STRICT;

         CREATE TABLE generation_suite_cases (
             generation_suite_manifest_id TEXT NOT NULL
                 CHECK(length(generation_suite_manifest_id) = 64)
                 CHECK(generation_suite_manifest_id = lower(generation_suite_manifest_id))
                 CHECK(generation_suite_manifest_id NOT GLOB '*[^0-9a-f]*'),
             semantic_ordinal INTEGER NOT NULL CHECK(semantic_ordinal BETWEEN 0 AND 255),
             generation_case_id TEXT NOT NULL
                 CHECK(length(generation_case_id) = 64)
                 CHECK(generation_case_id = lower(generation_case_id))
                 CHECK(generation_case_id NOT GLOB '*[^0-9a-f]*'),
             PRIMARY KEY (generation_suite_manifest_id, semantic_ordinal),
             UNIQUE (generation_suite_manifest_id, generation_case_id),
             FOREIGN KEY (generation_suite_manifest_id)
                 REFERENCES generation_suite_manifests(generation_suite_manifest_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (generation_case_id)
                 REFERENCES generation_case_manifests(generation_case_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT
         ) STRICT;

         CREATE TABLE generation_repetition_records (
             generation_repetition_id TEXT PRIMARY KEY NOT NULL
                 CHECK(length(generation_repetition_id) = 64)
                 CHECK(generation_repetition_id = lower(generation_repetition_id))
                 CHECK(generation_repetition_id NOT GLOB '*[^0-9a-f]*'),
             generation_suite_manifest_id TEXT NOT NULL
                 CHECK(length(generation_suite_manifest_id) = 64)
                 CHECK(generation_suite_manifest_id = lower(generation_suite_manifest_id))
                 CHECK(generation_suite_manifest_id NOT GLOB '*[^0-9a-f]*'),
             repetition_ordinal INTEGER NOT NULL CHECK(repetition_ordinal BETWEEN 0 AND 1023),
             canonical_json BLOB NOT NULL CHECK(length(canonical_json) BETWEEN 1 AND 16384),
             FOREIGN KEY (generation_suite_manifest_id)
                 REFERENCES generation_suite_manifests(generation_suite_manifest_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT
         ) STRICT;

         CREATE TABLE planned_candidate_attempts (
             planned_candidate_attempt_id TEXT PRIMARY KEY NOT NULL
                 CHECK(length(planned_candidate_attempt_id) = 64)
                 CHECK(planned_candidate_attempt_id = lower(planned_candidate_attempt_id))
                 CHECK(planned_candidate_attempt_id NOT GLOB '*[^0-9a-f]*'),
             generation_suite_manifest_id TEXT NOT NULL
                 CHECK(length(generation_suite_manifest_id) = 64)
                 CHECK(generation_suite_manifest_id = lower(generation_suite_manifest_id))
                 CHECK(generation_suite_manifest_id NOT GLOB '*[^0-9a-f]*'),
             generation_case_id TEXT NOT NULL
                 CHECK(length(generation_case_id) = 64)
                 CHECK(generation_case_id = lower(generation_case_id))
                 CHECK(generation_case_id NOT GLOB '*[^0-9a-f]*'),
             generation_cluster_id TEXT NOT NULL
                 CHECK(length(generation_cluster_id) = 64)
                 CHECK(generation_cluster_id = lower(generation_cluster_id))
                 CHECK(generation_cluster_id NOT GLOB '*[^0-9a-f]*'),
             generation_repetition_id TEXT NOT NULL
                 CHECK(length(generation_repetition_id) = 64)
                 CHECK(generation_repetition_id = lower(generation_repetition_id))
                 CHECK(generation_repetition_id NOT GLOB '*[^0-9a-f]*'),
             generation_system_id TEXT NOT NULL
                 CHECK(length(generation_system_id) = 64)
                 CHECK(generation_system_id = lower(generation_system_id))
                 CHECK(generation_system_id NOT GLOB '*[^0-9a-f]*'),
             attempt_ordinal INTEGER NOT NULL CHECK(attempt_ordinal BETWEEN 0 AND 1023),
             canonical_json BLOB NOT NULL CHECK(length(canonical_json) BETWEEN 1 AND 16384),
             FOREIGN KEY (generation_suite_manifest_id)
                 REFERENCES generation_suite_manifests(generation_suite_manifest_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (generation_case_id)
                 REFERENCES generation_case_manifests(generation_case_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (generation_cluster_id)
                 REFERENCES generation_cluster_records(generation_cluster_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (generation_repetition_id)
                 REFERENCES generation_repetition_records(generation_repetition_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (generation_system_id)
                 REFERENCES generation_system_records(generation_system_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT
         ) STRICT;

         CREATE TABLE generation_qualification_plans (
             generation_qualification_plan_id TEXT PRIMARY KEY NOT NULL
                 CHECK(length(generation_qualification_plan_id) = 64)
                 CHECK(generation_qualification_plan_id = lower(generation_qualification_plan_id))
                 CHECK(generation_qualification_plan_id NOT GLOB '*[^0-9a-f]*'),
             generation_suite_manifest_id TEXT NOT NULL
                 CHECK(length(generation_suite_manifest_id) = 64)
                 CHECK(generation_suite_manifest_id = lower(generation_suite_manifest_id))
                 CHECK(generation_suite_manifest_id NOT GLOB '*[^0-9a-f]*'),
             candidate_selection_policy_id TEXT NOT NULL
                 CHECK(length(candidate_selection_policy_id) = 64)
                 CHECK(candidate_selection_policy_id = lower(candidate_selection_policy_id))
                 CHECK(candidate_selection_policy_id NOT GLOB '*[^0-9a-f]*'),
             repetition_count INTEGER NOT NULL CHECK(repetition_count BETWEEN 1 AND 1024),
             generation_system_count INTEGER NOT NULL
                 CHECK(generation_system_count BETWEEN 1 AND 16),
             planned_attempt_count INTEGER NOT NULL CHECK(planned_attempt_count BETWEEN 1 AND 1024),
             canonical_json BLOB NOT NULL CHECK(length(canonical_json) BETWEEN 1 AND 4194304),
             FOREIGN KEY (generation_suite_manifest_id)
                 REFERENCES generation_suite_manifests(generation_suite_manifest_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (candidate_selection_policy_id)
                 REFERENCES candidate_selection_policies(candidate_selection_policy_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT
         ) STRICT;

         CREATE TABLE candidate_selection_policies (
             candidate_selection_policy_id TEXT PRIMARY KEY NOT NULL
                 CHECK(length(candidate_selection_policy_id) = 64)
                 CHECK(candidate_selection_policy_id = lower(candidate_selection_policy_id))
                 CHECK(candidate_selection_policy_id NOT GLOB '*[^0-9a-f]*'),
             generation_suite_manifest_id TEXT NOT NULL
                 CHECK(length(generation_suite_manifest_id) = 64)
                 CHECK(generation_suite_manifest_id = lower(generation_suite_manifest_id))
                 CHECK(generation_suite_manifest_id NOT GLOB '*[^0-9a-f]*'),
             entry_count INTEGER NOT NULL CHECK(entry_count BETWEEN 1 AND 256),
             canonical_json BLOB NOT NULL CHECK(length(canonical_json) BETWEEN 1 AND 4194304),
             FOREIGN KEY (generation_suite_manifest_id)
                 REFERENCES generation_suite_manifests(generation_suite_manifest_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT
         ) STRICT;

         CREATE TABLE generation_qualification_plan_repetitions (
             generation_qualification_plan_id TEXT NOT NULL
                 CHECK(length(generation_qualification_plan_id) = 64)
                 CHECK(generation_qualification_plan_id = lower(generation_qualification_plan_id))
                 CHECK(generation_qualification_plan_id NOT GLOB '*[^0-9a-f]*'),
             plan_ordinal INTEGER NOT NULL CHECK(plan_ordinal BETWEEN 0 AND 1023),
             generation_repetition_id TEXT NOT NULL
                 CHECK(length(generation_repetition_id) = 64)
                 CHECK(generation_repetition_id = lower(generation_repetition_id))
                 CHECK(generation_repetition_id NOT GLOB '*[^0-9a-f]*'),
             PRIMARY KEY (generation_qualification_plan_id, plan_ordinal),
             UNIQUE (generation_qualification_plan_id, generation_repetition_id),
             FOREIGN KEY (generation_qualification_plan_id)
                 REFERENCES generation_qualification_plans(generation_qualification_plan_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (generation_repetition_id)
                 REFERENCES generation_repetition_records(generation_repetition_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT
         ) STRICT;

         CREATE TABLE generation_qualification_plan_systems (
             generation_qualification_plan_id TEXT NOT NULL
                 CHECK(length(generation_qualification_plan_id) = 64)
                 CHECK(generation_qualification_plan_id = lower(generation_qualification_plan_id))
                 CHECK(generation_qualification_plan_id NOT GLOB '*[^0-9a-f]*'),
             plan_ordinal INTEGER NOT NULL CHECK(plan_ordinal BETWEEN 0 AND 15),
             generation_system_id TEXT NOT NULL
                 CHECK(length(generation_system_id) = 64)
                 CHECK(generation_system_id = lower(generation_system_id))
                 CHECK(generation_system_id NOT GLOB '*[^0-9a-f]*'),
             PRIMARY KEY (generation_qualification_plan_id, plan_ordinal),
             UNIQUE (generation_qualification_plan_id, generation_system_id),
             FOREIGN KEY (generation_qualification_plan_id)
                 REFERENCES generation_qualification_plans(generation_qualification_plan_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (generation_system_id)
                 REFERENCES generation_system_records(generation_system_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT
         ) STRICT;

         CREATE TABLE generation_qualification_plan_attempts (
             generation_qualification_plan_id TEXT NOT NULL
                 CHECK(length(generation_qualification_plan_id) = 64)
                 CHECK(generation_qualification_plan_id = lower(generation_qualification_plan_id))
                 CHECK(generation_qualification_plan_id NOT GLOB '*[^0-9a-f]*'),
             plan_ordinal INTEGER NOT NULL CHECK(plan_ordinal BETWEEN 0 AND 1023),
             planned_candidate_attempt_id TEXT NOT NULL
                 CHECK(length(planned_candidate_attempt_id) = 64)
                 CHECK(planned_candidate_attempt_id = lower(planned_candidate_attempt_id))
                 CHECK(planned_candidate_attempt_id NOT GLOB '*[^0-9a-f]*'),
             PRIMARY KEY (generation_qualification_plan_id, plan_ordinal),
             UNIQUE (generation_qualification_plan_id, planned_candidate_attempt_id),
             FOREIGN KEY (generation_qualification_plan_id)
                 REFERENCES generation_qualification_plans(generation_qualification_plan_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (planned_candidate_attempt_id)
                 REFERENCES planned_candidate_attempts(planned_candidate_attempt_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT
         ) STRICT;

         PRAGMA user_version = 9;",
    )?;
    Ok(())
}
