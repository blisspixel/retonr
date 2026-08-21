use rewrite_inference::{InferenceErrorKind, claim_output_contract};
use rewrite_types::CancellationToken;

use super::{
    assert_session_error,
    fixture::{SessionMode, SessionServer, config, context, judge_request},
};
use crate::OllamaLimits;

#[tokio::test]
async fn judge_completion_uses_the_second_exact_profile_on_the_retained_stream() {
    let server = SessionServer::start(SessionMode::JudgeOutput).await;
    let stream = server.supplied_stream().await;
    let token = CancellationToken::new();
    let mut session = config(server.endpoint.clone(), OllamaLimits::default())
        .open(stream, context(&token), |_| Ok::<(), ()>(()))
        .await
        .expect("open retained session");
    session
        .preflight(context(&token))
        .await
        .expect("judge preflight");
    let (response, receipt) = session
        .complete_structured(judge_request("neutral judge prompt"), context(&token))
        .await
        .expect("valid judge completion");
    assert_eq!(
        response.output_json(),
        r#"{"schema_version":1,"case_id":"case_01","choice":"first","rubric_clauses":["clarity"],"source_spans":[{"start":0,"end":4}],"first_candidate_spans":[],"second_candidate_spans":[]}"#
    );
    assert_eq!(receipt.first_response_ordinal(), 8);
    assert_eq!(receipt.last_response_ordinal(), 14);
    drop(session);
    let result = server.finish().await;
    assert_eq!(result.accepts, 1);
    assert_eq!(result.requests.len(), 14);
    assert_eq!(result.generate_requests.len(), 1);
    assert_eq!(
        result.generate_requests[0]["format"]["$id"],
        "urn:retonr:local-judge-attempt-output:v1"
    );
}

#[tokio::test]
async fn invalid_judge_output_is_fully_drained_then_poisons_without_reconnect() {
    let server = SessionServer::start(SessionMode::InvalidJudgeOutput).await;
    let stream = server.supplied_stream().await;
    let token = CancellationToken::new();
    let mut session = config(server.endpoint.clone(), OllamaLimits::default())
        .open(stream, context(&token), |_| Ok::<(), ()>(()))
        .await
        .expect("open retained session");
    session
        .preflight(context(&token))
        .await
        .expect("judge preflight");
    assert_session_error(
        session
            .complete_structured(judge_request("neutral judge prompt"), context(&token))
            .await
            .expect_err("unordered judge clauses fail"),
        InferenceErrorKind::MalformedResponse,
        "invalid_local_judge_attempt_output",
    );
    assert_session_error(
        session
            .complete_structured(judge_request("second attempt"), context(&token))
            .await
            .expect_err("poisoned session stays closed"),
        InferenceErrorKind::Policy,
        "retained_session_closed",
    );
    drop(session);
    let result = server.finish().await;
    assert_eq!(result.accepts, 1);
    assert_eq!(result.requests.len(), 11);
    assert_eq!(result.generate_requests.len(), 1);
}

#[tokio::test]
async fn judge_request_validation_sends_no_network_request() {
    let server = SessionServer::start(SessionMode::JudgeOutput).await;
    let stream = server.supplied_stream().await;
    let token = CancellationToken::new();
    let mut session = config(server.endpoint.clone(), OllamaLimits::default())
        .open(stream, context(&token), |_| Ok::<(), ()>(()))
        .await
        .expect("open retained session");
    session
        .preflight(context(&token))
        .await
        .expect("judge preflight");
    let mut invalid = judge_request("neutral judge prompt");
    invalid.source_byte_limit = 0;
    assert_session_error(
        session
            .complete_structured(invalid, context(&token))
            .await
            .expect_err("invalid judge request fails before network"),
        InferenceErrorKind::Policy,
        "invalid_structured_completion_request",
    );
    drop(session);
    let result = server.finish().await;
    assert_eq!(result.accepts, 1);
    assert_eq!(result.requests.len(), 7);
    assert!(result.generate_requests.is_empty());
}

#[tokio::test]
async fn explicit_invalidation_is_idempotent_and_sends_no_later_request() {
    let server = SessionServer::start(SessionMode::JudgeOutput).await;
    let stream = server.supplied_stream().await;
    let token = CancellationToken::new();
    let mut session = config(server.endpoint.clone(), OllamaLimits::default())
        .open(stream, context(&token), |_| Ok::<(), ()>(()))
        .await
        .expect("open retained session");
    session
        .preflight(context(&token))
        .await
        .expect("judge preflight");
    session.invalidate();
    session.invalidate();
    assert_session_error(
        session
            .complete_structured(judge_request("not sent"), context(&token))
            .await
            .expect_err("invalidated session stays closed"),
        InferenceErrorKind::Policy,
        "retained_session_closed",
    );
    drop(session);
    let result = server.finish().await;
    assert_eq!(result.accepts, 1);
    assert_eq!(result.requests.len(), 7);
    assert!(result.generate_requests.is_empty());
}

#[tokio::test]
async fn retained_session_rejects_every_third_output_contract_before_generation() {
    let server = SessionServer::start(SessionMode::JudgeOutput).await;
    let stream = server.supplied_stream().await;
    let token = CancellationToken::new();
    let mut session = config(server.endpoint.clone(), OllamaLimits::default())
        .open(stream, context(&token), |_| Ok::<(), ()>(()))
        .await
        .expect("open retained session");
    session
        .preflight(context(&token))
        .await
        .expect("judge preflight");
    let mut unsupported = judge_request("not sent");
    unsupported.output = claim_output_contract();
    assert_session_error(
        session
            .complete_structured(unsupported, context(&token))
            .await
            .expect_err("third output profile is unsupported"),
        InferenceErrorKind::Compatibility,
        "unsupported_output_contract",
    );
    drop(session);
    let result = server.finish().await;
    assert_eq!(result.accepts, 1);
    assert_eq!(result.requests.len(), 7);
    assert!(result.generate_requests.is_empty());
}
