use std::sync::{Arc, atomic::Ordering};

use rewrite_types::CancellationToken;

use super::{OllamaObservedPreflightError, OllamaResponseObservationPhase};
use crate::{OllamaBackend, OllamaLimits};

use self::fixture::{FirstResponseMode, FixtureServer, context, preflight, target};

mod adversarial;
mod fixture;

#[tokio::test]
async fn one_connection_yields_exact_ordered_response_checkpoints() {
    let server = FixtureServer::start(FirstResponseMode::Normal).await;
    let endpoint = server.endpoint.clone();
    let request_count = Arc::clone(&server.requests);
    let response_count = Arc::clone(&server.responses);
    let mut phases = Vec::new();
    let mut addresses = Vec::new();
    let token = CancellationToken::new();
    let report = preflight(endpoint.clone(), 1024 * 1024)
        .preflight_with_observer(context(&token), |observation| {
            let phase = observation.phase();
            if let OllamaResponseObservationPhase::AfterResponse { ordinal } = phase {
                assert_eq!(request_count.load(Ordering::SeqCst), ordinal);
                assert_eq!(response_count.load(Ordering::SeqCst), ordinal);
            }
            phases.push(phase);
            addresses.push(observation.addresses());
            Ok::<(), ()>(())
        })
        .await
        .expect("stable single connection preflight");
    assert_eq!(report.runtime.version, "0.32.14");
    assert_eq!(report.bindings.len(), 1);
    assert_eq!(phases.len(), 8);
    assert_eq!(phases[0], OllamaResponseObservationPhase::BeforeResponses);
    assert_eq!(
        &phases[1..],
        &(1..=7)
            .map(|ordinal| OllamaResponseObservationPhase::AfterResponse { ordinal })
            .collect::<Vec<_>>()
    );
    assert!(addresses.iter().all(|value| value == &addresses[0]));
    assert_eq!(addresses[0].server(), endpoint.socket_addr());
    assert!(addresses[0].client().ip().is_loopback());

    let result = server.finish().await;
    assert_eq!(result.accepts, 1);
    assert!(result.client_closed);
    assert_eq!(
        result.requests,
        [
            "GET /api/version",
            "GET /api/tags",
            "GET /api/ps",
            "POST /api/show",
            "GET /api/tags",
            "GET /api/version",
            "GET /api/ps",
        ]
    );
}

#[tokio::test]
async fn observer_failure_stops_before_the_next_request() {
    let server = FixtureServer::start(FirstResponseMode::Normal).await;
    let request_count = Arc::clone(&server.requests);
    let token = CancellationToken::new();
    let error = preflight(server.endpoint.clone(), 1024 * 1024)
        .preflight_with_observer(context(&token), |observation| match observation.phase() {
            OllamaResponseObservationPhase::AfterResponse { ordinal: 1 } => Err("stop"),
            _ => Ok(()),
        })
        .await
        .expect_err("observer failure stops the exchange");
    assert!(matches!(
        error,
        OllamaObservedPreflightError::Observation("stop")
    ));
    assert_eq!(request_count.load(Ordering::SeqCst), 1);
    let result = server.finish().await;
    assert_eq!(result.accepts, 1);
    assert_eq!(result.requests, ["GET /api/version"]);
}

#[tokio::test]
async fn rejects_server_close_without_reconnect() {
    for (mode, expected_code, completed_responses) in [
        (
            FirstResponseMode::ConnectionClose,
            "non_persistent_http_response",
            0,
        ),
        (
            FirstResponseMode::Upgrade,
            "non_persistent_http_response",
            0,
        ),
        (FirstResponseMode::SilentClose, "transport_failed", 1),
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
            .expect_err("server close fails closed");
        let OllamaObservedPreflightError::Preflight(error) = error else {
            panic!("unexpected observation failure");
        };
        assert!(
            error.code == expected_code
                || (matches!(mode, FirstResponseMode::SilentClose)
                    && error.code == "connection_closed")
        );
        assert_eq!(
            phases.last(),
            Some(&OllamaResponseObservationPhase::AfterFailedAttempt {
                completed_responses,
            })
        );
        let result = server.finish().await;
        assert_eq!(result.accepts, 1);
    }
}

#[tokio::test]
async fn enforces_response_and_aggregate_body_ceilings() {
    for (limits, session_bytes, expected_code) in [
        (
            OllamaLimits {
                discovery_body_bytes: 8,
                ..OllamaLimits::default()
            },
            1024,
            "response_body_too_large",
        ),
        (
            OllamaLimits::default(),
            8,
            "preflight_session_body_too_large",
        ),
    ] {
        let server = FixtureServer::start(FirstResponseMode::Normal).await;
        let configured = super::OllamaSingleConnectionPreflight::new(
            server.endpoint.clone(),
            vec![target()],
            limits,
            session_bytes,
        )
        .expect("bounded preflight");
        let token = CancellationToken::new();
        let error = configured
            .preflight_with_observer(context(&token), |_| Ok::<(), ()>(()))
            .await
            .expect_err("body ceiling fails closed");
        let OllamaObservedPreflightError::Preflight(error) = error else {
            panic!("unexpected observation failure");
        };
        assert_eq!(error.code, expected_code);
        let result = server.finish().await;
        assert_eq!(result.accepts, 1);
    }
}

#[tokio::test]
async fn matches_the_existing_preflight_report() {
    let retained_server = FixtureServer::start(FirstResponseMode::Normal).await;
    let token = CancellationToken::new();
    let retained = preflight(retained_server.endpoint.clone(), 1024 * 1024)
        .preflight_with_observer(context(&token), |_| Ok::<(), ()>(()))
        .await
        .expect("retained report");
    let retained_result = retained_server.finish().await;
    assert_eq!(retained_result.accepts, 1);

    let legacy_server = FixtureServer::start(FirstResponseMode::Normal).await;
    let legacy = OllamaBackend::new_preflight(
        legacy_server.endpoint.clone(),
        vec![target()],
        OllamaLimits::default(),
    )
    .expect("legacy backend")
    .preflight(context(&token))
    .await
    .expect("legacy report");
    let legacy_result = legacy_server.finish().await;
    assert_eq!(legacy_result.accepts, 1);
    assert_eq!(retained, legacy);
}
