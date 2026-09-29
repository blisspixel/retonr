use rewrite_model::GenerationResourcePolicyDenialRecordV1;
use rewrite_types::Digest;
use rusqlite::params;

use super::super::{GenerationResourcePolicyDenialV1Input, PhasePolicyDenialV1TransactionError};
use super::support;
use crate::{ArtifactStateStore, StoreError};

#[test]
fn tampered_json_does_not_replace_bytes_and_reads_corrupt() {
    let mut session = support::session();
    support::commit_resource(&mut session);
    let replacement = br#"{"schema_version":1}"#;
    session
        .store
        .connection()
        .execute(
            "UPDATE generation_resource_policy_denial_records SET canonical_json = ?1",
            [replacement.as_slice()],
        )
        .expect("tamper json");
    expect_store(
        &mut session.store,
        support::resource_input(&session.prepared),
        &StoreError::ImmutableConflict,
    );
    let stored: Vec<u8> = session
        .store
        .connection()
        .query_row(
            "SELECT canonical_json FROM generation_resource_policy_denial_records",
            [],
            |row| row.get(0),
        )
        .expect("stored json");
    assert_eq!(stored, replacement);
    expect_read(
        &session.store,
        &session.prepared,
        &StoreError::CorruptRecord,
    );
}

#[test]
fn substituted_identity_reads_corrupt() {
    let mut session = support::session();
    support::commit_resource(&mut session);
    let other = "b".repeat(64);
    session
        .store
        .connection()
        .execute(
            "UPDATE generation_resource_policy_denial_records
             SET generation_resource_policy_denial_record_id = ?1",
            [&other],
        )
        .expect("tamper identity");
    expect_read(
        &session.store,
        &session.prepared,
        &StoreError::CorruptRecord,
    );
}

#[test]
fn a_second_row_for_the_same_scope_is_corrupt() {
    let mut session = support::session();
    support::commit_resource(&mut session);
    let other = "c".repeat(64);
    let system = session.prepared.fixture.systems[0]
        .generation_system_id()
        .digest()
        .as_str()
        .to_owned();
    let plan = session
        .prepared
        .fixture
        .plan
        .qualification_plan_id()
        .digest()
        .as_str()
        .to_owned();
    let suite = session
        .prepared
        .fixture
        .suite
        .suite_manifest_id()
        .digest()
        .as_str()
        .to_owned();
    let policy = session.prepared.policy.as_str().to_owned();
    session
        .store
        .connection()
        .execute(
            "INSERT INTO generation_resource_policy_denial_records (
                generation_resource_policy_denial_record_id, schema_version, record_kind,
                generation_system_id, generation_qualification_plan_id,
                generation_suite_manifest_id, phase_policy_digest, reason, canonical_json
             ) VALUES (?1, 1, 'resource_policy_denial', ?2, ?3, ?4, ?5, 'policy_source_denied', x'7b7d')",
            params![other, system, plan, suite, policy],
        )
        .expect("duplicate scope");
    expect_read(
        &session.store,
        &session.prepared,
        &StoreError::CorruptRecord,
    );
    expect_store(
        &mut session.store,
        support::resource_input(&session.prepared),
        &StoreError::CorruptRecord,
    );
    assert_eq!(support::count(&session.store, support::RESOURCE_TABLE), 2);
}

#[test]
fn mismatched_relations_write_nothing() {
    let mut session = support::session();
    let other = Digest::sha256(b"other denied phase policy");
    let record = GenerationResourcePolicyDenialRecordV1::new(support::resource_relations(
        &session.prepared.fixture,
        &other,
    ))
    .expect("other denial");
    expect_store(
        &mut session.store,
        GenerationResourcePolicyDenialV1Input {
            record: &record,
            relations: support::resource_relations(
                &session.prepared.fixture,
                &session.prepared.policy,
            ),
        },
        &StoreError::ImmutableConflict,
    );
    assert_eq!(support::count(&session.store, support::RESOURCE_TABLE), 0);
}

#[test]
fn missing_foundation_is_a_missing_record() {
    let prepared = support::prepared();
    let directory = tempfile::tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    expect_store(
        &mut store,
        support::resource_input(&prepared),
        &StoreError::MissingRecord,
    );
    expect_read(&store, &prepared, &StoreError::MissingRecord);
    assert_eq!(support::count(&store, support::RESOURCE_TABLE), 0);
}

#[test]
fn a_deleted_plan_system_pair_writes_nothing() {
    let mut session = support::session();
    let system = session.prepared.fixture.systems[0]
        .generation_system_id()
        .digest()
        .as_str()
        .to_owned();
    let plan = session
        .prepared
        .fixture
        .plan
        .qualification_plan_id()
        .digest()
        .as_str()
        .to_owned();
    session
        .store
        .connection()
        .execute(
            "DELETE FROM generation_qualification_plan_systems
             WHERE generation_qualification_plan_id = ?1 AND generation_system_id = ?2",
            params![plan, system],
        )
        .expect("delete plan system");
    expect_store(
        &mut session.store,
        support::resource_input(&session.prepared),
        &StoreError::CorruptRecord,
    );
    expect_read(
        &session.store,
        &session.prepared,
        &StoreError::CorruptRecord,
    );
    assert_eq!(support::count(&session.store, support::RESOURCE_TABLE), 0);
}

#[test]
fn a_plan_suite_mismatch_writes_nothing() {
    let mut session = support::session();
    let other = "d".repeat(64);
    session
        .store
        .connection()
        .execute(
            "INSERT INTO generation_suite_manifests (
                generation_suite_manifest_id, case_count, canonical_json
             ) VALUES (?1, 1, x'7b7d')",
            [&other],
        )
        .expect("other suite");
    session
        .store
        .connection()
        .execute(
            "UPDATE generation_qualification_plans SET generation_suite_manifest_id = ?1",
            [&other],
        )
        .expect("retarget plan suite");
    expect_store(
        &mut session.store,
        support::resource_input(&session.prepared),
        &StoreError::CorruptRecord,
    );
    expect_read(
        &session.store,
        &session.prepared,
        &StoreError::CorruptRecord,
    );
    assert_eq!(support::count(&session.store, support::RESOURCE_TABLE), 0);
}

#[test]
fn third_gate_failure_hides_the_secret_and_writes_nothing() {
    let mut session = support::session();
    let mut calls = 0;
    let error = session
        .store
        .transact_generation_resource_policy_denial_v1(
            support::resource_input(&session.prepared),
            || {
                calls += 1;
                if calls == 3 { Err(Secret) } else { Ok(()) }
            },
        )
        .expect_err("gate");
    assert_eq!(calls, 3);
    assert_eq!(
        format!("{error:?}"),
        "PhasePolicyDenialV1TransactionError::Gate"
    );
    assert_eq!(
        error.to_string(),
        "phase policy denial gate rejected the commit"
    );
    assert!(!format!("{error:?}").contains("secret-token"));
    assert!(!error.to_string().contains("secret-token"));
    assert_eq!(support::count(&session.store, support::RESOURCE_TABLE), 0);
}

fn expect_store(
    store: &mut ArtifactStateStore,
    input: GenerationResourcePolicyDenialV1Input<'_>,
    expected: &StoreError,
) {
    let error = store
        .transact_generation_resource_policy_denial_v1(input, || Ok::<_, ()>(()))
        .expect_err("denied write");
    assert!(matches!(
        error,
        PhasePolicyDenialV1TransactionError::Store(value) if store_eq(&value, expected)
    ));
}

fn expect_read(store: &ArtifactStateStore, prepared: &support::Prepared, expected: &StoreError) {
    let error = store
        .generation_resource_policy_denial_v1(support::resource_read(prepared))
        .expect_err("denied read");
    assert!(store_eq(&error, expected));
}

fn store_eq(actual: &StoreError, expected: &StoreError) -> bool {
    std::mem::discriminant(actual) == std::mem::discriminant(expected)
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
