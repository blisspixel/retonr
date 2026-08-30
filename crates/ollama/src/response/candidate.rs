use rewrite_inference::{
    CandidateOutputError, CandidateOutputPolicy, GenerationCandidate, InferenceError,
    parse_candidate_output,
};

use super::malformed_error;

pub(crate) fn parse_candidates(
    bytes: &[u8],
    policy: CandidateOutputPolicy,
) -> Result<Vec<GenerationCandidate>, InferenceError> {
    parse_candidate_output(bytes, policy).map_err(candidate_output_error)
}

pub(crate) fn generation_candidate_output_policy(
    expected_count: u8,
    maximum_candidate_bytes: u64,
) -> Result<CandidateOutputPolicy, InferenceError> {
    let maximum_aggregate_candidate_bytes = maximum_candidate_bytes
        .checked_mul(u64::from(expected_count))
        .ok_or_else(|| candidate_output_error(CandidateOutputError::InvalidPolicy))?;
    CandidateOutputPolicy::new(
        expected_count,
        maximum_candidate_bytes,
        maximum_aggregate_candidate_bytes,
    )
    .map_err(candidate_output_error)
}

pub(crate) fn single_candidate_output_policy(
    maximum_output_bytes: u64,
) -> Result<CandidateOutputPolicy, InferenceError> {
    CandidateOutputPolicy::new(1, maximum_output_bytes, maximum_output_bytes)
        .map_err(candidate_output_error)
}

fn candidate_output_error(error: CandidateOutputError) -> InferenceError {
    malformed_error(error.code())
}

#[cfg(test)]
mod tests {
    use rewrite_inference::InferenceErrorKind;

    use super::{
        generation_candidate_output_policy, parse_candidates, single_candidate_output_policy,
    };

    #[test]
    fn adapter_policies_preserve_exact_counts_and_checked_bounds() {
        let generation = generation_candidate_output_policy(2, 5).expect("generation policy");
        assert_eq!(generation.expected_count(), 2);
        assert_eq!(generation.maximum_candidate_bytes(), 5);
        assert_eq!(generation.maximum_aggregate_candidate_bytes(), 10);

        let structured = single_candidate_output_policy(9).expect("structured policy");
        assert_eq!(structured.expected_count(), 1);
        assert_eq!(structured.maximum_candidate_bytes(), 9);
        assert_eq!(structured.maximum_aggregate_candidate_bytes(), 9);
    }

    #[test]
    fn adapter_policy_failures_are_redacted_and_checked() {
        for error in [
            generation_candidate_output_policy(2, u64::MAX)
                .expect_err("aggregate multiplication overflows"),
            generation_candidate_output_policy(0, 1).expect_err("zero count is rejected"),
            single_candidate_output_policy(0).expect_err("zero output limit is rejected"),
        ] {
            assert_eq!(error.kind, InferenceErrorKind::MalformedResponse);
            assert_eq!(error.code, "invalid_candidate_output_policy");
        }
    }

    #[test]
    fn provider_neutral_parser_errors_keep_one_adapter_code() {
        let policy = single_candidate_output_policy(64).expect("candidate policy");
        let error = parse_candidates(
            b"{\"candidates\":[{\"text\":\"one\"},{\"text\":\"two\"}]}",
            policy,
        )
        .expect_err("extra candidate is rejected");
        assert_eq!(error.kind, InferenceErrorKind::MalformedResponse);
        assert_eq!(error.code, "candidate_count_mismatch");
    }
}
