use super::*;
use crate::{DocumentReviewResult, EditorialReview};
use std::{
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

mod bounds;
mod lifecycle;

fn request(text: &[u8]) -> DocumentReviewOperationRequest {
    DocumentReviewOperationRequest::new(DocumentReviewRequest {
        source: text.to_vec(),
        candidate: None,
        protected_terms: Vec::new(),
    })
    .expect("bounded request")
}

fn until(mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !ready() {
        assert!(Instant::now() < deadline, "worker completion timed out");
        thread::sleep(Duration::from_millis(1));
    }
}

fn accept(
    operation: &mut DocumentReviewOperation,
    mut input: DocumentReviewOperationRequest,
) -> DocumentReviewOperationId {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match operation.submit(input) {
            Ok(id) => return id,
            Err(DocumentReviewOperationSubmissionFailure {
                request,
                error: DocumentReviewOperationError::Busy,
            }) => input = request,
            Err(other) => panic!("unexpected submission: {other:?}"),
        }
        assert!(Instant::now() < deadline, "mailbox contention timed out");
        thread::sleep(Duration::from_millis(1));
    }
}

fn completion(operation: &mut DocumentReviewOperation) -> DocumentReviewOperationCompletion {
    let mut value = None;
    until(|| {
        match operation.poll() {
            Ok(Some(result)) => value = Some(result),
            Ok(None) | Err(DocumentReviewOperationError::Busy) => {}
            other => panic!("unexpected poll: {other:?}"),
        }
        value.is_some()
    });
    value.expect("completion")
}

fn compare(actual: DocumentReviewResult, expected: DocumentReviewResult) {
    assert_eq!(actual.source_preview, expected.source_preview);
    assert_eq!(actual.candidate_preview, expected.candidate_preview);
    assert_eq!(actual.inspection, expected.inspection);
    assert_eq!(actual.candidate_inspection, expected.candidate_inspection);
    assert_eq!(actual.check, expected.check);
    match (actual.editorial, expected.editorial) {
        (EditorialReview::Document(a), EditorialReview::Document(b)) => assert_eq!(a, b),
        (EditorialReview::Comparison(a), EditorialReview::Comparison(b)) => assert_eq!(a, b),
        _ => panic!("different review modes"),
    }
}

#[test]
fn background_review_preserves_exact_service_evidence() {
    for candidate in [None, Some(b"Keep Elsewhere 43 exactly.\n".as_slice())] {
        let input = || DocumentReviewRequest {
            source: b"Keep Acme 42 exactly.\n".to_vec(),
            candidate: candidate.map(<[u8]>::to_vec),
            protected_terms: if candidate.is_some() {
                vec!["Acme".into()]
            } else {
                Vec::new()
            },
        };
        let expected =
            DocumentReviewService::review(input(), &CancellationToken::new()).expect("direct");
        let mut operation = DocumentReviewOperation::new().expect("worker");
        let id = accept(
            &mut operation,
            DocumentReviewOperationRequest::new(input()).expect("bounded"),
        );
        let result = completion(&mut operation);
        assert_eq!(result.operation_id, id);
        compare(result.result.expect("review"), expected);
        assert!(operation.poll().expect("poll").is_none());
    }
}

#[test]
fn latest_pending_coalesces_and_old_running_result_is_discarded() {
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let mut operation = DocumentReviewOperation::start(move |input, token| {
        started_tx
            .send((input.source.clone(), token.clone()))
            .expect("started");
        if input.source == b"first" {
            release_rx.recv().expect("release");
        }
        DocumentReviewService::review(input, &CancellationToken::new())
    })
    .expect("worker");
    let first = accept(&mut operation, request(b"first"));
    let (_, original) = started_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("running");
    let second = accept(&mut operation, request(b"second"));
    let latest = accept(&mut operation, request(b"latest"));
    assert!(first.get() < second.get() && second.get() < latest.get());
    assert!(original.is_cancelled());
    release_tx.send(()).expect("release");
    let (bytes, fresh) = started_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("latest runs");
    assert_eq!(bytes, b"latest");
    assert!(!fresh.is_cancelled());
    let result = completion(&mut operation);
    assert_eq!(result.operation_id, latest);
    assert_eq!(result.result.expect("review").source_preview, "latest");
    assert!(started_rx.try_recv().is_err());
}

#[test]
fn cancellation_reports_once_and_discards_noncooperative_late_result() {
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let mut operation = DocumentReviewOperation::start(move |input, token| {
        started_tx.send(token.clone()).expect("started");
        release_rx.recv().expect("release");
        DocumentReviewService::review(input, &CancellationToken::new())
    })
    .expect("worker");
    let id = accept(&mut operation, request(b"private"));
    let token = started_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("running");
    operation.cancel();
    assert!(token.is_cancelled());
    let result = completion(&mut operation);
    assert_eq!(result.operation_id, id);
    assert!(matches!(
        result.result,
        Err(DocumentReviewOperationError::Cancelled)
    ));
    release_tx.send(()).expect("release");
    operation.shutdown();
    until(|| operation.is_stopped());
    assert!(operation.poll().expect("poll").is_none());
    assert!(
        operation
            .shared
            .mailbox
            .lock()
            .expect("mailbox")
            .completed
            .is_none()
    );
}

#[test]
fn contention_preserves_request_identity_and_original_token() {
    let mut operation = DocumentReviewOperation::new().expect("worker");
    let shared = Arc::clone(&operation.shared);
    let guard = shared.mailbox.lock().expect("hold mailbox");
    let prior = CancellationToken::new();
    operation.latest = Some((DocumentReviewOperationId(7), prior.clone()));
    operation.sequence = 7;
    let failure = operation.submit(request(b"retry bytes")).expect_err("busy");
    assert!(matches!(failure.error, DocumentReviewOperationError::Busy));
    assert_eq!(failure.request.0.source, b"retry bytes");
    assert_eq!(operation.sequence, 7);
    assert!(!prior.is_cancelled());
    assert!(matches!(
        operation.poll(),
        Err(DocumentReviewOperationError::Busy)
    ));
    operation.cancel();
    assert!(prior.is_cancelled());
    drop(guard);
    let id = accept(&mut operation, failure.request);
    assert_eq!(id.get(), 8);
    assert_eq!(completion(&mut operation).operation_id, id);
}

#[test]
fn drop_returns_while_noncooperative_worker_runs_then_supervisor_joins() {
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let mut operation = DocumentReviewOperation::start(move |input, token| {
        started_tx.send(token.clone()).expect("started");
        release_rx.recv().expect("release");
        DocumentReviewService::review(input, &CancellationToken::new())
    })
    .expect("worker");
    accept(&mut operation, request(b"running"));
    let token = started_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("running");
    let shared = Arc::clone(&operation.shared);
    let (dropped_tx, dropped_rx) = mpsc::channel();
    let event = thread::spawn(move || {
        drop(operation);
        dropped_tx.send(()).expect("dropped");
    });
    // No work-release is sent until Drop has returned on the event thread.
    dropped_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("nonblocking drop");
    event.join().expect("event thread");
    assert!(token.is_cancelled());
    assert!(!shared.stopped.load(Ordering::Acquire));
    release_tx.send(()).expect("release");
    until(|| shared.stopped.load(Ordering::Acquire));
    assert!(shared.mailbox.lock().expect("mailbox").pending.is_none());
}

#[test]
fn worker_panic_is_observable_and_future_submission_is_preserved() {
    let mut operation =
        DocumentReviewOperation::start(|_, _| panic!("synthetic worker failure")).expect("worker");
    let id = accept(&mut operation, request(b"private"));
    let result = completion(&mut operation);
    assert_eq!(result.operation_id, id);
    assert!(matches!(
        result.result,
        Err(DocumentReviewOperationError::Stopped)
    ));
    let failure = operation
        .submit(request(b"preserved"))
        .expect_err("stopped");
    assert!(matches!(
        failure.error,
        DocumentReviewOperationError::Stopped
    ));
    assert_eq!(failure.request.0.source, b"preserved");
}

#[test]
fn exhausted_identity_and_stale_completion_cannot_replace_latest() {
    let mut operation = DocumentReviewOperation::new().expect("worker");
    operation.sequence = u64::MAX;
    let failure = operation
        .submit(request(b"unchanged"))
        .expect_err("exhausted");
    assert!(matches!(
        failure.error,
        DocumentReviewOperationError::IdentifierExhausted
    ));
    assert_eq!(failure.request.0.source, b"unchanged");
    operation.latest = Some((DocumentReviewOperationId(9), CancellationToken::new()));
    operation.shared.mailbox.lock().expect("mailbox").completed =
        Some(DocumentReviewOperationCompletion {
            operation_id: DocumentReviewOperationId(8),
            result: Err(DocumentReviewOperationError::Cancelled),
        });
    until(|| match operation.poll() {
        Ok(None) => true,
        Err(DocumentReviewOperationError::Busy) => false,
        other => panic!("unexpected stale poll: {other:?}"),
    });
    assert_eq!(operation.latest.as_ref().expect("latest").0.get(), 9);
    operation.shutdown();
    until(|| operation.is_stopped());
}
