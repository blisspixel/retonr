use serde_json::Value;

use super::super::CandidateJudgeExecutionV1WriteDisposition;
use super::support::{self, Session};
use crate::WriteDisposition;

#[test]
fn absent_cohort_inserts_once_and_reads_back_without_changes() {
    let mut session = support::session(42);
    let facts = support::facts(&session.cohort);
    let read = support::read_input(&session.cohort, &facts);
    let before = session.store.connection().total_changes();
    assert!(
        session
            .store
            .candidate_judge_execution_v1(read)
            .expect("absent read")
            .is_none()
    );
    assert_eq!(session.store.connection().total_changes(), before);
    let disposition = session
        .store
        .transact_candidate_judge_execution_v1(
            support::write_input(&session.cohort, &facts),
            || Ok::<_, ()>(()),
        )
        .expect("insert");
    assert_uniform(disposition, WriteDisposition::Inserted);
    let after_write = session.store.connection().total_changes();
    let stored = session
        .store
        .candidate_judge_execution_v1(support::read_input(&session.cohort, &facts))
        .expect("read")
        .expect("stored cohort");
    assert_eq!(session.store.connection().total_changes(), after_write);
    assert_eq!(stored.plan(), &session.cohort.plan);
    assert_eq!(stored.schedule(), &session.cohort.schedule);
    assert_eq!(stored.request_aggregate(), &session.cohort.requests);
    assert_eq!(stored.response_aggregate(), &session.cohort.responses);
    assert_eq!(stored.observation_batch(), &session.cohort.observations);
    assert_eq!(stored.managed_receipt(), &session.cohort.receipt);
    assert_eq!(stored.join(), &session.cohort.join);
    assert_indexed_closure(&session);
    assert_inert(&session);
}

#[test]
fn exact_replay_is_uniform_already_present() {
    let mut session = support::session(42);
    let facts = support::facts(&session.cohort);
    let write = || support::write_input(&session.cohort, &facts);
    session
        .store
        .transact_candidate_judge_execution_v1(write(), || Ok::<_, ()>(()))
        .expect("insert");
    let disposition = session
        .store
        .transact_candidate_judge_execution_v1(write(), || Ok::<_, ()>(()))
        .expect("replay");
    assert_uniform(disposition, WriteDisposition::AlreadyPresent);
    assert_eq!(count(&session, "candidate_judge_plans"), 1);
}

#[test]
fn presentation_seed_column_preserves_decimal_text() {
    for seed in [0_u64, 42, u64::MAX] {
        assert_seed(seed);
    }
}

fn assert_seed(seed: u64) {
    let mut session = support::session(seed);
    let facts = support::facts(&session.cohort);
    session
        .store
        .transact_candidate_judge_execution_v1(
            support::write_input(&session.cohort, &facts),
            || Ok::<_, ()>(()),
        )
        .expect("insert");
    let text = seed.to_string();
    for table in ["candidate_judge_plans", "candidate_judge_schedules"] {
        let column: String = session
            .store
            .connection()
            .query_row(
                &format!("SELECT presentation_seed FROM {table}"),
                [],
                |row| row.get(0),
            )
            .expect("seed column");
        assert_eq!(column, text);
    }
    let bytes: Vec<u8> = session
        .store
        .connection()
        .query_row(
            "SELECT canonical_json FROM candidate_judge_plans",
            [],
            |row| row.get(0),
        )
        .expect("plan json");
    let value: Value = serde_json::from_slice(&bytes).expect("plan value");
    assert_eq!(
        value.get("presentation_seed").and_then(Value::as_u64),
        Some(seed)
    );
}

fn assert_indexed_closure(session: &Session) {
    let connection = session.store.connection();
    let entry_count: i64 = connection
        .query_row(
            "SELECT entry_count FROM candidate_judge_schedules",
            [],
            |row| row.get(0),
        )
        .expect("entry count");
    let attempt_count: i64 = connection
        .query_row(
            "SELECT attempt_count FROM managed_local_judge_receipts",
            [],
            |row| row.get(0),
        )
        .expect("attempt count");
    let (first, last): (i64, i64) = connection
        .query_row(
            "SELECT first_response_ordinal, last_response_ordinal FROM managed_local_judge_receipts",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("ordinals");
    assert_eq!(entry_count, 2);
    assert_eq!(attempt_count, 2);
    assert_eq!(first, 8);
    assert_eq!(last, 25);
    for table in [
        "managed_local_judge_receipts",
        "candidate_judge_join_records",
    ] {
        let class: String = connection
            .query_row(&format!("SELECT evidence_class FROM {table}"), [], |row| {
                row.get(0)
            })
            .expect("evidence class");
        assert_eq!(class, "managed_local_judge_triage");
    }
    let claims: (i64, i64, i64) = connection
        .query_row(
            "SELECT candidate_semantics_proven, judge_correctness_proven, qualified
             FROM candidate_judge_join_records",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("claims");
    assert_eq!(claims, (0, 0, 0));
}

fn assert_inert(session: &Session) {
    let connection = session.store.connection();
    for table in [
        "qualification_records",
        "qualification_v2_records",
        "generation_qualification_records",
    ] {
        assert_eq!(count(session, table), 0, "{table}");
    }
    for table in [
        "generation_qualification_invalidations",
        "generation_activation_decisions",
        "active_generation_bindings",
    ] {
        let present: i64 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [table],
                |row| row.get(0),
            )
            .expect("schema lookup");
        assert_eq!(present, 0, "{table}");
    }
    let judge = session
        .cohort
        .judge
        .generation_system_id()
        .digest()
        .as_str();
    let membership: i64 = connection
        .query_row(
            "SELECT count(*) FROM generation_qualification_plan_systems
             WHERE generation_system_id = ?1",
            [judge],
            |row| row.get(0),
        )
        .expect("plan systems");
    assert_eq!(membership, 0);
    assert_eq!(count(session, "generation_repeatability_result_records"), 0);
}

fn assert_uniform(
    disposition: CandidateJudgeExecutionV1WriteDisposition,
    expected: WriteDisposition,
) {
    assert_eq!(disposition.plan, expected);
    assert_eq!(disposition.schedule, expected);
    assert_eq!(disposition.request_aggregate, expected);
    assert_eq!(disposition.response_aggregate, expected);
    assert_eq!(disposition.observation_batch, expected);
    assert_eq!(disposition.managed_receipt, expected);
    assert_eq!(disposition.join, expected);
}

fn count(session: &Session, table: &str) -> i64 {
    session
        .store
        .connection()
        .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("count")
}
