use super::*;

#[test]
fn stale_and_duplicate_completions_cannot_replace_latest_evidence() {
    let mut state = DocumentReviewState::new().expect("state");
    let old = accept(&mut state, request(b"old", None));
    let latest = accept(&mut state, request(b"latest", None));
    let old_completion = || DocumentReviewOperationCompletion {
        operation_id: old,
        result: Err(DocumentReviewOperationError::Cancelled),
    };
    assert!(!state.apply(old_completion()));
    assert_eq!(state.phase(), DocumentReviewPhase::Loading);
    assert_eq!(state.operation_id(), Some(latest));
    settle(&mut state);
    assert_eq!(state.result().expect("latest").source_preview, "latest");
    assert!(!state.apply(old_completion()));
    assert!(!state.apply(DocumentReviewOperationCompletion {
        operation_id: latest,
        result: Err(DocumentReviewOperationError::Stopped)
    }));
    assert_eq!(state.phase(), DocumentReviewPhase::Ready);
    assert_eq!(state.result().expect("unchanged").source_preview, "latest");
}

#[test]
fn actual_busy_submission_keeps_owned_request_and_ready_selection() {
    let mut state = DocumentReviewState::new().expect("state");
    let id = accept(&mut state, request(b"prior", None));
    settle(&mut state);
    let prior_disclosure = state.input_disclosure();
    let (release, holder) = state.operation.hold_mailbox_for_test();
    let failure = state
        .submit(request(b"retry", Some(b"candidate")))
        .expect_err("busy refused");
    assert!(matches!(failure.error, DocumentReviewOperationError::Busy));
    assert_eq!(failure.request.source_bytes(), 5);
    assert_eq!(failure.request.candidate_bytes(), Some(9));
    assert_eq!(state.operation_id(), Some(id));
    assert_eq!(state.input_disclosure(), prior_disclosure);
    assert_eq!(state.phase(), DocumentReviewPhase::Ready);
    assert_eq!(state.result().expect("retained").source_preview, "prior");
    assert!(!state.cancellation_requested());
    release.send(()).expect("release mailbox");
    holder.join().expect("holder");
    accept(&mut state, failure.request);
    assert_eq!(
        state.input_disclosure(),
        Some(DocumentReviewInputDisclosure {
            source_bytes: 5,
            candidate_bytes: Some(9)
        })
    );
}

#[test]
fn actual_busy_submission_and_poll_preserve_loading_selection_and_request() {
    let mut state = DocumentReviewState::new().expect("state");
    let id = accept(&mut state, request(b"prior", None));
    let (release, holder) = state.operation.hold_mailbox_for_test();
    let failed = state
        .submit(request(b"retry", Some(b"candidate")))
        .expect_err("actual busy");
    assert!(matches!(failed.error, DocumentReviewOperationError::Busy));
    assert_eq!(failed.request.source_bytes(), 5);
    assert_eq!(failed.request.candidate_bytes(), Some(9));
    assert_eq!(state.phase(), DocumentReviewPhase::Loading);
    assert_eq!(state.operation_id(), Some(id));
    assert_eq!(
        state.input_disclosure(),
        Some(DocumentReviewInputDisclosure {
            source_bytes: 5,
            candidate_bytes: None
        })
    );
    assert!(matches!(
        state.poll(),
        Err(DocumentReviewOperationError::Busy)
    ));
    assert_eq!(state.phase(), DocumentReviewPhase::Loading);
    assert!(!state.cancellation_requested());
    release.send(()).expect("release mailbox");
    holder.join().expect("holder");
    accept(&mut state, failed.request);
    settle(&mut state);
    assert_eq!(
        state.input_disclosure(),
        Some(DocumentReviewInputDisclosure {
            source_bytes: 5,
            candidate_bytes: Some(9)
        })
    );
}

#[test]
fn poisoned_mailbox_settles_loading_failure_without_claiming_worker_exit() {
    let mut state = DocumentReviewState::new().expect("state");
    let id = accept(&mut state, request(b"prior", None));
    state.operation.poison_mailbox_for_test();
    assert!(state.poll().expect("terminal failure accepted"));
    assert_eq!(state.phase(), DocumentReviewPhase::Failed);
    assert_eq!(state.operation_id(), Some(id));
    assert_eq!(
        state.input_disclosure(),
        Some(DocumentReviewInputDisclosure {
            source_bytes: 5,
            candidate_bytes: None
        })
    );
    assert!(matches!(
        state.error(),
        Some(DocumentReviewOperationError::Stopped)
    ));
    assert!(state.result().is_none());
    assert!(!state.cancellation_requested());
}

#[test]
fn real_shutdown_refusal_preserves_loading_selection_and_cancellation() {
    let mut state = DocumentReviewState::new().expect("state");
    let id = accept(&mut state, request(b"prior", None));
    state.cancel();
    state.operation.shutdown();
    let failure = state
        .submit(request(b"rejected candidate", Some(b"candidate")))
        .expect_err("stopped");
    assert!(matches!(
        failure.error,
        DocumentReviewOperationError::Stopped
    ));
    assert_eq!(failure.request.source_bytes(), 18);
    assert_eq!(state.phase(), DocumentReviewPhase::Loading);
    assert_eq!(state.operation_id(), Some(id));
    assert!(state.cancellation_requested());
    assert_eq!(
        state.input_disclosure(),
        Some(DocumentReviewInputDisclosure {
            source_bytes: 5,
            candidate_bytes: None
        })
    );
    settle(&mut state);
    assert_eq!(state.phase(), DocumentReviewPhase::Cancelled);
}

#[test]
fn original_service_cancellation_and_worker_failure_reduce_to_explicit_phases() {
    for error in [
        DocumentReviewOperationError::Review(DocumentReviewError::Cancelled),
        DocumentReviewOperationError::Stopped,
    ] {
        let mut state = DocumentReviewState::new().expect("state");
        let id = accept(&mut state, request(b"source", None));
        assert!(state.apply(DocumentReviewOperationCompletion {
            operation_id: id,
            result: Err(error)
        }));
        assert!(state.result().is_none());
        match state.phase() {
            DocumentReviewPhase::Cancelled => assert!(state.error().is_none()),
            DocumentReviewPhase::Failed => assert!(matches!(
                state.error(),
                Some(DocumentReviewOperationError::Stopped)
            )),
            other => panic!("unexpected phase: {other:?}"),
        }
    }
}
