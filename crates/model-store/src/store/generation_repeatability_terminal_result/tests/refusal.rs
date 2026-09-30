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
            "UPDATE generation_repeatability_terminal_result_records SET canonical_json = ?1",
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
fn swapped_stage_column_reads_corrupt_and_does_not_replace_the_row() {
    let mut session = support::session();
    support::commit(&mut session);
    session
        .store
        .connection()
        .execute(
            "UPDATE generation_repeatability_terminal_result_records
             SET terminal_stage = 'judge_failed'",
            [],
        )
        .expect("swap stage");
    support::expect_read(&session.store, &session.record, &StoreError::CorruptRecord);
    support::expect_store(
        &mut session.store,
        &session.record,
        &StoreError::ImmutableConflict,
    );
    let stored: String = session
        .store
        .connection()
        .query_row(
            "SELECT terminal_stage FROM generation_repeatability_terminal_result_records",
            [],
            |row| row.get(0),
        )
        .expect("stored stage");
    assert_eq!(stored, "judge_failed");
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
            "UPDATE generation_repeatability_terminal_result_records
             SET generation_repeatability_result_id = ?1",
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
            "SELECT generation_repeatability_result_id
             FROM generation_repeatability_terminal_result_records",
            [],
            |row| row.get(0),
        )
        .expect("stored identity");
    assert_eq!(stored, impostor);
}

#[test]
fn a_different_stage_for_the_same_repetition_is_corrupt() {
    let mut session = support::session();
    support::commit(&mut session);
    support::expect_store(
        &mut session.store,
        &session.judge_failed,
        &StoreError::CorruptRecord,
    );
    assert_eq!(support::count(&session.store, support::TABLE), 1);
    let stored: String = session
        .store
        .connection()
        .query_row(
            "SELECT terminal_stage FROM generation_repeatability_terminal_result_records",
            [],
            |row| row.get(0),
        )
        .expect("stored stage");
    assert_eq!(stored, "deterministic_failed");
    let stored_record = session
        .store
        .generation_repeatability_terminal_result_v1(support::read_input(&session))
        .expect("original read");
    assert_eq!(stored_record.as_ref(), Some(&session.record));
}

#[test]
fn candidate_generation_failure_is_rejected_before_storage() {
    let mut session = support::session();
    support::expect_store(
        &mut session.store,
        &session.generation_failed,
        &StoreError::CorruptRecord,
    );
    support::expect_read(
        &session.store,
        &session.generation_failed,
        &StoreError::CorruptRecord,
    );
    assert_eq!(support::count(&session.store, support::TABLE), 0);
    assert_eq!(
        support::count(&session.store, "generation_repeatability_result_records"),
        0
    );
}

#[test]
fn third_gate_failure_hides_the_secret_and_writes_nothing() {
    let mut session = support::session();
    let mut calls = 0;
    let error = session
        .store
        .transact_generation_repeatability_terminal_result_v1(
            super::super::GenerationRepeatabilityTerminalResultV1Input {
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
        "GenerationRepeatabilityTerminalResultV1TransactionError::Gate"
    );
    assert_eq!(
        error.to_string(),
        "generation repeatability terminal result gate rejected the commit"
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
