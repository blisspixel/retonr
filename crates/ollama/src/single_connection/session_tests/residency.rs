use std::{sync::Arc, time::Duration};

use rewrite_inference::{InferenceErrorKind, OperationContext, StructuredCompletionResponse};
use rewrite_model::ArtifactId;
use rewrite_types::{CancellationToken, Digest};

use super::{OllamaObservedSessionError, OllamaResponseObservationPhase, assert_session_error};
use crate::{
    OLLAMA_RESIDENT_COMPLETION_KEEP_ALIVE, OLLAMA_RESIDENT_COMPLETION_RUNTIME_VERSION,
    OLLAMA_RESIDENT_COMPLETION_SOURCE_REVISION, OLLAMA_RETAINED_SESSION_MAX_INPUT_BYTES,
    OllamaLimits, OllamaResidentSessionExecutionReceipt,
};

use super::fixture::{
    ARTIFACT_DIGEST, INVENTORY_DIGEST, MODEL, SessionMode, SessionServer, config, context,
    judge_request_with_relaxed_self_limit, request,
};

#[tokio::test]
async fn resident_completions_bind_exact_sequence_residency_and_ordinals() {
    let server = SessionServer::start(SessionMode::ResidentNormal { completions: 2 }).await;
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
        .expect("open resident session");
    let preflight = session
        .preflight(context(&token))
        .await
        .expect("idle reviewed preflight");
    assert_eq!(
        preflight.runtime.version,
        OLLAMA_RESIDENT_COMPLETION_RUNTIME_VERSION
    );

    let (first, first_receipt) = session
        .complete_structured_with_residency(request("first resident"), context(&token))
        .await
        .expect("first resident completion");
    let (_second, second_receipt) = session
        .complete_structured_with_residency(request("second resident"), context(&token))
        .await
        .expect("second resident completion");

    assert_eq!(first_receipt.execution().first_response_ordinal(), 8);
    assert_eq!(first_receipt.execution().last_response_ordinal(), 16);
    assert_eq!(first_receipt.first_residency_ordinal(), 12);
    assert_eq!(first_receipt.last_residency_ordinal(), 16);
    assert_eq!(second_receipt.execution().first_response_ordinal(), 17);
    assert_eq!(second_receipt.execution().last_response_ordinal(), 25);
    assert_eq!(second_receipt.first_residency_ordinal(), 21);
    assert_eq!(second_receipt.last_residency_ordinal(), 25);
    assert_distinct_model_identities(&first, &first_receipt);
    assert_eq!(
        first_receipt.runtime_reference_digest(),
        &Digest::sha256(MODEL.as_bytes())
    );
    assert_eq!(first_receipt.byte_size(), 1024);
    assert_eq!(first_receipt.accelerator_bytes(), 256);
    assert_eq!(first_receipt.context_tokens(), 2048);
    assert!(first_receipt.runtime_reported_residency_proven());
    assert!(!first_receipt.application_handler_proven());
    assert!(!first_receipt.model_use_proven());
    assert!(!first_receipt.resident_page_identity_proven());
    assert!(!first_receipt.effective_runtime_identity_proven());
    assert!(!first_receipt.qualified());
    assert_eq!(
        first_receipt.residency_contract_digest(),
        second_receipt.residency_contract_digest()
    );
    assert_ne!(
        first_receipt.residency_observation_digest(),
        second_receipt.residency_observation_digest()
    );
    let debug = format!("{first_receipt:?}");
    assert!(!debug.contains(MODEL));
    assert!(!debug.contains("first resident"));

    drop(session);
    assert_eq!(
        *phases.lock().expect("phase lock"),
        std::iter::once(OllamaResponseObservationPhase::BeforeResponses)
            .chain(
                (1..=25).map(|ordinal| OllamaResponseObservationPhase::AfterResponse { ordinal })
            )
            .collect::<Vec<_>>()
    );
    let result = server.finish().await;
    assert_eq!(result.accepts, 1);
    assert_eq!(
        &result.requests[7..16],
        [
            "GET /api/version",
            "GET /api/tags",
            "POST /api/show",
            "POST /api/generate",
            "GET /api/ps",
            "GET /api/version",
            "GET /api/tags",
            "POST /api/show",
            "GET /api/ps",
        ]
    );
    for generated in result.generate_requests {
        assert_eq!(
            generated["keep_alive"],
            OLLAMA_RESIDENT_COMPLETION_KEEP_ALIVE
        );
    }
    assert_eq!(
        OLLAMA_RESIDENT_COMPLETION_SOURCE_REVISION,
        "b7871fc0d1d82fe109536efa3e0e8e411c766c75"
    );
}

fn assert_distinct_model_identities(
    response: &StructuredCompletionResponse,
    receipt: &OllamaResidentSessionExecutionReceipt,
) {
    assert_ne!(receipt.inventory_digest(), response.artifact_digest());
    assert_eq!(receipt.inventory_digest().as_str(), INVENTORY_DIGEST);
    assert_eq!(response.artifact_digest().as_str(), ARTIFACT_DIGEST);
}

#[tokio::test]
async fn residency_absence_ambiguity_mismatch_and_drift_fail_closed() {
    for (mode, code, request_count) in [
        (
            SessionMode::ResidentDrift,
            "model_residency_changed_after_generation",
            16,
        ),
        (
            SessionMode::ResidentAmbiguous,
            "model_residency_ambiguous",
            12,
        ),
        (
            SessionMode::ResidentDelayed,
            "model_residency_not_observed",
            12,
        ),
        (
            SessionMode::ResidentUnloaded,
            "model_residency_not_observed",
            16,
        ),
        (
            SessionMode::ResidentWrongDigest,
            "model_residency_mismatch",
            12,
        ),
        (
            SessionMode::ResidentWrongSize,
            "model_residency_changed_after_generation",
            16,
        ),
        (
            SessionMode::ResidentWrongContext,
            "model_residency_mismatch",
            12,
        ),
        (
            SessionMode::ResidentWrongReference,
            "invalid_running_inventory_entry",
            12,
        ),
    ] {
        let (error, result) = resident_error(mode, OllamaLimits::default()).await;
        assert_session_error(
            error,
            if matches!(mode, SessionMode::ResidentWrongReference) {
                InferenceErrorKind::MalformedResponse
            } else {
                InferenceErrorKind::Compatibility
            },
            code,
        );
        assert_eq!(result.requests.len(), request_count);
        assert_eq!(result.generate_requests.len(), 1);
        assert_eq!(result.accepts, 1);
    }
}

#[tokio::test]
async fn resident_profile_rejects_unreviewed_or_nonidle_preflight_before_generation() {
    for (mode, kind, code) in [
        (
            SessionMode::Normal { completions: 0 },
            InferenceErrorKind::Compatibility,
            "resident_completion_runtime_unreviewed",
        ),
        (
            SessionMode::ResidentPreloaded,
            InferenceErrorKind::Policy,
            "resident_completion_requires_idle_preflight",
        ),
    ] {
        let (error, result) = resident_error(mode, OllamaLimits::default()).await;
        assert_session_error(error, kind, code);
        assert_eq!(result.requests.len(), 7);
        assert!(result.generate_requests.is_empty());
        assert_eq!(result.accepts, 1);
    }
}

#[tokio::test]
async fn resident_profile_rejects_closed_unpreflighted_and_unbound_use_without_traffic() {
    for failure in ["closed", "unpreflighted", "unbound"] {
        let server = SessionServer::start(SessionMode::ResidentNormal { completions: 0 }).await;
        let stream = server.supplied_stream().await;
        let token = CancellationToken::new();
        let mut session = config(server.endpoint.clone(), OllamaLimits::default())
            .open(stream, context(&token), |_| Ok::<(), ()>(()))
            .await
            .expect("open rejected resident session");
        if failure == "closed" {
            session.invalidate();
        } else if failure == "unbound" {
            session
                .preflight(context(&token))
                .await
                .expect("unbound preflight");
        }
        let mut attempted = request("not admitted");
        if failure == "unbound" {
            let other = Digest::sha256(b"unbound resident model");
            attempted.artifact_id = ArtifactId::from_digest(other.clone());
            attempted.artifact_digest = other;
        }
        let error = session
            .complete_structured_with_residency(attempted, context(&token))
            .await
            .expect_err("resident use fails before generation");
        assert_session_error(
            error,
            InferenceErrorKind::Policy,
            match failure {
                "closed" => "retained_session_closed",
                "unpreflighted" => "session_preflight_required",
                "unbound" => "artifact_not_bound",
                _ => unreachable!("fixed failure case"),
            },
        );
        drop(session);
        let result = server.finish().await;
        assert_eq!(
            result.requests.len(),
            if failure == "unbound" { 7 } else { 0 }
        );
        assert!(result.generate_requests.is_empty());
        assert_eq!(result.accepts, 1);
    }
}

#[tokio::test]
async fn oversized_judge_input_rejects_residency_before_completion_traffic() {
    let server = SessionServer::start(SessionMode::ResidentNormal { completions: 0 }).await;
    let stream = server.supplied_stream().await;
    let token = CancellationToken::new();
    let mut session = config(server.endpoint.clone(), OllamaLimits::default())
        .open(stream, context(&token), |_| Ok::<(), ()>(()))
        .await
        .expect("open resident input-limited session");
    session
        .preflight(context(&token))
        .await
        .expect("resident input-limit preflight");
    let limit =
        usize::try_from(OLLAMA_RETAINED_SESSION_MAX_INPUT_BYTES).expect("input ceiling fits usize");
    let oversized = judge_request_with_relaxed_self_limit("x".repeat(limit + 1));
    assert_session_error(
        session
            .complete_structured_with_residency(oversized, context(&token))
            .await
            .expect_err("resident profile rejects relaxed caller limit"),
        InferenceErrorKind::Policy,
        "retained_session_input_too_large",
    );
    assert_session_error(
        session
            .complete_structured_with_residency(request("not sent"), context(&token))
            .await
            .expect_err("rejected resident operation poisons the session"),
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
async fn callback_close_and_body_limit_poison_without_reconnect() {
    let callback_server =
        SessionServer::start(SessionMode::ResidentNormal { completions: 1 }).await;
    let callback_stream = callback_server.supplied_stream().await;
    let token = CancellationToken::new();
    let mut callback_session = config(callback_server.endpoint.clone(), OllamaLimits::default())
        .open(callback_stream, context(&token), |observation| {
            if observation.phase()
                == (OllamaResponseObservationPhase::AfterResponse { ordinal: 12 })
            {
                Err("residency observer stopped")
            } else {
                Ok(())
            }
        })
        .await
        .expect("open callback session");
    callback_session
        .preflight(context(&token))
        .await
        .expect("callback preflight");
    assert!(matches!(
        callback_session
            .complete_structured_with_residency(request("callback"), context(&token))
            .await,
        Err(OllamaObservedSessionError::Observation(
            "residency observer stopped"
        ))
    ));
    drop(callback_session);
    let callback_result = callback_server.finish().await;
    assert_eq!(callback_result.requests.len(), 12);
    assert_eq!(callback_result.accepts, 1);

    let (close_error, close_result) =
        resident_error(SessionMode::CloseResidency, OllamaLimits::default()).await;
    assert_session_error(
        close_error,
        InferenceErrorKind::MalformedResponse,
        "non_persistent_http_response",
    );
    assert_eq!(close_result.requests.len(), 12);
    assert_eq!(close_result.accepts, 1);

    let limits = OllamaLimits {
        discovery_body_bytes: 2048,
        ..OllamaLimits::default()
    };
    let (large_error, large_result) = resident_error(SessionMode::LargeResidency, limits).await;
    assert_session_error(
        large_error,
        InferenceErrorKind::MalformedResponse,
        "response_body_too_large",
    );
    assert_eq!(large_result.requests.len(), 12);
    assert_eq!(large_result.accepts, 1);
}

#[tokio::test]
async fn residency_wait_obeys_deadline_without_reconnect() {
    let (error, result) = stalled_residency_error(Duration::from_millis(40), None).await;
    assert_session_error(error, InferenceErrorKind::Deadline, "deadline");
    assert_eq!(result.requests.len(), 12);
    assert_eq!(result.accepts, 1);
}

#[tokio::test]
async fn residency_wait_obeys_cancellation_without_reconnect() {
    let (error, result) =
        stalled_residency_error(Duration::from_secs(30), Some(Duration::from_millis(20))).await;
    assert_session_error(error, InferenceErrorKind::Cancelled, "cancelled");
    assert_eq!(result.requests.len(), 12);
    assert_eq!(result.accepts, 1);
}

async fn stalled_residency_error(
    deadline_from_now: Duration,
    cancel_after: Option<Duration>,
) -> (
    OllamaObservedSessionError<()>,
    super::fixture::SessionServerResult,
) {
    let server = SessionServer::start(SessionMode::StallResidency).await;
    let stream = server.supplied_stream().await;
    let token = CancellationToken::new();
    let limits = OllamaLimits {
        connect_timeout: Duration::from_millis(100),
        request_timeout: Duration::from_secs(1),
        read_timeout: Duration::from_secs(1),
        ..OllamaLimits::default()
    };
    let mut session = config(server.endpoint.clone(), limits)
        .open(stream, context(&token), |_| Ok::<(), ()>(()))
        .await
        .expect("open timed resident session");
    session
        .preflight(context(&token))
        .await
        .expect("timed resident preflight");
    if let Some(delay) = cancel_after {
        let cancelling = token.clone();
        tokio::spawn(async move {
            tokio::time::sleep(delay).await;
            cancelling.cancel();
        });
    }
    let error = session
        .complete_structured_with_residency(
            request("timed"),
            OperationContext::new(&token, Some(std::time::Instant::now() + deadline_from_now)),
        )
        .await
        .expect_err("timed residency fails");
    drop(session);
    (error, server.finish().await)
}

async fn resident_error(
    mode: SessionMode,
    limits: OllamaLimits,
) -> (
    OllamaObservedSessionError<()>,
    super::fixture::SessionServerResult,
) {
    let server = SessionServer::start(mode).await;
    let stream = server.supplied_stream().await;
    let token = CancellationToken::new();
    let mut session = config(server.endpoint.clone(), limits)
        .open(stream, context(&token), |_| Ok::<(), ()>(()))
        .await
        .expect("open resident failure session");
    session
        .preflight(context(&token))
        .await
        .expect("resident failure preflight");
    let error = session
        .complete_structured_with_residency(request("resident failure"), context(&token))
        .await
        .expect_err("resident completion fails");
    drop(session);
    (error, server.finish().await)
}
