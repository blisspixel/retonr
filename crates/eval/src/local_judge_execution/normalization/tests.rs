use rewrite_inference::{LocalJudgeAttemptOutput, LocalJudgeChoice, StructuredCompletionResponse};
use rewrite_model::{
    CandidateJudgeChoiceV1, CandidateJudgePresentationV1, OllamaRetainedSessionResponseId,
};
use rewrite_types::Digest;

use super::*;

#[path = "tests/support.rs"]
mod support;

use support::{
    NormalizationFixture, response as structured_response, response_id as retained_response_id,
    span,
};

#[test]
fn every_choice_and_both_presentations_normalize_to_frozen_observations() {
    let fixture = NormalizationFixture::new();
    let choices = [
        (LocalJudgeChoice::First, CandidateJudgeChoiceV1::First),
        (LocalJudgeChoice::Second, CandidateJudgeChoiceV1::Second),
        (LocalJudgeChoice::Tie, CandidateJudgeChoiceV1::Tie),
        (LocalJudgeChoice::Abstain, CandidateJudgeChoiceV1::Abstain),
    ];
    let mut frozen = None;
    for cursor in 0..fixture.schedule.entries().len() {
        for (local, normalized) in choices {
            let response = fixture.response(cursor, &fixture.output(local));
            let response_id = retained_response_id(&response);
            let observation =
                normalize_candidate_judge_attempt(fixture.input(cursor, &response, &response_id))
                    .expect("normalized observation");
            assert_eq!(observation.choice(), normalized);
            assert_eq!(
                observation.presentation(),
                fixture.schedule.entries()[cursor].presentation()
            );
            assert_eq!(
                observation.case_id(),
                fixture.schedule.entries()[cursor].case_id()
            );
            assert_eq!(observation.retained_session_response_id(), &response_id);
            assert_eq!(observation.cited_rubric_clause_ids(), ["fidelity"]);
            if cursor == 0 && local == LocalJudgeChoice::First {
                frozen = Some(observation.observation_id().digest().as_str().to_owned());
            }
        }
    }
    assert_eq!(
        frozen.as_deref(),
        Some("69860bbf5deb30a32c46b7d922dab0f2e34ef0394da56796fb32c78e03998af6")
    );
    assert_ne!(
        fixture.schedule.entries()[0].presentation(),
        fixture.schedule.entries()[1].presentation()
    );
}

#[test]
fn malformed_foreign_case_clause_and_admission_sets_fail_closed() {
    let fixture = NormalizationFixture::new();
    let malformed = StructuredCompletionResponse::complete(
        &fixture.requests[0],
        runtime(),
        fixture.requests[0].artifact_id.clone(),
        fixture.requests[0].artifact_digest.clone(),
        "{}".to_owned(),
        usage(),
    )
    .expect("structurally complete JSON response");
    let malformed_id = retained_response_id(&malformed);
    assert!(matches!(
        normalize_candidate_judge_attempt(fixture.input(0, &malformed, &malformed_id)),
        Err(CandidateJudgeAttemptNormalizationError::InvalidOutput { .. })
    ));

    let mut foreign_case = fixture.output(LocalJudgeChoice::First);
    foreign_case.case_id = "foreign-case".to_owned();
    assert_relationship(
        &fixture,
        &foreign_case,
        &CandidateJudgeAttemptNormalizationError::CaseKeyRelationship,
    );
    let mut foreign_clause = fixture.output(LocalJudgeChoice::First);
    foreign_clause.rubric_clauses = vec!["foreign-clause".to_owned()];
    assert_relationship(
        &fixture,
        &foreign_clause,
        &CandidateJudgeAttemptNormalizationError::RubricClauseRelationship,
    );

    let output = fixture.output(LocalJudgeChoice::First);
    let response = fixture.response(0, &output);
    let response_id = retained_response_id(&response);

    let mut missing_admission = fixture.input(0, &response, &response_id);
    let missing = vec!["fidelity".to_owned()];
    missing_admission.admitted_rubric_clause_ids = &missing;
    assert!(matches!(
        normalize_candidate_judge_attempt(missing_admission),
        Err(CandidateJudgeAttemptNormalizationError::RubricClauseRelationship)
    ));

    let mut foreign_admission = fixture.input(0, &response, &response_id);
    let foreign = vec!["fidelity".to_owned(), "foreign-clause".to_owned()];
    foreign_admission.admitted_rubric_clause_ids = &foreign;
    assert!(matches!(
        normalize_candidate_judge_attempt(foreign_admission),
        Err(CandidateJudgeAttemptNormalizationError::RubricClauseRelationship)
    ));

    let mut input = fixture.input(0, &response, &response_id);
    let unsorted = vec!["protected-values".to_owned(), "fidelity".to_owned()];
    input.admitted_rubric_clause_ids = &unsorted;
    assert!(matches!(
        normalize_candidate_judge_attempt(input),
        Err(CandidateJudgeAttemptNormalizationError::RubricClauseRelationship)
    ));
}

#[test]
fn span_edges_require_exact_lengths_and_utf8_boundaries() {
    let fixture = NormalizationFixture::new();
    let mut output = fixture.output(LocalJudgeChoice::Tie);
    output.source_spans = vec![span(0, 2), span(2, 3)];
    output.first_candidate_spans = vec![span(0, 2)];
    output.second_candidate_spans = vec![span(0, 3)];
    let response = fixture.response(0, &output);
    let response_id = retained_response_id(&response);
    let mut input = fixture.input(0, &response, &response_id);
    input.source = "éx";
    input.presented_first = "é";
    input.presented_second = "éx";
    normalize_candidate_judge_attempt(input).expect("exact UTF-8 boundaries");

    let cases = [
        (
            vec![span(1, 2)],
            vec![span(0, 2)],
            vec![span(0, 3)],
            "éx",
            "é",
            "éx",
            "source",
        ),
        (
            vec![span(0, 2)],
            vec![span(0, 3)],
            vec![span(0, 3)],
            "é",
            "é",
            "éx",
            "first",
        ),
        (
            vec![span(0, 2)],
            vec![span(0, 2)],
            vec![span(1, 2)],
            "é",
            "é",
            "éx",
            "second",
        ),
    ];
    for (source_spans, first_spans, second_spans, source, first, second, expected) in cases {
        let mut output = fixture.output(LocalJudgeChoice::Tie);
        output.source_spans = source_spans;
        output.first_candidate_spans = first_spans;
        output.second_candidate_spans = second_spans;
        let response = fixture.response(0, &output);
        let response_id = retained_response_id(&response);
        let mut input = fixture.input(0, &response, &response_id);
        input.source = source;
        input.presented_first = first;
        input.presented_second = second;
        let error = normalize_candidate_judge_attempt(input).expect_err("invalid span");
        assert!(matches!(
            (expected, error),
            (
                "source",
                CandidateJudgeAttemptNormalizationError::SourceSpans
            ) | (
                "first",
                CandidateJudgeAttemptNormalizationError::FirstCandidateSpans
            ) | (
                "second",
                CandidateJudgeAttemptNormalizationError::SecondCandidateSpans
            )
        ));
    }
}

#[test]
fn schedule_cursor_request_and_response_substitution_are_rejected() {
    let fixture = NormalizationFixture::new();
    let output = fixture.output(LocalJudgeChoice::First);
    let response = fixture.response(0, &output);
    let response_id = retained_response_id(&response);

    let mut invalid_cursor = fixture.input(0, &response, &response_id);
    invalid_cursor.schedule_cursor = fixture.schedule.entries().len();
    assert!(matches!(
        normalize_candidate_judge_attempt(invalid_cursor),
        Err(CandidateJudgeAttemptNormalizationError::ScheduleRelationship)
    ));

    let mut wrong_presentation = fixture.input(0, &response, &response_id);
    wrong_presentation.presentation = match wrong_presentation.presentation {
        CandidateJudgePresentationV1::CandidateAFirst => {
            CandidateJudgePresentationV1::CandidateBFirst
        }
        CandidateJudgePresentationV1::CandidateBFirst => {
            CandidateJudgePresentationV1::CandidateAFirst
        }
    };
    assert!(matches!(
        normalize_candidate_judge_attempt(wrong_presentation),
        Err(CandidateJudgeAttemptNormalizationError::ScheduleRelationship)
    ));

    let foreign_fixture = NormalizationFixture::foreign();
    let foreign_schedule = &foreign_fixture.schedule;
    let mut foreign_schedule_input = fixture.input(0, &response, &response_id);
    foreign_schedule_input.schedule = foreign_schedule;
    assert!(matches!(
        normalize_candidate_judge_attempt(foreign_schedule_input),
        Err(CandidateJudgeAttemptNormalizationError::ScheduleRelationship)
    ));

    let mut wrong_request = fixture.input(0, &response, &response_id);
    wrong_request.request = &fixture.requests[1];
    assert!(matches!(
        normalize_candidate_judge_attempt(wrong_request),
        Err(CandidateJudgeAttemptNormalizationError::ScheduleRelationship)
    ));

    let foreign_id =
        OllamaRetainedSessionResponseId::from_derived_digest(Digest::sha256(b"foreign response"));
    assert!(matches!(
        normalize_candidate_judge_attempt(fixture.input(0, &response, &foreign_id)),
        Err(CandidateJudgeAttemptNormalizationError::ResponseRelationship)
    ));

    let foreign_response = structured_response(&fixture.requests[1], &output);
    let foreign_response_id = retained_response_id(&foreign_response);
    assert!(matches!(
        normalize_candidate_judge_attempt(fixture.input(
            0,
            &foreign_response,
            &foreign_response_id,
        )),
        Err(CandidateJudgeAttemptNormalizationError::ResponseRelationship)
    ));
}

#[test]
fn every_error_debug_view_is_redacted() {
    let errors = [
        CandidateJudgeAttemptNormalizationError::InvalidOutput {
            source: rewrite_inference::LocalJudgeAttemptOutputError::InvalidJson,
        },
        CandidateJudgeAttemptNormalizationError::ScheduleRelationship,
        CandidateJudgeAttemptNormalizationError::ResponseRelationship,
        CandidateJudgeAttemptNormalizationError::CaseKeyRelationship,
        CandidateJudgeAttemptNormalizationError::RubricClauseRelationship,
        CandidateJudgeAttemptNormalizationError::SourceSpans,
        CandidateJudgeAttemptNormalizationError::FirstCandidateSpans,
        CandidateJudgeAttemptNormalizationError::SecondCandidateSpans,
        CandidateJudgeAttemptNormalizationError::PortableContract {
            source: rewrite_model::GenerationQualificationContractError::InvalidEncoding,
        },
    ];
    for error in errors {
        let debug = format!("{error:?}");
        assert!(debug.contains("kind"));
        assert!(!debug.contains("Acme"));
        assert!(!debug.contains("foreign-clause"));
    }
}

fn assert_relationship(
    fixture: &NormalizationFixture,
    output: &LocalJudgeAttemptOutput,
    expected: &CandidateJudgeAttemptNormalizationError,
) {
    let response = fixture.response(0, output);
    let response_id = retained_response_id(&response);
    let error = normalize_candidate_judge_attempt(fixture.input(0, &response, &response_id))
        .expect_err("relationship must fail");
    assert_eq!(
        std::mem::discriminant(&error),
        std::mem::discriminant(expected)
    );
}

fn normalize_candidate_judge_attempt(
    input: CandidateJudgeAttemptNormalizationInput<'_>,
) -> Result<CandidateJudgeObservationV1, CandidateJudgeAttemptNormalizationError> {
    super::normalize_candidate_judge_attempt(&input)
}

fn runtime() -> rewrite_model::RuntimeIdentity {
    rewrite_model::RuntimeIdentity {
        backend: "normalization-test".to_owned(),
        version: "1".to_owned(),
        digest: None,
    }
}

const fn usage() -> rewrite_inference::UsageObservation {
    rewrite_inference::UsageObservation {
        input_tokens: None,
        output_tokens: None,
        generation_micros: None,
    }
}
