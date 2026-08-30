use std::{sync::Arc, thread};

use rewrite_model::{GenerationSystemRecordV1, MAX_GENERATION_SYSTEM_JSON_BYTES};
use rusqlite::{Connection, params};
use tempfile::tempdir;

use super::*;
use crate::ArtifactStateStore;
use crate::store::generation_qualification_preregistration::tests::support::{
    SystemFixture, system_fixture,
};

#[path = "tests/cold_bounds.rs"]
mod cold_bounds;

fn persist_dependencies(store: &mut ArtifactStateStore, fixture: &SystemFixture) {
    store
        .put_artifact_set_manifest(&fixture.runtime_set)
        .expect("runtime artifact set");
    store
        .put_runtime_package_manifest(&fixture.runtime_package)
        .expect("runtime package");
    store
        .put_runtime_build_identity(&fixture.runtime_build)
        .expect("runtime build");
    store
        .put_effective_runtime_state(&fixture.runtime_state)
        .expect("runtime state");
    store
        .put_artifact_set_manifest(&fixture.model_set)
        .expect("model artifact set");
    store
        .put_model_package_manifest(&fixture.model_package)
        .expect("model package");
}

fn input<'a>(
    fixture: &'a SystemFixture,
    system: &'a GenerationSystemRecordV1,
) -> GenerationSystemFoundationV1Input<'a> {
    GenerationSystemFoundationV1Input {
        generation_system: system,
        relations: fixture.relations(),
    }
}

#[test]
fn atomic_foundation_round_trips_and_is_idempotent() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("foundation.db");
    let fixture = system_fixture();
    let system = GenerationSystemRecordV1::new(fixture.relations(), fixture.input("target"))
        .expect("system");
    let mut store = ArtifactStateStore::open(&path).expect("open store");
    persist_dependencies(&mut store, &fixture);

    let (value, first) = store
        .transact_generation_system_foundation_v1(input(&fixture, &system), |readback| {
            assert_eq!(
                readback.effective_package_evidence_v2(),
                &fixture.effective_package
            );
            assert_eq!(readback.generation_system(), &system);
            Ok::<_, ()>("validated")
        })
        .expect("insert foundation");
    assert_eq!(value, "validated");
    assert_eq!(
        first.effective_package_evidence_v2,
        WriteDisposition::Inserted
    );
    assert_eq!(first.generation_system, WriteDisposition::Inserted);

    let ((), second) = store
        .transact_generation_system_foundation_v1(input(&fixture, &system), |_| Ok::<_, ()>(()))
        .expect("repeat foundation");
    assert_eq!(
        second.effective_package_evidence_v2,
        WriteDisposition::AlreadyPresent
    );
    assert_eq!(second.generation_system, WriteDisposition::AlreadyPresent);
    let stored = store
        .generation_system_foundation_v1(system.generation_system_id())
        .expect("read foundation")
        .expect("foundation present");
    assert_eq!(
        stored.effective_package_evidence_v2(),
        &fixture.effective_package
    );
    assert_eq!(stored.generation_system(), &system);
}

#[test]
fn absent_dependency_rejects_before_any_foundation_row_is_written() {
    let directory = tempdir().expect("temporary directory");
    let fixture = system_fixture();
    let system = GenerationSystemRecordV1::new(fixture.relations(), fixture.input("target"))
        .expect("system");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("missing.db")).expect("open store");
    let result = store
        .transact_generation_system_foundation_v1(input(&fixture, &system), |_| Ok::<_, ()>(()));
    assert!(matches!(
        result,
        Err(GenerationSystemFoundationV1TransactionError::Store(
            StoreError::MissingRecord
        ))
    ));
    assert_eq!(row_counts(store.connection()), (0, 0));
}

#[test]
fn callback_rejection_rolls_back_both_rows() {
    let directory = tempdir().expect("temporary directory");
    let fixture = system_fixture();
    let system = GenerationSystemRecordV1::new(fixture.relations(), fixture.input("target"))
        .expect("system");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("rollback.db")).expect("open store");
    persist_dependencies(&mut store, &fixture);

    let result = store.transact_generation_system_foundation_v1(input(&fixture, &system), |_| {
        Err::<(), _>("reject-sensitive-detail")
    });
    assert!(matches!(
        &result,
        Err(GenerationSystemFoundationV1TransactionError::Validation(_))
    ));
    assert_eq!(row_counts(store.connection()), (0, 0));
    let error = result.expect_err("callback must reject");
    assert_eq!(
        format!("{error:?}"),
        "GenerationSystemFoundationV1TransactionError::Validation"
    );
    assert!(!format!("{error}").contains("sensitive"));
}

#[test]
fn sql_failure_after_evidence_insert_rolls_back_both_rows() {
    let directory = tempdir().expect("temporary directory");
    let fixture = system_fixture();
    let system = GenerationSystemRecordV1::new(fixture.relations(), fixture.input("target"))
        .expect("system");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("sql-rollback.db")).expect("open store");
    persist_dependencies(&mut store, &fixture);
    store
        .connection()
        .execute_batch(
            "CREATE TEMP TRIGGER reject_generation_system
             BEFORE INSERT ON generation_system_records
             BEGIN
                 SELECT RAISE(ABORT, 'injected failure');
             END;",
        )
        .expect("install failure trigger");
    let result = store
        .transact_generation_system_foundation_v1(input(&fixture, &system), |_| Ok::<_, ()>(()));
    assert!(matches!(
        result,
        Err(GenerationSystemFoundationV1TransactionError::Store(
            StoreError::Database(_)
        ))
    ));
    assert_eq!(row_counts(store.connection()), (0, 0));
}

#[test]
fn an_existing_evidence_row_can_be_completed_atomically() {
    let directory = tempdir().expect("temporary directory");
    let fixture = system_fixture();
    let system = GenerationSystemRecordV1::new(fixture.relations(), fixture.input("target"))
        .expect("system");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("partial.db")).expect("open store");
    persist_dependencies(&mut store, &fixture);
    let encoded = serde_json::to_vec(&fixture.effective_package).expect("evidence JSON");
    insert_effective_package_evidence_v2(store.connection(), &fixture.effective_package, &encoded)
        .expect("seed exact evidence");

    let ((), disposition) = store
        .transact_generation_system_foundation_v1(input(&fixture, &system), |_| Ok::<_, ()>(()))
        .expect("complete foundation");
    assert_eq!(
        disposition.effective_package_evidence_v2,
        WriteDisposition::AlreadyPresent
    );
    assert_eq!(disposition.generation_system, WriteDisposition::Inserted);
}

#[test]
fn conflicting_existing_evidence_rolls_back_the_system_insert() {
    let directory = tempdir().expect("temporary directory");
    let fixture = system_fixture();
    let system = GenerationSystemRecordV1::new(fixture.relations(), fixture.input("target"))
        .expect("system");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("conflict.db")).expect("open store");
    persist_dependencies(&mut store, &fixture);
    let id = fixture.effective_package.effective_package_evidence_v2_id();
    store
        .connection()
        .execute(
            "INSERT INTO effective_package_evidence_v2 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                id.digest().as_str(),
                fixture.model_set.artifact_set_id().digest().as_str(),
                fixture.runtime_build.runtime_build_id().digest().as_str(),
                fixture
                    .runtime_state
                    .effective_runtime_state_id()
                    .digest()
                    .as_str(),
                i64::try_from(fixture.effective_package.member_evidence().len())
                    .expect("member count fits i64"),
                b"{}",
            ],
        )
        .expect("seed conflicting evidence");
    let result = store
        .transact_generation_system_foundation_v1(input(&fixture, &system), |_| Ok::<_, ()>(()));
    assert!(matches!(
        result,
        Err(GenerationSystemFoundationV1TransactionError::Store(
            StoreError::ImmutableConflict
        ))
    ));
    assert_eq!(row_counts(store.connection()), (1, 0));
}

#[test]
fn cold_reads_reject_blob_and_index_corruption() {
    for (table, column, value) in [
        ("generation_system_records", "canonical_json", "X'7B7D'"),
        (
            "generation_system_records",
            "runtime_package_manifest_id",
            "lower(hex(randomblob(32)))",
        ),
        (
            "generation_system_records",
            "runtime_build_id",
            "lower(hex(randomblob(32)))",
        ),
        (
            "generation_system_records",
            "effective_runtime_state_id",
            "lower(hex(randomblob(32)))",
        ),
        (
            "generation_system_records",
            "model_artifact_set_id",
            "lower(hex(randomblob(32)))",
        ),
        (
            "generation_system_records",
            "model_package_manifest_id",
            "lower(hex(randomblob(32)))",
        ),
        (
            "generation_system_records",
            "model_artifact_id",
            "lower(hex(randomblob(32)))",
        ),
        (
            "generation_system_records",
            "effective_package_evidence_v2_id",
            "lower(hex(randomblob(32)))",
        ),
        (
            "effective_package_evidence_v2",
            "artifact_set_id",
            "lower(hex(randomblob(32)))",
        ),
        (
            "effective_package_evidence_v2",
            "runtime_build_id",
            "lower(hex(randomblob(32)))",
        ),
        (
            "effective_package_evidence_v2",
            "effective_runtime_state_id",
            "lower(hex(randomblob(32)))",
        ),
        ("effective_package_evidence_v2", "member_count", "2"),
        ("effective_package_evidence_v2", "canonical_json", "X'7B7D'"),
    ] {
        let directory = tempdir().expect("temporary directory");
        let path = directory
            .path()
            .join(format!("corrupt-{table}-{column}.db"));
        let fixture = system_fixture();
        let system = GenerationSystemRecordV1::new(fixture.relations(), fixture.input("target"))
            .expect("system");
        let mut store = ArtifactStateStore::open(&path).expect("open store");
        persist_dependencies(&mut store, &fixture);
        store
            .transact_generation_system_foundation_v1(input(&fixture, &system), |_| Ok::<_, ()>(()))
            .expect("store foundation");
        store
            .connection()
            .pragma_update(None, "foreign_keys", false)
            .expect("disable foreign keys for corruption fixture");
        store
            .connection()
            .execute_batch(&format!("UPDATE {table} SET {column} = {value}"))
            .expect("tamper row");
        drop(store);

        match ArtifactStateStore::open_existing_read_only(&path) {
            Err(StoreError::CorruptRecord) => {}
            Ok(reopened) => assert!(matches!(
                reopened.generation_system_foundation_v1(system.generation_system_id()),
                Err(StoreError::CorruptRecord)
            )),
            Err(error) => panic!("unexpected cold-open error: {error}"),
        }
    }
}

#[test]
fn strict_storage_rejects_text_and_cold_read_bounds_oversized_blob() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("bounded.db");
    let fixture = system_fixture();
    let system = GenerationSystemRecordV1::new(fixture.relations(), fixture.input("target"))
        .expect("system");
    let mut store = ArtifactStateStore::open(&path).expect("open store");
    persist_dependencies(&mut store, &fixture);
    store
        .transact_generation_system_foundation_v1(input(&fixture, &system), |_| Ok::<_, ()>(()))
        .expect("store foundation");
    assert!(
        store
            .connection()
            .execute(
                "UPDATE generation_system_records SET canonical_json = CAST('{}' AS TEXT)",
                [],
            )
            .is_err()
    );
    store
        .connection()
        .pragma_update(None, "ignore_check_constraints", true)
        .expect("disable check constraints for corruption fixture");
    store
        .connection()
        .execute(
            "UPDATE generation_system_records SET canonical_json = zeroblob(?1)",
            [i64::try_from(MAX_GENERATION_SYSTEM_JSON_BYTES + 1).expect("test bound fits i64")],
        )
        .expect("store oversized blob");
    drop(store);
    let reopened = ArtifactStateStore::open_existing_read_only(&path).expect("cold reopen");
    assert!(matches!(
        reopened.generation_system_foundation_v1(system.generation_system_id()),
        Err(StoreError::CorruptRecord)
    ));
}

#[test]
fn missing_recursive_dependency_is_cold_corruption() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("orphan.db");
    let fixture = system_fixture();
    let system = GenerationSystemRecordV1::new(fixture.relations(), fixture.input("target"))
        .expect("system");
    let mut store = ArtifactStateStore::open(&path).expect("open store");
    persist_dependencies(&mut store, &fixture);
    store
        .transact_generation_system_foundation_v1(input(&fixture, &system), |_| Ok::<_, ()>(()))
        .expect("store foundation");
    drop(store);
    let connection = Connection::open(&path).expect("open corruption fixture");
    connection
        .pragma_update(None, "foreign_keys", false)
        .expect("disable foreign keys");
    connection
        .execute(
            "DELETE FROM runtime_build_identities WHERE runtime_build_id = ?1",
            [fixture.runtime_build.runtime_build_id().digest().as_str()],
        )
        .expect("remove dependency");
    drop(connection);
    let reopened = ArtifactStateStore::open_existing_read_only(&path).expect("cold reopen");
    assert!(matches!(
        reopened.generation_system_foundation_v1(system.generation_system_id()),
        Err(StoreError::CorruptRecord)
    ));
}

#[test]
fn concurrent_exact_writers_converge_without_duplicate_rows() {
    let directory = tempdir().expect("temporary directory");
    let path = Arc::new(directory.path().join("concurrent.db"));
    let fixture = system_fixture();
    let mut initial = ArtifactStateStore::open(path.as_ref()).expect("open initial store");
    persist_dependencies(&mut initial, &fixture);
    drop(initial);
    let mut workers = Vec::new();
    for _ in 0..2 {
        let path = Arc::clone(&path);
        workers.push(thread::spawn(move || {
            let fixture = system_fixture();
            let system =
                GenerationSystemRecordV1::new(fixture.relations(), fixture.input("target"))
                    .expect("system");
            let mut store = ArtifactStateStore::open_existing_writable_exact(path.as_ref())
                .expect("open writer");
            store
                .transact_generation_system_foundation_v1(input(&fixture, &system), |_| {
                    Ok::<_, ()>(())
                })
                .expect("concurrent transaction")
                .1
        }));
    }
    let results = workers
        .into_iter()
        .map(|worker| worker.join().expect("join writer"))
        .collect::<Vec<_>>();
    assert!(results.iter().any(|result| {
        result.generation_system == WriteDisposition::Inserted
            && result.effective_package_evidence_v2 == WriteDisposition::Inserted
    }));
    assert!(results.iter().any(|result| {
        result.generation_system == WriteDisposition::AlreadyPresent
            && result.effective_package_evidence_v2 == WriteDisposition::AlreadyPresent
    }));
    let store = ArtifactStateStore::open_existing_read_only(path.as_ref()).expect("reopen store");
    assert_eq!(row_counts(store.connection()), (1, 1));
}

fn row_counts(connection: &Connection) -> (i64, i64) {
    let evidence = connection
        .query_row(
            "SELECT COUNT(*) FROM effective_package_evidence_v2",
            [],
            |row| row.get(0),
        )
        .expect("count evidence");
    let systems = connection
        .query_row(
            "SELECT COUNT(*) FROM generation_system_records",
            [],
            |row| row.get(0),
        )
        .expect("count systems");
    (evidence, systems)
}
