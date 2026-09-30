use super::support::{self, Session};
use crate::WriteDisposition;

#[test]
fn insert_and_exact_replay_keep_one_canonical_row() {
    let mut session = support::session();
    let absent = session
        .parents
        .store
        .generation_qualification_invalidation_v1(support::read_input(&session))
        .expect("absent read");
    assert!(absent.is_none());
    assert_eq!(support::commit(&mut session), WriteDisposition::Inserted);
    assert_eq!(
        support::commit(&mut session),
        WriteDisposition::AlreadyPresent
    );
    assert_eq!(support::count(&session.parents.store, support::TABLE), 1);
    let stored = session
        .parents
        .store
        .generation_qualification_invalidation_v1(support::read_input(&session))
        .expect("invalidation read");
    assert_eq!(stored.as_ref(), Some(&session.invalidation));
    let encoded = serde_json::to_vec(&session.invalidation).expect("encode invalidation");
    assert_eq!(support::canonical_json(&session), encoded);
    let qualification = session
        .invalidation
        .generation_qualification_id()
        .digest()
        .as_str()
        .to_owned();
    assert!(
        encoded
            .windows(64)
            .any(|window| window == qualification.as_bytes())
    );
    assert_eq!(support::qualification_status(&session), "rejected");
}

#[test]
fn stored_invalidations_grant_no_authority() {
    let mut session = support::session();
    let qualification_json = support::qualification_json(&session);
    let receipts = support::count(
        &session.parents.store,
        "generation_qualification_operation_receipts",
    );
    let schema_eleven = support::count(
        &session.parents.store,
        "generation_repeatability_result_records",
    );
    support::commit(&mut session);
    assert_eq!(support::qualification_json(&session), qualification_json);
    assert_eq!(support::qualification_status(&session), "rejected");
    assert_eq!(
        support::count(&session.parents.store, "generation_qualification_records"),
        1
    );
    assert_eq!(
        support::count(
            &session.parents.store,
            "generation_qualification_operation_receipts"
        ),
        receipts
    );
    assert_eq!(
        support::count(
            &session.parents.store,
            "generation_repeatability_result_records"
        ),
        schema_eleven
    );
    assert_eq!(support::count(&session.parents.store, support::TABLE), 1);
    let user_version: i64 = session
        .parents
        .store
        .connection()
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("user version");
    assert_eq!(user_version, 19);
    for table in [
        "qualification_records",
        "qualification_v2_records",
        "generation_qualification_selections",
    ] {
        assert_eq!(support::count(&session.parents.store, table), 0, "{table}");
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
        .parents
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
        .parents
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
        .parents
        .store
        .connection()
        .prepare("SELECT name FROM pragma_table_info('generation_qualification_invalidations')")
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
                | "reason"
                | "reason_code"
                | "qualified"
                | "role"
                | "action"
                | "schema_version"
        )
    }) && names.iter().any(|name| name == "canonical_json")
        && names
            .iter()
            .any(|name| name == "generation_qualification_id")
}
