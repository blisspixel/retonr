use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use rewrite_inference::{
    InferenceErrorKind, OperationContext, StructuredCompletionRequest, StructuredCompletionResponse,
};
use rewrite_types::{CancellationToken, Digest};

use super::{
    OllamaResponseObservationPhase, assert_session_error,
    fixture::{
        SessionMode, SessionServer, SessionServerResult, config, context, judge_request,
        judge_request_with_relaxed_self_limit,
    },
};
use crate::{
    OLLAMA_RETAINED_SESSION_MAX_INPUT_BYTES, OllamaLimits, OllamaResidentSessionExecutionReceipt,
    derive_ollama_retained_session_response_id,
};

struct CompletionRun {
    response: StructuredCompletionResponse,
    receipt: OllamaResidentSessionExecutionReceipt,
    phases: Vec<OllamaResponseObservationPhase>,
    server: SessionServerResult,
}

#[tokio::test]
async fn borrowed_and_consuming_resident_paths_are_exactly_equal() {
    let borrowed = complete_once(true).await;
    let consuming = complete_once(false).await;

    assert_eq!(borrowed.response, consuming.response);
    assert_eq!(borrowed.receipt, consuming.receipt);
    assert_eq!(
        derive_ollama_retained_session_response_id(&borrowed.response),
        derive_ollama_retained_session_response_id(&consuming.response)
    );
    assert_eq!(
        borrowed.receipt.complete_binding_digest(),
        consuming.receipt.complete_binding_digest()
    );
    assert_eq!(borrowed.phases, consuming.phases);
    assert_eq!(borrowed.server.requests, consuming.server.requests);
    assert_eq!(
        borrowed.server.generate_request_bodies,
        consuming.server.generate_request_bodies
    );
    assert_eq!(
        borrowed.server.generate_requests,
        consuming.server.generate_requests
    );
}

#[tokio::test]
async fn borrowed_request_stays_caller_owned_and_is_not_retained() {
    let server = SessionServer::start(SessionMode::ResidentJudgeOutput { completions: 1 }).await;
    let stream = server.supplied_stream().await;
    let token = CancellationToken::new();
    let mut session = config(server.endpoint.clone(), OllamaLimits::default())
        .open(stream, context(&token), |_| Ok::<(), ()>(()))
        .await
        .expect("open borrowed ownership session");
    session
        .preflight(context(&token))
        .await
        .expect("borrowed ownership preflight");
    let limit =
        usize::try_from(OLLAMA_RETAINED_SESSION_MAX_INPUT_BYTES).expect("input limit fits usize");
    let drops = Arc::new(AtomicUsize::new(0));
    let mut owner = RequestOwner {
        request: judge_request_with_relaxed_self_limit("x".repeat(limit)),
        drops: Arc::clone(&drops),
    };
    let original_binding = owner.request.binding_digest();
    let original_pointer = owner.request.input.as_ptr();
    let original_capacity = owner.request.input.capacity();

    let (response, receipt) = session
        .complete_structured_with_residency_borrowed(&owner.request, context(&token))
        .await
        .expect("borrowed exact-limit request");

    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert_eq!(owner.request.input.as_ptr(), original_pointer);
    assert_eq!(owner.request.input.capacity(), original_capacity);
    assert_eq!(owner.request.binding_digest(), original_binding);
    assert_eq!(response.request_binding_digest(), &original_binding);
    assert_eq!(receipt.execution().request_digest(), &original_binding);

    owner.request.input.push('!');
    owner.request.source_byte_count += 1;
    owner.request.source_byte_limit += 1;
    assert_ne!(owner.request.binding_digest(), original_binding);
    assert_eq!(receipt.execution().request_digest(), &original_binding);

    drop(owner);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    drop(session);
    let result = server.finish().await;
    assert_eq!(result.generate_request_bodies.len(), 1);
    assert_eq!(
        result.generate_requests[0]["prompt"]
            .as_str()
            .expect("wire prompt")
            .len(),
        limit
    );
}

#[tokio::test]
async fn borrowed_validation_and_cancellation_fail_before_traffic_and_poison() {
    let invalid_server =
        SessionServer::start(SessionMode::ResidentJudgeOutput { completions: 0 }).await;
    let invalid_stream = invalid_server.supplied_stream().await;
    let invalid_token = CancellationToken::new();
    let mut invalid_session = config(invalid_server.endpoint.clone(), OllamaLimits::default())
        .open(
            invalid_stream,
            context(&invalid_token),
            |_| Ok::<(), ()>(()),
        )
        .await
        .expect("open invalid borrowed session");
    invalid_session
        .preflight(context(&invalid_token))
        .await
        .expect("invalid borrowed preflight");
    let mut invalid_request = judge_request("mutated invalid request");
    invalid_request.source_byte_limit = 0;
    assert_session_error(
        invalid_session
            .complete_structured_with_residency_borrowed(&invalid_request, context(&invalid_token))
            .await
            .expect_err("borrowed mutation fails"),
        InferenceErrorKind::Policy,
        "invalid_structured_completion_request",
    );
    assert_eq!(invalid_request.input, "mutated invalid request");
    drop(invalid_session);
    let invalid_result = invalid_server.finish().await;
    assert_eq!(invalid_result.requests.len(), 7);
    assert!(invalid_result.generate_request_bodies.is_empty());

    let cancelled_server =
        SessionServer::start(SessionMode::ResidentJudgeOutput { completions: 0 }).await;
    let cancelled_stream = cancelled_server.supplied_stream().await;
    let cancelled_token = CancellationToken::new();
    let mut cancelled_session = config(cancelled_server.endpoint.clone(), OllamaLimits::default())
        .open(cancelled_stream, context(&cancelled_token), |_| {
            Ok::<(), ()>(())
        })
        .await
        .expect("open cancelled borrowed session");
    cancelled_session
        .preflight(context(&cancelled_token))
        .await
        .expect("cancelled borrowed preflight");
    let cancelled_request = judge_request("cancelled borrowed request");
    let binding = cancelled_request.binding_digest();
    cancelled_token.cancel();
    assert_session_error(
        cancelled_session
            .complete_structured_with_residency_borrowed(
                &cancelled_request,
                OperationContext::new(
                    &cancelled_token,
                    Some(std::time::Instant::now() + Duration::from_secs(5)),
                ),
            )
            .await
            .expect_err("borrowed cancellation fails"),
        InferenceErrorKind::Cancelled,
        "cancelled",
    );
    assert_eq!(cancelled_request.binding_digest(), binding);
    let fresh_token = CancellationToken::new();
    assert_session_error(
        cancelled_session
            .complete_structured_with_residency_borrowed(&cancelled_request, context(&fresh_token))
            .await
            .expect_err("cancelled operation permanently poisons session"),
        InferenceErrorKind::Policy,
        "retained_session_closed",
    );
    drop(cancelled_session);
    let cancelled_result = cancelled_server.finish().await;
    assert_eq!(cancelled_result.requests.len(), 7);
    assert!(cancelled_result.generate_request_bodies.is_empty());
}

#[tokio::test]
async fn borrowed_and_consuming_calls_share_ordinals_callbacks_and_wire() {
    let server = SessionServer::start(SessionMode::ResidentJudgeOutput { completions: 2 }).await;
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
        .expect("open mixed ownership session");
    session
        .preflight(context(&token))
        .await
        .expect("mixed ownership preflight");
    let borrowed_request = judge_request("same judge request");
    let (borrowed_response, borrowed_receipt) = session
        .complete_structured_with_residency_borrowed(&borrowed_request, context(&token))
        .await
        .expect("borrowed completion");
    let (consuming_response, consuming_receipt) = session
        .complete_structured_with_residency(judge_request("same judge request"), context(&token))
        .await
        .expect("consuming completion");

    assert_eq!(borrowed_response, consuming_response);
    assert_eq!(borrowed_receipt.execution().first_response_ordinal(), 8);
    assert_eq!(borrowed_receipt.execution().last_response_ordinal(), 16);
    assert_eq!(borrowed_receipt.first_residency_ordinal(), 12);
    assert_eq!(borrowed_receipt.last_residency_ordinal(), 16);
    assert_eq!(consuming_receipt.execution().first_response_ordinal(), 17);
    assert_eq!(consuming_receipt.execution().last_response_ordinal(), 25);
    assert_eq!(consuming_receipt.first_residency_ordinal(), 21);
    assert_eq!(consuming_receipt.last_residency_ordinal(), 25);
    assert_eq!(
        borrowed_receipt.execution().request_digest(),
        consuming_receipt.execution().request_digest()
    );
    assert_eq!(
        borrowed_receipt.execution().response_digest(),
        consuming_receipt.execution().response_digest()
    );
    assert_ne!(
        borrowed_receipt.complete_binding_digest(),
        consuming_receipt.complete_binding_digest()
    );

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
    assert_eq!(result.generate_request_bodies.len(), 2);
    assert_eq!(
        result.generate_request_bodies[0],
        result.generate_request_bodies[1]
    );
}

async fn complete_once(borrowed: bool) -> CompletionRun {
    let server = SessionServer::start(SessionMode::ResidentJudgeOutput { completions: 1 }).await;
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
        .expect("open equality session");
    session
        .preflight(context(&token))
        .await
        .expect("equality preflight");
    let request = judge_request("identity equality judge request");
    let expected_binding = request.binding_digest();
    let expected_binding_id = request.structured_request_binding_id();
    let (response, receipt) = if borrowed {
        session
            .complete_structured_with_residency_borrowed(&request, context(&token))
            .await
            .expect("borrowed resident completion")
    } else {
        session
            .complete_structured_with_residency(request, context(&token))
            .await
            .expect("consuming resident completion")
    };
    assert_eq!(response.request_binding_digest(), &expected_binding);
    assert_eq!(receipt.execution().request_digest(), &expected_binding);
    assert_eq!(expected_binding_id.digest(), &expected_binding);
    drop(session);
    let phases = phases.lock().expect("phase lock").clone();
    CompletionRun {
        response,
        receipt,
        phases,
        server: server.finish().await,
    }
}

struct RequestOwner {
    request: StructuredCompletionRequest,
    drops: Arc<AtomicUsize>,
}

impl Drop for RequestOwner {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn mutation_changes_the_frozen_request_binding() {
    let mut request = judge_request("binding mutation");
    let original = request.binding_digest();
    request.sampling.seed = Some(8);
    assert_ne!(request.binding_digest(), original);
    assert_ne!(request.binding_digest(), Digest::sha256(b"unrelated"));
}
