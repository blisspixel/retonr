//! Synthetic authorities exercise real schema transactions without managed traffic.

use super::*;
use crate::generation_qualification_preregistration::active::deterministic::tests::{
    SyntheticPair, material::MaterialFixture,
};
use crate::generation_qualification_preregistration::active::receipt_set::tests::positive::prepare_active;
use crate::{
    CandidateJudgePreparationInput, CandidateJudgePreparationOutcome,
    GenerationQualificationOperationDraft, LocalJudgeRubric, LocalJudgeRubricClause,
    prepare_candidate_judge,
};
use rewrite_app::{
    GenerationQualificationLicenseAssessmentCompiler, SyntheticGenerationQualificationScenario,
    with_synthetic_generation_qualification_fixture,
};
use rewrite_model::{
    CandidateJudgeCaseV1, CandidateJudgeLimitsV1, CandidateJudgeOrderPolicyV1,
    CandidateJudgePlanV1, CandidateJudgePlanV1Input, CandidateJudgePlanV1Relations,
    GenerationSystemRecordV1,
};
use rewrite_model_store::{GenerationSystemFoundationV1Input, WriteDisposition};

#[test]
fn exact_live_projection_settles_replays_and_rejects_missing_parents_abort_and_corruption() {
    exercise_scenarios(&[
        "none",
        "repeatability_none",
        "repeatability_result_corruption",
        "repeatability_manifest_corruption",
        "repeatability_manifest_abort",
        "repeatability_manifest_tamper",
        "repeatability_missing",
        "repeatability_corrupt_parent",
        "repeatability_foreign",
        "repeatability_unsettled",
        "repeatability_substituted_execution",
        "repeatability_wrong_execution",
        "repeatability_abort",
        "repeatability_cancel",
        "repeatability_postcommit_finalization",
        "repeatability_postcommit_cancel",
        "missing",
        "abort",
        "corrupt",
        "not_executed",
        "foreign",
        "foreign_subject",
        "finalization",
        "cancel",
    ]);
}

#[test]
fn resource_phase_publication_passes_fails_and_refuses_corruption_and_foreign_subjects() {
    exercise_scenarios(&[
        "resource_passed",
        "resource_failed",
        "resource_abort",
        "resource_tamper",
        "resource_corrupt_parent",
        "resource_missing_repeatability",
        "resource_foreign",
        "resource_unsettled",
        "resource_substituted_execution",
        "resource_cancel",
        "resource_postcommit_finalization",
        "resource_postcommit_cancel",
        "resource_deadline",
    ]);
}

#[expect(
    clippy::too_many_lines,
    reason = "one synthetic retained authority fixture exercises exact durable cohort boundaries"
)]
fn exercise_scenarios(failures: &[&str]) {
    for failure in failures.iter().copied() {
        with_synthetic_generation_qualification_fixture(
            SyntheticGenerationQualificationScenario::TrafficEligible,
            |mut input, platform_owners, license_proof, license_policy, production_policy| {
                // Declare the test's complete budget before starting the real
                // operation. Slow instrumented storage must still use every gate.
                let mut limits = serde_json::to_value(input.operation_policy_input.limits)
                    .expect("synthetic limits");
                limits["maximum_elapsed_milliseconds"] = serde_json::json!(120_000);
                input.operation_policy_input.limits =
                    serde_json::from_value(limits).expect("bounded synthetic elapsed budget");
                let source = input.case_authorities[0]
                    .source
                    .with_source_bytes(&CancellationToken::new(), <[u8]>::to_vec)
                    .expect("source");
                let material_fixture =
                    MaterialFixture::new(input.foundation.plan_foundation, &source);
                let material = material_fixture.verify();
                prepare_active!(
                    input,
                    platform_owners,
                    license_proof,
                    license_policy,
                    production_policy,
                    directory,
                    repository,
                    active,
                    cancellation,
                    evidence
                );
                let mut pair = SyntheticPair::new(
                    input.foundation.plan_foundation,
                    input.operation_policy_relations,
                    &active,
                    "qualification-closure-passing",
                );
                if failure.starts_with("resource_") {
                    pair.attach_resource_observations(
                        input.foundation.plan_foundation,
                        input.operation_policy_relations,
                        &active,
                        failure == "resource_failed",
                    );
                }
                let repeatability_failure_control = pair.target_failure_control();
                let (target, baseline) = pair.bind(
                    &mut active,
                    directory.path(),
                    &evidence,
                    &cancellation,
                    true,
                );
                if failure != "missing" {
                    active
                        .persist_candidate_deterministic_evaluation(
                            &mut repository,
                            &target,
                            &baseline,
                            &material,
                            &cancellation,
                        )
                        .expect("deterministic parents");
                }
                let base = input.operation_policy_relations.baseline_system;
                let judge_json = serde_json::to_string(base.generation_system)
                    .expect("canonical synthetic judge source")
                    .replace(
                        &format!(
                            "\"prompt_digest\":\"{}\"",
                            base.generation_system.prompt_digest()
                        ),
                        &format!(
                            "\"prompt_digest\":\"{}\"",
                            crate::local_judge_prompt_contract_digest()
                        ),
                    )
                    .replace(
                        &format!(
                            "\"output_schema_digest\":\"{}\"",
                            base.generation_system.output_schema_digest()
                        ),
                        &format!(
                            "\"output_schema_digest\":\"{}\"",
                            rewrite_inference::local_judge_attempt_output_contract().schema_digest
                        ),
                    );
                let judge = GenerationSystemRecordV1::from_json_bytes(
                    judge_json.as_bytes(),
                    base.relations,
                )
                .expect("exact synthetic judge");
                rewrite_model_store::ArtifactStateStore::open(
                    &directory.path().join("qualification.db"),
                )
                .expect("synthetic judge store")
                .transact_generation_system_foundation_v1(
                    GenerationSystemFoundationV1Input {
                        generation_system: &judge,
                        relations: base.relations,
                    },
                    |_| Ok::<_, ()>(()),
                )
                .expect("real judge foundation transaction");
                let foundation = input.foundation.plan_foundation;
                let relations = CandidateJudgePlanV1Relations {
                    qualification_plan: foundation.plan,
                    suite: foundation.suite,
                    repetition: &foundation.repetitions[0],
                    selection_policy: foundation.candidate_selection_policy,
                    planned_attempts: foundation.planned_attempts,
                    candidate_a_system: input
                        .operation_policy_relations
                        .target_system
                        .generation_system,
                    candidate_b_system: base.generation_system,
                    judge_system: &judge,
                };
                let rubric = LocalJudgeRubric {
                    schema_version: crate::LOCAL_JUDGE_RUBRIC_SCHEMA_VERSION,
                    clauses: vec![LocalJudgeRubricClause {
                        id: "fidelity".to_owned(),
                        instruction: "Preserve meaning exactly.".to_owned(),
                    }],
                };
                let plan = CandidateJudgePlanV1::new(
                    relations,
                    CandidateJudgePlanV1Input {
                        case_material_set_digest: material.case_material_set_digest().clone(),
                        rubric_digest: crate::local_judge_rubric_digest(&rubric)
                            .expect("rubric digest"),
                        cases: vec![
                            CandidateJudgeCaseV1::new(
                                foundation.cases[0].case_id().clone(),
                                foundation.deterministic_case_contracts[0]
                                    .rubric_clause_ids()
                                    .to_vec(),
                            )
                            .expect("judge case"),
                        ],
                        order_policy: CandidateJudgeOrderPolicyV1::BothOrders,
                        presentation_seed: 29,
                        attempts_per_order: 1,
                        limits: CandidateJudgeLimitsV1::new(
                            1, 4096, 4096, 32768, 8192, 512, 4096, 30000,
                        )
                        .expect("judge limits"),
                        prompt_contract_digest: crate::local_judge_prompt_contract_digest(),
                        output_schema_digest:
                            rewrite_inference::local_judge_attempt_output_contract().schema_digest,
                    },
                )
                .expect("judge plan");
                let CandidateJudgePreparationOutcome::Ready(ready) = prepare_candidate_judge(
                    CandidateJudgePreparationInput {
                        plan_relations: relations,
                        judge_plan: &plan,
                        rubric: &rubric,
                        candidate_a: target,
                        candidate_b: baseline,
                        case_material: material,
                    },
                    &cancellation,
                )
                .expect("judge preparation") else {
                    panic!("passing synthetic candidates");
                };
                let handoff = ready.into_runner_handoff(&cancellation).expect("handoff");
                let mut join = crate::local_ollama_managed_preflight::synthetic_join(handoff);
                // Private test fixture stands in for one successful consumed managed
                // call. No production constructor can seed this execution identity.
                if failure != "not_executed" {
                    active.next_judge_repetition = 1;
                    active
                        .executed_judge_joins
                        .push(join.record().candidate_judge_join_id().clone());
                }
                if failure == "foreign" {
                    active.executed_judge_joins[0] = serde_json::from_str(&format!(
                        "\"{}\"",
                        rewrite_types::Digest::sha256(b"foreign successful execution")
                    ))
                    .expect("foreign inert join identity");
                }
                if failure == "foreign_subject" {
                    active.subject = crate::active_generation_qualification_subject::ActiveGenerationQualificationSubject::new();
                }
                let db = directory.path().join("qualification.db");
                let connection = rusqlite::Connection::open(&db).expect("fixture state");
                connection
                    .pragma_update(None, "foreign_keys", true)
                    .expect("foreign keys");
                if failure == "abort" {
                    connection.execute_batch("CREATE TRIGGER abort_judge BEFORE INSERT ON candidate_judge_join_records BEGIN SELECT RAISE(ABORT, 'synthetic cohort failure'); END;").expect("fixture abort");
                }
                if failure == "finalization" {
                    material_fixture.invalidate_source_installation();
                }
                if failure == "cancel" {
                    cancellation.cancel();
                }
                let result = active.persist_candidate_judge_execution(
                    &mut repository,
                    &mut join,
                    &cancellation,
                );
                if matches!(
                    failure,
                    "missing"
                        | "abort"
                        | "not_executed"
                        | "foreign"
                        | "foreign_subject"
                        | "finalization"
                        | "cancel"
                ) {
                    assert!(result.is_err(), "{failure}");
                    assert_eq!(
                        connection
                            .query_row(
                                "SELECT count(*) FROM candidate_judge_join_records",
                                [],
                                |row| row.get::<_, i64>(0)
                            )
                            .expect("join count"),
                        0
                    );
                    for table in COHORT_TABLES {
                        assert_eq!(
                            connection
                                .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| row
                                    .get::<_, i64>(
                                    0
                                ))
                                .expect("rollback cohort count"),
                            0
                        );
                    }
                    return;
                }
                let (stored, disposition) = result.expect("settled real seven-row cohort");
                assert_eq!(stored.join(), join.record());
                assert_eq!(disposition.join, WriteDisposition::Inserted);
                for table in COHORT_TABLES {
                    assert_eq!(
                        connection
                            .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| row
                                .get::<_, i64>(0))
                            .expect("cohort count"),
                        1
                    );
                }
                assert_eq!(active.next_judge_settlement, 1);
                if failure.starts_with("resource_") {
                    super::super::resource_settlement::tests::exercise(
                        &mut active,
                        &mut repository,
                        join,
                        &db,
                        failure,
                        &cancellation,
                        &repeatability_failure_control,
                    );
                    return;
                }
                if failure.starts_with("repeatability_") {
                    super::super::repeatability_settlement::tests::exercise(
                        &mut active,
                        &mut repository,
                        join,
                        &db,
                        failure,
                        &cancellation,
                        &repeatability_failure_control,
                    );
                    return;
                }
                if failure == "corrupt" {
                    connection
                        .execute(
                            "UPDATE candidate_judge_join_records SET canonical_json = ?1",
                            [b"{}".as_slice()],
                        )
                        .expect("corrupt inert fixture row");
                    assert!(
                        active
                            .persist_candidate_judge_execution(
                                &mut repository,
                                &mut join,
                                &cancellation
                            )
                            .is_err()
                    );
                    assert!(active.terminal);
                } else {
                    let mut reopened = GenerationQualificationPreregistrationRepository::open(&db)
                        .expect("cold reopened repository");
                    let (replayed, replay_disposition) = active
                        .persist_candidate_judge_execution(&mut reopened, &mut join, &cancellation)
                        .expect("immutable replay");
                    assert_eq!(replayed, stored);
                    assert_eq!(replay_disposition.join, WriteDisposition::AlreadyPresent);
                    assert_eq!(active.next_judge_settlement, 1);
                }
            },
        );
    }
}

const COHORT_TABLES: [&str; 7] = [
    "candidate_judge_plans",
    "candidate_judge_schedules",
    "candidate_judge_request_aggregates",
    "candidate_judge_response_aggregates",
    "candidate_judge_observation_batches",
    "managed_local_judge_receipts",
    "candidate_judge_join_records",
];

#[test]
fn terminal_gate_has_deadline_and_cancellation_priority_with_finalization_failures() {
    use ActiveGenerationQualificationJudgeSettlementError as Error;
    for primary in [Ok(7), Err(Error::OperationScope)] {
        assert_eq!(
            finish_settlement(primary, true, Err(Error::DeadlineExceeded)),
            Err(Error::DeadlineAndFinalization)
        );
        assert_eq!(
            finish_settlement(primary, true, Err(Error::Cancelled)),
            Err(Error::CancelledAndFinalization)
        );
        assert_eq!(
            finish_settlement(primary, false, Err(Error::DeadlineExceeded)),
            Err(Error::DeadlineExceeded)
        );
        assert_eq!(
            finish_settlement(primary, false, Err(Error::Cancelled)),
            Err(Error::Cancelled)
        );
    }
    assert_eq!(
        finish_settlement(Ok(7), true, Ok(())),
        Err(Error::MandatoryFinalization)
    );
    assert_eq!(
        finish_settlement::<u8>(Err(Error::Publication), true, Ok(())),
        Err(Error::PrimaryAndFinalization)
    );
    assert_eq!(finish_settlement(Ok(7), false, Ok(())), Ok(7));
    assert_eq!(
        map_preparation_error(GenerationQualificationPreparationError::Cancelled),
        Error::Cancelled
    );
    assert_eq!(
        map_preparation_error(GenerationQualificationPreparationError::DeadlineExceeded),
        Error::DeadlineExceeded
    );
    assert!(!format!("{:?} {}", Error::Publication, Error::Publication).contains("Retain Acme"));
}
