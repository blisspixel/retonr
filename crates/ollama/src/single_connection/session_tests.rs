use std::{sync::Arc, time::Duration};

use rewrite_inference::{InferenceErrorKind, OperationContext};
use rewrite_model::ArtifactId;
use rewrite_types::{CancellationToken, Digest};

use super::{OllamaObservedSessionError, OllamaResponseObservationPhase};
use crate::{
    OLLAMA_RETAINED_SESSION_MAX_INPUT_BYTES, OllamaLimits, OllamaRetainedStreamSessionConfig,
};

use self::fixture::{
    SessionMode, SessionServer, binding, config, context, request, request_with_relaxed_self_limit,
};

mod fixture;
mod judge;
mod residency;

#[tokio::test]
async fn sequential_completions_retain_transport_observer_and_ordinals() {
    let server = SessionServer::start(SessionMode::Normal { completions: 2 }).await;
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
        .expect("open retained session");
    let preflight = session
        .preflight(context(&token))
        .await
        .expect("exact preflight");
    let (first, first_receipt) = session
        .complete_structured(request("first prompt"), context(&token))
        .await
        .expect("first completion");
    let (second, second_receipt) = session
        .complete_structured(request("second prompt"), context(&token))
        .await
        .expect("second completion");
    assert_eq!(first.output_json(), r#"{"candidates":[{"text":"ok"}]}"#);
    assert_eq!(second.output_json(), first.output_json());
    assert_eq!(first_receipt.first_response_ordinal(), 8);
    assert_eq!(first_receipt.last_response_ordinal(), 14);
    assert_eq!(second_receipt.first_response_ordinal(), 15);
    assert_eq!(second_receipt.last_response_ordinal(), 21);
    assert_eq!(
        first_receipt.preflight_digest(),
        second_receipt.preflight_digest()
    );
    assert_ne!(
        first_receipt.request_digest(),
        second_receipt.request_digest()
    );
    assert_ne!(
        first_receipt.response_digest(),
        second_receipt.response_digest()
    );
    assert!(!format!("{first_receipt:?}").contains("first prompt"));
    assert_eq!(preflight.bindings.len(), 1);
    drop(session);
    assert_eq!(
        *phases.lock().expect("phase lock"),
        std::iter::once(OllamaResponseObservationPhase::BeforeResponses)
            .chain(
                (1..=21)
                    .map(|ordinal| { OllamaResponseObservationPhase::AfterResponse { ordinal } })
            )
            .collect::<Vec<_>>()
    );
    let result = server.finish().await;
    assert_eq!(result.accepts, 1);
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
            "GET /api/version",
            "GET /api/tags",
            "POST /api/show",
            "POST /api/generate",
            "GET /api/version",
            "GET /api/tags",
            "POST /api/show",
            "GET /api/version",
            "GET /api/tags",
            "POST /api/show",
            "POST /api/generate",
            "GET /api/version",
            "GET /api/tags",
            "POST /api/show",
        ]
    );
    assert_eq!(result.generate_requests.len(), 2);
    for generated in result.generate_requests {
        assert!(generated.get("keep_alive").is_none());
        assert_eq!(generated["stream"], false);
        assert_eq!(generated["think"], false);
        assert_eq!(generated["raw"], false);
        assert_eq!(generated["options"]["temperature"], 0.0);
    }
}

#[tokio::test]
async fn use_before_preflight_poisons_the_only_transport() {
    let server = SessionServer::start(SessionMode::Normal { completions: 0 }).await;
    let stream = server.supplied_stream().await;
    let token = CancellationToken::new();
    let configured = config(server.endpoint.clone(), OllamaLimits::default());
    let mut session = configured
        .open(stream, context(&token), |_| Ok::<(), ()>(()))
        .await
        .expect("open retained session");
    assert_session_error(
        session
            .complete_structured(request("not admitted"), context(&token))
            .await
            .expect_err("completion before preflight fails"),
        InferenceErrorKind::Policy,
        "session_preflight_required",
    );
    assert_session_error(
        session
            .preflight(context(&token))
            .await
            .expect_err("poisoned session stays closed"),
        InferenceErrorKind::Policy,
        "retained_session_closed",
    );
    drop(session);
    let result = server.finish().await;
    assert_eq!(result.accepts, 1);
    assert!(result.requests.is_empty());
}

#[tokio::test]
async fn oversized_candidate_input_rejects_relaxed_self_limit_without_completion_traffic() {
    let server = SessionServer::start(SessionMode::Normal { completions: 0 }).await;
    let stream = server.supplied_stream().await;
    let token = CancellationToken::new();
    let mut session = config(server.endpoint.clone(), OllamaLimits::default())
        .open(stream, context(&token), |_| Ok::<(), ()>(()))
        .await
        .expect("open input-limited session");
    session
        .preflight(context(&token))
        .await
        .expect("input-limit preflight");
    let limit =
        usize::try_from(OLLAMA_RETAINED_SESSION_MAX_INPUT_BYTES).expect("input ceiling fits usize");
    let oversized = request_with_relaxed_self_limit("x".repeat(limit + 1));
    assert_session_error(
        session
            .complete_structured(oversized, context(&token))
            .await
            .expect_err("protocol ceiling rejects relaxed caller limit"),
        InferenceErrorKind::Policy,
        "retained_session_input_too_large",
    );
    assert_session_error(
        session
            .complete_structured(request("not sent"), context(&token))
            .await
            .expect_err("rejected operation poisons the session"),
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
async fn candidate_input_at_absolute_ceiling_remains_accepted() {
    let server = SessionServer::start(SessionMode::Normal { completions: 1 }).await;
    let stream = server.supplied_stream().await;
    let token = CancellationToken::new();
    let mut session = config(server.endpoint.clone(), OllamaLimits::default())
        .open(stream, context(&token), |_| Ok::<(), ()>(()))
        .await
        .expect("open boundary session");
    session
        .preflight(context(&token))
        .await
        .expect("boundary preflight");
    let limit =
        usize::try_from(OLLAMA_RETAINED_SESSION_MAX_INPUT_BYTES).expect("input ceiling fits usize");
    let at_limit = request_with_relaxed_self_limit("x".repeat(limit));
    session
        .complete_structured(at_limit, context(&token))
        .await
        .expect("exact protocol ceiling remains valid");
    drop(session);
    let result = server.finish().await;
    assert_eq!(result.accepts, 1);
    assert_eq!(result.requests.len(), 14);
    assert_eq!(result.generate_requests.len(), 1);
    assert_eq!(
        result.generate_requests[0]["prompt"]
            .as_str()
            .expect("generated prompt")
            .len(),
        limit
    );
}

#[tokio::test]
async fn wrong_artifact_and_digest_never_send_generation() {
    for wrong_digest in [false, true] {
        let server = SessionServer::start(SessionMode::Normal { completions: 0 }).await;
        let stream = server.supplied_stream().await;
        let token = CancellationToken::new();
        let mut session = config(server.endpoint.clone(), OllamaLimits::default())
            .open(stream, context(&token), |_| Ok::<(), ()>(()))
            .await
            .expect("open retained session");
        session
            .preflight(context(&token))
            .await
            .expect("preflight binding");
        let mut completion = request("wrong identity");
        let other = Digest::sha256(b"other artifact");
        if wrong_digest {
            completion.artifact_digest = other;
        } else {
            completion.artifact_id = ArtifactId::from_digest(other.clone());
            completion.artifact_digest = other;
        }
        let error = session
            .complete_structured(completion, context(&token))
            .await
            .expect_err("wrong identity fails");
        assert_session_error(
            error,
            InferenceErrorKind::Policy,
            if wrong_digest {
                "invalid_structured_completion_request"
            } else {
                "artifact_not_bound"
            },
        );
        drop(session);
        let result = server.finish().await;
        assert_eq!(result.requests.len(), 7);
        assert!(result.generate_requests.is_empty());
    }
}

#[tokio::test]
async fn runtime_inventory_and_details_drift_fail_after_full_checks() {
    for (mode, code, expected_requests) in [
        (
            SessionMode::RuntimeDrift,
            "runtime_changed_after_preflight",
            12,
        ),
        (
            SessionMode::InventoryDrift,
            "inventory_changed_after_preflight",
            13,
        ),
        (
            SessionMode::DetailsDrift,
            "model_details_changed_after_preflight",
            14,
        ),
    ] {
        let (error, result) = complete_once(mode, OllamaLimits::default()).await;
        assert_session_error(error, InferenceErrorKind::Compatibility, code);
        assert_eq!(result.requests.len(), expected_requests);
        assert_eq!(result.generate_requests.len(), 1);
        assert_eq!(result.accepts, 1);
    }
}

#[tokio::test]
async fn rejects_remote_nonterminal_invalid_and_truncated_generation() {
    for (mode, kind, code) in [
        (
            SessionMode::RemoteGeneration,
            InferenceErrorKind::MalformedResponse,
            "invalid_generation_response",
        ),
        (
            SessionMode::NonterminalGeneration,
            InferenceErrorKind::MalformedResponse,
            "invalid_generation_response",
        ),
        (
            SessionMode::InvalidGenerationOutput,
            InferenceErrorKind::MalformedResponse,
            "invalid_candidate_envelope",
        ),
        (
            SessionMode::TruncatedGeneration,
            InferenceErrorKind::Retryable,
            "transport_failed",
        ),
    ] {
        let (error, result) = complete_once(mode, OllamaLimits::default()).await;
        assert_session_error(error, kind, code);
        assert_eq!(result.accepts, 1);
    }
}

#[tokio::test]
async fn observer_failure_closes_before_the_next_request() {
    let server = SessionServer::start(SessionMode::Normal { completions: 1 }).await;
    let stream = server.supplied_stream().await;
    let token = CancellationToken::new();
    let mut session = config(server.endpoint.clone(), OllamaLimits::default())
        .open(stream, context(&token), |observation| {
            if observation.phase()
                == (OllamaResponseObservationPhase::AfterResponse { ordinal: 11 })
            {
                Err("observer stopped")
            } else {
                Ok(())
            }
        })
        .await
        .expect("open retained session");
    session
        .preflight(context(&token))
        .await
        .expect("preflight binding");
    assert!(matches!(
        session
            .complete_structured(request("callback"), context(&token))
            .await,
        Err(OllamaObservedSessionError::Observation("observer stopped"))
    ));
    drop(session);
    let result = server.finish().await;
    assert_eq!(result.requests.len(), 11);
    assert_eq!(result.accepts, 1);
}

#[tokio::test]
async fn close_and_body_limit_fail_without_reconnect() {
    for (mode, limits, kind, code) in [
        (
            SessionMode::CloseGeneration,
            OllamaLimits::default(),
            InferenceErrorKind::MalformedResponse,
            "non_persistent_http_response",
        ),
        (
            SessionMode::LargeGeneration,
            OllamaLimits {
                generation_body_bytes: 64,
                ..OllamaLimits::default()
            },
            InferenceErrorKind::MalformedResponse,
            "response_body_too_large",
        ),
    ] {
        let (error, result) = complete_once(mode, limits).await;
        assert_session_error(error, kind, code);
        assert_eq!(result.accepts, 1);
        assert_eq!(result.requests.len(), 11);
    }
}

#[tokio::test]
async fn deadline_and_cancellation_during_generation_poison_the_session() {
    let deadline_server = SessionServer::start(SessionMode::StallGeneration).await;
    let deadline_stream = deadline_server.supplied_stream().await;
    let deadline_token = CancellationToken::new();
    let limits = OllamaLimits {
        connect_timeout: Duration::from_millis(100),
        request_timeout: Duration::from_secs(1),
        read_timeout: Duration::from_secs(1),
        ..OllamaLimits::default()
    };
    let mut deadline_session = config(deadline_server.endpoint.clone(), limits)
        .open(deadline_stream, context(&deadline_token), |_| {
            Ok::<(), ()>(())
        })
        .await
        .expect("open deadline session");
    deadline_session
        .preflight(context(&deadline_token))
        .await
        .expect("deadline preflight");
    let short = OperationContext::new(
        &deadline_token,
        Some(std::time::Instant::now() + Duration::from_millis(40)),
    );
    assert_session_error(
        deadline_session
            .complete_structured(request("deadline"), short)
            .await
            .expect_err("deadline fails"),
        InferenceErrorKind::Deadline,
        "deadline",
    );
    drop(deadline_session);
    assert_eq!(deadline_server.finish().await.accepts, 1);

    let cancelled_server = SessionServer::start(SessionMode::StallGeneration).await;
    let cancelled_stream = cancelled_server.supplied_stream().await;
    let cancelled_token = CancellationToken::new();
    let mut cancelled_session = config(cancelled_server.endpoint.clone(), limits)
        .open(cancelled_stream, context(&cancelled_token), |_| {
            Ok::<(), ()>(())
        })
        .await
        .expect("open cancelled session");
    cancelled_session
        .preflight(context(&cancelled_token))
        .await
        .expect("cancelled preflight");
    let cancelling = cancelled_token.clone();
    let task = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(40)).await;
        cancelling.cancel();
    });
    assert_session_error(
        cancelled_session
            .complete_structured(request("cancelled"), context(&cancelled_token))
            .await
            .expect_err("cancellation fails"),
        InferenceErrorKind::Cancelled,
        "cancelled",
    );
    task.await.expect("cancellation task");
    drop(cancelled_session);
    assert_eq!(cancelled_server.finish().await.accepts, 1);
}

async fn complete_once(
    mode: SessionMode,
    limits: OllamaLimits,
) -> (OllamaObservedSessionError<()>, fixture::SessionServerResult) {
    let server = SessionServer::start(mode).await;
    let stream = server.supplied_stream().await;
    let token = CancellationToken::new();
    let mut session = config(server.endpoint.clone(), limits)
        .open(stream, context(&token), |_| Ok::<(), ()>(()))
        .await
        .expect("open retained session");
    session
        .preflight(context(&token))
        .await
        .expect("preflight binding");
    let error = session
        .complete_structured(request("judge prompt"), context(&token))
        .await
        .expect_err("adversarial completion fails");
    drop(session);
    (error, server.finish().await)
}

fn assert_session_error<E: std::fmt::Debug>(
    error: OllamaObservedSessionError<E>,
    expected_kind: InferenceErrorKind,
    expected_code: &str,
) {
    let OllamaObservedSessionError::Session(error) = error else {
        panic!("unexpected observer error: {error:?}");
    };
    assert_eq!(error.kind, expected_kind);
    assert_eq!(error.code, expected_code);
}

#[test]
fn config_rejects_duplicate_and_unbounded_session_identity() {
    let endpoint =
        crate::OllamaEndpoint::parse("http://127.0.0.1:11434").expect("loopback endpoint");
    let duplicate = OllamaRetainedStreamSessionConfig::new(
        endpoint.clone(),
        vec![binding(), binding()],
        OllamaLimits::default(),
        1024,
    )
    .expect_err("duplicate bindings fail");
    assert_eq!(duplicate.code, "duplicate_session_binding");
    let empty = OllamaRetainedStreamSessionConfig::new(
        endpoint,
        Vec::new(),
        OllamaLimits::default(),
        usize::MAX,
    )
    .expect_err("empty bindings fail");
    assert_eq!(empty.code, "invalid_session_bindings");
}
