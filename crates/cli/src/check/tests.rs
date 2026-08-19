use std::{io::Cursor, process::ExitCode};

use rewrite_types::{ReasonCode, RewriteStatus};

use super::{
    DocumentRender, OutputSink, exit_status,
    report::{reason_name, status_name},
    resolve_document_render, resolve_output_sink,
};
use crate::contract::{EXIT_CANCELLED, EXIT_POLICY, read_bounded};

#[test]
fn stable_exit_code_policy() {
    assert_eq!(
        exit_status(RewriteStatus::Rewritten, None, true),
        ExitCode::SUCCESS
    );
    assert_eq!(
        exit_status(RewriteStatus::Abstained, None, true),
        ExitCode::from(EXIT_POLICY)
    );
    assert_eq!(
        exit_status(RewriteStatus::Abstained, None, false),
        ExitCode::SUCCESS
    );
    assert_eq!(
        exit_status(RewriteStatus::Failed, None, false),
        ExitCode::FAILURE
    );
    assert_eq!(
        exit_status(RewriteStatus::Abstained, Some(ReasonCode::Cancelled), false),
        ExitCode::from(EXIT_CANCELLED)
    );
}

#[test]
fn text_names_match_serialized_contract() {
    assert_eq!(status_name(RewriteStatus::Rewritten), "rewritten");
    assert_eq!(
        reason_name(ReasonCode::ProtectedValueChanged),
        "protected_value_changed"
    );
}

#[test]
fn bounded_reader_stops_oversized_input() {
    let exact = read_bounded(Cursor::new(b"abc"), 3).expect("exact limit is valid");
    assert_eq!(exact, b"abc");
    let oversized =
        read_bounded(Cursor::new(b"abcd"), 3).expect_err("input beyond the limit must fail");
    assert_eq!(oversized.kind(), std::io::ErrorKind::InvalidData);
}

#[test]
fn a_terminal_without_the_double_opt_in_uses_escaped_rendering() {
    assert_eq!(
        resolve_document_render(true, false, false),
        DocumentRender::Escaped
    );
    assert_eq!(
        resolve_document_render(true, true, false),
        DocumentRender::Escaped
    );
    assert_eq!(
        resolve_document_render(true, false, true),
        DocumentRender::Escaped
    );
    assert_eq!(
        resolve_document_render(true, true, true),
        DocumentRender::Exact
    );
    assert_eq!(
        resolve_document_render(false, false, false),
        DocumentRender::Exact
    );
}

#[test]
fn absent_output_emits_no_document_bytes() {
    let sink = resolve_output_sink(None).expect("no output is valid");
    assert_eq!(sink, OutputSink::None);
}

#[test]
fn a_new_destination_is_accepted_and_an_existing_one_is_refused() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let fresh = directory.path().join("out.txt");
    let sink = resolve_output_sink(Some(fresh.as_path())).expect("a new destination is valid");
    assert_eq!(sink, OutputSink::File(fresh.clone()));

    std::fs::write(&fresh, b"existing").expect("create the destination");
    let refused = resolve_output_sink(Some(fresh.as_path()))
        .expect_err("an existing destination is never replaced");
    assert_eq!(refused.exit_code, ExitCode::from(EXIT_POLICY));
}
