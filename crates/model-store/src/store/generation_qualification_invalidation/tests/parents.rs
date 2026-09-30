use super::support;
use crate::StoreError;

#[test]
fn missing_qualification_is_a_missing_record() {
    let mut session = support::without_qualification();
    support::expect_store(
        &mut session.parents.store,
        &session.invalidation,
        &StoreError::MissingRecord,
    );
    support::expect_read(
        &session.parents.store,
        &session.invalidation,
        &StoreError::MissingRecord,
    );
    assert_eq!(support::count(&session.parents.store, support::TABLE), 0);
    assert_eq!(
        support::count(&session.parents.store, "generation_qualification_records"),
        0
    );
}
