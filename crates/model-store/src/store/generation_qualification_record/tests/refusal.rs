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
            "UPDATE generation_qualification_records SET canonical_json = ?1",
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
fn swapped_status_column_reads_corrupt_and_does_not_replace_the_row() {
    let mut session = support::session();
    support::commit(&mut session);
    session
        .store
        .connection()
        .execute(
            "UPDATE generation_qualification_records SET status = 'qualified'",
            [],
        )
        .expect("swap status");
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
            "SELECT status FROM generation_qualification_records",
            [],
            |row| row.get(0),
        )
        .expect("stored status");
    assert_eq!(stored, "qualified");
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
            "UPDATE generation_qualification_records SET generation_qualification_id = ?1",
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
            "SELECT generation_qualification_id FROM generation_qualification_records",
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
        .transact_generation_qualification_record_v1(
            super::super::GenerationQualificationRecordV1Input {
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
        "GenerationQualificationRecordV1TransactionError::Gate"
    );
    assert_eq!(
        error.to_string(),
        "generation qualification record gate rejected the commit"
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
