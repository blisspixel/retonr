use rewrite_model::{
    GenerationResourcePolicyDenialRecordV1, GenerationResourcePolicyDenialRecordV1Relations,
};
use rewrite_types::Digest;

use super::support::{self, Session};
use crate::GenerationResourcePolicyDenialV1Input;
use crate::WriteDisposition;

#[test]
fn insert_and_exact_replay_keep_one_canonical_row() {
    let mut session = support::session();
    let absent = session
        .store
        .generation_resource_attempt_result_v1(support::read_input(&session))
        .expect("absent read");
    assert!(absent.is_none());
    assert_eq!(support::commit(&mut session), WriteDisposition::Inserted);
    assert_eq!(
        support::commit(&mut session),
        WriteDisposition::AlreadyPresent
    );
    assert_eq!(support::count(&session.store, support::TABLE), 1);
    let stored = session
        .store
        .generation_resource_attempt_result_v1(support::read_input(&session))
        .expect("resource read");
    assert_eq!(stored.as_ref(), Some(&session.record));
    assert_eq!(
        support::canonical_json(&session),
        serde_json::to_vec(&session.record).expect("encode result")
    );
    let profile: String = session
        .store
        .connection()
        .query_row(
            "SELECT observation_profile FROM generation_resource_attempt_result_records",
            [],
            |row| row.get(0),
        )
        .expect("profile");
    assert_eq!(profile, "managed_ollama_v0_32_15_linux_v1");
}

#[test]
fn a_different_observation_for_the_same_attempt_is_corrupt() {
    let mut session = support::session();
    support::commit(&mut session);
    let original = support::canonical_json(&session);
    let other = support::other_record(&session);
    support::expect_store(
        &mut session.store,
        &other,
        &crate::StoreError::CorruptRecord,
    );
    assert_eq!(support::canonical_json(&session), original);
    assert_eq!(support::count(&session.store, support::TABLE), 1);
    let stored = session
        .store
        .generation_resource_attempt_result_v1(support::read_input(&session))
        .expect("original read");
    assert_eq!(stored.as_ref(), Some(&session.record));
    support::expect_read(&session.store, &other, &crate::StoreError::CorruptRecord);
}

#[test]
fn stored_results_grant_no_authority_and_leave_repeatability_unchanged() {
    let mut session = support::session();
    let membership = support::count(&session.store, "generation_qualification_plan_systems");
    support::commit(&mut session);
    assert_eq!(
        support::count(&session.store, "generation_qualification_plan_systems"),
        membership
    );
    for table in [
        "qualification_records",
        "qualification_v2_records",
        "generation_resource_policy_denial_records",
        "generation_human_adjudication_policy_denial_records",
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
    assert!(observation_counts_stay_in_json(&session));
}

#[test]
fn a_stored_result_leaves_policy_denials_independent() {
    let mut session = support::session();
    support::commit(&mut session);
    let policy = Digest::sha256(b"denied phase policy");
    let record = GenerationResourcePolicyDenialRecordV1::new(
        GenerationResourcePolicyDenialRecordV1Relations {
            scope: support::scope(&session.fixture),
            phase_policy_digest: &policy,
        },
    )
    .expect("resource denial");
    let disposition = session
        .store
        .transact_generation_resource_policy_denial_v1(
            GenerationResourcePolicyDenialV1Input {
                record: &record,
                relations: GenerationResourcePolicyDenialRecordV1Relations {
                    scope: support::scope(&session.fixture),
                    phase_policy_digest: &policy,
                },
            },
            || Ok::<_, ()>(()),
        )
        .expect("denial write");
    assert_eq!(disposition, WriteDisposition::Inserted);
    assert_eq!(support::count(&session.store, support::TABLE), 1);
    assert_eq!(
        support::count(&session.store, "generation_resource_policy_denial_records"),
        1
    );
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

fn observation_counts_stay_in_json(session: &Session) -> bool {
    let mut statement = session
        .store
        .connection()
        .prepare("SELECT name FROM pragma_table_info('generation_resource_attempt_result_records')")
        .expect("table info");
    let names = statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("column names")
        .collect::<Result<Vec<_>, _>>()
        .expect("read column names");
    [
        "prompt_token_count",
        "generated_token_count",
        "total_duration_nanoseconds",
        "evaluation_duration_nanoseconds",
        "installed_footprint_bytes",
        "exceeded_limits",
    ]
    .iter()
    .all(|column| !names.iter().any(|name| name == column))
}
