use rewrite_model::CandidateDeterministicEvaluationRecordV1;
use rewrite_types::CancellationToken;

use crate::candidate_judge_preparation::tests::{ready_config, request_branches};
use crate::compile_candidate_deterministic_evaluation;
use crate::generation_case_material::verified_material_test_support::Fixture;

use super::*;

#[test]
fn private_validation_rejects_each_candidate_receipt_side() {
    let fixture = Fixture::judge_pair();
    let deterministic = deterministic_record(&fixture, "private-validation-side");
    for (side, fail_a, fail_b) in [
        (CandidateJudgePreparationSide::CandidateA, Some(3), None),
        (CandidateJudgePreparationSide::CandidateB, None, Some(3)),
    ] {
        let mut config = ready_config(vec![0]);
        config.fail_candidate_a_on_call = fail_a;
        config.fail_candidate_b_on_call = fail_b;
        let error =
            request_branches::with_input(&fixture, "private-validation-side", config, |input| {
                match validate_pre_output_closure(&input, &deterministic, &CancellationToken::new())
                {
                    Ok(_) => panic!("receipt authority must fail"),
                    Err(error) => error,
                }
            });
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
fn private_validation_rejects_a_swapped_deterministic_pair() {
    let fixture = Fixture::judge_pair();
    let error = request_branches::with_input(
        &fixture,
        "private-validation-swapped",
        ready_config(vec![0]),
        |input| {
            let cancellation = CancellationToken::new();
            let swapped = compile_candidate_deterministic_evaluation(
                &input.candidate_b,
                &input.candidate_a,
                &input.case_material,
                &cancellation,
            )
            .expect("swapped deterministic record");
            match validate_pre_output_closure(&input, &swapped, &cancellation) {
                Ok(_) => panic!("swapped deterministic relationship must fail"),
                Err(error) => error,
            }
        },
    );
    assert!(matches!(
        error,
        CandidateJudgePreparationError::Relationship(
            CandidateJudgePreparationRelationship::CandidatePlanClosure
        )
    ));
}

fn deterministic_record(
    fixture: &Fixture,
    suffix: &str,
) -> CandidateDeterministicEvaluationRecordV1 {
    request_branches::with_input(fixture, suffix, ready_config(vec![0]), |input| {
        compile_candidate_deterministic_evaluation(
            &input.candidate_a,
            &input.candidate_b,
            &input.case_material,
            &CancellationToken::new(),
        )
        .expect("deterministic record")
    })
}
