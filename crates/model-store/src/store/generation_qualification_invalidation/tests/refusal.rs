use super::support;
use crate::StoreError;

#[test]
fn tampered_json_does_not_replace_bytes_and_reads_corrupt() {
    let mut session = support::session();
    support::commit(&mut session);
    let replacement = br#"{"schema_version":1}"#;
    session
        .parents
        .store
        .connection()
        .execute(
            "UPDATE generation_qualification_invalidations SET canonical_json = ?1",
            [replacement.as_slice()],
        )
        .expect("tamper json");
    support::expect_store(
        &mut session.parents.store,
        &session.invalidation,
        &StoreError::ImmutableConflict,
    );
    assert_eq!(support::canonical_json(&session), replacement);
    support::expect_read(
        &session.parents.store,
        &session.invalidation,
        &StoreError::CorruptRecord,
    );
    assert_eq!(support::qualification_status(&session), "rejected");
}

#[test]
fn substituted_identity_is_corrupt_and_is_not_rewritten() {
    let mut session = support::session();
    support::commit(&mut session);
    let impostor = "b".repeat(64);
    session
        .parents
        .store
        .connection()
        .execute(
            "UPDATE generation_qualification_invalidations
             SET generation_qualification_invalidation_id = ?1",
            [&impostor],
        )
        .expect("tamper identity");
    support::expect_read(
        &session.parents.store,
        &session.invalidation,
        &StoreError::CorruptRecord,
    );
    support::expect_store(
        &mut session.parents.store,
        &session.invalidation,
        &StoreError::CorruptRecord,
    );
    assert_eq!(support::count(&session.parents.store, support::TABLE), 1);
    let stored: String = session
        .parents
        .store
        .connection()
        .query_row(
            "SELECT generation_qualification_invalidation_id
             FROM generation_qualification_invalidations",
            [],
            |row| row.get(0),
        )
        .expect("stored identity");
    assert_eq!(stored, impostor);
    assert_eq!(support::qualification_status(&session), "rejected");
}

#[test]
fn third_gate_failure_hides_the_secret_and_writes_nothing() {
    let mut session = support::session();
    let qualification_json = support::qualification_json(&session);
    let mut calls = 0;
    let error = session
        .parents
        .store
        .transact_generation_qualification_invalidation_v1(
            super::super::GenerationQualificationInvalidationV1Input {
                invalidation: &session.invalidation,
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
        "GenerationQualificationInvalidationV1TransactionError::Gate"
    );
    assert_eq!(
        error.to_string(),
        "generation qualification invalidation gate rejected the commit"
    );
    assert!(!format!("{error:?}").contains("secret-token"));
    assert!(!error.to_string().contains("secret-token"));
    assert_eq!(support::count(&session.parents.store, support::TABLE), 0);
    assert_eq!(support::qualification_json(&session), qualification_json);
    assert_eq!(support::qualification_status(&session), "rejected");
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
