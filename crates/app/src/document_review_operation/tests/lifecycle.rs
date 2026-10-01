use super::*;

#[test]
fn accepting_new_request_discards_already_completed_previous_result() {
    let mut operation = DocumentReviewOperation::new().expect("worker");
    let old = operation.submit(request(b"old")).expect("old");
    until(|| {
        operation
            .shared
            .mailbox
            .lock()
            .expect("mailbox")
            .completed
            .is_some()
    });
    let next = operation.submit(request(b"next")).expect("next");
    assert!(next.get() > old.get());
    let result = completion(&mut operation);
    assert_eq!(result.operation_id, next);
    assert_eq!(result.result.expect("review").source_preview, "next");
    assert!(operation.poll().expect("poll").is_none());
}

#[test]
fn idle_shutdown_cleans_completed_mailbox_and_rejects_submission() {
    let mut operation = DocumentReviewOperation::new().expect("worker");
    operation.submit(request(b"retained")).expect("accepted");
    until(|| {
        operation
            .shared
            .mailbox
            .lock()
            .expect("mailbox")
            .completed
            .is_some()
    });
    operation.shutdown();
    until(|| operation.is_stopped());
    let mailbox = operation.shared.mailbox.lock().expect("mailbox");
    assert!(mailbox.pending.is_none());
    assert!(mailbox.completed.is_none());
    drop(mailbox);
    let failed = operation
        .submit(request(b"preserved"))
        .expect_err("shutdown");
    assert!(matches!(
        failed.error,
        DocumentReviewOperationError::Stopped
    ));
    assert_eq!(failed.request.0.source, b"preserved");
    assert!(matches!(
        completion(&mut operation).result,
        Err(DocumentReviewOperationError::Cancelled)
    ));
}

#[test]
fn poisoned_mailbox_fails_without_accepting_or_cancelling_work() {
    let mut operation = DocumentReviewOperation::new().expect("worker");
    let shared = Arc::clone(&operation.shared);
    thread::spawn(move || {
        let _guard = shared.mailbox.lock().expect("mailbox");
        panic!("synthetic mailbox poison");
    })
    .join()
    .expect_err("poisoned");
    let token = CancellationToken::new();
    operation.latest = Some((DocumentReviewOperationId(1), token.clone()));
    let failure = operation.submit(request(b"private")).expect_err("poison");
    assert!(matches!(
        failure.error,
        DocumentReviewOperationError::Stopped
    ));
    assert_eq!(failure.request.0.source, b"private");
    assert!(!token.is_cancelled());
    assert!(matches!(
        operation.poll(),
        Err(DocumentReviewOperationError::Stopped)
    ));
    operation.shutdown();
    until(|| operation.is_stopped());
}
