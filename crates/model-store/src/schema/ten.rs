use rusqlite::Connection;

use crate::StoreResult;

#[expect(clippy::too_many_lines, reason = "one atomic schema cohort")]
pub(super) fn migrate_schema_nine(connection: &Connection) -> StoreResult<()> {
    connection.execute_batch(
        "CREATE TABLE candidate_generation_attempt_precursors (
             candidate_generation_attempt_precursor_id TEXT PRIMARY KEY NOT NULL
                 CHECK(length(candidate_generation_attempt_precursor_id) = 64)
                 CHECK(candidate_generation_attempt_precursor_id =
                       lower(candidate_generation_attempt_precursor_id))
                 CHECK(candidate_generation_attempt_precursor_id NOT GLOB '*[^0-9a-f]*'),
             generation_qualification_plan_id TEXT NOT NULL
                 CHECK(length(generation_qualification_plan_id) = 64)
                 CHECK(generation_qualification_plan_id = lower(generation_qualification_plan_id))
                 CHECK(generation_qualification_plan_id NOT GLOB '*[^0-9a-f]*'),
             planned_candidate_attempt_id TEXT NOT NULL
                 CHECK(length(planned_candidate_attempt_id) = 64)
                 CHECK(planned_candidate_attempt_id = lower(planned_candidate_attempt_id))
                 CHECK(planned_candidate_attempt_id NOT GLOB '*[^0-9a-f]*'),
             structured_request_binding_id TEXT NOT NULL
                 CHECK(length(structured_request_binding_id) = 64)
                 CHECK(structured_request_binding_id = lower(structured_request_binding_id))
                 CHECK(structured_request_binding_id NOT GLOB '*[^0-9a-f]*'),
             canonical_json BLOB NOT NULL CHECK(length(canonical_json) BETWEEN 1 AND 16384),
             UNIQUE (generation_qualification_plan_id, planned_candidate_attempt_id),
             UNIQUE (
                 candidate_generation_attempt_precursor_id,
                 generation_qualification_plan_id,
                 planned_candidate_attempt_id
             ),
             FOREIGN KEY (generation_qualification_plan_id, planned_candidate_attempt_id)
                 REFERENCES generation_qualification_plan_attempts(
                     generation_qualification_plan_id,
                     planned_candidate_attempt_id
                 )
                 ON UPDATE RESTRICT ON DELETE RESTRICT
         ) STRICT;

         CREATE TABLE managed_candidate_generation_evidence (
             managed_candidate_generation_evidence_id TEXT PRIMARY KEY NOT NULL
                 CHECK(length(managed_candidate_generation_evidence_id) = 64)
                 CHECK(managed_candidate_generation_evidence_id =
                       lower(managed_candidate_generation_evidence_id))
                 CHECK(managed_candidate_generation_evidence_id NOT GLOB '*[^0-9a-f]*'),
             candidate_generation_attempt_precursor_id TEXT NOT NULL
                 CHECK(length(candidate_generation_attempt_precursor_id) = 64)
                 CHECK(candidate_generation_attempt_precursor_id =
                       lower(candidate_generation_attempt_precursor_id))
                 CHECK(candidate_generation_attempt_precursor_id NOT GLOB '*[^0-9a-f]*'),
             bracket_observation_v1_id TEXT NOT NULL
                 CHECK(length(bracket_observation_v1_id) = 64)
                 CHECK(bracket_observation_v1_id = lower(bracket_observation_v1_id))
                 CHECK(bracket_observation_v1_id NOT GLOB '*[^0-9a-f]*'),
             effective_package_evidence_v2_id TEXT NOT NULL
                 CHECK(length(effective_package_evidence_v2_id) = 64)
                 CHECK(effective_package_evidence_v2_id = lower(effective_package_evidence_v2_id))
                 CHECK(effective_package_evidence_v2_id NOT GLOB '*[^0-9a-f]*'),
             effective_runtime_state_id TEXT NOT NULL
                 CHECK(length(effective_runtime_state_id) = 64)
                 CHECK(effective_runtime_state_id = lower(effective_runtime_state_id))
                 CHECK(effective_runtime_state_id NOT GLOB '*[^0-9a-f]*'),
             effective_runtime_state_join_id TEXT NOT NULL
                 CHECK(length(effective_runtime_state_join_id) = 64)
                 CHECK(effective_runtime_state_join_id = lower(effective_runtime_state_join_id))
                 CHECK(effective_runtime_state_join_id NOT GLOB '*[^0-9a-f]*'),
             generation_request_binding_id TEXT NOT NULL
                 CHECK(length(generation_request_binding_id) = 64)
                 CHECK(generation_request_binding_id = lower(generation_request_binding_id))
                 CHECK(generation_request_binding_id NOT GLOB '*[^0-9a-f]*'),
             structured_request_binding_id TEXT NOT NULL
                 CHECK(length(structured_request_binding_id) = 64)
                 CHECK(structured_request_binding_id = lower(structured_request_binding_id))
                 CHECK(structured_request_binding_id NOT GLOB '*[^0-9a-f]*'),
             response_id TEXT NOT NULL
                 CHECK(length(response_id) = 64)
                 CHECK(response_id = lower(response_id))
                 CHECK(response_id NOT GLOB '*[^0-9a-f]*'),
             canonical_json BLOB NOT NULL CHECK(length(canonical_json) BETWEEN 1 AND 65536),
             UNIQUE (candidate_generation_attempt_precursor_id),
             UNIQUE (
                 managed_candidate_generation_evidence_id,
                 candidate_generation_attempt_precursor_id
             ),
             UNIQUE (
                 managed_candidate_generation_evidence_id,
                 candidate_generation_attempt_precursor_id,
                 response_id
             ),
             FOREIGN KEY (candidate_generation_attempt_precursor_id)
                 REFERENCES candidate_generation_attempt_precursors(
                     candidate_generation_attempt_precursor_id
                 )
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (effective_package_evidence_v2_id)
                 REFERENCES effective_package_evidence_v2(effective_package_evidence_v2_id)
                 ON UPDATE RESTRICT ON DELETE RESTRICT
         ) STRICT;

         CREATE TABLE candidate_generation_cleanup_records (
             candidate_generation_cleanup_id TEXT PRIMARY KEY NOT NULL
                 CHECK(length(candidate_generation_cleanup_id) = 64)
                 CHECK(candidate_generation_cleanup_id = lower(candidate_generation_cleanup_id))
                 CHECK(candidate_generation_cleanup_id NOT GLOB '*[^0-9a-f]*'),
             candidate_generation_attempt_precursor_id TEXT NOT NULL
                 CHECK(length(candidate_generation_attempt_precursor_id) = 64)
                 CHECK(candidate_generation_attempt_precursor_id =
                       lower(candidate_generation_attempt_precursor_id))
                 CHECK(candidate_generation_attempt_precursor_id NOT GLOB '*[^0-9a-f]*'),
             managed_candidate_generation_evidence_id TEXT NOT NULL
                 CHECK(length(managed_candidate_generation_evidence_id) = 64)
                 CHECK(managed_candidate_generation_evidence_id =
                       lower(managed_candidate_generation_evidence_id))
                 CHECK(managed_candidate_generation_evidence_id NOT GLOB '*[^0-9a-f]*'),
             canonical_json BLOB NOT NULL CHECK(length(canonical_json) BETWEEN 1 AND 16384),
             UNIQUE (managed_candidate_generation_evidence_id),
             UNIQUE (
                 candidate_generation_cleanup_id,
                 candidate_generation_attempt_precursor_id,
                 managed_candidate_generation_evidence_id
             ),
             FOREIGN KEY (
                 managed_candidate_generation_evidence_id,
                 candidate_generation_attempt_precursor_id
             ) REFERENCES managed_candidate_generation_evidence(
                 managed_candidate_generation_evidence_id,
                 candidate_generation_attempt_precursor_id
             )
             ON UPDATE RESTRICT ON DELETE RESTRICT
         ) STRICT;

         CREATE TABLE generation_evidence_bundles (
             candidate_generation_evidence_bundle_id TEXT PRIMARY KEY NOT NULL
                 CHECK(length(candidate_generation_evidence_bundle_id) = 64)
                 CHECK(candidate_generation_evidence_bundle_id =
                       lower(candidate_generation_evidence_bundle_id))
                 CHECK(candidate_generation_evidence_bundle_id NOT GLOB '*[^0-9a-f]*'),
             candidate_generation_attempt_precursor_id TEXT NOT NULL
                 CHECK(length(candidate_generation_attempt_precursor_id) = 64)
                 CHECK(candidate_generation_attempt_precursor_id =
                       lower(candidate_generation_attempt_precursor_id))
                 CHECK(candidate_generation_attempt_precursor_id NOT GLOB '*[^0-9a-f]*'),
             managed_candidate_generation_evidence_id TEXT NOT NULL
                 CHECK(length(managed_candidate_generation_evidence_id) = 64)
                 CHECK(managed_candidate_generation_evidence_id =
                       lower(managed_candidate_generation_evidence_id))
                 CHECK(managed_candidate_generation_evidence_id NOT GLOB '*[^0-9a-f]*'),
             response_id TEXT NOT NULL
                 CHECK(length(response_id) = 64)
                 CHECK(response_id = lower(response_id))
                 CHECK(response_id NOT GLOB '*[^0-9a-f]*'),
             candidate_generation_cleanup_id TEXT NOT NULL
                 CHECK(length(candidate_generation_cleanup_id) = 64)
                 CHECK(candidate_generation_cleanup_id = lower(candidate_generation_cleanup_id))
                 CHECK(candidate_generation_cleanup_id NOT GLOB '*[^0-9a-f]*'),
             canonical_json BLOB NOT NULL CHECK(length(canonical_json) BETWEEN 1 AND 4194304),
             UNIQUE (candidate_generation_cleanup_id),
             UNIQUE (
                 candidate_generation_evidence_bundle_id,
                 candidate_generation_attempt_precursor_id,
                 managed_candidate_generation_evidence_id,
                 candidate_generation_cleanup_id
             ),
             FOREIGN KEY (
                 managed_candidate_generation_evidence_id,
                 candidate_generation_attempt_precursor_id,
                 response_id
             ) REFERENCES managed_candidate_generation_evidence(
                 managed_candidate_generation_evidence_id,
                 candidate_generation_attempt_precursor_id,
                 response_id
             )
             ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (
                 candidate_generation_cleanup_id,
                 candidate_generation_attempt_precursor_id,
                 managed_candidate_generation_evidence_id
             ) REFERENCES candidate_generation_cleanup_records(
                 candidate_generation_cleanup_id,
                 candidate_generation_attempt_precursor_id,
                 managed_candidate_generation_evidence_id
             )
             ON UPDATE RESTRICT ON DELETE RESTRICT
         ) STRICT;

         CREATE TABLE generation_evidence_bundle_storage (
             candidate_generation_evidence_bundle_id TEXT PRIMARY KEY NOT NULL
                 CHECK(length(candidate_generation_evidence_bundle_id) = 64)
                 CHECK(candidate_generation_evidence_bundle_id =
                       lower(candidate_generation_evidence_bundle_id))
                 CHECK(candidate_generation_evidence_bundle_id NOT GLOB '*[^0-9a-f]*'),
             generation_qualification_plan_id TEXT NOT NULL
                 CHECK(length(generation_qualification_plan_id) = 64)
                 CHECK(generation_qualification_plan_id = lower(generation_qualification_plan_id))
                 CHECK(generation_qualification_plan_id NOT GLOB '*[^0-9a-f]*'),
             planned_candidate_attempt_id TEXT NOT NULL
                 CHECK(length(planned_candidate_attempt_id) = 64)
                 CHECK(planned_candidate_attempt_id = lower(planned_candidate_attempt_id))
                 CHECK(planned_candidate_attempt_id NOT GLOB '*[^0-9a-f]*'),
             storage_root_id TEXT NOT NULL
                 CHECK(length(storage_root_id) = 64)
                 CHECK(storage_root_id = lower(storage_root_id))
                 CHECK(storage_root_id NOT GLOB '*[^0-9a-f]*'),
             relative_reference TEXT NOT NULL
                 CHECK(length(CAST(relative_reference AS BLOB)) BETWEEN 1 AND 512)
                 CHECK(relative_reference NOT GLOB '*[^ -~]*')
                 CHECK(substr(relative_reference, 1, 1) <> '/')
                 CHECK(substr(relative_reference, -1, 1) <> '/')
                 CHECK(instr(relative_reference, '//') = 0)
                 CHECK(instr(relative_reference, '\\') = 0)
                 CHECK(instr(relative_reference, '<') = 0)
                 CHECK(instr(relative_reference, '>') = 0)
                 CHECK(instr(relative_reference, ':') = 0)
                 CHECK(instr(relative_reference, '\"') = 0)
                 CHECK(instr(relative_reference, '|') = 0)
                 CHECK(instr(relative_reference, '?') = 0)
                 CHECK(instr(relative_reference, '*') = 0)
                 CHECK('/' || relative_reference || '/' NOT LIKE '%/./%')
                 CHECK('/' || relative_reference || '/' NOT LIKE '%/../%')
                 CHECK('/' || relative_reference || '/' NOT GLOB '*/ /*')
                 CHECK('/' || relative_reference || '/' NOT GLOB '* /*')
                 CHECK('/' || relative_reference || '/' NOT GLOB '*./*')
                 CHECK(relative_reference =
                       'bundles/v1/' || generation_qualification_plan_id || '/' ||
                       planned_candidate_attempt_id || '/' ||
                       candidate_generation_evidence_bundle_id),
             maximum_tree_entries INTEGER NOT NULL
                 CHECK(maximum_tree_entries BETWEEN 1 AND 8193),
             maximum_tree_depth INTEGER NOT NULL
                 CHECK(maximum_tree_depth BETWEEN 1 AND 256),
             maximum_aggregate_bytes INTEGER NOT NULL
                 CHECK(maximum_aggregate_bytes BETWEEN 1 AND 268435456),
             UNIQUE (storage_root_id, relative_reference),
             FOREIGN KEY (generation_qualification_plan_id, planned_candidate_attempt_id)
                 REFERENCES generation_qualification_plan_attempts(
                     generation_qualification_plan_id,
                     planned_candidate_attempt_id
                 )
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (candidate_generation_evidence_bundle_id)
                 REFERENCES generation_evidence_bundles(
                     candidate_generation_evidence_bundle_id
                 )
                 ON UPDATE RESTRICT ON DELETE RESTRICT
         ) STRICT;

         CREATE TABLE generation_evidence_bundle_readbacks (
             candidate_generation_evidence_bundle_readback_id TEXT PRIMARY KEY NOT NULL
                 CHECK(length(candidate_generation_evidence_bundle_readback_id) = 64)
                 CHECK(candidate_generation_evidence_bundle_readback_id =
                       lower(candidate_generation_evidence_bundle_readback_id))
                 CHECK(candidate_generation_evidence_bundle_readback_id NOT GLOB '*[^0-9a-f]*'),
             candidate_generation_evidence_bundle_id TEXT NOT NULL
                 CHECK(length(candidate_generation_evidence_bundle_id) = 64)
                 CHECK(candidate_generation_evidence_bundle_id =
                       lower(candidate_generation_evidence_bundle_id))
                 CHECK(candidate_generation_evidence_bundle_id NOT GLOB '*[^0-9a-f]*'),
             canonical_json BLOB NOT NULL CHECK(length(canonical_json) BETWEEN 1 AND 16384),
             UNIQUE (candidate_generation_evidence_bundle_id),
             UNIQUE (
                 candidate_generation_evidence_bundle_readback_id,
                 candidate_generation_evidence_bundle_id
             ),
             FOREIGN KEY (candidate_generation_evidence_bundle_id)
                 REFERENCES generation_evidence_bundles(
                     candidate_generation_evidence_bundle_id
                 )
                 ON UPDATE RESTRICT ON DELETE RESTRICT
         ) STRICT;

         CREATE TABLE candidate_generation_receipts (
             candidate_generation_receipt_id TEXT PRIMARY KEY NOT NULL
                 CHECK(length(candidate_generation_receipt_id) = 64)
                 CHECK(candidate_generation_receipt_id = lower(candidate_generation_receipt_id))
                 CHECK(candidate_generation_receipt_id NOT GLOB '*[^0-9a-f]*'),
             generation_qualification_plan_id TEXT NOT NULL
                 CHECK(length(generation_qualification_plan_id) = 64)
                 CHECK(generation_qualification_plan_id = lower(generation_qualification_plan_id))
                 CHECK(generation_qualification_plan_id NOT GLOB '*[^0-9a-f]*'),
             planned_candidate_attempt_id TEXT NOT NULL
                 CHECK(length(planned_candidate_attempt_id) = 64)
                 CHECK(planned_candidate_attempt_id = lower(planned_candidate_attempt_id))
                 CHECK(planned_candidate_attempt_id NOT GLOB '*[^0-9a-f]*'),
             candidate_generation_attempt_precursor_id TEXT NOT NULL
                 CHECK(length(candidate_generation_attempt_precursor_id) = 64)
                 CHECK(candidate_generation_attempt_precursor_id =
                       lower(candidate_generation_attempt_precursor_id))
                 CHECK(candidate_generation_attempt_precursor_id NOT GLOB '*[^0-9a-f]*'),
             managed_candidate_generation_evidence_id TEXT NOT NULL
                 CHECK(length(managed_candidate_generation_evidence_id) = 64)
                 CHECK(managed_candidate_generation_evidence_id =
                       lower(managed_candidate_generation_evidence_id))
                 CHECK(managed_candidate_generation_evidence_id NOT GLOB '*[^0-9a-f]*'),
             candidate_generation_cleanup_id TEXT NOT NULL
                 CHECK(length(candidate_generation_cleanup_id) = 64)
                 CHECK(candidate_generation_cleanup_id = lower(candidate_generation_cleanup_id))
                 CHECK(candidate_generation_cleanup_id NOT GLOB '*[^0-9a-f]*'),
             candidate_generation_evidence_bundle_id TEXT NOT NULL
                 CHECK(length(candidate_generation_evidence_bundle_id) = 64)
                 CHECK(candidate_generation_evidence_bundle_id =
                       lower(candidate_generation_evidence_bundle_id))
                 CHECK(candidate_generation_evidence_bundle_id NOT GLOB '*[^0-9a-f]*'),
             candidate_generation_evidence_bundle_readback_id TEXT NOT NULL
                 CHECK(length(candidate_generation_evidence_bundle_readback_id) = 64)
                 CHECK(candidate_generation_evidence_bundle_readback_id =
                       lower(candidate_generation_evidence_bundle_readback_id))
                 CHECK(candidate_generation_evidence_bundle_readback_id NOT GLOB '*[^0-9a-f]*'),
             canonical_json BLOB NOT NULL CHECK(length(canonical_json) BETWEEN 1 AND 16384),
             UNIQUE (generation_qualification_plan_id, planned_candidate_attempt_id),
             UNIQUE (
                 candidate_generation_receipt_id,
                 generation_qualification_plan_id,
                 planned_candidate_attempt_id,
                 candidate_generation_attempt_precursor_id
             ),
             FOREIGN KEY (generation_qualification_plan_id, planned_candidate_attempt_id)
                 REFERENCES generation_qualification_plan_attempts(
                     generation_qualification_plan_id,
                     planned_candidate_attempt_id
                 )
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (
                 candidate_generation_attempt_precursor_id,
                 generation_qualification_plan_id,
                 planned_candidate_attempt_id
             ) REFERENCES candidate_generation_attempt_precursors(
                 candidate_generation_attempt_precursor_id,
                 generation_qualification_plan_id,
                 planned_candidate_attempt_id
             )
             ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (
                 managed_candidate_generation_evidence_id,
                 candidate_generation_attempt_precursor_id
             ) REFERENCES managed_candidate_generation_evidence(
                 managed_candidate_generation_evidence_id,
                 candidate_generation_attempt_precursor_id
             )
             ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (
                 candidate_generation_cleanup_id,
                 candidate_generation_attempt_precursor_id,
                 managed_candidate_generation_evidence_id
             ) REFERENCES candidate_generation_cleanup_records(
                 candidate_generation_cleanup_id,
                 candidate_generation_attempt_precursor_id,
                 managed_candidate_generation_evidence_id
             )
             ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (
                 candidate_generation_evidence_bundle_id,
                 candidate_generation_attempt_precursor_id,
                 managed_candidate_generation_evidence_id,
                 candidate_generation_cleanup_id
             ) REFERENCES generation_evidence_bundles(
                 candidate_generation_evidence_bundle_id,
                 candidate_generation_attempt_precursor_id,
                 managed_candidate_generation_evidence_id,
                 candidate_generation_cleanup_id
             )
             ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (
                 candidate_generation_evidence_bundle_readback_id,
                 candidate_generation_evidence_bundle_id
             ) REFERENCES generation_evidence_bundle_readbacks(
                 candidate_generation_evidence_bundle_readback_id,
                 candidate_generation_evidence_bundle_id
             )
             ON UPDATE RESTRICT ON DELETE RESTRICT
         ) STRICT;

         CREATE TABLE candidate_generation_attempt_records (
             candidate_generation_attempt_record_id TEXT PRIMARY KEY NOT NULL
                 CHECK(length(candidate_generation_attempt_record_id) = 64)
                 CHECK(candidate_generation_attempt_record_id =
                       lower(candidate_generation_attempt_record_id))
                 CHECK(candidate_generation_attempt_record_id NOT GLOB '*[^0-9a-f]*'),
             generation_qualification_plan_id TEXT NOT NULL
                 CHECK(length(generation_qualification_plan_id) = 64)
                 CHECK(generation_qualification_plan_id = lower(generation_qualification_plan_id))
                 CHECK(generation_qualification_plan_id NOT GLOB '*[^0-9a-f]*'),
             planned_candidate_attempt_id TEXT NOT NULL
                 CHECK(length(planned_candidate_attempt_id) = 64)
                 CHECK(planned_candidate_attempt_id = lower(planned_candidate_attempt_id))
                 CHECK(planned_candidate_attempt_id NOT GLOB '*[^0-9a-f]*'),
             outcome TEXT NOT NULL CHECK(outcome IN ('completed', 'failed')),
             candidate_generation_attempt_precursor_id TEXT
                 CHECK(candidate_generation_attempt_precursor_id IS NULL OR (
                     length(candidate_generation_attempt_precursor_id) = 64
                     AND candidate_generation_attempt_precursor_id =
                         lower(candidate_generation_attempt_precursor_id)
                     AND candidate_generation_attempt_precursor_id NOT GLOB '*[^0-9a-f]*'
                 )),
             candidate_generation_receipt_id TEXT
                 CHECK(candidate_generation_receipt_id IS NULL OR (
                     length(candidate_generation_receipt_id) = 64
                     AND candidate_generation_receipt_id = lower(candidate_generation_receipt_id)
                     AND candidate_generation_receipt_id NOT GLOB '*[^0-9a-f]*'
                 )),
             canonical_json BLOB NOT NULL CHECK(length(canonical_json) BETWEEN 1 AND 16384),
             UNIQUE (generation_qualification_plan_id, planned_candidate_attempt_id),
             CHECK(
                 (outcome = 'completed'
                     AND candidate_generation_attempt_precursor_id IS NOT NULL
                     AND candidate_generation_receipt_id IS NOT NULL)
                 OR
                 (outcome = 'failed' AND candidate_generation_receipt_id IS NULL)
             ),
             FOREIGN KEY (generation_qualification_plan_id, planned_candidate_attempt_id)
                 REFERENCES generation_qualification_plan_attempts(
                     generation_qualification_plan_id,
                     planned_candidate_attempt_id
                 )
                 ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (
                 candidate_generation_attempt_precursor_id,
                 generation_qualification_plan_id,
                 planned_candidate_attempt_id
             ) REFERENCES candidate_generation_attempt_precursors(
                 candidate_generation_attempt_precursor_id,
                 generation_qualification_plan_id,
                 planned_candidate_attempt_id
             )
             ON UPDATE RESTRICT ON DELETE RESTRICT,
             FOREIGN KEY (
                 candidate_generation_receipt_id,
                 generation_qualification_plan_id,
                 planned_candidate_attempt_id,
                 candidate_generation_attempt_precursor_id
             ) REFERENCES candidate_generation_receipts(
                 candidate_generation_receipt_id,
                 generation_qualification_plan_id,
                 planned_candidate_attempt_id,
                 candidate_generation_attempt_precursor_id
             )
             ON UPDATE RESTRICT ON DELETE RESTRICT
         ) STRICT;

         PRAGMA user_version = 10;",
    )?;
    Ok(())
}
