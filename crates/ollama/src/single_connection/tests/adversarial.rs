use std::{
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};

use rewrite_inference::{InferenceError, InferenceErrorKind, OperationContext};
use rewrite_types::CancellationToken;

use super::fixture::{
    FirstResponseMode, FixtureServer, configured_preflight, context, preflight, response_body,
    second_target, target,
};
use crate::{
    OllamaBackend, OllamaLimits, OllamaObservedPreflightError, OllamaResponseObservationPhase,
    OllamaSingleConnectionPreflight,
};

#[tokio::test]
async fn rejects_non_persistent_versions_upgrades_and_trailers() {
    for (mode, expected_code) in [
        (FirstResponseMode::Http10, "non_persistent_http_response"),
        (
            FirstResponseMode::SwitchingProtocols,
            "non_persistent_http_response",
        ),
        (
            FirstResponseMode::UpgradeHeaderOnly,
            "non_persistent_http_response",
        ),
        (
            FirstResponseMode::DeclaredTrailer,
            "unexpected_response_trailers",
        ),
        (
            FirstResponseMode::ActualTrailer,
            "unexpected_response_trailers",
        ),
    ] {
        let server = FixtureServer::start(mode).await;
        let token = CancellationToken::new();
        let mut phases = Vec::new();
        let error = preflight(server.endpoint.clone(), 1024 * 1024)
            .preflight_with_observer(context(&token), |observation| {
                phases.push(observation.phase());
                Ok::<(), ()>(())
            })
            .await
            .expect_err("unsupported HTTP response fails closed");
        assert_preflight_error(error, InferenceErrorKind::MalformedResponse, expected_code);
        assert_eq!(
            phases,
            [
                OllamaResponseObservationPhase::BeforeResponses,
                OllamaResponseObservationPhase::AfterFailedAttempt {
                    completed_responses: 0,
                },
            ]
        );
        let result = server.finish().await;
        assert_eq!(result.accepts, 1);
        assert!(result.client_closed);
    }
}

#[tokio::test]
async fn preserves_status_and_content_type_classification() {
    for (mode, expected_kind, expected_code) in [
        (
            FirstResponseMode::WrongContentType,
            InferenceErrorKind::MalformedResponse,
            "unexpected_content_type",
        ),
        (
            FirstResponseMode::NotFound,
            InferenceErrorKind::Compatibility,
            "api_not_found",
        ),
        (
            FirstResponseMode::TooManyRequests,
            InferenceErrorKind::Retryable,
            "http_transient",
        ),
        (
            FirstResponseMode::Rejected,
            InferenceErrorKind::Permanent,
            "http_rejected",
        ),
    ] {
        let server = FixtureServer::start(mode).await;
        let token = CancellationToken::new();
        let error = preflight(server.endpoint.clone(), 1024 * 1024)
            .preflight_with_observer(context(&token), |_| Ok::<(), ()>(()))
            .await
            .expect_err("invalid response head fails closed");
        assert_preflight_error(error, expected_kind, expected_code);
        let result = server.finish().await;
        assert_eq!(result.accepts, 1);
        assert_eq!(result.requests, ["GET /api/version"]);
    }
}

#[tokio::test]
async fn invalid_json_is_observed_only_after_its_complete_drain() {
    let server = FixtureServer::start(FirstResponseMode::InvalidJson).await;
    let token = CancellationToken::new();
    let mut phases = Vec::new();
    let error = preflight(server.endpoint.clone(), 1024 * 1024)
        .preflight_with_observer(context(&token), |observation| {
            phases.push(observation.phase());
            Ok::<(), ()>(())
        })
        .await
        .expect_err("invalid JSON fails closed");
    assert_preflight_error(
        error,
        InferenceErrorKind::MalformedResponse,
        "invalid_json_response",
    );
    assert_eq!(
        phases,
        [
            OllamaResponseObservationPhase::BeforeResponses,
            OllamaResponseObservationPhase::AfterResponse { ordinal: 1 },
        ]
    );
    let result = server.finish().await;
    assert_eq!(result.requests, ["GET /api/version"]);
}

#[tokio::test]
async fn aggregate_ceiling_accumulates_across_responses() {
    let first = response_body("/api/version", FirstResponseMode::Normal, 1, 1).len();
    let second = response_body("/api/tags", FirstResponseMode::Normal, 2, 1).len();
    let server = FixtureServer::start(FirstResponseMode::Normal).await;
    let token = CancellationToken::new();
    let mut phases = Vec::new();
    let error = preflight(server.endpoint.clone(), first + second - 1)
        .preflight_with_observer(context(&token), |observation| {
            phases.push(observation.phase());
            Ok::<(), ()>(())
        })
        .await
        .expect_err("cumulative body limit fails closed");
    assert_preflight_error(
        error,
        InferenceErrorKind::MalformedResponse,
        "preflight_session_body_too_large",
    );
    assert_eq!(
        phases,
        [
            OllamaResponseObservationPhase::BeforeResponses,
            OllamaResponseObservationPhase::AfterResponse { ordinal: 1 },
            OllamaResponseObservationPhase::AfterFailedAttempt {
                completed_responses: 1,
            },
        ]
    );
    let result = server.finish().await;
    assert_eq!(result.requests, ["GET /api/version", "GET /api/tags"]);
}

#[tokio::test]
async fn enforces_idle_read_and_request_timeouts() {
    for (mode, limits) in [
        (
            FirstResponseMode::StallBody,
            OllamaLimits {
                connect_timeout: Duration::from_millis(100),
                request_timeout: Duration::from_secs(1),
                read_timeout: Duration::from_millis(30),
                ..OllamaLimits::default()
            },
        ),
        (
            FirstResponseMode::StallHeaders,
            OllamaLimits {
                connect_timeout: Duration::from_millis(30),
                request_timeout: Duration::from_millis(60),
                read_timeout: Duration::from_millis(30),
                ..OllamaLimits::default()
            },
        ),
    ] {
        let server = FixtureServer::start(mode).await;
        let token = CancellationToken::new();
        let error =
            configured_preflight(server.endpoint.clone(), vec![target()], limits, 1024 * 1024)
                .preflight_with_observer(context(&token), |_| Ok::<(), ()>(()))
                .await
                .expect_err("transport timeout fails closed");
        assert_preflight_error(error, InferenceErrorKind::Deadline, "deadline");
        let result = server.finish().await;
        assert_eq!(result.accepts, 1);
        assert!(result.client_closed);
    }
}

#[tokio::test]
async fn cancellation_during_body_read_aborts_the_connection() {
    let server = FixtureServer::start(FirstResponseMode::StallBody).await;
    let request_count = Arc::clone(&server.requests);
    let token = CancellationToken::new();
    let cancelling = token.clone();
    let cancellation = tokio::spawn(async move {
        while request_count.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
        cancelling.cancel();
    });
    let error = preflight(server.endpoint.clone(), 1024 * 1024)
        .preflight_with_observer(context(&token), |_| Ok::<(), ()>(()))
        .await
        .expect_err("cancellation fails closed");
    cancellation.await.expect("cancellation task");
    assert_preflight_error(error, InferenceErrorKind::Cancelled, "cancelled");
    let result = server.finish().await;
    assert_eq!(result.requests, ["GET /api/version"]);
    assert!(result.client_closed);
}

#[tokio::test]
async fn callback_context_changes_never_admit_another_request() {
    let initial_server = FixtureServer::start(FirstResponseMode::Normal).await;
    let initial_token = CancellationToken::new();
    let initial_error = preflight(initial_server.endpoint.clone(), 1024 * 1024)
        .preflight_with_observer(context(&initial_token), |_| {
            initial_token.cancel();
            Ok::<(), ()>(())
        })
        .await
        .expect_err("initial callback cancellation fails closed");
    assert_preflight_error(initial_error, InferenceErrorKind::Cancelled, "cancelled");
    let initial_result = initial_server.finish().await;
    assert!(initial_result.requests.is_empty());
    assert!(initial_result.client_closed);

    let post_server = FixtureServer::start(FirstResponseMode::Normal).await;
    let post_token = CancellationToken::new();
    let post_context = OperationContext::new(
        &post_token,
        Some(Instant::now() + Duration::from_millis(30)),
    );
    let post_error = preflight(post_server.endpoint.clone(), 1024 * 1024)
        .preflight_with_observer(post_context, |observation| {
            if matches!(
                observation.phase(),
                OllamaResponseObservationPhase::AfterResponse { ordinal: 1 }
            ) {
                std::thread::sleep(Duration::from_millis(50));
            }
            Ok::<(), ()>(())
        })
        .await
        .expect_err("post-response callback overrun fails closed");
    assert_preflight_error(post_error, InferenceErrorKind::Deadline, "deadline");
    let post_result = post_server.finish().await;
    assert_eq!(post_result.requests, ["GET /api/version"]);
    assert!(post_result.client_closed);
}

#[test]
fn rejects_unbounded_timeout_values_without_panicking() {
    let endpoint =
        crate::OllamaEndpoint::parse("http://127.0.0.1:11434").expect("loopback endpoint");
    let result = OllamaSingleConnectionPreflight::new(
        endpoint,
        vec![target()],
        OllamaLimits {
            connect_timeout: Duration::MAX,
            request_timeout: Duration::MAX,
            read_timeout: Duration::MAX,
            ..OllamaLimits::default()
        },
        1024,
    );
    let Err(error) = result else {
        panic!("unbounded limits must fail closed");
    };
    assert_eq!(error.code, "invalid_limits");
}

#[tokio::test]
async fn multi_target_and_runtime_drift_match_the_legacy_preflight() {
    let targets = vec![target(), second_target()];
    let retained_server = FixtureServer::start_with_targets(FirstResponseMode::Normal, 2).await;
    let token = CancellationToken::new();
    let retained = configured_preflight(
        retained_server.endpoint.clone(),
        targets.clone(),
        OllamaLimits::default(),
        1024 * 1024,
    )
    .preflight_with_observer(context(&token), |_| Ok::<(), ()>(()))
    .await
    .expect("multi-target retained report");
    let retained_result = retained_server.finish().await;
    assert_eq!(retained_result.accepts, 1);
    assert_eq!(retained_result.requests.len(), 8);

    let legacy_server = FixtureServer::start_with_targets(FirstResponseMode::Normal, 2).await;
    let legacy = OllamaBackend::new_preflight(
        legacy_server.endpoint.clone(),
        targets,
        OllamaLimits::default(),
    )
    .expect("multi-target legacy backend")
    .preflight(context(&token))
    .await
    .expect("multi-target legacy report");
    assert_eq!(legacy_server.finish().await.accepts, 1);
    assert_eq!(retained, legacy);

    let retained_drift_server = FixtureServer::start(FirstResponseMode::RuntimeDrift).await;
    let retained_drift = preflight(retained_drift_server.endpoint.clone(), 1024 * 1024)
        .preflight_with_observer(context(&token), |_| Ok::<(), ()>(()))
        .await
        .expect_err("retained runtime drift");
    let OllamaObservedPreflightError::Preflight(retained_drift) = retained_drift else {
        panic!("unexpected observation failure");
    };
    retained_drift_server.finish().await;

    let legacy_drift_server = FixtureServer::start(FirstResponseMode::RuntimeDrift).await;
    let legacy_drift = OllamaBackend::new_preflight(
        legacy_drift_server.endpoint.clone(),
        vec![target()],
        OllamaLimits::default(),
    )
    .expect("legacy drift backend")
    .preflight(context(&token))
    .await
    .expect_err("legacy runtime drift");
    legacy_drift_server.finish().await;
    assert_eq!(retained_drift, legacy_drift);
}

fn assert_preflight_error(
    error: OllamaObservedPreflightError<()>,
    expected_kind: InferenceErrorKind,
    expected_code: &str,
) {
    let OllamaObservedPreflightError::Preflight(InferenceError { kind, code }) = error else {
        panic!("unexpected observation failure");
    };
    assert_eq!(kind, expected_kind);
    assert_eq!(code, expected_code);
}
