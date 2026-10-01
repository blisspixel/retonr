use super::*;
use rewrite_engine::{
    MAX_PROTECTED_TERM_BYTES, MAX_PROTECTED_TERM_TOTAL_BYTES, MAX_PROTECTED_TERMS,
};

#[test]
fn exact_transport_budgets_and_allocation_normalization() {
    let mut bytes = Vec::with_capacity(4096);
    bytes.extend_from_slice(b"secret\x1b[2J");
    let mut term = String::with_capacity(4096);
    term.push_str("secret");
    let mut terms = Vec::with_capacity(4096);
    terms.push(term);
    let bounded = DocumentReviewOperationRequest::new(DocumentReviewRequest {
        source: bytes.clone(),
        candidate: Some(bytes),
        protected_terms: terms,
    })
    .expect("bounded");
    assert_eq!(bounded.0.source.len(), bounded.0.source.capacity());
    let candidate = bounded.0.candidate.as_ref().expect("candidate");
    assert_eq!(candidate.len(), candidate.capacity());
    assert_eq!(
        bounded.0.protected_terms.len(),
        bounded.0.protected_terms.capacity()
    );
    assert_eq!(
        bounded.0.protected_terms[0].len(),
        bounded.0.protected_terms[0].capacity()
    );
    assert!(!format!("{bounded:?}").contains("secret"));
    assert_eq!(bounded.source_bytes(), 10);
    assert_eq!(bounded.candidate_bytes(), Some(10));
    let max = crate::MAX_CANDIDATE_CHECK_BYTES;
    assert!(
        DocumentReviewOperationRequest::new(DocumentReviewRequest {
            source: vec![b'x'; max],
            candidate: Some(vec![b'x'; max]),
            protected_terms: vec![
                "x".repeat(MAX_PROTECTED_TERM_BYTES);
                MAX_PROTECTED_TERM_TOTAL_BYTES / MAX_PROTECTED_TERM_BYTES
            ],
        })
        .is_ok()
    );
    for input in [
        DocumentReviewRequest {
            source: vec![0; max + 1],
            candidate: None,
            protected_terms: Vec::new(),
        },
        DocumentReviewRequest {
            source: Vec::new(),
            candidate: Some(vec![0; max + 1]),
            protected_terms: Vec::new(),
        },
        DocumentReviewRequest {
            source: Vec::new(),
            candidate: None,
            protected_terms: vec!["x".into(); MAX_PROTECTED_TERMS + 1],
        },
        DocumentReviewRequest {
            source: Vec::new(),
            candidate: None,
            protected_terms: vec!["x".repeat(MAX_PROTECTED_TERM_BYTES + 1)],
        },
        DocumentReviewRequest {
            source: Vec::new(),
            candidate: None,
            protected_terms: vec![
                "x".repeat(MAX_PROTECTED_TERM_BYTES);
                MAX_PROTECTED_TERM_TOTAL_BYTES / MAX_PROTECTED_TERM_BYTES + 1
            ],
        },
    ] {
        assert!(matches!(
            DocumentReviewOperationRequest::new(input),
            Err(DocumentReviewOperationError::InputLimitExceeded)
        ));
    }
}

#[test]
fn candidate_disclosure_distinguishes_absence_and_empty_complete_input() {
    assert_eq!(request(b"source").source_bytes(), 6);
    assert_eq!(request(b"source").candidate_bytes(), None);
    let empty = DocumentReviewOperationRequest::new(DocumentReviewRequest {
        source: Vec::new(),
        candidate: Some(Vec::new()),
        protected_terms: Vec::new(),
    })
    .expect("empty candidate transport");
    assert_eq!(empty.source_bytes(), 0);
    assert_eq!(empty.candidate_bytes(), Some(0));
}

#[test]
fn semantic_and_encoding_validation_remain_in_shared_service() {
    for input in [
        DocumentReviewRequest {
            source: vec![0xff],
            candidate: None,
            protected_terms: Vec::new(),
        },
        DocumentReviewRequest {
            source: b"valid".to_vec(),
            candidate: None,
            protected_terms: vec!["protected".into()],
        },
    ] {
        let mut operation = DocumentReviewOperation::new().expect("worker");
        operation
            .submit(DocumentReviewOperationRequest::new(input).expect("transport accepts"))
            .expect("submitted");
        assert!(matches!(
            completion(&mut operation).result,
            Err(DocumentReviewOperationError::Review(_))
        ));
    }
}
