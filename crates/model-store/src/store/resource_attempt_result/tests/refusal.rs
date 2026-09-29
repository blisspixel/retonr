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
            "UPDATE generation_resource_attempt_result_records SET canonical_json = ?1",
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
fn substituted_identity_reads_corrupt() {
    let mut session = support::session();
    support::commit(&mut session);
    let other = "b".repeat(64);
    session
        .store
        .connection()
        .execute(
            "UPDATE generation_resource_attempt_result_records
             SET generation_resource_attempt_result_id = ?1",
            [&other],
        )
        .expect("tamper identity");
    support::expect_read(&session.store, &session.record, &StoreError::CorruptRecord);
}

#[test]
fn an_impostor_row_on_the_same_attempt_is_corrupt() {
    let mut session = support::session();
    let ids = support::parent_ids(&session);
    let impostor = "e".repeat(64);
    session
        .store
        .connection()
        .execute(
            "INSERT INTO generation_resource_attempt_result_records (
                generation_resource_attempt_result_id, schema_version, generation_system_id,
                generation_qualification_plan_id, generation_suite_manifest_id,
                generation_case_id, generation_repetition_id, planned_candidate_attempt_id,
                candidate_generation_attempt_record_id, candidate_generation_receipt_id,
                resource_policy_digest, observation_profile, canonical_json
             ) VALUES (?1, 1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                       'managed_ollama_v0_32_15_linux_v1', x'7b7d')",
            params![
                impostor,
                ids.system,
                ids.plan,
                ids.suite,
                ids.case_id,
                ids.repetition,
                ids.planned,
                ids.attempt_record,
                ids.receipt,
                ids.policy,
            ],
        )
        .expect("impostor row");
    support::expect_read(&session.store, &session.record, &StoreError::CorruptRecord);
    support::expect_store(
        &mut session.store,
        &session.record,
        &StoreError::CorruptRecord,
    );
    assert_eq!(support::count(&session.store, support::TABLE), 1);
    assert_eq!(support::canonical_json(&session), b"{}");
}

#[test]
fn third_gate_failure_hides_the_secret_and_writes_nothing() {
    let mut session = support::session();
    let mut calls = 0;
    let error = session
        .store
        .transact_generation_resource_attempt_result_v1(
            super::super::GenerationResourceAttemptResultV1Input {
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
        "GenerationResourceAttemptResultV1TransactionError::Gate"
    );
    assert_eq!(
        error.to_string(),
        "resource attempt result gate rejected the commit"
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
