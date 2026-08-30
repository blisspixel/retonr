use super::*;
use crate::generation_qualification::{CandidateOutputCeilingsV1, MAX_PLANNED_GENERATION_ATTEMPTS};

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one table-driven test covers every field and V1 hard boundary"
)]
fn limits_expose_all_fields_and_enforce_every_boundary() {
    let limits = test_support::operation_limits(ATTEMPT_COUNT);
    assert_eq!(limits.maximum_source_bytes(), 1_024);
    assert_eq!(limits.maximum_complete_input_bytes(), 4_096);
    assert_eq!(limits.maximum_context_tokens(), 4_096);
    assert_eq!(limits.maximum_output_tokens(), 1_024);
    assert_eq!(limits.maximum_output_bytes(), OUTPUT_ENVELOPE_BYTES);
    assert_eq!(limits.maximum_candidates_per_completion(), 1);
    assert_eq!(limits.maximum_candidate_bytes(), CANDIDATE_BYTES);
    assert_eq!(limits.maximum_aggregate_candidate_bytes(), CANDIDATE_BYTES);
    assert_eq!(limits.maximum_predeclared_attempts(), ATTEMPT_COUNT);
    assert_eq!(limits.maximum_concurrent_attempts(), 1);
    assert_eq!(limits.maximum_elapsed_milliseconds(), 60_000);

    assert!(
        GenerationQualificationOperationLimitsV1::new(
            MAX_GENERATION_QUALIFICATION_OPERATION_INPUT_BYTES,
            MAX_GENERATION_QUALIFICATION_OPERATION_INPUT_BYTES,
            MAX_GENERATION_QUALIFICATION_OPERATION_CONTEXT_TOKENS,
            MAX_GENERATION_QUALIFICATION_OPERATION_OUTPUT_TOKENS,
            290,
            1,
            1,
            1,
            u32::try_from(MAX_PLANNED_GENERATION_ATTEMPTS).expect("hard attempt cap"),
            1,
            u32::MAX,
        )
        .is_ok()
    );
    for result in [
        limits_with(0, 4_096, 4_096, 1_024, 1_024, 1_024, ATTEMPT_COUNT, 1, 1),
        limits_with(
            MAX_GENERATION_QUALIFICATION_OPERATION_INPUT_BYTES + 1,
            MAX_GENERATION_QUALIFICATION_OPERATION_INPUT_BYTES + 1,
            4_096,
            1_024,
            1_024,
            1_024,
            ATTEMPT_COUNT,
            1,
            1,
        ),
        limits_with(2, 1, 4_096, 1_024, 1_024, 1_024, ATTEMPT_COUNT, 1, 1),
        limits_with(
            1,
            MAX_GENERATION_QUALIFICATION_OPERATION_INPUT_BYTES + 1,
            4_096,
            1_024,
            1_024,
            1_024,
            ATTEMPT_COUNT,
            1,
            1,
        ),
        limits_with(1, 1, 0, 1_024, 1_024, 1_024, ATTEMPT_COUNT, 1, 1),
        limits_with(
            1,
            1,
            MAX_GENERATION_QUALIFICATION_OPERATION_CONTEXT_TOKENS + 1,
            1_024,
            1_024,
            1_024,
            ATTEMPT_COUNT,
            1,
            1,
        ),
        limits_with(1, 1, 1, 0, 1_024, 1_024, ATTEMPT_COUNT, 1, 1),
        limits_with(
            1,
            1,
            1,
            MAX_GENERATION_QUALIFICATION_OPERATION_OUTPUT_TOKENS + 1,
            1_024,
            1_024,
            ATTEMPT_COUNT,
            1,
            1,
        ),
        limits_with(1, 1, 1, 1, 0, 1, ATTEMPT_COUNT, 1, 1),
        GenerationQualificationOperationLimitsV1::new(1, 1, 1, 1, 1, 2, 1, 1, ATTEMPT_COUNT, 1, 1),
        limits_with(1, 1, 1, 1, u64::MAX, u64::MAX, ATTEMPT_COUNT, 1, 1),
        limits_with(1, 1, 1, 1, 1, 0, ATTEMPT_COUNT, 1, 1),
        limits_with(1, 1, 1, 1, 1, 1, 0, 1, 1),
        limits_with(1, 1, 1, 1, 1, 1, ATTEMPT_COUNT, 0, 1),
        limits_with(1, 1, 1, 1, 1, 1, ATTEMPT_COUNT, 2, 1),
        limits_with(1, 1, 1, 1, 1, 1, ATTEMPT_COUNT, 1, 0),
        GenerationQualificationOperationLimitsV1::new(
            1,
            1,
            1,
            1,
            291,
            1,
            1,
            1,
            ATTEMPT_COUNT,
            1,
            1,
        ),
        GenerationQualificationOperationLimitsV1::new(
            1,
            1,
            1,
            1,
            MAX_GENERATION_QUALIFICATION_OPERATION_OUTPUT_BYTES + 1,
            1,
            1,
            1,
            ATTEMPT_COUNT,
            1,
            1,
        ),
    ] {
        assert_eq!(
            result,
            Err(GenerationQualificationOperationContractError::InvalidLimits)
        );
    }
    assert_eq!(
        limits_with(
            1,
            1,
            1,
            1,
            1,
            1,
            u32::try_from(MAX_PLANNED_GENERATION_ATTEMPTS).expect("hard attempt cap") + 1,
            1,
            1,
        ),
        Err(GenerationQualificationOperationContractError::InvalidLimits)
    );
}

#[test]
fn direct_deserialization_cannot_bypass_limit_validation() {
    let limits = test_support::operation_limits(ATTEMPT_COUNT);
    let canonical = serde_json::to_vec(&limits).expect("serialize valid limits");
    assert_eq!(
        serde_json::from_slice::<GenerationQualificationOperationLimitsV1>(&canonical)
            .expect("deserialize valid limits"),
        limits
    );

    let invalid = canonical
        .windows(b"\"maximum_elapsed_milliseconds\":60000".len())
        .position(|window| window == b"\"maximum_elapsed_milliseconds\":60000")
        .map(|offset| {
            let mut bytes = canonical.clone();
            bytes.splice(
                offset..offset + b"\"maximum_elapsed_milliseconds\":60000".len(),
                b"\"maximum_elapsed_milliseconds\":0".iter().copied(),
            );
            bytes
        })
        .expect("elapsed field is present");
    assert!(serde_json::from_slice::<GenerationQualificationOperationLimitsV1>(&invalid).is_err());
}

#[expect(
    clippy::too_many_arguments,
    reason = "test helper mirrors the limits contract"
)]
fn limits_with(
    source: u64,
    complete: u64,
    context: u32,
    output_tokens: u32,
    candidate: u64,
    aggregate: u64,
    attempts: u32,
    concurrent: u32,
    elapsed: u32,
) -> Result<GenerationQualificationOperationLimitsV1, GenerationQualificationOperationContractError>
{
    let envelope = CandidateOutputCeilingsV1::new(1, candidate, aggregate)
        .map_or(1, CandidateOutputCeilingsV1::maximum_envelope_bytes);
    GenerationQualificationOperationLimitsV1::new(
        source,
        complete,
        context,
        output_tokens,
        envelope,
        1,
        candidate,
        aggregate,
        attempts,
        concurrent,
        elapsed,
    )
}
