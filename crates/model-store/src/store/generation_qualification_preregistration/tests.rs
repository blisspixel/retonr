use std::cell::Cell;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::{thread, time::Duration};

use rusqlite::params;
use tempfile::tempdir;

use super::*;
use crate::ArtifactStateStore;

#[path = "tests/concurrency.rs"]
mod concurrency;
#[path = "tests/corruption.rs"]
mod corruption;
#[path = "tests/support.rs"]
pub(crate) mod support;
#[path = "tests/transaction.rs"]
mod transaction;

fn input(fixture: &support::Fixture) -> GenerationQualificationPreregistrationV1Input<'_> {
    GenerationQualificationPreregistrationV1Input {
        operation_policy: &fixture.policy,
        operation_policy_relations: fixture.relations(),
        operation_policy_input: &fixture.policy_input,
        request_projection: &fixture.projection,
        request_projection_entry_inputs: &fixture.entry_inputs,
    }
}

fn read_input(fixture: &support::Fixture) -> GenerationQualificationPreregistrationReadInput<'_> {
    GenerationQualificationPreregistrationReadInput {
        operation_policy_id: fixture.policy.operation_policy_id(),
        request_projection_id: fixture.projection.request_projection_id(),
        operation_policy_relations: fixture.relations(),
        operation_policy_input: &fixture.policy_input,
        request_projection_entry_inputs: &fixture.entry_inputs,
    }
}

#[test]
fn atomic_pair_round_trips_and_repeats_idempotently() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("preregistration.db");
    let fixture = support::fixture();
    let mut store = ArtifactStateStore::open(&path).expect("open store");
    support::persist_plan_foundation(&mut store, &fixture);
    let gates = Cell::new(0);
    let (validated, first) = store
        .transact_generation_qualification_preregistration(
            input(&fixture),
            || {
                gates.set(gates.get() + 1);
                Ok::<_, ()>(())
            },
            |readback| {
                assert_eq!(readback.plan_foundation().plan(), &fixture.plan);
                assert_eq!(readback.operation_policy(), &fixture.policy);
                assert_eq!(readback.request_projection(), &fixture.projection);
                Ok::<_, ()>("validated")
            },
        )
        .expect("insert pair");
    assert_eq!(validated, "validated");
    assert_eq!(gates.get(), 3);
    assert_eq!(first.operation_policy, WriteDisposition::Inserted);
    assert_eq!(first.request_projection, WriteDisposition::Inserted);

    let ((), repeated) = store
        .transact_generation_qualification_preregistration(
            input(&fixture),
            || Ok::<_, ()>(()),
            |_| Ok::<_, ()>(()),
        )
        .expect("repeat exact pair");
    assert_eq!(repeated.operation_policy, WriteDisposition::AlreadyPresent);
    assert_eq!(
        repeated.request_projection,
        WriteDisposition::AlreadyPresent
    );
    let stored = store
        .generation_qualification_preregistration(read_input(&fixture))
        .expect("read pair")
        .expect("pair present");
    assert_eq!(stored.operation_policy(), &fixture.policy);
    assert_eq!(stored.request_projection(), &fixture.projection);
}

#[test]
fn missing_foundation_blocks_preregistration_until_exact_materialization() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("missing-foundation.db");
    let fixture = support::fixture();
    let mut store = ArtifactStateStore::open(&path).expect("open store");
    let callback_called = Cell::new(false);

    let missing = store.transact_generation_qualification_preregistration(
        input(&fixture),
        || Ok::<_, ()>(()),
        |_| {
            callback_called.set(true);
            Ok::<_, ()>(())
        },
    );
    assert!(matches!(
        missing,
        Err(
            GenerationQualificationPreregistrationTransactionError::Store(
                StoreError::MissingRecord
            )
        )
    ));
    assert!(!callback_called.get());
    assert_eq!(preregistration_row_count(store.connection()), 0);

    support::persist_plan_foundation(&mut store, &fixture);
    let ((), disposition) = store
        .transact_generation_qualification_preregistration(
            input(&fixture),
            || Ok::<_, ()>(()),
            |readback| {
                assert_eq!(readback.plan_foundation().plan(), &fixture.plan);
                Ok::<_, ()>(())
            },
        )
        .expect("preregister after foundation materialization");
    assert_eq!(disposition.operation_policy, WriteDisposition::Inserted);
    assert_eq!(disposition.request_projection, WriteDisposition::Inserted);
    assert_eq!(preregistration_row_count(store.connection()), 2);
}

#[test]
fn retry_and_cold_read_reject_a_corrupt_foundation_without_healing() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("corrupt-foundation.db");
    let fixture = support::fixture();
    let mut store = ArtifactStateStore::open(&path).expect("open store");
    support::persist_plan_foundation(&mut store, &fixture);
    store
        .transact_generation_qualification_preregistration(
            input(&fixture),
            || Ok::<_, ()>(()),
            |_| Ok::<_, ()>(()),
        )
        .expect("initial preregistration");
    store
        .connection()
        .pragma_update(None, "foreign_keys", false)
        .expect("disable foreign keys for corruption fixture");
    store
        .connection()
        .execute(
            "DELETE FROM generation_qualification_plan_attempts WHERE plan_ordinal = 1",
            [],
        )
        .expect("remove plan association");
    let callback_called = Cell::new(false);

    let retry = store.transact_generation_qualification_preregistration(
        input(&fixture),
        || Ok::<_, ()>(()),
        |_| {
            callback_called.set(true);
            Ok::<_, ()>(())
        },
    );
    assert!(matches!(
        retry,
        Err(
            GenerationQualificationPreregistrationTransactionError::Store(
                StoreError::CorruptRecord
            )
        )
    ));
    assert!(!callback_called.get());
    assert!(matches!(
        store.generation_qualification_preregistration(read_input(&fixture)),
        Err(StoreError::CorruptRecord)
    ));
    assert_eq!(preregistration_row_count(store.connection()), 2);
}

fn preregistration_row_count(connection: &rusqlite::Connection) -> i64 {
    connection
        .query_row(
            "SELECT
                 (SELECT COUNT(*) FROM generation_qualification_operation_policies) +
                 (SELECT COUNT(*) FROM generation_qualification_request_projections)",
            [],
            |row| row.get(0),
        )
        .expect("count preregistration rows")
}

#[test]
fn every_rejection_point_rolls_back_staged_records() {
    for rejected_gate in [1, 2, 3] {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join(format!("gate-{rejected_gate}.db"));
        let fixture = support::fixture();
        let mut store = ArtifactStateStore::open(&path).expect("open store");
        support::persist_plan_foundation(&mut store, &fixture);
        let calls = Cell::new(0);
        let result = store.transact_generation_qualification_preregistration(
            input(&fixture),
            || {
                calls.set(calls.get() + 1);
                if calls.get() == rejected_gate {
                    Err("gate")
                } else {
                    Ok(())
                }
            },
            |_| Ok::<_, &str>(()),
        );
        assert!(matches!(
            result,
            Err(GenerationQualificationPreregistrationTransactionError::Gate("gate"))
        ));
        assert!(
            store
                .generation_qualification_preregistration(read_input(&fixture))
                .expect("read after gate rollback")
                .is_none()
        );
    }

    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("validation.db");
    let fixture = support::fixture();
    let mut store = ArtifactStateStore::open(&path).expect("open store");
    support::persist_plan_foundation(&mut store, &fixture);
    let result = store.transact_generation_qualification_preregistration(
        input(&fixture),
        || Ok::<_, &str>(()),
        |_| Err::<(), _>("validation"),
    );
    assert!(matches!(
        result,
        Err(GenerationQualificationPreregistrationTransactionError::Validation("validation"))
    ));
    assert!(
        store
            .generation_qualification_preregistration(read_input(&fixture))
            .expect("read after validation rollback")
            .is_none()
    );
}

#[test]
fn partial_policy_is_completed_but_collisions_fail_closed() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("partial.db");
    let fixture = support::fixture();
    let mut store = ArtifactStateStore::open(&path).expect("open store");
    support::persist_plan_foundation(&mut store, &fixture);
    let policy_json = serde_json::to_vec(&fixture.policy).expect("policy JSON");
    store
        .connection()
        .execute(
            "INSERT INTO generation_qualification_operation_policies
                 (operation_policy_id, generation_qualification_plan_id, suite_manifest_id,
                  target_generation_system_id, baseline_generation_system_id, canonical_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                fixture.policy.operation_policy_id().digest().as_str(),
                fixture
                    .policy
                    .generation_qualification_plan_id()
                    .digest()
                    .as_str(),
                fixture.policy.suite_manifest_id().digest().as_str(),
                fixture
                    .policy
                    .target_generation_system_id()
                    .digest()
                    .as_str(),
                fixture
                    .policy
                    .baseline_generation_system_id()
                    .digest()
                    .as_str(),
                policy_json,
            ],
        )
        .expect("seed exact policy");
    let ((), disposition) = store
        .transact_generation_qualification_preregistration(
            input(&fixture),
            || Ok::<_, ()>(()),
            |_| Ok::<_, ()>(()),
        )
        .expect("complete partial pair");
    assert_eq!(
        disposition,
        GenerationQualificationPreregistrationWriteDisposition {
            operation_policy: WriteDisposition::AlreadyPresent,
            request_projection: WriteDisposition::Inserted,
        }
    );

    store
        .connection()
        .execute(
            "UPDATE generation_qualification_operation_policies
             SET canonical_json = X'7B7D' WHERE operation_policy_id = ?1",
            [fixture.policy.operation_policy_id().digest().as_str()],
        )
        .expect("corrupt policy bytes");
    let result = store.transact_generation_qualification_preregistration(
        input(&fixture),
        || Ok::<_, ()>(()),
        |_| Ok::<_, ()>(()),
    );
    assert!(matches!(
        result,
        Err(
            GenerationQualificationPreregistrationTransactionError::Store(
                StoreError::ImmutableConflict
            )
        )
    ));
}

#[test]
fn alternate_projection_for_the_same_policy_is_an_immutable_conflict() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("projection-conflict.db");
    let fixture = support::fixture();
    let mut store = ArtifactStateStore::open(&path).expect("open store");
    support::persist_plan_foundation(&mut store, &fixture);
    let policy_json = serde_json::to_vec(&fixture.policy).expect("policy JSON");
    store
        .connection()
        .execute(
            "INSERT INTO generation_qualification_operation_policies VALUES
                 (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                fixture.policy.operation_policy_id().digest().as_str(),
                fixture
                    .policy
                    .generation_qualification_plan_id()
                    .digest()
                    .as_str(),
                fixture.policy.suite_manifest_id().digest().as_str(),
                fixture
                    .policy
                    .target_generation_system_id()
                    .digest()
                    .as_str(),
                fixture
                    .policy
                    .baseline_generation_system_id()
                    .digest()
                    .as_str(),
                policy_json,
            ],
        )
        .expect("seed policy");
    store
        .connection()
        .execute(
            "INSERT INTO generation_qualification_request_projections VALUES
                 (?1, ?2, ?3, ?4, ?5, ?6, ?7, X'7B7D')",
            params![
                "f".repeat(64),
                fixture.policy.operation_policy_id().digest().as_str(),
                fixture
                    .policy
                    .generation_qualification_plan_id()
                    .digest()
                    .as_str(),
                fixture.policy.suite_manifest_id().digest().as_str(),
                fixture
                    .policy
                    .target_generation_system_id()
                    .digest()
                    .as_str(),
                fixture
                    .policy
                    .baseline_generation_system_id()
                    .digest()
                    .as_str(),
                i64::try_from(fixture.projection.entry_count()).expect("entry count"),
            ],
        )
        .expect("seed alternate projection owner");
    let result = store.transact_generation_qualification_preregistration(
        input(&fixture),
        || Ok::<_, ()>(()),
        |_| Ok::<_, ()>(()),
    );
    assert!(matches!(
        result,
        Err(
            GenerationQualificationPreregistrationTransactionError::Store(
                StoreError::ImmutableConflict
            )
        )
    ));
}

#[test]
fn gate_rechecks_after_waiting_for_the_immediate_writer_lock() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("lock-wait.db");
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
        first_store
            .transact_generation_qualification_preregistration(
                input(&fixture),
                || Ok::<_, ()>(()),
                |_| {
                    staged_tx.send(()).expect("signal staged writer");
                    release_rx.recv().expect("release staged writer");
                    Ok::<_, ()>(())
                },
            )
            .expect("commit first writer");
    });
    staged_rx.recv().expect("wait for staged writer");

    let expired = Arc::new(AtomicBool::new(false));
    let second_expired = Arc::clone(&expired);
    let (prelock_tx, prelock_rx) = mpsc::sync_channel(0);
    let second = thread::spawn(move || {
        let fixture = support::fixture();
        let calls = Cell::new(0);
        second_store.transact_generation_qualification_preregistration(
            input(&fixture),
            || {
                calls.set(calls.get() + 1);
                if calls.get() == 1 {
                    prelock_tx.send(()).expect("signal pre-lock gate");
                }
                if second_expired.load(Ordering::SeqCst) {
                    Err("expired")
                } else {
                    Ok(())
                }
            },
            |_| Ok::<_, &str>(()),
        )
    });
    prelock_rx.recv().expect("wait for pre-lock gate");
    thread::sleep(Duration::from_millis(50));
    expired.store(true, Ordering::SeqCst);
    release_tx.send(()).expect("release first writer");
    first.join().expect("join first writer");
    assert!(matches!(
        second.join().expect("join second writer"),
        Err(GenerationQualificationPreregistrationTransactionError::Gate("expired"))
    ));

    let fixture = support::fixture();
    let store = ArtifactStateStore::open(&path).expect("reopen committed store");
    assert!(
        store
            .generation_qualification_preregistration(read_input(&fixture))
            .expect("read first writer pair")
            .is_some()
    );
}

#[test]
fn bounded_read_rejects_oversize_length() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("corrupt.db");
    let fixture = support::fixture();
    let mut store = ArtifactStateStore::open(&path).expect("open store");
    support::persist_plan_foundation(&mut store, &fixture);
    store
        .transact_generation_qualification_preregistration(
            input(&fixture),
            || Ok::<_, ()>(()),
            |_| Ok::<_, ()>(()),
        )
        .expect("insert pair");
    store
        .connection()
        .pragma_update(None, "ignore_check_constraints", true)
        .expect("allow corruption fixture");
    store
        .connection()
        .execute(
            "UPDATE generation_qualification_request_projections
             SET canonical_json = zeroblob(4194305) WHERE request_projection_id = ?1",
            [fixture.projection.request_projection_id().digest().as_str()],
        )
        .expect("store oversized corrupt blob");
    assert!(matches!(
        store.generation_qualification_preregistration(read_input(&fixture)),
        Err(StoreError::CorruptRecord)
    ));
}

#[test]
fn transaction_errors_redact_caller_values() {
    let error = GenerationQualificationPreregistrationTransactionError::Gate("secret");
    assert!(!format!("{error:?}").contains("secret"));
    assert!(!error.to_string().contains("secret"));
}
