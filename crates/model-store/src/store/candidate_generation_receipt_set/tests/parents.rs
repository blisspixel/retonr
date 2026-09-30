use rusqlite::params;

use super::support;
use crate::StoreError;

#[test]
fn missing_foundation_is_a_missing_record() {
    let mut session = support::bare();
    support::expect_store(
        &mut session.store,
        &session.record,
        &StoreError::MissingRecord,
    );
    support::expect_read(&session.store, &session.record, &StoreError::MissingRecord);
    assert_eq!(support::count(&session.store, support::TABLE), 0);
}

#[test]
fn a_plan_without_execution_is_a_missing_record() {
    let mut session = support::foundation();
    support::expect_store(
        &mut session.store,
        &session.record,
        &StoreError::MissingRecord,
    );
    support::expect_read(&session.store, &session.record, &StoreError::MissingRecord);
    assert_eq!(support::count(&session.store, support::TABLE), 0);
}

#[test]
fn a_deleted_plan_system_pair_writes_nothing() {
    let mut session = support::session();
    let ids = support::parent_ids(&session);
    session
        .store
        .connection()
        .execute(
            "DELETE FROM generation_qualification_plan_systems
             WHERE generation_qualification_plan_id = ?1 AND generation_system_id = ?2",
            params![ids.plan, ids.system],
        )
        .expect("delete plan system");
    support::expect_store(
        &mut session.store,
        &session.record,
        &StoreError::CorruptRecord,
    );
    support::expect_read(&session.store, &session.record, &StoreError::CorruptRecord);
    assert_eq!(support::count(&session.store, support::TABLE), 0);
}

#[test]
fn a_failed_attempt_writes_nothing() {
    let mut session = support::session();
    let ids = support::parent_ids(&session);
    session
        .store
        .connection()
        .execute(
            "UPDATE candidate_generation_attempt_records
             SET outcome = 'failed', candidate_generation_receipt_id = NULL
             WHERE candidate_generation_attempt_record_id = ?1",
            [ids.attempt_record],
        )
        .expect("mark attempt failed");
    support::expect_store(
        &mut session.store,
        &session.record,
        &StoreError::CorruptRecord,
    );
    support::expect_read(&session.store, &session.record, &StoreError::CorruptRecord);
    assert_eq!(support::count(&session.store, support::TABLE), 0);
}

#[test]
fn a_case_removed_from_its_suite_writes_nothing() {
    let mut session = support::session();
    let ids = support::parent_ids(&session);
    session
        .store
        .connection()
        .execute(
            "DELETE FROM generation_suite_cases WHERE generation_suite_manifest_id = ?1",
            [ids.suite],
        )
        .expect("delete suite cases");
    support::expect_store(
        &mut session.store,
        &session.record,
        &StoreError::CorruptRecord,
    );
    support::expect_read(&session.store, &session.record, &StoreError::CorruptRecord);
    assert_eq!(support::count(&session.store, support::TABLE), 0);
}
