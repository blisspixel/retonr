use rewrite_inference::CandidateOutputError;
use rewrite_model::CandidateOutputCeilingsV1;

use super::super::LocalOllamaManagedGenerationError;
use super::validation::parse_response_candidates;

fn ceilings() -> CandidateOutputCeilingsV1 {
    CandidateOutputCeilingsV1::new(1, 8, 8).expect("valid single-candidate ceilings")
}

#[test]
fn response_candidates_are_derived_in_declared_order() {
    let candidates =
        parse_response_candidates(br#"{"candidates":[{"text":"answer"}]}"#, ceilings())
            .expect("valid bounded candidate envelope");
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].ordinal, 0);
    assert_eq!(candidates[0].text, "answer");
}

#[test]
fn malformed_count_and_ceiling_violations_are_response_failures() {
    for (bytes, expected) in [
        (
            br#"{"candidate":["answer"]}"#.as_slice(),
            CandidateOutputError::InvalidEnvelope,
        ),
        (
            br#"{"candidates":[]}"#.as_slice(),
            CandidateOutputError::CountMismatch,
        ),
        (
            br#"{"candidates":[{"text":"too-long!"}]}"#.as_slice(),
            CandidateOutputError::CandidateTooLarge,
        ),
    ] {
        assert!(matches!(
            parse_response_candidates(bytes, ceilings()),
            Err(LocalOllamaManagedGenerationError::ResponseValidation(error))
                if error == expected
        ));
    }
}
