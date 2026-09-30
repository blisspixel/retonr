use super::support::{self, Session};
use crate::WriteDisposition;

#[test]
fn insert_and_exact_replay_keep_one_canonical_row() {
    let mut session = support::session();
    let absent = session
        .store
        .candidate_generation_receipt_set_v1(support::read_input(&session))
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
        .candidate_generation_receipt_set_v1(support::read_input(&session))
        .expect("receipt set read");
    assert_eq!(stored.as_ref(), Some(&session.record));
    let encoded = serde_json::to_vec(&session.record).expect("encode receipt set");
    assert_eq!(support::canonical_json(&session), encoded);
    let entry_count: i64 = session
        .store
        .connection()
        .query_row(
            "SELECT entry_count FROM candidate_generation_receipt_sets",
            [],
            |row| row.get(0),
        )
        .expect("entry count");
    assert_eq!(entry_count, i64::from(session.record.entry_count()));
    assert!(encoded.windows(64).any(|window| {
        window
            == session.record.entries()[0]
                .receipt_id()
                .digest()
                .as_str()
                .as_bytes()
    }));
}

#[test]
fn stored_receipt_sets_grant_no_authority() {
    let mut session = support::session();
    let receipts = support::count(&session.store, "candidate_generation_receipts");
    let attempts = support::count(&session.store, "candidate_generation_attempt_records");
    support::commit(&mut session);
    assert_eq!(
        support::count(&session.store, "candidate_generation_receipts"),
        receipts
    );
    assert_eq!(
        support::count(&session.store, "candidate_generation_attempt_records"),
        attempts
    );
    for table in [
        "qualification_records",
        "qualification_v2_records",
        "candidate_deterministic_evaluation_records",
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
    assert!(entries_stay_inside_json(&session));
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

fn entries_stay_inside_json(session: &Session) -> bool {
    let mut statement = session
        .store
        .connection()
        .prepare("SELECT name FROM pragma_table_info('candidate_generation_receipt_sets')")
        .expect("table info");
    let names = statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("column names")
        .collect::<Result<Vec<_>, _>>()
        .expect("columns");
    !names.iter().any(|name| {
        matches!(
            name.as_str(),
            "entries" | "status" | "reason_code" | "qualified" | "role" | "action"
        )
    }) && names.iter().any(|name| name == "canonical_json")
}
