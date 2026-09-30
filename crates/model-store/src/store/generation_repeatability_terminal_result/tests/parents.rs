use super::support;
use crate::StoreError;

#[test]
fn missing_ledger_is_a_missing_record() {
    let mut session = support::without_ledger();
    support::expect_store(
        &mut session.store,
        &session.record,
        &StoreError::MissingRecord,
    );
    support::expect_read(&session.store, &session.record, &StoreError::MissingRecord);
    assert_eq!(support::count(&session.store, support::TABLE), 0);
}

#[test]
fn missing_evaluation_is_a_missing_record() {
    let mut session = support::session();
    let evaluation = session
        .record
        .candidate_deterministic_evaluation_id()
        .expect("evaluation")
        .digest()
        .as_str()
        .to_owned();
    session
        .store
        .connection()
        .execute(
            "DELETE FROM candidate_deterministic_evaluation_records
             WHERE candidate_deterministic_evaluation_id = ?1",
            [evaluation.as_str()],
        )
        .expect("delete evaluation");
    support::expect_store(
        &mut session.store,
        &session.record,
        &StoreError::MissingRecord,
    );
    support::expect_read(&session.store, &session.record, &StoreError::MissingRecord);
    assert_eq!(support::count(&session.store, support::TABLE), 0);
}
