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
fn one_receipt_set_is_a_missing_record() {
    let mut session = support::one_receipt_set();
    support::expect_store(
        &mut session.store,
        &session.record,
        &StoreError::MissingRecord,
    );
    support::expect_read(&session.store, &session.record, &StoreError::MissingRecord);
    assert_eq!(support::count(&session.store, support::TABLE), 0);
}

#[test]
fn a_changed_receipt_set_entry_count_writes_nothing() {
    let mut session = support::session();
    let ids = support::parent_ids(&session);
    session
        .store
        .connection()
        .execute(
            "UPDATE candidate_generation_receipt_sets SET entry_count = 2
             WHERE candidate_generation_receipt_set_id = ?1",
            params![ids.receipt_a],
        )
        .expect("tamper entry count");
    support::expect_store(
        &mut session.store,
        &session.record,
        &StoreError::CorruptRecord,
    );
    support::expect_read(&session.store, &session.record, &StoreError::CorruptRecord);
    assert_eq!(support::count(&session.store, support::TABLE), 0);
}
