use super::support;
use crate::StoreError;

#[test]
fn missing_cohort_is_a_missing_record() {
    let mut session = support::without_cohort();
    support::expect_store(
        &mut session.store,
        &session.record,
        &StoreError::MissingRecord,
    );
    support::expect_read(&session.store, &session.record, &StoreError::MissingRecord);
    assert_eq!(support::count(&session.store, support::TABLE), 0);
}

#[test]
fn missing_receipt_is_a_missing_record() {
    let mut session = support::without_receipt();
    support::expect_store(
        &mut session.store,
        &session.record,
        &StoreError::MissingRecord,
    );
    support::expect_read(&session.store, &session.record, &StoreError::MissingRecord);
    assert_eq!(support::count(&session.store, support::TABLE), 0);
}
