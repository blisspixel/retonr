use std::{net::TcpStream as StandardTcpStream, sync::Arc, time::Duration};

use rewrite_inference::{InferenceErrorKind, OperationContext};
use rewrite_types::CancellationToken;
use tokio::net::TcpStream;

use super::fixture::{FirstResponseMode, FixtureServer, context};
use crate::{
    OllamaLimits, OllamaObservedRuntimeProbeError, OllamaResponseObservationPhase,
    OllamaSingleConnectionRuntimeProbe, OllamaVersion,
};

const EXPECTED_VERSION: &str = "0.32.14";

#[tokio::test]
async fn one_retained_stream_issues_only_the_exact_version_request() {
    let server = FixtureServer::start(FirstResponseMode::Normal).await;
    let stream = connect(&server).await;
    let request_count = Arc::clone(&server.requests);
    let response_count = Arc::clone(&server.responses);
    let token = CancellationToken::new();
    let mut phases = Vec::new();
    let mut addresses = Vec::new();
    let evidence = probe(&server, OllamaLimits::default())
        .probe_on_connected_stream_with_observer(context(&token), stream, |observation| {
            match observation.phase() {
                OllamaResponseObservationPhase::BeforeResponses => {
                    assert_eq!(request_count.load(std::sync::atomic::Ordering::SeqCst), 0);
                }
                OllamaResponseObservationPhase::AfterResponse { ordinal } => {
                    assert_eq!(ordinal, 1);
                    assert_eq!(request_count.load(std::sync::atomic::Ordering::SeqCst), 1);
                    assert_eq!(response_count.load(std::sync::atomic::Ordering::SeqCst), 1);
                }
                OllamaResponseObservationPhase::AfterFailedAttempt { .. } => {
                    panic!("runtime probe must not emit failed-attempt observations");
                }
            }
            phases.push(observation.phase());
            addresses.push(observation.addresses());
            Ok::<(), ()>(())
        })
        .await
        .expect("probe exact runtime version");
    assert_eq!(evidence.runtime_version(), version(EXPECTED_VERSION));
    assert_eq!(
        phases,
        [
            OllamaResponseObservationPhase::BeforeResponses,
            OllamaResponseObservationPhase::AfterResponse { ordinal: 1 },
        ]
    );
    assert_eq!(addresses.len(), 2);
    assert!(addresses[0] == addresses[1]);
    assert!(addresses[0].client().ip().is_loopback());
    assert_eq!(addresses[0].server(), server.endpoint.socket_addr());
    let result = server.finish().await;
    assert_eq!(result.accepts, 1);
    assert_eq!(result.requests, ["GET /api/version"]);
    assert!(result.client_closed);
    assert!(result.requests.iter().all(|request| {
        !["tags", "show", "generate", "chat"]
            .iter()
            .any(|surface| request.contains(surface))
    }));
}

#[tokio::test]
async fn exact_version_mismatch_returns_no_evidence_after_complete_observation() {
    let server = FixtureServer::start(FirstResponseMode::Normal).await;
    let stream = connect(&server).await;
    let token = CancellationToken::new();
    let mut phases = Vec::new();
    let error = OllamaSingleConnectionRuntimeProbe::new(
        server.endpoint.clone(),
        version("0.32.15"),
        OllamaLimits::default(),
    )
    .expect("configured probe")
    .probe_on_connected_stream_with_observer(context(&token), stream, |observation| {
        phases.push(observation.phase());
        Ok::<(), ()>(())
    })
    .await
    .expect_err("version mismatch fails closed");
    assert_probe_error(
        error,
        InferenceErrorKind::Compatibility,
        "runtime_version_mismatch",
    );
    assert_eq!(
        phases,
        [
            OllamaResponseObservationPhase::BeforeResponses,
            OllamaResponseObservationPhase::AfterResponse { ordinal: 1 },
        ]
    );
    assert_one_version_request(server).await;
}

#[tokio::test]
async fn extra_truncated_malformed_and_oversized_bodies_fail_closed() {
    for (mode, expected_code, fully_drained) in [
        (FirstResponseMode::ExtraJson, "invalid_json_response", true),
        (
            FirstResponseMode::ExtraVersionField,
            "invalid_json_response",
            true,
        ),
        (
            FirstResponseMode::ExtraAfterDeclaredBody,
            "connection_closed",
            true,
        ),
        (
            FirstResponseMode::InvalidJson,
            "invalid_json_response",
            true,
        ),
        (
            FirstResponseMode::OversizedVersionBody,
            "response_body_too_large",
            false,
        ),
        (FirstResponseMode::TruncatedBody, "transport_failed", false),
    ] {
        let server = FixtureServer::start(mode).await;
        let stream = connect(&server).await;
        let token = CancellationToken::new();
        let mut phases = Vec::new();
        let error = probe(&server, OllamaLimits::default())
            .probe_on_connected_stream_with_observer(context(&token), stream, |observation| {
                phases.push(observation.phase());
                Ok::<(), ()>(())
            })
            .await
            .expect_err("invalid body fails closed");
        let expected_kind = if matches!(
            mode,
            FirstResponseMode::TruncatedBody | FirstResponseMode::ExtraAfterDeclaredBody
        ) {
            InferenceErrorKind::Retryable
        } else {
            InferenceErrorKind::MalformedResponse
        };
        assert_probe_error(error, expected_kind, expected_code);
        assert_eq!(
            phases,
            if fully_drained {
                vec![
                    OllamaResponseObservationPhase::BeforeResponses,
                    OllamaResponseObservationPhase::AfterResponse { ordinal: 1 },
                ]
            } else {
                vec![OllamaResponseObservationPhase::BeforeResponses]
            }
        );
        assert_one_version_request(server).await;
    }
}

#[tokio::test]
async fn close_upgrade_trailer_and_persistence_failures_are_rejected() {
    for (mode, expected_kind, expected_code, fully_drained) in [
        (
            FirstResponseMode::Http10,
            InferenceErrorKind::MalformedResponse,
            "non_persistent_http_response",
            false,
        ),
        (
            FirstResponseMode::ConnectionClose,
            InferenceErrorKind::MalformedResponse,
            "non_persistent_http_response",
            false,
        ),
        (
            FirstResponseMode::SwitchingProtocols,
            InferenceErrorKind::MalformedResponse,
            "non_persistent_http_response",
            false,
        ),
        (
            FirstResponseMode::UpgradeHeaderOnly,
            InferenceErrorKind::MalformedResponse,
            "non_persistent_http_response",
            false,
        ),
        (
            FirstResponseMode::DeclaredTrailer,
            InferenceErrorKind::MalformedResponse,
            "unexpected_response_trailers",
            false,
        ),
        (
            FirstResponseMode::ActualTrailer,
            InferenceErrorKind::MalformedResponse,
            "unexpected_response_trailers",
            false,
        ),
        (
            FirstResponseMode::SilentClose,
            InferenceErrorKind::Retryable,
            "connection_closed",
            true,
        ),
    ] {
        let server = FixtureServer::start(mode).await;
        let stream = connect(&server).await;
        let token = CancellationToken::new();
        let mut phases = Vec::new();
        let error = probe(&server, OllamaLimits::default())
            .probe_on_connected_stream_with_observer(context(&token), stream, |observation| {
                phases.push(observation.phase());
                Ok::<(), ()>(())
            })
            .await
            .expect_err("nonpersistent response fails closed");
        assert_probe_error(error, expected_kind, expected_code);
        assert_eq!(
            phases,
            if fully_drained {
                vec![
                    OllamaResponseObservationPhase::BeforeResponses,
                    OllamaResponseObservationPhase::AfterResponse { ordinal: 1 },
                ]
            } else {
                vec![OllamaResponseObservationPhase::BeforeResponses]
            }
        );
        assert_one_version_request(server).await;
    }
}

#[tokio::test]
async fn callback_failures_stop_before_any_additional_protocol_work() {
    let before_server = FixtureServer::start(FirstResponseMode::Normal).await;
    let before_stream = connect(&before_server).await;
    let token = CancellationToken::new();
    let error = probe(&before_server, OllamaLimits::default())
        .probe_on_connected_stream_with_observer(context(&token), before_stream, |_| {
            Err("before request")
        })
        .await
        .expect_err("before-request callback failure");
    assert!(matches!(
        error,
        OllamaObservedRuntimeProbeError::Observation("before request")
    ));
    let before_result = before_server.finish().await;
    assert!(before_result.requests.is_empty());
    assert_eq!(before_result.accepts, 1);

    let after_server = FixtureServer::start(FirstResponseMode::Normal).await;
    let after_stream = connect(&after_server).await;
    let mut callbacks = 0;
    let error = probe(&after_server, OllamaLimits::default())
        .probe_on_connected_stream_with_observer(context(&token), after_stream, |_| {
            callbacks += 1;
            if callbacks == 2 {
                Err("after response")
            } else {
                Ok(())
            }
        })
        .await
        .expect_err("after-response callback failure");
    assert!(matches!(
        error,
        OllamaObservedRuntimeProbeError::Observation("after response")
    ));
    assert_eq!(callbacks, 2);
    assert_one_version_request(after_server).await;
}

#[tokio::test]
async fn timeout_and_cancellation_abort_the_only_retained_stream() {
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
        let stream = connect(&server).await;
        let token = CancellationToken::new();
        let error = probe(&server, limits)
            .probe_on_connected_stream_with_observer(context(&token), stream, |_| Ok::<(), ()>(()))
            .await
            .expect_err("bounded timeout fails closed");
        assert_probe_error(error, InferenceErrorKind::Deadline, "deadline");
        assert_one_version_request(server).await;
    }

    let server = FixtureServer::start(FirstResponseMode::StallBody).await;
    let stream = connect(&server).await;
    let requests = Arc::clone(&server.requests);
    let token = CancellationToken::new();
    let cancelling = token.clone();
    let cancellation = tokio::spawn(async move {
        while requests.load(std::sync::atomic::Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
        cancelling.cancel();
    });
    let error = probe(&server, OllamaLimits::default())
        .probe_on_connected_stream_with_observer(context(&token), stream, |_| Ok::<(), ()>(()))
        .await
        .expect_err("in-flight cancellation fails closed");
    cancellation.await.expect("cancellation task");
    assert_probe_error(error, InferenceErrorKind::Cancelled, "cancelled");
    assert_one_version_request(server).await;
}

#[tokio::test]
async fn callback_context_change_prevents_the_version_request() {
    let server = FixtureServer::start(FirstResponseMode::Normal).await;
    let stream = connect(&server).await;
    let token = CancellationToken::new();
    let error = probe(&server, OllamaLimits::default())
        .probe_on_connected_stream_with_observer(context(&token), stream, |_| {
            token.cancel();
            Ok::<(), ()>(())
        })
        .await
        .expect_err("callback cancellation fails closed");
    assert_probe_error(error, InferenceErrorKind::Cancelled, "cancelled");
    let result = server.finish().await;
    assert_eq!(result.accepts, 1);
    assert!(result.requests.is_empty());
}

fn probe(server: &FixtureServer, limits: OllamaLimits) -> OllamaSingleConnectionRuntimeProbe {
    OllamaSingleConnectionRuntimeProbe::new(
        server.endpoint.clone(),
        version(EXPECTED_VERSION),
        limits,
    )
    .expect("configured runtime probe")
}

async fn connect(server: &FixtureServer) -> StandardTcpStream {
    TcpStream::connect(server.endpoint.socket_addr())
        .await
        .expect("connect retained stream")
        .into_std()
        .expect("convert retained stream")
}

fn version(value: &str) -> OllamaVersion {
    value.parse().expect("exact fixture version")
}

async fn assert_one_version_request(server: FixtureServer) {
    let result = server.finish().await;
    assert_eq!(result.accepts, 1);
    assert_eq!(result.requests, ["GET /api/version"]);
    assert!(result.client_closed);
}

fn assert_probe_error(
    error: OllamaObservedRuntimeProbeError<()>,
    expected_kind: InferenceErrorKind,
    expected_code: &str,
) {
    let OllamaObservedRuntimeProbeError::Probe(error) = error else {
        panic!("unexpected observer error");
    };
    assert_eq!(error.kind, expected_kind);
    assert_eq!(error.code, expected_code);
}

#[test]
fn invalid_shared_limits_are_rejected_without_models_or_generation_configuration() {
    let endpoint =
        crate::OllamaEndpoint::parse("http://127.0.0.1:11434").expect("loopback fixture endpoint");
    let error = OllamaSingleConnectionRuntimeProbe::new(
        endpoint,
        version(EXPECTED_VERSION),
        OllamaLimits {
            request_timeout: Duration::ZERO,
            ..OllamaLimits::default()
        },
    )
    .err()
    .expect("invalid limits fail closed");
    assert_eq!(error.kind, InferenceErrorKind::Policy);
    assert_eq!(error.code, "invalid_limits");
}

#[test]
fn observed_probe_errors_are_redacted_and_preserve_sources() {
    let probe = OllamaObservedRuntimeProbeError::<std::io::Error>::Probe(
        rewrite_inference::InferenceError::new(InferenceErrorKind::Compatibility, "fixture"),
    );
    assert_eq!(
        probe.to_string(),
        "single-connection Ollama runtime probe failed"
    );
    assert!(std::error::Error::source(&probe).is_some());
    let observation =
        OllamaObservedRuntimeProbeError::Observation(std::io::Error::other("fixture"));
    assert_eq!(
        observation.to_string(),
        "retained Ollama connection observation failed"
    );
    assert!(std::error::Error::source(&observation).is_some());
}

#[test]
fn operation_context_type_is_the_only_runtime_control_input() {
    fn accepts_context(_: OperationContext<'_>) {}
    let token = CancellationToken::new();
    accepts_context(OperationContext::new(&token, None));
}
