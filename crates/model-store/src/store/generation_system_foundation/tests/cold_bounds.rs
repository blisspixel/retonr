use rewrite_model::{
    GenerationSystemRecordV1, MAX_ARTIFACT_SET_MANIFEST_JSON_BYTES,
    MAX_MODEL_PACKAGE_MANIFEST_JSON_BYTES, MAX_RUNTIME_IDENTITY_JSON_BYTES,
    MAX_RUNTIME_PACKAGE_MANIFEST_JSON_BYTES,
};
use rusqlite::params;
use tempfile::tempdir;

use super::*;

const EVIDENCE_RELATIONSHIPS: [(&str, &str); 3] = [
    ("effective_package_evidence_v2", "artifact_set_id"),
    ("effective_package_evidence_v2", "runtime_build_id"),
    (
        "effective_package_evidence_v2",
        "effective_runtime_state_id",
    ),
];
const SYSTEM_RELATIONSHIPS: [(&str, &str); 7] = [
    ("generation_system_records", "runtime_package_manifest_id"),
    ("generation_system_records", "runtime_build_id"),
    ("generation_system_records", "effective_runtime_state_id"),
    ("generation_system_records", "model_artifact_set_id"),
    ("generation_system_records", "model_package_manifest_id"),
    ("generation_system_records", "model_artifact_id"),
    (
        "generation_system_records",
        "effective_package_evidence_v2_id",
    ),
];

#[test]
fn every_recursive_dependency_rejects_bound_plus_one_before_decode() {
    for dependency in dependency_cases() {
        let directory = tempdir().expect("temporary directory");
        let path = directory
            .path()
            .join(format!("oversized-{}.db", dependency.table));
        let fixture = system_fixture();
        let system = GenerationSystemRecordV1::new(fixture.relations(), fixture.input("target"))
            .expect("generation system");
        let mut store = ArtifactStateStore::open(&path).expect("open store");
        persist_dependencies(&mut store, &fixture);
        store
            .transact_generation_system_foundation_v1(input(&fixture, &system), |_| Ok::<_, ()>(()))
            .expect("persist foundation");
        store
            .connection()
            .pragma_update(None, "ignore_check_constraints", true)
            .expect("ignore check constraints");
        store
            .connection()
            .execute(
                &format!(
                    "UPDATE {} SET record_json = CAST(zeroblob(?1) AS TEXT) WHERE {} = ?2",
                    dependency.table, dependency.key_column
                ),
                params![
                    i64::try_from(dependency.maximum + 1).expect("bound fits i64"),
                    (dependency.key)(&fixture),
                ],
            )
            .expect("inject oversized dependency JSON");
        drop(store);

        let reopened = ArtifactStateStore::open_existing_read_only(&path).expect("cold reopen");
        assert!(matches!(
            reopened.generation_system_foundation_v1(system.generation_system_id()),
            Err(StoreError::CorruptRecord)
        ));
    }
}

#[test]
fn every_schema_eight_relationship_rejects_bound_plus_one_and_large_text() {
    for &(table, column) in EVIDENCE_RELATIONSHIPS
        .iter()
        .chain(SYSTEM_RELATIONSHIPS.iter())
    {
        for length in [65usize, 4_096] {
            let directory = tempdir().expect("temporary directory");
            let path = directory
                .path()
                .join(format!("oversized-{table}-{column}-{length}.db"));
            let fixture = system_fixture();
            let system =
                GenerationSystemRecordV1::new(fixture.relations(), fixture.input("target"))
                    .expect("generation system");
            let mut store = ArtifactStateStore::open(&path).expect("open store");
            persist_dependencies(&mut store, &fixture);
            store
                .transact_generation_system_foundation_v1(input(&fixture, &system), |_| {
                    Ok::<_, ()>(())
                })
                .expect("persist foundation");
            store
                .connection()
                .pragma_update(None, "foreign_keys", false)
                .expect("disable foreign keys");
            store
                .connection()
                .pragma_update(None, "ignore_check_constraints", true)
                .expect("ignore check constraints");
            store
                .connection()
                .execute(
                    &format!("UPDATE {table} SET {column} = ?1"),
                    ["a".repeat(length)],
                )
                .expect("inject oversized relationship");
            drop(store);

            let reopened = ArtifactStateStore::open_existing_read_only(&path).expect("cold reopen");
            assert!(matches!(
                reopened.generation_system_foundation_v1(system.generation_system_id()),
                Err(StoreError::CorruptRecord)
            ));
        }
    }
}

#[test]
fn every_schema_eight_relationship_accepts_the_exact_digest_bound() {
    let directory = tempdir().expect("temporary directory");
    let fixture = system_fixture();
    let system = GenerationSystemRecordV1::new(fixture.relations(), fixture.input("target"))
        .expect("generation system");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("exact-digests.db")).expect("open store");
    persist_dependencies(&mut store, &fixture);
    store
        .transact_generation_system_foundation_v1(input(&fixture, &system), |_| Ok::<_, ()>(()))
        .expect("persist foundation");
    for &(table, column) in EVIDENCE_RELATIONSHIPS
        .iter()
        .chain(SYSTEM_RELATIONSHIPS.iter())
    {
        let length: i64 = store
            .connection()
            .query_row(
                &format!("SELECT length({column}) FROM {table}"),
                [],
                |row| row.get(0),
            )
            .expect("read relationship length");
        assert_eq!(length, 64, "unexpected length for {table}.{column}");
    }
    assert!(
        store
            .generation_system_foundation_v1(system.generation_system_id())
            .expect("read exact-bound foundation")
            .is_some()
    );
}

struct DependencyCase {
    table: &'static str,
    key_column: &'static str,
    maximum: usize,
    key: fn(&SystemFixture) -> String,
}

fn dependency_cases() -> [DependencyCase; 5] {
    [
        DependencyCase {
            table: "runtime_package_manifests",
            key_column: "runtime_package_manifest_id",
            maximum: MAX_RUNTIME_PACKAGE_MANIFEST_JSON_BYTES,
            key: |fixture| {
                fixture
                    .runtime_package
                    .runtime_package_manifest_id()
                    .digest()
                    .as_str()
                    .to_owned()
            },
        },
        DependencyCase {
            table: "runtime_build_identities",
            key_column: "runtime_build_id",
            maximum: MAX_RUNTIME_IDENTITY_JSON_BYTES,
            key: |fixture| {
                fixture
                    .runtime_build
                    .runtime_build_id()
                    .digest()
                    .as_str()
                    .to_owned()
            },
        },
        DependencyCase {
            table: "effective_runtime_states",
            key_column: "effective_runtime_state_id",
            maximum: MAX_RUNTIME_IDENTITY_JSON_BYTES,
            key: |fixture| {
                fixture
                    .runtime_state
                    .effective_runtime_state_id()
                    .digest()
                    .as_str()
                    .to_owned()
            },
        },
        DependencyCase {
            table: "artifact_set_manifests",
            key_column: "artifact_set_id",
            maximum: MAX_ARTIFACT_SET_MANIFEST_JSON_BYTES,
            key: |fixture| {
                fixture
                    .model_set
                    .artifact_set_id()
                    .digest()
                    .as_str()
                    .to_owned()
            },
        },
        DependencyCase {
            table: "model_package_manifests",
            key_column: "model_package_manifest_id",
            maximum: MAX_MODEL_PACKAGE_MANIFEST_JSON_BYTES,
            key: |fixture| {
                fixture
                    .model_package
                    .model_package_manifest_id()
                    .digest()
                    .as_str()
                    .to_owned()
            },
        },
    ]
}
