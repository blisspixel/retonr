use rewrite_model::{
    CandidateJudgeCaseV1, CandidateJudgePlanV1Input, CandidateJudgePlanV1Relations,
    CandidateJudgeRequestAggregateV1, CandidateJudgeScheduleV1, CandidateReceiptPairSetId,
};
use rewrite_types::CancellationToken;

use crate::candidate_judge_preparation::request::derive_request_aggregate;
use crate::generation_case_material::verified_material_test_support::Fixture;
use crate::local_judge_execution::prompt::LocalJudgeAttemptBuildError;
use crate::verified_candidate_batch_set::tests::{
    OfflinePairedAuthorityFixture, offline_judge_system, offline_paired_authorities,
};

use super::*;

#[test]
fn selected_candidate_failures_preserve_the_exact_side() {
    let material = Fixture::judge_pair();
    for (side, fail_a, fail_b) in [
        (CandidateJudgePreparationSide::CandidateA, Some(5), None),
        (CandidateJudgePreparationSide::CandidateB, None, Some(5)),
    ] {
        let mut config = ready_config(vec![0]);
        config.fail_candidate_a_on_call = fail_a;
        config.fail_candidate_b_on_call = fail_b;
        let error = with_input(&material, "request-side", config, |input| {
            let schedule = schedule(&input);
            derive_request_aggregate(
                &input,
                input.judge_plan,
                &schedule,
                &[0],
                &CancellationToken::new(),
            )
        })
        .expect_err("selected candidate revalidation must fail");
        assert!(matches!(
            error,
            CandidateJudgePreparationError::CandidateBatchSet {
                side: observed,
                ..
            } if observed == side
        ));
    }
}

#[test]
fn request_projection_closes_non_utf8_rubric_empty_set_and_cancellation() {
    let non_utf8 = Fixture::judge_non_utf8();
    let error = request_result(
        &non_utf8,
        "request-non-utf8",
        ready_config(vec![0]),
        &[0],
        &CancellationToken::new(),
    )
    .expect_err("eligible source must be UTF-8");
    assert_request_failure(
        &error,
        0,
        CandidateJudgePreparationRequestFailure::SourceNotUtf8,
    );

    let material = Fixture::judge_pair();
    let mut missing_rubric = ready_config(vec![0]);
    missing_rubric.supplied_rubric.clauses.pop();
    let error = request_result(
        &material,
        "request-rubric",
        missing_rubric,
        &[0],
        &CancellationToken::new(),
    )
    .expect_err("contract rubric subset must resolve exactly");
    assert_request_failure(
        &error,
        0,
        CandidateJudgePreparationRequestFailure::RubricMismatch,
    );

    assert!(matches!(
        request_result(
            &material,
            "request-empty",
            ready_config(vec![0]),
            &[],
            &CancellationToken::new(),
        ),
        Err(CandidateJudgePreparationError::PortableContract { .. })
    ));

    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(matches!(
        request_result(
            &material,
            "request-cancelled",
            ready_config(vec![0]),
            &[0],
            &cancelled,
        ),
        Err(CandidateJudgePreparationError::AuthorityValidation {
            cancelled: true,
            ..
        })
    ));
}

#[test]
fn every_local_builder_failure_maps_to_a_closed_request_kind() {
    for (source, expected) in [
        (
            LocalJudgeAttemptBuildError::RubricMismatch,
            CandidateJudgePreparationRequestFailure::RubricMismatch,
        ),
        (
            LocalJudgeAttemptBuildError::InputLimitExceeded,
            CandidateJudgePreparationRequestFailure::InputLimitExceeded,
        ),
        (
            LocalJudgeAttemptBuildError::PromptEncoding,
            CandidateJudgePreparationRequestFailure::PromptEncoding,
        ),
        (
            LocalJudgeAttemptBuildError::InvalidRequest,
            CandidateJudgePreparationRequestFailure::InvalidRequest,
        ),
    ] {
        assert_eq!(
            CandidateJudgePreparationRequestFailure::from(source),
            expected
        );
    }
}

fn request_result(
    material: &Fixture,
    suffix: &str,
    config: PreparationConfig,
    eligible: &[usize],
    cancellation: &CancellationToken,
) -> Result<CandidateJudgeRequestAggregateV1, CandidateJudgePreparationError> {
    with_input(material, suffix, config, |input| {
        let schedule = schedule(&input);
        derive_request_aggregate(&input, input.judge_plan, &schedule, eligible, cancellation)
    })
}

pub(in crate::candidate_judge_preparation) fn schedule(
    input: &CandidateJudgePreparationInput<'_, '_>,
) -> CandidateJudgeScheduleV1 {
    let cancellation = CancellationToken::new();
    let receipt_a = input
        .candidate_a
        .receipt_set(&cancellation)
        .expect("candidate A receipt set");
    let receipt_b = input
        .candidate_b
        .receipt_set(&cancellation)
        .expect("candidate B receipt set");
    let pair_id = CandidateReceiptPairSetId::from_receipt_sets(receipt_a, receipt_b)
        .expect("candidate receipt pair");
    CandidateJudgeScheduleV1::new(input.judge_plan, &pair_id).expect("judge schedule")
}

fn assert_request_failure(
    error: &CandidateJudgePreparationError,
    schedule_index: usize,
    expected: CandidateJudgePreparationRequestFailure,
) {
    assert!(matches!(
        error,
        CandidateJudgePreparationError::Request {
            schedule_index: observed,
            failure,
        } if *observed == schedule_index && *failure == expected
    ));
}

pub(crate) fn with_input<'store, R>(
    material_fixture: &'store Fixture,
    suffix: &str,
    config: PreparationConfig,
    action: impl for<'records> FnOnce(CandidateJudgePreparationInput<'records, 'store>) -> R,
) -> R {
    let material = material_fixture.verify().expect("material authority");
    let pair = offline_paired_authorities(&material_fixture.suite, &material_fixture.cases, suffix);
    if let Some(call) = config.fail_candidate_a_on_call {
        pair.candidate_a_control.fail_on_call(call);
    }
    if let Some(call) = config.fail_candidate_b_on_call {
        pair.candidate_b_control.fail_on_call(call);
    }
    let OfflinePairedAuthorityFixture {
        qualification_plan,
        suite,
        repetition,
        selection_policy,
        planned_attempts,
        candidate_a_system,
        candidate_b_system,
        candidate_a,
        candidate_b,
        ..
    } = pair;
    let judge_system = offline_judge_system(
        "request branch judge",
        config.prompt_digest.clone(),
        config.output_schema_digest.clone(),
    );
    let relations = CandidateJudgePlanV1Relations {
        qualification_plan: &qualification_plan,
        suite: &suite,
        repetition: &repetition,
        selection_policy: &selection_policy,
        planned_attempts: &planned_attempts,
        candidate_a_system: &candidate_a_system,
        candidate_b_system: &candidate_b_system,
        judge_system: &judge_system,
    };
    let cases = config
        .case_indices
        .iter()
        .map(|index| {
            CandidateJudgeCaseV1::new(
                material_fixture.cases[*index].case_id().clone(),
                material_fixture.contracts[*index]
                    .rubric_clause_ids()
                    .to_vec(),
            )
            .expect("candidate judge case")
        })
        .collect();
    let judge_plan = CandidateJudgePlanV1::new(
        relations,
        CandidateJudgePlanV1Input {
            case_material_set_digest: material.case_material_set_digest().clone(),
            rubric_digest: local_judge_rubric_digest(&config.plan_rubric).expect("plan rubric"),
            cases,
            order_policy: CandidateJudgeOrderPolicyV1::BothOrders,
            presentation_seed: 29,
            attempts_per_order: 1,
            limits: config.limits,
            prompt_contract_digest: config.prompt_digest,
            output_schema_digest: config.output_schema_digest,
        },
    )
    .expect("candidate judge plan");
    action(CandidateJudgePreparationInput {
        plan_relations: relations,
        judge_plan: &judge_plan,
        rubric: &config.supplied_rubric,
        candidate_a,
        candidate_b,
        case_material: material,
    })
}
