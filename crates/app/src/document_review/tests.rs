use super::*;
use rewrite_types::{Digest, RewriteStatus};

fn request(source: &[u8], candidate: Option<&[u8]>) -> DocumentReviewRequest {
    DocumentReviewRequest {
        source: source.to_vec(),
        candidate: candidate.map(<[u8]>::to_vec),
        protected_terms: Vec::new(),
    }
}

#[test]
fn source_review_preserves_exact_inventory_and_untrusted_text() {
    let source = "Certainly!\r\nHello\u{1b}[2J\u{202e} world.\n";
    let result =
        DocumentReviewService::review(request(source.as_bytes(), None), &CancellationToken::new())
            .expect("review");
    assert_eq!(
        result.inspection,
        inspect_plain_text(source.as_bytes()).expect("inventory")
    );
    assert_eq!(result.source_preview, source);
    assert!(result.candidate_preview.is_none());
    assert!(result.candidate_inspection.is_none());
    assert!(result.check.is_none());
    let EditorialReview::Document(findings) = result.editorial else {
        panic!("source review");
    };
    assert_eq!(findings, EditorialLintService::lint(source));
}

#[test]
fn candidate_review_is_identical_to_direct_shared_checks() {
    let source = include_bytes!("../../../../fixtures/cli/source.txt");
    let candidate = include_bytes!("../../../../fixtures/cli/candidate.txt");
    let result =
        DocumentReviewService::review(request(source, Some(candidate)), &CancellationToken::new())
            .expect("candidate review");
    let check = CandidateCheckService::check(CandidateCheckRequest::new(
        source.to_vec(),
        String::from_utf8(candidate.to_vec()).expect("candidate"),
        Vec::new(),
    ))
    .expect("direct check");
    assert_eq!(result.check, Some(check.record));
    assert_eq!(
        result.candidate_inspection.expect("inventory").digest,
        Digest::sha256(candidate)
    );
    let EditorialReview::Comparison(comparison) = result.editorial else {
        panic!("comparison");
    };
    assert_eq!(
        comparison,
        EditorialLintService::compare(
            std::str::from_utf8(source).expect("source"),
            std::str::from_utf8(candidate).expect("candidate")
        )
    );
}

#[test]
fn rejection_keeps_proposed_preview_and_exact_abstention_record() {
    let mut input = request(
        b"Keep Acme 42 exactly.\n",
        Some(b"Keep Elsewhere 43 exactly.\n"),
    );
    input.protected_terms.push("Acme".to_owned());
    let result =
        DocumentReviewService::review(input, &CancellationToken::new()).expect("review rejection");
    let record = result.check.expect("check");
    assert_eq!(record.status, RewriteStatus::Abstained);
    assert_eq!(
        record.source_digest,
        Digest::sha256(b"Keep Acme 42 exactly.\n")
    );
    assert_eq!(
        result.candidate_preview.as_deref(),
        Some("Keep Elsewhere 43 exactly.\n")
    );
}

#[test]
fn complete_input_validation_precedes_utf8_preview_clipping() {
    let source = format!(
        "{}Keep Acme 42 exactly.\n",
        "alpha ".repeat(MAX_DOCUMENT_REVIEW_PREVIEW_BYTES / 6 + 2)
    );
    let candidate = source.replace("42", "43");
    let result = DocumentReviewService::review(
        request(source.as_bytes(), Some(candidate.as_bytes())),
        &CancellationToken::new(),
    )
    .expect("full review");
    assert_eq!(result.inspection.digest, Digest::sha256(source.as_bytes()));
    assert_eq!(
        result.check.expect("check").status,
        RewriteStatus::Abstained
    );
    assert!(result.source_preview.len() <= MAX_DOCUMENT_REVIEW_PREVIEW_BYTES);
    assert!(result.candidate_preview.expect("preview").len() <= MAX_DOCUMENT_REVIEW_PREVIEW_BYTES);
    assert!(
        result
            .source_preview
            .ends_with("validation used the complete input.]")
    );
    let unicode = "\u{754c}".repeat(MAX_DOCUMENT_REVIEW_PREVIEW_BYTES / 3 + 30);
    let result =
        DocumentReviewService::review(request(unicode.as_bytes(), None), &CancellationToken::new())
            .expect("UTF8 clipping");
    assert!(result.source_preview.len() <= MAX_DOCUMENT_REVIEW_PREVIEW_BYTES);
    assert!(result.source_preview.starts_with('\u{754c}'));
}

#[test]
fn refuses_both_encoding_roles_byte_limits_and_invalid_protection() {
    for bad in [b"\xffprivate".as_slice(), b"\xff\xfeh\0", b"\xfe\xff\0h"] {
        for input in [request(bad, None), request(b"valid", Some(bad))] {
            assert!(matches!(
                DocumentReviewService::review(input, &CancellationToken::new()),
                Err(DocumentReviewError::UnsupportedEncoding)
            ));
        }
    }
    let huge = vec![b'x'; crate::MAX_CANDIDATE_CHECK_BYTES + 1];
    for input in [request(&huge, None), request(b"valid", Some(&huge))] {
        assert!(matches!(
            DocumentReviewService::review(input, &CancellationToken::new()),
            Err(DocumentReviewError::Application(_))
        ));
    }
    let mut input = request(b"source", None);
    input.protected_terms.push("source".to_owned());
    assert!(matches!(
        DocumentReviewService::review(input, &CancellationToken::new()),
        Err(DocumentReviewError::CandidateRequired)
    ));
    let mut input = request(b"source", Some(b"source"));
    input.protected_terms = (0..33).map(|n| format!("term-{n}")).collect();
    assert!(matches!(
        DocumentReviewService::review(input, &CancellationToken::new()),
        Err(DocumentReviewError::Application(_))
    ));
}

#[test]
fn original_cancellation_wins_before_work_and_discards_late_lint() {
    let token = CancellationToken::new();
    let result = review_with_post_lint(request(b"Certainly! Hello world.", None), &token, || {
        token.cancel();
    });
    assert!(matches!(result, Err(DocumentReviewError::Cancelled)));
    assert!(matches!(
        DocumentReviewService::review(request(b"\xff", None), &token),
        Err(DocumentReviewError::Cancelled)
    ));
    let token = CancellationToken::new();
    let dense = "\u{2014} ".repeat(MAX_DOCUMENT_REVIEW_FINDINGS + 1);
    assert!(matches!(
        review_with_post_lint(request(dense.as_bytes(), None), &token, || {
            token.cancel();
        }),
        Err(DocumentReviewError::Cancelled)
    ));
}

#[test]
fn combined_findings_are_bounded_before_return() {
    let source = "\u{2014} ".repeat(MAX_DOCUMENT_REVIEW_FINDINGS + 1);
    assert!(matches!(
        DocumentReviewService::review(request(source.as_bytes(), None), &CancellationToken::new()),
        Err(DocumentReviewError::FindingLimitExceeded)
    ));
    let source = "\u{2014} ".repeat(MAX_DOCUMENT_REVIEW_FINDINGS / 2 + 1);
    assert!(matches!(
        DocumentReviewService::review(
            request(source.as_bytes(), Some(source.as_bytes())),
            &CancellationToken::new()
        ),
        Err(DocumentReviewError::FindingLimitExceeded)
    ));
}

#[test]
fn debug_output_omits_documents_terms_and_finding_evidence() {
    let mut input = request(b"Certainly! PRIVATE-DOCUMENT", Some(b"PRIVATE-CANDIDATE"));
    input.protected_terms.push("PRIVATE-TERM".to_owned());
    let debug = format!("{input:?}");
    for secret in ["PRIVATE-DOCUMENT", "PRIVATE-CANDIDATE", "PRIVATE-TERM"] {
        assert!(!debug.contains(secret));
    }
    let result = DocumentReviewService::review(
        request(b"Certainly! PRIVATE-DOCUMENT", None),
        &CancellationToken::new(),
    )
    .expect("review");
    assert!(!format!("{result:?} {:?}", result.editorial).contains("PRIVATE-DOCUMENT"));
}
