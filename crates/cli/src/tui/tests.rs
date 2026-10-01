use super::*;

fn request(plain: bool) -> TuiArgs {
    TuiArgs {
        source: PathBuf::from("source.txt"),
        candidate: None,
        protected_terms: Vec::new(),
        plain,
    }
}

#[test]
fn interactive_mode_refuses_redirected_streams_and_machine_format_before_reading() {
    for (input, output, format) in [
        (false, true, None),
        (true, false, None),
        (false, false, Some(ReportFormat::Text)),
        (true, true, Some(ReportFormat::Json)),
    ] {
        let failure = validate_mode(&request(false), format, input, output)
            .expect_err("interactive mode requires two terminal streams");
        assert_eq!(failure.command, CommandName::Tui);
        assert_eq!(
            serde_json::to_value(&failure.body).expect("error body")["code"],
            "invalid_invocation"
        );
        assert!(failure.message.contains("--plain"));
        validate_mode(&request(true), format, input, output).expect("linear fallback");
    }
    validate_mode(&request(false), None, true, true).expect("interactive terminal");
}

#[test]
fn plain_review_preserves_safe_preview_and_explicit_candidate_summary_in_both_formats() {
    let mut state = state::State::default();
    let operation = state.begin().expect("begin");
    state.complete(
        operation,
        Ok(state::Snapshot {
            source_label: "draft\n\u{202e}.txt".into(),
            candidate_label: Some("candidate.txt".into()),
            source: "hello\u{1b}[2J\nworld".into(),
            candidate: Some("hello, world!".into()),
            findings: vec!["hostile\u{1b}]52;clipboard".into()],
            status: "Candidate accepted".into(),
        }),
    );
    for format in [ReportFormat::Json, ReportFormat::Text] {
        let mut output = Vec::new();
        write_plain(&state.snapshot, format, &mut output).expect("linear output");
        let text = String::from_utf8(output).expect("UTF-8");
        assert!(!text.contains('\u{1b}'));
        assert!(!text.contains('\u{202e}'));
        assert!(text.contains("Candidate accepted"));
        assert!(text.contains("candidate.txt"));
        assert!(text.contains("hello, world!"));
        if format == ReportFormat::Json {
            let value: serde_json::Value = serde_json::from_str(&text).expect("JSON envelope");
            assert_eq!(value["command"], "tui");
            assert_eq!(value["result"]["source_preview"], state.snapshot.source);
            assert_eq!(value["schema_version"], 1);
        }
    }
}

#[test]
fn source_only_linear_review_has_no_candidate_section_and_propagates_sink_failure() {
    let mut state = state::State::default();
    let operation = state.begin().expect("begin");
    state.complete(
        operation,
        Ok(state::Snapshot {
            source_label: "draft.txt".into(),
            source: "source bytes".into(),
            status: "Read-only inspection".into(),
            ..state::Snapshot::default()
        }),
    );
    let mut output = Vec::new();
    write_plain(&state.snapshot, ReportFormat::Text, &mut output).expect("source review");
    assert!(
        !String::from_utf8(output)
            .expect("text")
            .contains("Candidate:")
    );
    for format in [ReportFormat::Json, ReportFormat::Text] {
        assert_eq!(
            write_plain(&state.snapshot, format, &mut BrokenSink)
                .expect_err("failed sink")
                .kind(),
            io::ErrorKind::BrokenPipe
        );
    }
}

struct BrokenSink;

#[test]
fn cancellation_during_output_or_cleanup_takes_precedence_over_sink_errors() {
    let cancellation = CancellationToken::new();
    finish_output(Ok(()), &cancellation).expect("normal completion");
    assert_eq!(
        serde_json::to_value(
            &finish_output(Err(io::ErrorKind::BrokenPipe.into()), &cancellation)
                .expect_err("failed output")
                .body
        )
        .expect("error body")["code"],
        "operational_failure"
    );
    cancellation.cancel();
    for result in [Ok(()), Err(io::ErrorKind::BrokenPipe.into())] {
        assert_eq!(
            serde_json::to_value(
                &finish_output(result, &cancellation)
                    .expect_err("cancelled output")
                    .body
            )
            .expect("error body")["code"],
            "operation_cancelled"
        );
    }
}

impl Write for BrokenSink {
    fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
        Err(io::Error::from(io::ErrorKind::BrokenPipe))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn original_cancellation_is_not_lost_after_successful_review() {
    let cancellation = CancellationToken::new();
    require_not_cancelled(&cancellation).expect("not cancelled");
    cancellation.cancel();
    assert_eq!(
        serde_json::to_value(
            &require_not_cancelled(&cancellation)
                .expect_err("cancelled")
                .body
        )
        .expect("error body")["code"],
        "operation_cancelled"
    );
}
