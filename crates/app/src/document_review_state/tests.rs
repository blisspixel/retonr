use super::*;
use crate::{DocumentReviewRequest, DocumentReviewService, EditorialReview};
use rewrite_types::CancellationToken;
use std::{
    thread,
    time::{Duration, Instant},
};

mod transitions;

fn request(source: &[u8], candidate: Option<&[u8]>) -> DocumentReviewOperationRequest {
    DocumentReviewOperationRequest::new(DocumentReviewRequest {
        source: source.to_vec(),
        candidate: candidate.map(<[u8]>::to_vec),
        protected_terms: Vec::new(),
    })
    .expect("bounded input")
}

fn settle(state: &mut DocumentReviewState) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while state.phase() == DocumentReviewPhase::Loading {
        match state.poll() {
            Ok(_) | Err(DocumentReviewOperationError::Busy) => {}
            other => panic!("unexpected poll: {other:?}"),
        }
        assert!(Instant::now() < deadline, "state completion timed out");
        thread::sleep(Duration::from_millis(1));
    }
}

fn accept(
    state: &mut DocumentReviewState,
    mut input: DocumentReviewOperationRequest,
) -> DocumentReviewOperationId {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match state.submit(input) {
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

#[test]
fn idle_loading_and_source_candidate_disclosure_are_explicit() {
    let mut state = DocumentReviewState::new().expect("state");
    assert_eq!(state.phase(), DocumentReviewPhase::Idle);
    assert!(state.operation_id().is_none());
    assert!(state.input_disclosure().is_none());
    assert!(!state.cancellation_requested());
    assert!(state.result().is_none());
    assert!(state.error().is_none());
    assert!(!state.poll().expect("idle poll"));
    state.cancel();
    assert!(!state.cancellation_requested());
    for candidate in [None, Some(b"".as_slice()), Some(b"candidate".as_slice())] {
        let id = accept(&mut state, request(b"source", candidate));
        assert_eq!(state.operation_id(), Some(id));
        assert_eq!(state.phase(), DocumentReviewPhase::Loading);
        assert_eq!(
            state.input_disclosure(),
            Some(DocumentReviewInputDisclosure {
                source_bytes: 6,
                candidate_bytes: candidate.map(<[u8]>::len)
            })
        );
        assert!(state.result().is_none());
        assert!(state.error().is_none());
        settle(&mut state);
        assert_eq!(state.operation_id(), Some(id));
    }
}

#[test]
fn ready_keeps_exact_evidence_and_untrusted_text_untouched() {
    let source = b"Certainly!\nKeep Acme 42 exactly.\x1b[2J\n";
    let candidate = b"Keep Elsewhere 43 exactly.\x1b[2J\n";
    let input = || DocumentReviewRequest {
        source: source.to_vec(),
        candidate: Some(candidate.to_vec()),
        protected_terms: vec!["Acme".into()],
    };
    let expected =
        DocumentReviewService::review(input(), &CancellationToken::new()).expect("direct service");
    let mut state = DocumentReviewState::new().expect("state");
    accept(
        &mut state,
        DocumentReviewOperationRequest::new(input()).expect("bounded"),
    );
    settle(&mut state);
    assert_eq!(state.phase(), DocumentReviewPhase::Ready);
    let actual = state.result().expect("ready evidence");
    assert_eq!(actual.source_preview, expected.source_preview);
    assert_eq!(actual.candidate_preview, expected.candidate_preview);
    assert_eq!(actual.inspection, expected.inspection);
    assert_eq!(actual.candidate_inspection, expected.candidate_inspection);
    assert_eq!(actual.check, expected.check);
    let (EditorialReview::Comparison(actual), EditorialReview::Comparison(expected)) =
        (&actual.editorial, &expected.editorial)
    else {
        panic!("comparison")
    };
    assert_eq!(actual, expected);
    assert!(!format!("{state:?}").contains("Acme"));
    assert!(!format!("{state:?}").contains('\u{1b}'));
    state.cancel();
    assert_eq!(state.phase(), DocumentReviewPhase::Ready);
    assert!(!state.cancellation_requested());
    assert!(!state.poll().expect("terminal poll"));
}

#[test]
fn failed_review_has_typed_error_and_next_acceptance_clears_it() {
    let mut state = DocumentReviewState::new().expect("state");
    let id = accept(&mut state, request(&[0xff], None));
    settle(&mut state);
    assert_eq!(state.phase(), DocumentReviewPhase::Failed);
    assert_eq!(state.operation_id(), Some(id));
    assert!(matches!(
        state.error(),
        Some(DocumentReviewOperationError::Review(
            DocumentReviewError::UnsupportedEncoding
        ))
    ));
    assert!(state.result().is_none());
    state.cancel();
    assert_eq!(state.phase(), DocumentReviewPhase::Failed);
    let next = accept(&mut state, request(b"valid", None));
    assert!(next.get() > id.get());
    assert_eq!(state.phase(), DocumentReviewPhase::Loading);
    assert!(state.error().is_none());
    settle(&mut state);
    assert_eq!(state.phase(), DocumentReviewPhase::Ready);
}

#[test]
fn original_cancellation_is_loading_until_tagged_terminal_result() {
    let mut state = DocumentReviewState::new().expect("state");
    let id = accept(&mut state, request(b"private", Some(b"candidate")));
    state.cancel();
    assert_eq!(state.phase(), DocumentReviewPhase::Loading);
    assert!(state.cancellation_requested());
    settle(&mut state);
    assert_eq!(state.phase(), DocumentReviewPhase::Cancelled);
    assert_eq!(state.operation_id(), Some(id));
    assert!(state.result().is_none());
    assert!(state.error().is_none());
    assert!(!state.poll().expect("reported once"));
    accept(&mut state, request(b"fresh", None));
    assert!(!state.cancellation_requested());
    settle(&mut state);
    assert_eq!(state.phase(), DocumentReviewPhase::Ready);
}
