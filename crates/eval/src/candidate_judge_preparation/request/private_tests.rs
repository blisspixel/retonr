use rewrite_types::CancellationToken;

use crate::candidate_judge_preparation::tests::{ready_config, request_branches};
use crate::generation_case_material::verified_material_test_support::Fixture;
use crate::local_judge_execution::prompt::LocalJudgeAttemptLimits;

use super::*;

#[derive(Clone, Copy)]
enum InvalidProjection {
    MissingCandidateA,
    MissingCandidateB,
    MissingScheduleEntry,
}

#[test]
fn private_projection_rejects_missing_candidates_and_schedule_entries() {
    for (scenario, expected_index) in [
        (InvalidProjection::MissingCandidateA, 0),
        (InvalidProjection::MissingCandidateB, 0),
        (InvalidProjection::MissingScheduleEntry, 2),
    ] {
        let error = projection_error(scenario);
        assert!(matches!(
            error,
            CandidateJudgePreparationError::Request {
                schedule_index,
                failure: CandidateJudgePreparationRequestFailure::InvalidRequest,
            } if schedule_index == expected_index
        ));
    }
}

fn projection_error(scenario: InvalidProjection) -> CandidateJudgePreparationError {
    let fixture = Fixture::judge_pair();
    request_branches::with_input(
        &fixture,
        "private-request-projection",
        ready_config(vec![0]),
        |input| {
            let cancellation = CancellationToken::new();
            let schedule = request_branches::schedule(&input);
            let mut candidate_a = input
                .candidate_a
                .selected_candidates(&cancellation)
                .expect("candidate A");
            let mut candidate_b = input
                .candidate_b
                .selected_candidates(&cancellation)
                .expect("candidate B");
            if matches!(scenario, InvalidProjection::MissingCandidateA) {
                candidate_a.clear();
            }
            if matches!(scenario, InvalidProjection::MissingCandidateB) {
                candidate_b.clear();
            }
            let eligible_semantic_indices =
                if matches!(scenario, InvalidProjection::MissingScheduleEntry) {
                    &[0, 1][..]
                } else {
                    &[0][..]
                };
            let limits = input.judge_plan.limits();
            let projection = RequestProjection {
                input: &input,
                schedule: &schedule,
                eligible_semantic_indices,
                candidate_a,
                candidate_b,
                attempt_limits: LocalJudgeAttemptLimits {
                    maximum_source_bytes: u64::from(limits.maximum_source_bytes()),
                    maximum_candidate_bytes: u64::from(limits.maximum_candidate_bytes()),
                    maximum_input_bytes: u64::from(limits.maximum_complete_input_bytes()),
                    context_token_limit: limits.maximum_context_tokens(),
                    output_token_limit: limits.maximum_output_tokens(),
                    maximum_response_bytes: u64::from(limits.maximum_response_bytes()),
                },
                cancellation: &cancellation,
            };
            let (semantic_index, source) =
                if matches!(scenario, InvalidProjection::MissingScheduleEntry) {
                    (1, b"Retain Acme 42 exactly.".as_slice())
                } else {
                    (0, b"Acme 42 needs polish.".as_slice())
                };
            projection
                .derive_case_ids(semantic_index, source)
                .expect_err("malformed private projection must fail")
        },
    )
}
