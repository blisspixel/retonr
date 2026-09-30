use super::support::{self, Session};
use crate::WriteDisposition;

#[test]
fn insert_and_exact_replay_keep_one_canonical_row() {
    let mut session = support::session();
    let absent = session
        .store
        .candidate_deterministic_evaluation_v1(support::read_input(&session))
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
        .candidate_deterministic_evaluation_v1(support::read_input(&session))
        .expect("evaluation read");
    assert_eq!(stored.as_ref(), Some(&session.record));
    let encoded = serde_json::to_vec(&session.record).expect("encode evaluation");
    assert_eq!(support::canonical_json(&session), encoded);
    let schema_version: i64 = session
        .store
        .connection()
        .query_row(
            "SELECT schema_version FROM candidate_deterministic_evaluation_records",
            [],
            |row| row.get(0),
        )
        .expect("schema version");
    assert_eq!(schema_version, i64::from(session.record.schema_version()));
    let ids = support::parent_ids(&session);
    assert!(
        encoded
            .windows(64)
            .any(|window| window == ids.receipt_a.as_bytes())
    );
    assert!(
        encoded
            .windows(64)
            .any(|window| window == ids.receipt_b.as_bytes())
    );
}

#[test]
fn the_same_receipt_pair_can_store_a_second_evaluation() {
    let mut session = support::session();
    assert_eq!(support::commit(&mut session), WriteDisposition::Inserted);
    let disposition = session
        .store
        .transact_candidate_deterministic_evaluation_v1(
            super::super::CandidateDeterministicEvaluationV1Input {
                record: &session.alternate,
            },
            || Ok::<_, ()>(()),
        )
        .expect("second evaluation");
    assert_eq!(disposition, WriteDisposition::Inserted);
    assert_eq!(support::count(&session.store, support::TABLE), 2);
    let first = session
        .store
        .candidate_deterministic_evaluation_v1(support::read_input(&session))
        .expect("first read");
    let second = session
        .store
        .candidate_deterministic_evaluation_v1(
            super::super::CandidateDeterministicEvaluationV1ReadInput {
                record: &session.alternate,
            },
        )
        .expect("second read");
    assert_eq!(first.as_ref(), Some(&session.record));
    assert_eq!(second.as_ref(), Some(&session.alternate));
    assert_ne!(session.record, session.alternate);
}

#[test]
fn stored_evaluations_grant_no_authority() {
    let mut session = support::session();
    let receipt_sets = support::count(&session.store, "candidate_generation_receipt_sets");
    let attempts = support::count(&session.store, "candidate_generation_attempt_records");
    support::commit(&mut session);
    assert_eq!(
        support::count(&session.store, "candidate_generation_receipt_sets"),
        receipt_sets
    );
    assert_eq!(
        support::count(&session.store, "candidate_generation_attempt_records"),
        attempts
    );
    let user_version: i64 = session
        .store
        .connection()
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("user version");
    assert_eq!(user_version, 19);
    for table in [
        "qualification_records",
        "qualification_v2_records",
        "generation_repeatability_terminal_result_records",
        "generation_qualification_records",
        "generation_qualification_invalidations",
        "generation_qualification_selections",
    ] {
        assert_eq!(support::count(&session.store, table), 0, "{table}");
    }
    for table in [
        "generation_activation_decisions",
        "active_generation_bindings",
    ] {
        assert_eq!(table_present(&session, table), 0, "{table}");
    }
    assert_eq!(explicit_index_count(&session), 0);
    assert!(facts_stay_inside_json(&session));
}

fn table_present(session: &Session, table: &str) -> i64 {
    session
        .store
        .connection()
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table],
            |row| row.get(0),
        )
        .expect("schema lookup")
}

fn explicit_index_count(session: &Session) -> i64 {
    session
        .store
        .connection()
        .query_row(
            "SELECT count(*) FROM sqlite_master
             WHERE type = 'index' AND tbl_name = ?1 AND sql IS NOT NULL",
            [support::TABLE],
            |row| row.get(0),
        )
        .expect("index count")
}

fn facts_stay_inside_json(session: &Session) -> bool {
    let mut statement = session
        .store
        .connection()
        .prepare("SELECT name FROM pragma_table_info('candidate_deterministic_evaluation_records')")
        .expect("table info");
    let names = statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("column names")
        .collect::<Result<Vec<_>, _>>()
        .expect("columns");
    !names.iter().any(|name| {
        matches!(
            name.as_str(),
            "status"
                | "total"
                | "passed"
                | "report"
                | "reason_code"
                | "qualified"
                | "role"
                | "action"
        )
    }) && names.iter().any(|name| name == "canonical_json")
}
