use rewrite_model::GenerationResourcePolicyDenialRecordV1;
use rewrite_types::Digest;

use super::super::{
    GenerationResourcePolicyDenialV1Input, GenerationResourcePolicyDenialV1ReadInput,
};
use super::support::{self, Session};
use crate::WriteDisposition;

#[test]
fn insert_and_exact_replay_keep_one_canonical_row() {
    let mut session = support::session();
    assert_eq!(
        support::commit_resource(&mut session),
        WriteDisposition::Inserted
    );
    assert_eq!(
        support::commit_resource(&mut session),
        WriteDisposition::AlreadyPresent
    );
    assert_eq!(support::count(&session.store, support::RESOURCE_TABLE), 1);
    let stored = session
        .store
        .generation_resource_policy_denial_v1(support::resource_read(&session.prepared))
        .expect("resource read");
    assert_eq!(stored.as_ref(), Some(&session.prepared.resource));
    let json: Vec<u8> = session
        .store
        .connection()
        .query_row(
            "SELECT canonical_json FROM generation_resource_policy_denial_records",
            [],
            |row| row.get(0),
        )
        .expect("canonical json");
    assert_eq!(
        json,
        serde_json::to_vec(&session.prepared.resource).expect("encode resource")
    );
}

#[test]
fn a_second_policy_digest_is_a_second_row() {
    let mut session = support::session();
    support::commit_resource(&mut session);
    let other = Digest::sha256(b"second denied phase policy");
    let record = GenerationResourcePolicyDenialRecordV1::new(support::resource_relations(
        &session.prepared.fixture,
        &other,
    ))
    .expect("second denial");
    let disposition = session
        .store
        .transact_generation_resource_policy_denial_v1(
            GenerationResourcePolicyDenialV1Input {
                record: &record,
                relations: support::resource_relations(&session.prepared.fixture, &other),
            },
            || Ok::<_, ()>(()),
        )
        .expect("second write");
    assert_eq!(disposition, WriteDisposition::Inserted);
    assert_eq!(support::count(&session.store, support::RESOURCE_TABLE), 2);
    let first = session
        .store
        .generation_resource_policy_denial_v1(support::resource_read(&session.prepared))
        .expect("first read");
    assert_eq!(first.as_ref(), Some(&session.prepared.resource));
    let second = session
        .store
        .generation_resource_policy_denial_v1(GenerationResourcePolicyDenialV1ReadInput {
            relations: support::resource_relations(&session.prepared.fixture, &other),
        })
        .expect("second read");
    assert_eq!(second.as_ref(), Some(&record));
}

#[test]
fn the_same_scope_is_independent_in_both_denial_tables() {
    let mut session = support::session();
    assert_eq!(
        support::commit_resource(&mut session),
        WriteDisposition::Inserted
    );
    assert_eq!(
        support::commit_human(&mut session),
        WriteDisposition::Inserted
    );
    assert_eq!(
        support::commit_human(&mut session),
        WriteDisposition::AlreadyPresent
    );
    assert_ne!(
        session
            .prepared
            .resource
            .resource_policy_denial_record_id()
            .digest()
            .as_str(),
        session
            .prepared
            .human
            .human_adjudication_policy_denial_record_id()
            .digest()
            .as_str()
    );
    assert_eq!(support::count(&session.store, support::RESOURCE_TABLE), 1);
    assert_eq!(support::count(&session.store, support::HUMAN_TABLE), 1);
    let human = session
        .store
        .generation_human_adjudication_policy_denial_v1(support::human_read(&session.prepared))
        .expect("human read");
    assert_eq!(human.as_ref(), Some(&session.prepared.human));
    let human_json: Vec<u8> = session
        .store
        .connection()
        .query_row(
            "SELECT canonical_json FROM generation_human_adjudication_policy_denial_records",
            [],
            |row| row.get(0),
        )
        .expect("human json");
    assert_eq!(
        human_json,
        serde_json::to_vec(&session.prepared.human).expect("encode human")
    );
}

#[test]
fn an_unread_policy_is_absent_after_the_foundation_loads() {
    let session = support::session();
    let other = Digest::sha256(b"unread denied phase policy");
    let stored = session
        .store
        .generation_resource_policy_denial_v1(GenerationResourcePolicyDenialV1ReadInput {
            relations: support::resource_relations(&session.prepared.fixture, &other),
        })
        .expect("absent read");
    assert!(stored.is_none());
}

#[test]
fn stored_denials_grant_no_authority_and_leave_repeatability_unchanged() {
    let mut session = support::session();
    let membership = support::count(&session.store, "generation_qualification_plan_systems");
    support::commit_resource(&mut session);
    support::commit_human(&mut session);
    assert_eq!(membership, 2);
    assert_eq!(
        support::count(&session.store, "generation_qualification_plan_systems"),
        membership
    );
    for table in [
        "qualification_records",
        "qualification_v2_records",
        "generation_resource_attempt_result_records",
        "generation_repeatability_result_records",
        "generation_qualification_records",
    ] {
        assert_eq!(support::count(&session.store, table), 0, "{table}");
    }
    for table in [
        "generation_qualification_invalidations",
        "generation_activation_decisions",
        "active_generation_bindings",
    ] {
        let present: i64 = session
            .store
            .connection()
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [table],
                |row| row.get(0),
            )
            .expect("schema lookup");
        assert_eq!(present, 0, "{table}");
    }
    assert_repeatability_stays_closed(&session);
}

fn assert_repeatability_stays_closed(session: &Session) {
    let sql: String = session
        .store
        .connection()
        .query_row(
            "SELECT sql FROM sqlite_master WHERE name = 'generation_repeatability_result_records'",
            [],
            |row| row.get(0),
        )
        .expect("repeatability sql");
    assert!(sql.contains("terminal_stage = 'candidate_generation_failed'"));
    let hex = "a".repeat(64);
    assert!(
        session
            .store
            .connection()
            .execute(
                "INSERT INTO generation_repeatability_result_records (
                    generation_repeatability_result_id, generation_system_id,
                    generation_qualification_plan_id, generation_suite_manifest_id,
                    generation_repetition_id, generation_attempt_ledger_manifest_id,
                    terminal_stage, candidate_generation_receipt_set_id,
                    candidate_deterministic_evaluation_id, candidate_judge_join_id,
                    canonical_json
                 ) VALUES (?1, ?1, ?1, ?1, ?1, ?1, 'passed', NULL, NULL, NULL, x'7b7d')",
                [&hex],
            )
            .is_err()
    );
    assert_eq!(
        support::count(&session.store, "generation_repeatability_result_records"),
        0
    );
}
