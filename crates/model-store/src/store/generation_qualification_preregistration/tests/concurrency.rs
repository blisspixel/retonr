use std::{sync::mpsc, thread, time::Duration};

use tempfile::tempdir;

use super::{input, read_input, support};
use crate::{
    ArtifactStateStore, GenerationQualificationPreregistrationTransactionError,
    GenerationQualificationPreregistrationV1Input, StoreError, WriteDisposition,
};

#[test]
fn concurrent_exact_writers_resolve_to_inserted_then_already_present() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("concurrent-exact.db");
    let fixture = support::fixture();
    let mut initial = ArtifactStateStore::open(&path).expect("initialize store");
    support::persist_plan_foundation(&mut initial, &fixture);
    drop(initial);
    let mut first_store = ArtifactStateStore::open(&path).expect("open first writer");
    let mut second_store = ArtifactStateStore::open(&path).expect("open second writer");
    let (staged_tx, staged_rx) = mpsc::sync_channel(0);
    let (release_tx, release_rx) = mpsc::sync_channel(0);
    let first = thread::spawn(move || {
        let fixture = support::fixture();
        first_store.transact_generation_qualification_preregistration(
            input(&fixture),
            || Ok::<_, ()>(()),
            |_| {
                staged_tx.send(()).expect("signal staged writer");
                release_rx.recv().expect("release staged writer");
                Ok::<_, ()>(())
            },
        )
    });
    staged_rx.recv().expect("wait for staged writer");
    let (prelock_tx, prelock_rx) = mpsc::sync_channel(0);
    let second = thread::spawn(move || {
        let fixture = support::fixture();
        let calls = std::cell::Cell::new(0);
        second_store.transact_generation_qualification_preregistration(
            input(&fixture),
            || {
                calls.set(calls.get() + 1);
                if calls.get() == 1 {
                    prelock_tx.send(()).expect("signal pre-lock gate");
                }
                Ok::<_, ()>(())
            },
            |_| Ok::<_, ()>(()),
        )
    });
    prelock_rx.recv().expect("wait for competing writer");
    thread::sleep(Duration::from_millis(50));
    release_tx.send(()).expect("release first writer");
    let ((), first_disposition) = first
        .join()
        .expect("join first writer")
        .expect("commit first writer");
    let ((), second_disposition) = second
        .join()
        .expect("join second writer")
        .expect("commit exact second writer");
    assert_eq!(
        first_disposition.operation_policy,
        WriteDisposition::Inserted
    );
    assert_eq!(
        first_disposition.request_projection,
        WriteDisposition::Inserted
    );
    assert_eq!(
        second_disposition.operation_policy,
        WriteDisposition::AlreadyPresent
    );
    assert_eq!(
        second_disposition.request_projection,
        WriteDisposition::AlreadyPresent
    );
}

#[test]
fn concurrent_foreign_projection_conflicts_without_partial_state() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("concurrent-conflict.db");
    let fixture = support::fixture();
    let mut initial = ArtifactStateStore::open(&path).expect("initialize store");
    support::persist_plan_foundation(&mut initial, &fixture);
    drop(initial);
    let mut first_store = ArtifactStateStore::open(&path).expect("open first writer");
    let mut second_store = ArtifactStateStore::open(&path).expect("open second writer");
    let (staged_tx, staged_rx) = mpsc::sync_channel(0);
    let (release_tx, release_rx) = mpsc::sync_channel(0);
    let first = thread::spawn(move || {
        let fixture = support::fixture();
        first_store.transact_generation_qualification_preregistration(
            input(&fixture),
            || Ok::<_, ()>(()),
            |_| {
                staged_tx.send(()).expect("signal staged writer");
                release_rx.recv().expect("release staged writer");
                Ok::<_, ()>(())
            },
        )
    });
    staged_rx.recv().expect("wait for staged writer");
    let (prelock_tx, prelock_rx) = mpsc::sync_channel(0);
    let second = thread::spawn(move || {
        let fixture = support::fixture();
        let (entry_inputs, projection) = fixture.alternate_projection();
        let calls = std::cell::Cell::new(0);
        second_store.transact_generation_qualification_preregistration(
            GenerationQualificationPreregistrationV1Input {
                operation_policy: &fixture.policy,
                operation_policy_relations: fixture.relations(),
                operation_policy_input: &fixture.policy_input,
                request_projection: &projection,
                request_projection_entry_inputs: &entry_inputs,
            },
            || {
                calls.set(calls.get() + 1);
                if calls.get() == 1 {
                    prelock_tx.send(()).expect("signal pre-lock gate");
                }
                Ok::<_, ()>(())
            },
            |_| Ok::<_, ()>(()),
        )
    });
    prelock_rx.recv().expect("wait for competing writer");
    thread::sleep(Duration::from_millis(50));
    release_tx.send(()).expect("release first writer");
    first
        .join()
        .expect("join first writer")
        .expect("commit first writer");
    assert!(matches!(
        second.join().expect("join second writer"),
        Err(
            GenerationQualificationPreregistrationTransactionError::Store(
                StoreError::ImmutableConflict
            )
        )
    ));
    let fixture = support::fixture();
    let store = ArtifactStateStore::open(&path).expect("reopen store");
    assert!(
        store
            .generation_qualification_preregistration(read_input(&fixture))
            .expect("read winning pair")
            .is_some()
    );
    let projection_count: i64 = store
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM generation_qualification_request_projections",
            [],
            |row| row.get(0),
        )
        .expect("count projections");
    assert_eq!(projection_count, 1);
}
