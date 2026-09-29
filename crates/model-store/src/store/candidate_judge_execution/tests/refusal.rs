use rewrite_model::{
    CandidateDeterministicEvaluationRecordV1, CandidateDeterministicEvaluationRecordV1Input,
};

use super::super::CandidateJudgeExecutionV1TransactionError;
use super::support::{self, Session};
use crate::{ArtifactStateStore, StoreError};

#[test]
fn borrowed_plan_schedule_receipt_and_join_disagreements_write_nothing() {
    reject_borrowed_plan();
    reject_borrowed_schedule();
    reject_borrowed_receipt();
    reject_borrowed_join();
}

#[test]
fn same_primary_key_with_different_json_does_not_replace_bytes() {
    let mut session = support::session(42);
    commit(&mut session);
    let replacement = br#"{"schema_version":1}"#;
    session
        .store
        .connection()
        .execute(
            "UPDATE candidate_judge_plans SET canonical_json = ?1",
            [replacement.as_slice()],
        )
        .expect("tamper plan json");
    let facts = support::facts(&session.cohort);
    let error = session
        .store
        .transact_candidate_judge_execution_v1(
            support::write_input(&session.cohort, &facts),
            || Ok::<_, ()>(()),
        )
        .expect_err("replay");
    assert!(matches!(
        error,
        CandidateJudgeExecutionV1TransactionError::Store(StoreError::ImmutableConflict)
    ));
    let stored: Vec<u8> = session
        .store
        .connection()
        .query_row(
            "SELECT canonical_json FROM candidate_judge_plans",
            [],
            |row| row.get(0),
        )
        .expect("stored json");
    assert_eq!(stored, replacement);
}

#[test]
fn mixed_dispositions_roll_back() {
    let mut session = support::session(42);
    insert_plan_row(&session);
    let facts = support::facts(&session.cohort);
    let error = session
        .store
        .transact_candidate_judge_execution_v1(
            support::write_input(&session.cohort, &facts),
            || Ok::<_, ()>(()),
        )
        .expect_err("mixed");
    assert!(matches!(
        error,
        CandidateJudgeExecutionV1TransactionError::Store(StoreError::ImmutableConflict)
    ));
    assert_eq!(rows(&session.store, "candidate_judge_plans"), 1);
    assert_eq!(rows(&session.store, "candidate_judge_schedules"), 0);
    assert_eq!(rows(&session.store, "candidate_judge_join_records"), 0);
}

#[test]
fn third_gate_failure_hides_the_secret_and_writes_nothing() {
    let mut session = support::session(42);
    let facts = support::facts(&session.cohort);
    let mut calls = 0;
    let error = session
        .store
        .transact_candidate_judge_execution_v1(
            support::write_input(&session.cohort, &facts),
            || {
                calls += 1;
                if calls == 3 { Err(Secret) } else { Ok(()) }
            },
        )
        .expect_err("gate");
    assert_eq!(calls, 3);
    assert_eq!(
        format!("{error:?}"),
        "CandidateJudgeExecutionV1TransactionError::Gate"
    );
    assert_eq!(
        error.to_string(),
        "candidate judge execution gate rejected the commit"
    );
    assert!(!format!("{error:?}").contains("secret-token"));
    assert!(!error.to_string().contains("secret-token"));
    assert_absent(&session.store);
}

#[test]
fn missing_foundations_are_missing_records() {
    let cohort = support::cohort(42);
    let facts = support::facts(&cohort);
    let directory = tempfile::tempdir().expect("temp");
    let mut store = ArtifactStateStore::open(&directory.path().join("state.db")).expect("open");
    let write_error = store
        .transact_candidate_judge_execution_v1(support::write_input(&cohort, &facts), || {
            Ok::<_, ()>(())
        })
        .expect_err("missing plan");
    assert!(matches!(
        write_error,
        CandidateJudgeExecutionV1TransactionError::Store(StoreError::MissingRecord)
    ));
    let read_error = store
        .candidate_judge_execution_v1(support::read_input(&cohort, &facts))
        .expect_err("missing plan read");
    assert!(matches!(read_error, StoreError::MissingRecord));
    crate::store::generation_qualification_preregistration::tests::support::persist_plan_foundation(
        &mut store,
        &cohort.fixture,
    );
    let write_error = store
        .transact_candidate_judge_execution_v1(support::write_input(&cohort, &facts), || {
            Ok::<_, ()>(())
        })
        .expect_err("missing judge");
    assert!(matches!(
        write_error,
        CandidateJudgeExecutionV1TransactionError::Store(StoreError::MissingRecord)
    ));
    let read_error = store
        .candidate_judge_execution_v1(support::read_input(&cohort, &facts))
        .expect_err("missing judge read");
    assert!(matches!(read_error, StoreError::MissingRecord));
}

#[test]
fn tampered_or_partial_rows_are_corrupt_not_absent() {
    let mut session = support::session(42);
    commit(&mut session);
    session
        .store
        .connection()
        .execute(
            "UPDATE candidate_judge_plans SET canonical_json = ?1",
            [br"{}".as_slice()],
        )
        .expect("tamper");
    assert_corrupt(&mut session);
    session
        .store
        .connection()
        .execute("DELETE FROM candidate_judge_join_records", [])
        .expect("delete join");
    assert_corrupt(&mut session);
}

#[test]
fn failed_closure_writes_nothing() {
    reject_failed_deterministic();
    reject_judge_equal_to_candidate();
    reject_empty_triage();
    reject_zero_generation();
}

#[test]
fn stored_checks_reject_authority_and_passed_repeatability() {
    let mut session = support::session(42);
    commit(&mut session);
    let connection = session.store.connection();
    assert!(
        connection
            .execute(
                "UPDATE managed_local_judge_receipts SET evidence_class = 'other'",
                [],
            )
            .is_err()
    );
    assert!(
        connection
            .execute(
                "UPDATE candidate_judge_join_records SET evidence_class = 'other'",
                [],
            )
            .is_err()
    );
    assert!(
        connection
            .execute(
                "UPDATE candidate_judge_join_records SET candidate_semantics_proven = 1",
                [],
            )
            .is_err()
    );
    assert!(
        connection
            .execute(
                "UPDATE candidate_judge_join_records SET judge_correctness_proven = 1",
                [],
            )
            .is_err()
    );
    assert!(
        connection
            .execute("UPDATE candidate_judge_join_records SET qualified = 1", [])
            .is_err()
    );
    let hex = "a".repeat(64);
    assert!(
        connection
            .execute(
                "INSERT INTO generation_repeatability_result_records (
                generation_repeatability_result_id, generation_system_id,
                generation_qualification_plan_id, generation_suite_manifest_id,
                generation_repetition_id, generation_attempt_ledger_manifest_id,
                terminal_stage, candidate_generation_receipt_set_id,
                candidate_deterministic_evaluation_id, candidate_judge_join_id, canonical_json
             ) VALUES (?1, ?1, ?1, ?1, ?1, ?1, 'passed', NULL, NULL, NULL, x'7b7d')",
                [&hex],
            )
            .is_err()
    );
    assert!(connection
        .execute(
            "INSERT INTO generation_repeatability_result_records (
                generation_repeatability_result_id, generation_system_id,
                generation_qualification_plan_id, generation_suite_manifest_id,
                generation_repetition_id, generation_attempt_ledger_manifest_id,
                terminal_stage, candidate_generation_receipt_set_id,
                candidate_deterministic_evaluation_id, candidate_judge_join_id, canonical_json
             ) VALUES (?1, ?1, ?1, ?1, ?1, ?1, 'candidate_generation_failed', NULL, NULL, ?1, x'7b7d')",
            [&hex],
        )
        .is_err());
    assert_eq!(
        rows(&session.store, "generation_repeatability_result_records"),
        0
    );
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

fn reject_borrowed_plan() {
    let mut session = support::session(42);
    let other = support::cohort(43);
    let facts = support::facts(&session.cohort);
    let mut input = support::write_input(&session.cohort, &facts);
    input.plan_input = &other.plan_input;
    expect_conflict(&mut session.store, input);
}

fn reject_borrowed_schedule() {
    let mut session = support::session(42);
    let other = support::cohort(43);
    let facts = support::facts(&session.cohort);
    let mut input = support::write_input(&session.cohort, &facts);
    input.schedule = &other.schedule;
    expect_conflict(&mut session.store, input);
}

fn reject_borrowed_receipt() {
    let mut session = support::session(42);
    let facts = support::facts(&session.cohort);
    let mut input = support::write_input(&session.cohort, &facts);
    let mut alternate = support::receipt_input(1, 2);
    alternate.judge_runtime_installation_generation = 8;
    input.managed_receipt_input = &alternate;
    expect_conflict(&mut session.store, input);
}

fn reject_borrowed_join() {
    let mut session = support::session(42);
    let other = support::cohort(43);
    let facts = support::facts(&session.cohort);
    let mut input = support::write_input(&session.cohort, &facts);
    input.join = &other.join;
    expect_conflict(&mut session.store, input);
}

fn expect_conflict(
    store: &mut ArtifactStateStore,
    input: super::super::CandidateJudgeExecutionV1Input<'_>,
) {
    let error = store
        .transact_candidate_judge_execution_v1(input, || Ok::<_, ()>(()))
        .expect_err("conflict");
    assert!(matches!(
        error,
        CandidateJudgeExecutionV1TransactionError::Store(StoreError::ImmutableConflict)
    ));
    assert_absent(store);
}

fn reject_failed_deterministic() {
    let mut session = support::session(42);
    let facts = support::facts(&session.cohort);
    let reports = support::reports(false);
    let deterministic = CandidateDeterministicEvaluationRecordV1::new(
        &session.cohort.receipt_a,
        &session.cohort.receipt_b,
        CandidateDeterministicEvaluationRecordV1Input {
            case_material_set_digest: &session.cohort.case_material,
            suite_pair_digest: &session.cohort.suite_pair,
            report_relationship: &reports,
        },
    )
    .expect("failed evaluation");
    let mut input = support::write_input(&session.cohort, &facts);
    input.deterministic_evaluation = &deterministic;
    input.deterministic_input.report_relationship = &reports;
    expect_contract(&mut session.store, input);
}

fn reject_judge_equal_to_candidate() {
    let mut session = support::session(42);
    let facts = support::facts(&session.cohort);
    let mut input = support::write_input(&session.cohort, &facts);
    input.judge_generation_system_id = session.cohort.fixture.systems[0].generation_system_id();
    expect_contract(&mut session.store, input);
    let mut input = support::write_input(&session.cohort, &facts);
    input.judge_generation_system_id = session.cohort.fixture.systems[1].generation_system_id();
    expect_contract(&mut session.store, input);
}

fn reject_empty_triage() {
    let mut session = support::session(42);
    let facts = support::facts(&session.cohort);
    let mut input = support::write_input(&session.cohort, &facts);
    input.triage_report = b"";
    expect_contract(&mut session.store, input);
}

fn reject_zero_generation() {
    let mut session = support::session(42);
    let facts = support::facts(&session.cohort);
    let mut input = support::write_input(&session.cohort, &facts);
    let mut receipt = support::receipt_input(1, 2);
    receipt.judge_runtime_installation_generation = 0;
    input.managed_receipt_input = &receipt;
    expect_contract(&mut session.store, input);
}

fn expect_contract(
    store: &mut ArtifactStateStore,
    input: super::super::CandidateJudgeExecutionV1Input<'_>,
) {
    let error = store
        .transact_candidate_judge_execution_v1(input, || Ok::<_, ()>(()))
        .expect_err("contract");
    assert!(matches!(
        error,
        CandidateJudgeExecutionV1TransactionError::Store(
            StoreError::InvalidGenerationQualificationPlan(_)
        )
    ));
    assert_absent(store);
}

fn commit(session: &mut Session) {
    let facts = support::facts(&session.cohort);
    session
        .store
        .transact_candidate_judge_execution_v1(
            support::write_input(&session.cohort, &facts),
            || Ok::<_, ()>(()),
        )
        .expect("insert");
}

fn assert_corrupt(session: &mut Session) {
    let facts = support::facts(&session.cohort);
    let read = session
        .store
        .candidate_judge_execution_v1(support::read_input(&session.cohort, &facts));
    assert!(matches!(read, Err(StoreError::CorruptRecord)));
}

fn assert_absent(store: &ArtifactStateStore) {
    for table in [
        "candidate_judge_plans",
        "candidate_judge_schedules",
        "candidate_judge_request_aggregates",
        "candidate_judge_response_aggregates",
        "candidate_judge_observation_batches",
        "managed_local_judge_receipts",
        "candidate_judge_join_records",
    ] {
        assert_eq!(rows(store, table), 0, "{table}");
    }
}

fn rows(store: &ArtifactStateStore, table: &str) -> i64 {
    store
        .connection()
        .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("count")
}

fn insert_plan_row(session: &Session) {
    let plan = &session.cohort.plan;
    let limits = plan.limits();
    session
        .store
        .connection()
        .execute(
            "INSERT INTO candidate_judge_plans (
                candidate_judge_plan_id, schema_version, generation_qualification_plan_id,
                generation_suite_manifest_id, generation_repetition_id, candidate_selection_policy_id,
                candidate_a_generation_system_id, candidate_b_generation_system_id,
                judge_generation_system_id, case_material_set_digest, rubric_digest,
                prompt_contract_digest, output_schema_digest, case_count, presentation_seed,
                order_policy, attempts_per_order, maximum_judge_cases, maximum_source_bytes,
                maximum_candidate_bytes, maximum_complete_input_bytes, maximum_context_tokens,
                maximum_output_tokens, maximum_response_bytes, maximum_elapsed_milliseconds,
                canonical_json
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, 'both_orders',
                ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25
            )",
            rusqlite::params![
                plan.candidate_judge_plan_id().digest().as_str(),
                i64::from(plan.schema_version()),
                plan.qualification_plan_id().digest().as_str(),
                plan.suite_manifest_id().digest().as_str(),
                plan.repetition_id().digest().as_str(),
                plan.selection_policy_id().digest().as_str(),
                plan.candidate_a_generation_system_id().digest().as_str(),
                plan.candidate_b_generation_system_id().digest().as_str(),
                plan.judge_generation_system_id().digest().as_str(),
                plan.case_material_set_digest().as_str(),
                plan.rubric_digest().as_str(),
                plan.prompt_contract_digest().as_str(),
                plan.output_schema_digest().as_str(),
                i64::try_from(plan.cases().len()).expect("case count"),
                plan.presentation_seed().to_string(),
                i64::from(plan.attempts_per_order()),
                i64::from(limits.maximum_judge_cases()),
                i64::from(limits.maximum_source_bytes()),
                i64::from(limits.maximum_candidate_bytes()),
                i64::from(limits.maximum_complete_input_bytes()),
                i64::from(limits.maximum_context_tokens()),
                i64::from(limits.maximum_output_tokens()),
                i64::from(limits.maximum_response_bytes()),
                i64::from(limits.maximum_elapsed_milliseconds()),
                serde_json::to_vec(plan).expect("plan json"),
            ],
        )
        .expect("insert plan");
}
