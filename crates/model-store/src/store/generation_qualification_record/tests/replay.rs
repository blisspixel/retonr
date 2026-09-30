use super::support::{self, Session};
use crate::WriteDisposition;

#[test]
fn insert_and_exact_replay_keep_one_canonical_row() {
    let mut session = support::session();
    let absent = session
        .store
        .generation_qualification_record_v1(support::read_input(&session))
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
        .generation_qualification_record_v1(support::read_input(&session))
        .expect("qualification read");
    assert_eq!(stored.as_ref(), Some(&session.record));
    let encoded = serde_json::to_vec(&session.record).expect("encode record");
    assert_eq!(support::canonical_json(&session), encoded);
    let status: String = session
        .store
        .connection()
        .query_row(
            "SELECT status FROM generation_qualification_records",
            [],
            |row| row.get(0),
        )
        .expect("status");
    assert_eq!(status, "rejected");
    let receipt = session
        .record
        .operation_receipt_id()
        .digest()
        .as_str()
        .to_owned();
    assert!(
        encoded
            .windows(64)
            .any(|window| window == receipt.as_bytes())
    );
}

#[test]
fn stored_qualification_records_grant_no_authority() {
    let mut session = support::session();
    let receipts = support::count(
        &session.store,
        "generation_qualification_operation_receipts",
    );
    let schema_eleven = support::count(&session.store, "generation_repeatability_result_records");
    support::commit(&mut session);
    assert_eq!(
        support::count(
            &session.store,
            "generation_qualification_operation_receipts"
        ),
        receipts
    );
    assert_eq!(
        support::count(&session.store, "generation_repeatability_result_records"),
        schema_eleven
    );
    assert_eq!(support::count(&session.store, support::TABLE), 1);
    assert_eq!(
        support::count(
            &session.store,
            "generation_repeatability_terminal_result_records"
        ),
        0
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
        .prepare("SELECT name FROM pragma_table_info('generation_qualification_records')")
        .expect("table info");
    let names = statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("column names")
        .collect::<Result<Vec<_>, _>>()
        .expect("columns");
    !names.iter().any(|name| {
        matches!(
            name.as_str(),
            "reason" | "reason_code" | "qualified" | "role" | "action" | "schema_version"
        )
    }) && names.iter().any(|name| name == "canonical_json")
        && names.iter().any(|name| name == "status")
}
