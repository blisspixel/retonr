use super::*;
use std::{fs, path::PathBuf};

mod input;

struct Fixture {
    directory: tempfile::TempDir,
    source: PathBuf,
    candidate: PathBuf,
}

impl Fixture {
    fn new(source: &[u8], candidate: &[u8]) -> Self {
        let directory = tempfile::tempdir().expect("review fixture");
        let source_path = directory.path().join("source.txt");
        let candidate_path = directory.path().join("candidate.txt");
        fs::write(&source_path, source).expect("source fixture");
        fs::write(&candidate_path, candidate).expect("candidate fixture");
        Self {
            directory,
            source: source_path,
            candidate: candidate_path,
        }
    }

    fn request(&self, candidate: bool) -> TuiArgs {
        TuiArgs {
            source: self.source.clone(),
            candidate: candidate.then(|| self.candidate.clone()),
            protected_terms: Vec::new(),
            plain: true,
        }
    }
}

#[test]
fn review_keeps_exact_app_check_record_and_editorial_comparison_without_mutation() {
    let source = include_bytes!("../../../../../fixtures/cli/source.txt");
    let candidate = include_str!("../../../../../fixtures/cli/candidate.txt");
    let fixture = Fixture::new(source, candidate.as_bytes());
    let request = fixture.request(true);
    let reference = CandidateCheckService::check_with_cancellation(
        CandidateCheckRequest::new(source.to_vec(), candidate.to_owned(), Vec::new()),
        &CancellationToken::new(),
    )
    .expect("CLI application path");
    let snapshot = load(&request, &CancellationToken::new()).expect("TUI review");
    assert_eq!(snapshot.check.as_ref(), Some(&reference.record));
    assert_eq!(
        snapshot.inspection.digest,
        rewrite_types::Digest::sha256(source)
    );
    let EditorialSnapshot::Comparison(comparison) = &snapshot.editorial else {
        panic!("comparative lint");
    };
    assert_eq!(
        comparison,
        &EditorialLintService::compare(
            std::str::from_utf8(source).expect("UTF-8 source"),
            candidate
        )
    );
    let presentation = snapshot.into_presentation();
    assert!(presentation.status.contains("generation unavailable"));
    assert_eq!(fs::read(&fixture.source).expect("retained source"), source);
    assert_eq!(
        fs::read(&fixture.candidate).expect("retained candidate"),
        candidate.as_bytes()
    );
    assert_eq!(
        fs::read_dir(fixture.directory.path())
            .expect("fixture entries")
            .count(),
        2
    );
}

#[test]
fn protected_value_rejection_remains_the_same_app_abstention_and_proposed_preview() {
    let fixture = Fixture::new(b"Keep Acme 42 exactly.\n", b"Keep Elsewhere 43 exactly.\n");
    let mut request = fixture.request(true);
    request.protected_terms = vec!["Acme".to_owned()];
    let snapshot =
        load(&request, &CancellationToken::new()).expect("rejected candidate remains a review");
    let reference = CandidateCheckService::check(CandidateCheckRequest::new(
        fs::read(&fixture.source).expect("source"),
        fs::read_to_string(&fixture.candidate).expect("candidate"),
        request.protected_terms,
    ))
    .expect("app rejection");
    assert_eq!(snapshot.check.as_ref(), Some(&reference.record));
    assert_eq!(
        reference.record.status,
        rewrite_types::RewriteStatus::Abstained
    );
    assert_eq!(
        snapshot.candidate.as_deref(),
        Some("Keep Elsewhere 43 exactly.\n")
    );
    assert!(snapshot.into_presentation().status.contains("abstained"));
    assert_eq!(
        fs::read(&fixture.source).expect("source untouched"),
        b"Keep Acme 42 exactly.\n"
    );
}

#[test]
fn source_only_lint_is_exact_and_hostile_text_never_becomes_terminal_control() {
    let text = "Certainly!\nHello\u{1b}[2J\u{85}\u{202e}\u{200b}\r\tworld.\n";
    let fixture = Fixture::new(text.as_bytes(), b"unused");
    let snapshot = load(&fixture.request(false), &CancellationToken::new())
        .expect("read-only hostile document inspection");
    let EditorialSnapshot::Document(findings) = &snapshot.editorial else {
        panic!("single lint");
    };
    assert_eq!(findings, &EditorialLintService::lint(text));
    assert!(snapshot.check.is_none());
    let presentation = snapshot.into_presentation();
    assert!(presentation.source.contains("\\e[2J"));
    for control in ['\u{1b}', '\u{85}', '\u{202e}', '\u{200b}', '\r', '\t'] {
        assert!(!presentation.source.contains(control));
        assert!(!presentation.source_label.contains(control));
        assert!(
            presentation
                .findings
                .iter()
                .all(|finding| !finding.contains(control))
        );
    }
    assert_eq!(
        fs::read(&fixture.source).expect("hostile source unchanged"),
        text.as_bytes()
    );
}

#[test]
fn cancelled_noncooperative_lint_results_are_discarded_and_pre_cancel_skips_file_work() {
    let fixture = Fixture::new(b"Certainly! Hello world.\n", b"Hello, world.\n");
    let cancellation = CancellationToken::new();
    let result = load_with_post_lint(&fixture.request(false), &cancellation, || {
        cancellation.cancel();
    });
    let failure = result.expect_err("late cancellation discards lint");
    assert_eq!(failure.body, expected_body(ErrorCode::OperationCancelled));
    let mut missing = fixture.request(true);
    missing.source = fixture.directory.path().join("missing-private-path");
    let failure = load(&missing, &cancellation).expect_err("pre-cancel");
    assert_eq!(failure.body, expected_body(ErrorCode::OperationCancelled));
    assert!(!format!("{failure:?}").contains("missing-private-path"));
    assert_eq!(
        fs::read(&fixture.source).expect("source unchanged"),
        b"Certainly! Hello world.\n"
    );
}

#[test]
fn preview_clipping_does_not_shorten_validation_or_typed_findings() {
    let source = "alpha ".repeat(PREVIEW_BYTES / 6 + 20);
    let fixture = Fixture::new(source.as_bytes(), source.as_bytes());
    let snapshot = load(&fixture.request(true), &CancellationToken::new())
        .expect("complete check with bounded preview");
    assert_eq!(
        snapshot.check.as_ref().expect("check").source_digest,
        rewrite_types::Digest::sha256(source.as_bytes())
    );
    assert_eq!(
        snapshot.inspection.byte_size,
        u64::try_from(source.len()).expect("bounded size")
    );
    let presentation = snapshot.into_presentation();
    assert!(presentation.source.len() <= PREVIEW_BYTES);
    assert!(
        presentation
            .source
            .contains("validation used the complete input")
    );
    assert!(presentation.candidate.expect("candidate preview").len() <= PREVIEW_BYTES);
    assert_eq!(
        fs::read(&fixture.source).expect("unclipped file"),
        source.as_bytes()
    );
}

#[test]
fn display_omission_keeps_exact_findings_and_large_domain_snapshots_fail_closed() {
    let source = "\u{2014} ".repeat(state::FINDING_LIMIT + 10);
    let fixture = Fixture::new(source.as_bytes(), b"unused");
    let snapshot =
        load(&fixture.request(false), &CancellationToken::new()).expect("bounded domain findings");
    let EditorialSnapshot::Document(findings) = &snapshot.editorial else {
        panic!("document lint");
    };
    assert_eq!(findings.len(), EditorialLintService::lint(&source).len());
    assert!(findings.len() > state::FINDING_LIMIT);
    let presentation = snapshot.into_presentation();
    assert!(
        presentation
            .findings
            .last()
            .expect("omission disclosure")
            .contains("omitted")
    );
    let excessive = "\u{2014} ".repeat(MAX_DOMAIN_FINDINGS + 1);
    fs::write(&fixture.source, excessive.as_bytes()).expect("finding limit fixture");
    let failure = load(&fixture.request(false), &CancellationToken::new())
        .expect_err("domain findings bounded");
    assert_eq!(
        failure.body,
        expected_body(ErrorCode::ResourceLimitExceeded)
    );
}

fn expected_body(code: ErrorCode) -> ErrorBody {
    let category = match code {
        ErrorCode::OperationCancelled => ErrorCategory::Cancelled,
        ErrorCode::ResourceLimitExceeded | ErrorCode::Unsupported => ErrorCategory::Compatibility,
        _ => ErrorCategory::Usage,
    };
    ErrorBody::new(category, code, false)
}
