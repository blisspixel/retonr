use std::{io::Cursor, path::PathBuf, process::ExitCode};

use rewrite_types::{ReasonCode, RewriteStatus};

use super::{
    CheckRequest, OutputSink, exit_status,
    report::{reason_name, status_name},
    resolve_output_sink,
};
use crate::contract::{EXIT_CANCELLED, EXIT_POLICY, read_bounded};

fn request(output: Option<&str>, raw_terminal: bool, confirmed: bool) -> CheckRequest {
    CheckRequest {
        source: PathBuf::from("source.txt"),
        candidate: PathBuf::from("candidate.txt"),
        protected_terms: Vec::new(),
        fail_on_abstain: false,
        output: output.map(PathBuf::from),
        raw_terminal,
        confirmed,
    }
}

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
fn absent_output_emits_no_document_bytes() {
    let sink = resolve_output_sink(&request(None, false, false)).expect("no output is valid");
    assert_eq!(sink, OutputSink::None);
}

#[test]
fn a_new_destination_is_accepted_and_an_existing_one_is_refused() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let fresh = directory.path().join("out.txt");
    let sink = resolve_output_sink(&request(
        Some(fresh.to_str().expect("path is UTF-8")),
        false,
        false,
    ))
    .expect("a new destination is valid");
    assert_eq!(sink, OutputSink::File(fresh.clone()));

    std::fs::write(&fresh, b"existing").expect("create the destination");
    let refused = resolve_output_sink(&request(
        Some(fresh.to_str().expect("path is UTF-8")),
        false,
        false,
    ))
    .expect_err("an existing destination is never replaced");
    assert_eq!(refused.exit_code, ExitCode::from(EXIT_POLICY));
}
