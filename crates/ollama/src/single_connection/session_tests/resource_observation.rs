use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use rewrite_inference::{
    InferenceErrorKind, OperationContext, StructuredCompletionRequest, StructuredCompletionResponse,
};
use rewrite_types::CancellationToken;

use super::{OllamaResponseObservationPhase, assert_session_error};
use crate::{
    OllamaLimits, OllamaResidentResourceObservedCompletion,
    derive_ollama_retained_session_response_id,
};

use super::fixture::{SessionMode, SessionServer, config, context, request};

const COMPLETE_METRICS: &str = concat!(
    "\"total_duration\":9000,",
    "\"load_duration\":1000,",
    "\"prompt_eval_count\":4,",
    "\"prompt_eval_duration\":2000,",
    "\"eval_count\":2,",
    "\"eval_duration\":3000"
);

const MAX_METRICS: &str = concat!(
    "\"total_duration\":18446744073709551615,",
    "\"load_duration\":18446744073709551614,",
    "\"prompt_eval_count\":18446744073709551613,",
    "\"prompt_eval_duration\":18446744073709551612,",
    "\"eval_count\":18446744073709551611,",
    "\"eval_duration\":18446744073709551610"
);

mod binding;

#[tokio::test]
async fn exact_u64_values_bind_request_response_ordinal_and_checkpoint() {
    let server = SessionServer::start(SessionMode::ResidentResource {
        metrics: MAX_METRICS,
        delayed_body: false,
    })
    .await;
    let stream = server.supplied_stream().await;
    let token = CancellationToken::new();
    let phases = Arc::new(std::sync::Mutex::new(Vec::new()));
    let observed = Arc::clone(&phases);
    let mut session = config(server.endpoint.clone(), OllamaLimits::default())
        .open(stream, context(&token), move |observation| {
            observed
                .lock()
                .expect("phase lock")
                .push(observation.phase());
            Ok::<(), ()>(())
        })
        .await
        .expect("open resource session");
    session
        .preflight(context(&token))
        .await
        .expect("resource preflight");
    let exact_request = request("resource exactness");
    let start = Instant::now();
    let evidence = session
        .complete_structured_with_residency_and_resource_observation_borrowed(
            &exact_request,
            context(&token),
        )
        .await
        .expect("resource observation");
    let response = evidence.response();
    let receipt = evidence.resident_execution_receipt();

    assert_eq!(evidence.total_duration_nanoseconds(), u64::MAX);
    assert_eq!(evidence.load_duration_nanoseconds(), u64::MAX - 1);
    assert_eq!(evidence.prompt_token_count(), u64::MAX - 2);
    assert_eq!(
        evidence.prompt_evaluation_duration_nanoseconds(),
        u64::MAX - 3
    );
    assert_eq!(evidence.generated_token_count(), u64::MAX - 4);
    assert_eq!(evidence.evaluation_duration_nanoseconds(), u64::MAX - 5);
    evidence.verify_completion_binding().expect("exact binding");
    assert_eq!(
        receipt.execution().retained_response_id(),
        derive_ollama_retained_session_response_id(response)
    );
    assert_eq!(receipt.execution().first_response_ordinal(), 8);
    assert_eq!(receipt.execution().last_response_ordinal(), 16);
    assert!(evidence.response_head_elapsed_since(start).is_ok());
    assert_eq!(
        response.usage().generation_micros,
        Some((u64::MAX - 5) / 1_000)
    );
    assert_ne!(
        response
            .usage()
            .generation_micros
            .map(|value| value * 1_000),
        Some(evidence.evaluation_duration_nanoseconds())
    );

    #[cfg(feature = "test-support")]
    assert_test_support_substitutions(&exact_request, &evidence);
    let later = Instant::now() + Duration::from_millis(1);
    let reversed = evidence
        .response_head_elapsed_since(later)
        .expect_err("later start is reversed");
    assert_eq!(
        format!("{reversed:?}"),
        "OllamaGenerateResourceObservationError"
    );
    assert_eq!(reversed.to_string(), "Ollama resource observation failed");
    assert_eq!(
        format!("{evidence:?}"),
        "OllamaResidentResourceObservedCompletion { content: \"redacted\" }"
    );
    assert!(!format!("{evidence:?}").contains("184467"));

    drop(session);
    let result = server.finish().await;
    assert_eq!(result.accepts, 1);
    assert_eq!(result.requests.len(), 16);
    assert_eq!(result.generate_requests.len(), 1);
    assert_eq!(
        *phases.lock().expect("phase lock"),
        std::iter::once(OllamaResponseObservationPhase::BeforeResponses)
            .chain(
                (1..=16)
                    .map(|ordinal| { OllamaResponseObservationPhase::AfterResponse { ordinal } })
            )
            .collect::<Vec<_>>()
    );
}

#[cfg(feature = "test-support")]
fn assert_test_support_substitutions(
    exact_request: &StructuredCompletionRequest,
    evidence: &OllamaResidentResourceObservedCompletion,
) {
    let response = evidence.response();
    let receipt = evidence.resident_execution_receipt();
    let inert = OllamaResidentResourceObservedCompletion::for_test(
        response.clone(),
        receipt.clone(),
        11,
        Instant::now(),
        1,
        2,
        3,
        4,
        5,
        6,
    )
    .expect("inert exact binding");
    inert
        .verify_completion_binding()
        .expect("inert exact binding");
    let substituted_request = request("substituted request");
    let wrong_request_response = StructuredCompletionResponse::complete(
        &substituted_request,
        response.runtime().clone(),
        response.artifact_id().clone(),
        response.artifact_digest().clone(),
        response.output_json().to_owned(),
        response.usage(),
    )
    .expect("substituted request response");
    assert!(
        OllamaResidentResourceObservedCompletion::for_test(
            wrong_request_response,
            receipt.clone(),
            11,
            Instant::now(),
            1,
            2,
            3,
            4,
            5,
            6,
        )
        .is_err()
    );
    let wrong_output_response = StructuredCompletionResponse::complete(
        exact_request,
        response.runtime().clone(),
        response.artifact_id().clone(),
        response.artifact_digest().clone(),
        r#"{"candidates":[{"text":"different"}]}"#.to_owned(),
        response.usage(),
    )
    .expect("substituted output response");
    assert!(
        OllamaResidentResourceObservedCompletion::for_test(
            wrong_output_response,
            receipt.clone(),
            11,
            Instant::now(),
            1,
            2,
            3,
            4,
            5,
            6,
        )
        .is_err()
    );
    assert!(
        OllamaResidentResourceObservedCompletion::for_test(
            response.clone(),
            receipt.clone(),
            12,
            Instant::now(),
            1,
            2,
            3,
            4,
            5,
            6,
        )
        .is_err()
    );
}

#[tokio::test]
async fn every_missing_provider_value_fails_only_the_resource_path() {
    let missing = [
        concat!(
            "\"load_duration\":1000,\"prompt_eval_count\":4,",
            "\"prompt_eval_duration\":2000,\"eval_count\":2,\"eval_duration\":3000"
        ),
        concat!(
            "\"total_duration\":9000,\"prompt_eval_count\":4,",
            "\"prompt_eval_duration\":2000,\"eval_count\":2,\"eval_duration\":3000"
        ),
        concat!(
            "\"total_duration\":9000,\"load_duration\":1000,",
            "\"prompt_eval_duration\":2000,\"eval_count\":2,\"eval_duration\":3000"
        ),
        concat!(
            "\"total_duration\":9000,\"load_duration\":1000,",
            "\"prompt_eval_count\":4,\"eval_count\":2,\"eval_duration\":3000"
        ),
        concat!(
            "\"total_duration\":9000,\"load_duration\":1000,",
            "\"prompt_eval_count\":4,\"prompt_eval_duration\":2000,",
            "\"eval_duration\":3000"
        ),
        concat!(
            "\"total_duration\":9000,\"load_duration\":1000,",
            "\"prompt_eval_count\":4,\"prompt_eval_duration\":2000,\"eval_count\":2"
        ),
    ];
    for metrics in missing {
        assert_resource_failure(metrics, "resource_observation_unavailable").await;
    }

    for metrics in [
        "\"prompt_eval_count\":4,\"eval_count\":2,\"eval_duration\":3000",
        concat!(
            "\"total_duration\":\"invalid\",\"load_duration\":-1,",
            "\"prompt_eval_count\":4,\"prompt_eval_duration\":1.5,",
            "\"eval_count\":2,\"eval_duration\":3000"
        ),
    ] {
        let compatibility_server = SessionServer::start(SessionMode::ResidentResource {
            metrics,
            delayed_body: false,
        })
        .await;
        let stream = compatibility_server.supplied_stream().await;
        let token = CancellationToken::new();
        let mut session = config(
            compatibility_server.endpoint.clone(),
            OllamaLimits::default(),
        )
        .open(stream, context(&token), |_| Ok::<(), ()>(()))
        .await
        .expect("open compatibility session");
        session.preflight(context(&token)).await.expect("preflight");
        session
            .complete_structured_with_residency_borrowed(&request("compatibility"), context(&token))
            .await
            .expect("resource-only field shape remains compatible");
        drop(session);
        assert_eq!(compatibility_server.finish().await.requests.len(), 16);
    }
}

#[tokio::test]
async fn malformed_negative_float_and_overflow_values_fail_content_free() {
    for metrics in [
        COMPLETE_METRICS.replace("9000", "\"invalid\""),
        COMPLETE_METRICS.replace("9000", "-1"),
        COMPLETE_METRICS.replace("9000", "1.5"),
        COMPLETE_METRICS.replace("9000", "18446744073709551616"),
    ] {
        assert_resource_failure(
            Box::leak(metrics.into_boxed_str()),
            "resource_observation_unavailable",
        )
        .await;
    }
}

#[tokio::test]
async fn response_head_checkpoint_precedes_delayed_body_drain() {
    let server = SessionServer::start(SessionMode::ResidentResource {
        metrics: COMPLETE_METRICS,
        delayed_body: true,
    })
    .await;
    let stream = server.supplied_stream().await;
    let token = CancellationToken::new();
    let mut session = config(server.endpoint.clone(), OllamaLimits::default())
        .open(stream, context(&token), |_| Ok::<(), ()>(()))
        .await
        .expect("open delayed body session");
    session.preflight(context(&token)).await.expect("preflight");
    let started = Instant::now();
    let evidence = session
        .complete_structured_with_residency_and_resource_observation_borrowed(
            &request("delayed body"),
            context(&token),
        )
        .await
        .expect("delayed resource observation");
    let completed = started.elapsed();
    let response_head = evidence
        .response_head_elapsed_since(started)
        .expect("ordered response head");
    assert!(completed >= Duration::from_millis(140));
    assert!(response_head + Duration::from_millis(100) < completed);
    drop(session);
    assert_eq!(server.finish().await.requests.len(), 16);
}

#[tokio::test]
async fn deadline_during_body_drain_fails_without_resource_escape_or_reconnect() {
    let server = SessionServer::start(SessionMode::ResidentResource {
        metrics: COMPLETE_METRICS,
        delayed_body: true,
    })
    .await;
    let stream = server.supplied_stream().await;
    let token = CancellationToken::new();
    let mut session = config(server.endpoint.clone(), OllamaLimits::default())
        .open(stream, context(&token), |_| Ok::<(), ()>(()))
        .await
        .expect("open deadline session");
    session.preflight(context(&token)).await.expect("preflight");
    let deadline = OperationContext::new(&token, Some(Instant::now() + Duration::from_millis(40)));
    let error = session
        .complete_structured_with_residency_and_resource_observation_borrowed(
            &request("deadline"),
            deadline,
        )
        .await
        .expect_err("deadline prevents resource observation");
    assert_session_error(error, InferenceErrorKind::Deadline, "deadline");
    drop(session);
    let result = server.finish().await;
    assert_eq!(result.accepts, 1);
    assert_eq!(result.generate_requests.len(), 1);
}

#[tokio::test]
async fn cancellation_during_body_drain_fails_without_resource_escape_or_reconnect() {
    let server = SessionServer::start(SessionMode::ResidentResource {
        metrics: COMPLETE_METRICS,
        delayed_body: true,
    })
    .await;
    let stream = server.supplied_stream().await;
    let token = CancellationToken::new();
    let mut session = config(server.endpoint.clone(), OllamaLimits::default())
        .open(stream, context(&token), |_| Ok::<(), ()>(()))
        .await
        .expect("open cancellation session");
    session.preflight(context(&token)).await.expect("preflight");
    let cancelling = token.clone();
    let task = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(40)).await;
        cancelling.cancel();
    });
    let error = session
        .complete_structured_with_residency_and_resource_observation_borrowed(
            &request("cancelled"),
            context(&token),
        )
        .await
        .expect_err("cancellation prevents resource observation");
    assert_session_error(error, InferenceErrorKind::Cancelled, "cancelled");
    task.await.expect("cancellation task");
    drop(session);
    let result = server.finish().await;
    assert_eq!(result.accepts, 1);
    assert_eq!(result.generate_requests.len(), 1);
}

#[test]
fn source_guards_keep_the_checkpoint_private_and_the_exchange_single_path() {
    let transport = include_str!("../transport/exchange.rs");
    let completion = include_str!("../session/completion.rs");
    let observation = include_str!("../resource_observation.rs");
    let head = transport.find("let response_head =").expect("head capture");
    let drain = transport[head..]
        .find(".read_response(")
        .expect("body drain after head");
    assert!(drain > 0);
    assert!(!observation.contains("AfterResponse"));
    assert!(!observation.contains("Incoming"));
    assert!(!observation.contains("Vec<u8>"));
    assert!(!observation.contains("TcpStream"));
    assert!(!observation.contains("reqwest"));
    assert!(!observation.contains("pub fn into_"));
    assert!(!observation.contains("pub fn observation("));
    assert!(!completion.contains("retry"));
    assert_eq!(
        completion.matches("generate_with_response_head(").count(),
        1
    );
}

async fn assert_resource_failure(metrics: &'static str, expected_code: &str) {
    let server = SessionServer::start(SessionMode::ResidentResource {
        metrics,
        delayed_body: false,
    })
    .await;
    let stream = server.supplied_stream().await;
    let token = CancellationToken::new();
    let mut session = config(server.endpoint.clone(), OllamaLimits::default())
        .open(stream, context(&token), |_| Ok::<(), ()>(()))
        .await
        .expect("open invalid resource session");
    session.preflight(context(&token)).await.expect("preflight");
    let error = session
        .complete_structured_with_residency_and_resource_observation_borrowed(
            &request("invalid resource"),
            context(&token),
        )
        .await
        .expect_err("resource evidence must fail");
    assert_session_error(error, InferenceErrorKind::MalformedResponse, expected_code);
    drop(session);
    let result = server.finish().await;
    assert_eq!(result.accepts, 1);
    assert_eq!(result.generate_requests.len(), 1);
}
