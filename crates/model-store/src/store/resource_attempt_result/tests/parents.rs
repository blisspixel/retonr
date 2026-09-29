use rusqlite::params;

use super::support;
use crate::StoreError;

#[test]
fn missing_foundation_is_a_missing_record() {
    let mut session = support::bare();
    support::expect_store(
        &mut session.store,
        &session.record,
        &StoreError::MissingRecord,
    );
    support::expect_read(&session.store, &session.record, &StoreError::MissingRecord);
    assert_eq!(support::count(&session.store, support::TABLE), 0);
}

#[test]
fn a_plan_without_execution_is_a_missing_record() {
    let mut session = support::foundation();
    support::expect_store(
        &mut session.store,
        &session.record,
        &StoreError::MissingRecord,
    );
    support::expect_read(&session.store, &session.record, &StoreError::MissingRecord);
    assert_eq!(support::count(&session.store, support::TABLE), 0);
}

#[test]
fn a_deleted_plan_system_pair_writes_nothing() {
    let mut session = support::session();
    let ids = support::parent_ids(&session);
    session
        .store
        .connection()
        .execute(
            "DELETE FROM generation_qualification_plan_systems
             WHERE generation_qualification_plan_id = ?1 AND generation_system_id = ?2",
            params![ids.plan, ids.system],
        )
        .expect("delete plan system");
    support::expect_store(
        &mut session.store,
        &session.record,
        &StoreError::CorruptRecord,
    );
    support::expect_read(&session.store, &session.record, &StoreError::CorruptRecord);
    assert_eq!(support::count(&session.store, support::TABLE), 0);
}

#[test]
fn a_deleted_plan_attempt_pair_writes_nothing() {
    let mut session = support::foundation();
    let ids = support::parent_ids(&session);
    session
        .store
        .connection()
        .execute(
            "DELETE FROM generation_qualification_plan_attempts
             WHERE generation_qualification_plan_id = ?1 AND planned_candidate_attempt_id = ?2",
            params![ids.plan, ids.planned],
        )
        .expect("delete plan attempt");
    support::expect_store(
        &mut session.store,
        &session.record,
        &StoreError::CorruptRecord,
    );
    support::expect_read(&session.store, &session.record, &StoreError::CorruptRecord);
    assert_eq!(support::count(&session.store, support::TABLE), 0);
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
    support::expect_store(
        &mut session.store,
        &session.record,
        &StoreError::CorruptRecord,
    );
    support::expect_read(&session.store, &session.record, &StoreError::CorruptRecord);
    assert_eq!(support::count(&session.store, support::TABLE), 0);
}

#[test]
fn a_planned_attempt_system_mismatch_writes_nothing() {
    let mut session = support::session();
    let ids = support::parent_ids(&session);
    session
        .store
        .connection()
        .execute(
            "UPDATE planned_candidate_attempts SET generation_system_id = ?1
             WHERE planned_candidate_attempt_id = ?2",
            params![ids.other_system, ids.planned],
        )
        .expect("retarget planned system");
    support::expect_store(
        &mut session.store,
        &session.record,
        &StoreError::CorruptRecord,
    );
    support::expect_read(&session.store, &session.record, &StoreError::CorruptRecord);
    assert_eq!(support::count(&session.store, support::TABLE), 0);
}

#[test]
fn a_case_removed_from_its_suite_writes_nothing() {
    let mut session = support::session();
    let ids = support::parent_ids(&session);
    session
        .store
        .connection()
        .execute(
            "DELETE FROM generation_suite_cases
             WHERE generation_suite_manifest_id = ?1 AND generation_case_id = ?2",
            params![ids.suite, ids.case_id],
        )
        .expect("delete suite case");
    support::expect_store(
        &mut session.store,
        &session.record,
        &StoreError::CorruptRecord,
    );
    support::expect_read(&session.store, &session.record, &StoreError::CorruptRecord);
    assert_eq!(support::count(&session.store, support::TABLE), 0);
}
