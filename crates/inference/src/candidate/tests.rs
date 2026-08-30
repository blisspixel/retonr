use super::{
    CandidateOutputError, CandidateOutputPolicy, parse_candidate_output,
    parser::checked_aggregate_byte_count,
};

fn policy(count: u8, each: u64, aggregate: u64) -> CandidateOutputPolicy {
    CandidateOutputPolicy::new(count, each, aggregate).expect("valid candidate policy")
}

#[test]
fn parses_exact_order_and_utf8_byte_counts() {
    let candidates = parse_candidate_output(
        "{\"candidates\":[{\"text\":\"first\"},{\"text\":\"é\"}]}".as_bytes(),
        policy(2, 5, 7),
    )
    .expect("bounded candidates parse");
    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].ordinal, 0);
    assert_eq!(candidates[0].text, "first");
    assert_eq!(candidates[1].ordinal, 1);
    assert_eq!(candidates[1].text, "é");
}

#[test]
fn accepts_worst_case_json_escaping_within_the_decoded_limits() {
    let candidates = parse_candidate_output(
        br#"{"candidates":[{"text":"\u0000\u0001\u0002\u0003"}]}"#,
        policy(1, 4, 4),
    )
    .expect("bounded JSON escapes parse");
    assert_eq!(candidates[0].text.as_bytes(), &[0, 1, 2, 3]);
}

#[test]
fn rejects_invalid_or_noncanonical_policy() {
    for values in std::hint::black_box([
        (0, 1, 1),
        (17, 1, 1),
        (1, 0, 1),
        (1, 1, 0),
        (2, 2, 5),
        (2, u64::MAX, 1),
        (1, u64::MAX, u64::MAX),
        (1, u64::MAX / 6, u64::MAX / 6),
    ]) {
        assert_eq!(
            CandidateOutputPolicy::new(values.0, values.1, values.2),
            Err(CandidateOutputError::InvalidPolicy)
        );
    }
    let valid = std::hint::black_box(policy(1, 2, 2));
    assert_eq!(valid.expected_count(), 1);
    assert_eq!(valid.maximum_candidate_bytes(), 2);
    assert_eq!(valid.maximum_aggregate_candidate_bytes(), 2);
    assert!(valid.maximum_envelope_bytes() > 2);
}

#[test]
fn rejects_input_above_the_derived_raw_envelope_ceiling() {
    let policy = policy(1, 1, 1);
    let excess = usize::try_from(policy.maximum_envelope_bytes() + 1).expect("fixture ceiling");
    let mut input = vec![b' '; excess];
    input.extend_from_slice(b"{\"candidates\":[{\"text\":\"\"}]}");
    assert_eq!(
        parse_candidate_output(&input, policy),
        Err(CandidateOutputError::EnvelopeTooLarge)
    );
}

#[test]
fn rejects_more_than_the_provider_neutral_candidate_maximum() {
    let items = std::iter::repeat_n("{\"text\":\"x\"}", 17)
        .collect::<Vec<_>>()
        .join(",");
    let input = format!("{{\"candidates\":[{items}]}}");
    assert_eq!(
        parse_candidate_output(input.as_bytes(), policy(16, 1, 16)),
        Err(CandidateOutputError::CountMismatch)
    );
}

#[test]
fn rejects_invalid_utf8_and_json_framing() {
    assert_eq!(
        parse_candidate_output(&[0xff], policy(1, 8, 8)),
        Err(CandidateOutputError::InvalidUtf8)
    );
    for input in [
        "not json",
        "[]",
        "{\"candidates\":{}}",
        "{\"candidates\":[[]]}",
        "{\"candidates\":[]}{\"candidates\":[]}",
        "{\"candidates\":[{\"text\":\"ok\"}]",
    ] {
        assert_eq!(
            parse_candidate_output(input.as_bytes(), policy(1, 8, 8)),
            Err(CandidateOutputError::InvalidEnvelope)
        );
    }
}

#[test]
fn rejects_unknown_duplicate_and_missing_fields() {
    for input in [
        "{}",
        "{\"other\":[]}",
        "{\"candidates\":[],\"candidates\":[]}",
        "{\"candidates\":[{}]}",
        "{\"candidates\":[{\"other\":true}]}",
        "{\"candidates\":[{\"text\":\"ok\",\"other\":true}]}",
        "{\"candidates\":[{\"text\":\"a\",\"text\":\"b\"}]}",
    ] {
        assert_eq!(
            parse_candidate_output(input.as_bytes(), policy(1, 8, 8)),
            Err(CandidateOutputError::InvalidEnvelope),
            "{input}"
        );
    }
}

#[test]
fn rejects_wrong_count_and_byte_limit_violations() {
    assert_eq!(
        parse_candidate_output(b"{\"candidates\":[]}", policy(1, 8, 8)),
        Err(CandidateOutputError::CountMismatch)
    );
    assert_eq!(
        parse_candidate_output(b"{\"candidates\":[{\"text\":\"large\"}]}", policy(1, 4, 4)),
        Err(CandidateOutputError::CandidateTooLarge)
    );
    assert_eq!(
        parse_candidate_output(
            b"{\"candidates\":[{\"text\":\"abc\"},{\"text\":\"def\"}]}",
            policy(2, 3, 5)
        ),
        Err(CandidateOutputError::AggregateTooLarge)
    );
}

#[test]
fn reports_stable_error_codes() {
    assert_eq!(
        checked_aggregate_byte_count(u64::MAX, 1),
        Err(CandidateOutputError::AggregateByteCountOverflow)
    );
    for (error, code) in [
        (
            CandidateOutputError::InvalidPolicy,
            "invalid_candidate_output_policy",
        ),
        (
            CandidateOutputError::EnvelopeTooLarge,
            "candidate_envelope_too_large",
        ),
        (
            CandidateOutputError::InvalidUtf8,
            "invalid_candidate_output_utf8",
        ),
        (
            CandidateOutputError::InvalidEnvelope,
            "invalid_candidate_envelope",
        ),
        (
            CandidateOutputError::CountMismatch,
            "candidate_count_mismatch",
        ),
        (
            CandidateOutputError::CandidateTooLarge,
            "candidate_too_large",
        ),
        (
            CandidateOutputError::AggregateByteCountOverflow,
            "candidate_byte_count_overflow",
        ),
        (
            CandidateOutputError::AggregateTooLarge,
            "candidate_aggregate_too_large",
        ),
    ] {
        assert!(!error.to_string().is_empty());
        assert_eq!(error.code(), code);
    }
}
