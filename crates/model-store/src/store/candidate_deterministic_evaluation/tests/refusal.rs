use rusqlite::params;

use super::support;
use crate::StoreError;

#[test]
fn tampered_json_does_not_replace_bytes_and_reads_corrupt() {
    let mut session = support::session();
    support::commit(&mut session);
    let replacement = br#"{"schema_version":1}"#;
    session
        .store
        .connection()
        .execute(
            "UPDATE candidate_deterministic_evaluation_records SET canonical_json = ?1",
            [replacement.as_slice()],
        )
        .expect("tamper json");
    support::expect_store(
        &mut session.store,
        &session.record,
        &StoreError::ImmutableConflict,
    );
    assert_eq!(support::canonical_json(&session), replacement);
    support::expect_read(&session.store, &session.record, &StoreError::CorruptRecord);
}

#[test]
fn swapped_receipt_set_columns_read_corrupt() {
    let mut session = support::session();
    support::commit(&mut session);
    let ids = support::parent_ids(&session);
    session
        .store
        .connection()
        .execute(
            "UPDATE candidate_deterministic_evaluation_records
             SET candidate_a_receipt_set_id = ?1, candidate_b_receipt_set_id = ?2",
            params![ids.receipt_b, ids.receipt_a],
        )
        .expect("swap receipt sets");
    support::expect_read(&session.store, &session.record, &StoreError::CorruptRecord);
    support::expect_store(
        &mut session.store,
        &session.record,
        &StoreError::ImmutableConflict,
    );
    let stored_a: String = session
        .store
        .connection()
        .query_row(
            "SELECT candidate_a_receipt_set_id FROM candidate_deterministic_evaluation_records",
            [],
            |row| row.get(0),
        )
        .expect("stored receipt set");
    assert_eq!(stored_a, ids.receipt_b);
}

#[test]
fn substituted_identity_is_corrupt_and_is_not_rewritten() {
    let mut session = support::session();
    support::commit(&mut session);
    let impostor = "b".repeat(64);
    session
        .store
        .connection()
        .execute(
            "UPDATE candidate_deterministic_evaluation_records
             SET candidate_deterministic_evaluation_id = ?1",
            [&impostor],
        )
        .expect("tamper identity");
    support::expect_read(&session.store, &session.record, &StoreError::CorruptRecord);
    support::expect_store(
        &mut session.store,
        &session.record,
        &StoreError::CorruptRecord,
    );
    assert_eq!(support::count(&session.store, support::TABLE), 1);
    let stored: String = session
        .store
        .connection()
        .query_row(
            "SELECT candidate_deterministic_evaluation_id
             FROM candidate_deterministic_evaluation_records",
            [],
            |row| row.get(0),
        )
        .expect("stored identity");
    assert_eq!(stored, impostor);
}

#[test]
fn third_gate_failure_hides_the_secret_and_writes_nothing() {
    let mut session = support::session();
    let mut calls = 0;
    let error = session
        .store
        .transact_candidate_deterministic_evaluation_v1(
            super::super::CandidateDeterministicEvaluationV1Input {
                record: &session.record,
            },
            || {
                calls += 1;
                if calls == 3 { Err(Secret) } else { Ok(()) }
            },
        )
        .expect_err("gate");
    assert_eq!(calls, 3);
    assert_eq!(
        format!("{error:?}"),
        "CandidateDeterministicEvaluationV1TransactionError::Gate"
    );
    assert_eq!(
        error.to_string(),
        "candidate deterministic evaluation gate rejected the commit"
    );
    assert!(!format!("{error:?}").contains("secret-token"));
    assert!(!error.to_string().contains("secret-token"));
    assert_eq!(support::count(&session.store, support::TABLE), 0);
}

struct Secret;

impl std::fmt::Debug for Secret {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("secret-token")
    }
}

impl std::fmt::Display for Secret {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("secret-token")
    }
}
