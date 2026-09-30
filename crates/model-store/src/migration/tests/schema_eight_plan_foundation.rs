use std::collections::BTreeMap;

use rusqlite::Connection;
use tempfile::tempdir;

use super::{reserve_file, schema_five_packages, schema_version};
use crate::store::generation_qualification_preregistration::tests::support::{self, Fixture};
use crate::{ArtifactStateStore, GenerationSystemFoundationV1Input, StoreMigrationDisposition};

const PLAN_FOUNDATION_TABLES: [&str; 12] = [
    "candidate_selection_policies",
    "generation_case_manifests",
    "generation_cluster_records",
    "generation_deterministic_case_contracts",
    "generation_qualification_plan_attempts",
    "generation_qualification_plan_repetitions",
    "generation_qualification_plan_systems",
    "generation_qualification_plans",
    "generation_repetition_records",
    "generation_suite_cases",
    "generation_suite_manifests",
    "planned_candidate_attempts",
];

#[test]
fn schema_nine_plan_foundation_is_strict_bounded_and_foreign_key_clean() {
    let connection = Connection::open_in_memory().expect("open memory database");
    let mut connection = connection;
    crate::schema::initialize_empty(&mut connection).expect("initialize schema nine");
    for table in PLAN_FOUNDATION_TABLES {
        let sql: String = connection
            .query_row(
                "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = ?1",
                [table],
                |row| row.get(0),
            )
            .expect("read schema-nine table SQL");
        assert!(sql.contains("STRICT"), "{table} must be strict");
        assert!(sql.contains("NOT GLOB '*[^0-9a-f]*'"));
        if !table.ends_with("_cases")
            && !table.ends_with("_attempts")
            && !table.ends_with("_repetitions")
            && !table.ends_with("_systems")
        {
            assert!(sql.contains("canonical_json BLOB NOT NULL"));
        }
    }
    let violations: i64 = connection
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .expect("check schema-nine foreign keys");
    assert_eq!(violations, 0);
}

#[test]
fn populated_schema_eight_migrates_after_verified_byte_preserving_backup() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("schema-eight.db");
    let backup = directory.path().join("schema-eight-backup.db");
    let fixture = support::fixture();
    let mut store = ArtifactStateStore::open(&source).expect("create current store");
    persist_schema_eight_foundation(&mut store, &fixture);
    drop(store);
    restore_schema_eight_shape(&source);
    let before_connection = Connection::open(&source).expect("open schema eight");
    let before = all_rows(&before_connection);
    drop(before_connection);

    let mut backup_file = reserve_file(&backup);
    let mut session = ArtifactStateStore::begin_existing_migration(&source)
        .expect("begin schema-eight migration");
    assert_eq!(
        (
            session.schema_status().found,
            session.schema_status().current
        ),
        (8, 17)
    );
    session
        .backup_to(&mut backup_file, 16 * 1024 * 1024, || false)
        .expect("write verified backup");
    let result = session.migrate().expect("migrate schema eight");
    assert_eq!(result.disposition, StoreMigrationDisposition::Migrated);

    let backup_connection = Connection::open(&backup).expect("open backup");
    assert_eq!(schema_version(&backup), 8);
    assert_eq!(all_rows(&backup_connection), before);
    for table in PLAN_FOUNDATION_TABLES {
        assert!(!schema_five_packages::table_exists(
            &backup_connection,
            table
        ));
    }

    let migrated = Connection::open(&source).expect("open migrated source");
    assert_eq!(schema_version(&source), 17);
    for (table, expected) in before {
        assert_eq!(
            schema_five_packages::table_rows(&migrated, &table),
            expected
        );
    }
    for table in PLAN_FOUNDATION_TABLES {
        let count: i64 = migrated
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .expect("count new plan-foundation table");
        assert_eq!(count, 0);
    }
}

fn persist_schema_eight_foundation(store: &mut ArtifactStateStore, fixture: &Fixture) {
    store
        .put_artifact_set_manifest(&fixture.system.runtime_set)
        .expect("runtime artifact set");
    store
        .put_runtime_package_manifest(&fixture.system.runtime_package)
        .expect("runtime package");
    store
        .put_runtime_build_identity(&fixture.system.runtime_build)
        .expect("runtime build");
    store
        .put_effective_runtime_state(&fixture.system.runtime_state)
        .expect("runtime state");
    store
        .put_artifact_set_manifest(&fixture.system.model_set)
        .expect("model artifact set");
    store
        .put_model_package_manifest(&fixture.system.model_package)
        .expect("model package");
    for system in &fixture.systems {
        store
            .transact_generation_system_foundation_v1(
                GenerationSystemFoundationV1Input {
                    generation_system: system,
                    relations: fixture.system.relations(),
                },
                |_| Ok::<_, ()>(()),
            )
            .expect("generation-system foundation");
    }
}

fn restore_schema_eight_shape(path: &std::path::Path) {
    Connection::open(path)
        .expect("open current store for fixture downgrade")
        .execute_batch(
            "DROP TABLE generation_qualification_records;
             DROP TABLE generation_repeatability_terminal_result_records;
             DROP TABLE candidate_deterministic_evaluation_records;
             DROP TABLE candidate_generation_receipt_sets;
             DROP TABLE generation_human_adjudication_policy_denial_records;
             DROP TABLE generation_resource_policy_denial_records;
             DROP TABLE generation_resource_attempt_result_records;
             DROP TABLE candidate_judge_join_records;
             DROP TABLE managed_local_judge_receipts;
             DROP TABLE candidate_judge_observation_batches;
             DROP TABLE candidate_judge_response_aggregates;
             DROP TABLE candidate_judge_request_aggregates;
             DROP TABLE candidate_judge_schedules;
             DROP TABLE candidate_judge_plans;
             DROP TABLE generation_qualification_phase_interruption_records;
             DROP TABLE generation_qualification_operation_receipts;
             DROP TABLE generation_repeatability_result_records;
             DROP TABLE generation_human_adjudication_evidence_manifests;
             DROP TABLE generation_resource_evidence_manifests;
             DROP TABLE generation_repeatability_evidence_manifests;
             DROP TABLE generation_attempt_ledger_manifests;
             DROP TABLE generation_qualification_license_evidence;
             DROP TABLE generation_qualification_platform_evidence;
             DROP TABLE candidate_generation_attempt_records;
             DROP TABLE candidate_generation_receipts;
             DROP TABLE generation_evidence_bundle_readbacks;
             DROP TABLE generation_evidence_bundle_storage;
             DROP TABLE generation_evidence_bundles;
             DROP TABLE candidate_generation_cleanup_records;
             DROP TABLE managed_candidate_generation_evidence;
             DROP TABLE candidate_generation_attempt_precursors;
             DROP TABLE generation_qualification_plan_attempts;
             DROP TABLE generation_qualification_plan_systems;
             DROP TABLE generation_qualification_plan_repetitions;
             DROP TABLE generation_qualification_plans;
             DROP TABLE candidate_selection_policies;
             DROP TABLE planned_candidate_attempts;
             DROP TABLE generation_repetition_records;
             DROP TABLE generation_suite_cases;
             DROP TABLE generation_suite_manifests;
             DROP TABLE generation_case_manifests;
             DROP TABLE generation_deterministic_case_contracts;
             DROP TABLE generation_cluster_records;
             PRAGMA user_version = 8;",
        )
        .expect("restore exact schema-eight shape");
}

fn all_rows(connection: &Connection) -> BTreeMap<String, Vec<Vec<Vec<u8>>>> {
    let mut statement = connection
        .prepare(
            "SELECT name FROM sqlite_schema
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%'
             ORDER BY name",
        )
        .expect("prepare table inventory");
    statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query table inventory")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect table inventory")
        .into_iter()
        .map(|table| {
            let rows = schema_five_packages::table_rows(connection, &table);
            (table, rows)
        })
        .collect()
}
