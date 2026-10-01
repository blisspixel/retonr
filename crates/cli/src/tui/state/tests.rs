use super::*;

#[test]
fn reducer_navigation_help_reload_and_scroll_are_bounded() {
    let mut state = State::default();
    assert!(state.apply(Action::Reload));
    state.apply(Action::Previous);
    assert_eq!(state.pane, Pane::Candidate);
    state.apply(Action::Next);
    assert_eq!(state.pane, Pane::Findings);
    state.apply(Action::Scroll(-10));
    state.apply(Action::Horizontal(-10));
    assert_eq!(state.scroll, [0; 3]);
    assert_eq!(state.horizontal, [0; 3]);
    state.apply(Action::Scroll(i16::MAX));
    state.apply(Action::Scroll(i16::MAX));
    state.apply(Action::Scroll(10));
    assert_eq!(state.scroll[0], u16::MAX);
    state.apply(Action::Horizontal(i16::MAX));
    assert_eq!(state.horizontal[0], 32_767);
    state.apply(Action::Home);
    assert_eq!(state.scroll[0], 0);
    assert_eq!(state.horizontal[0], 0);
    state.apply(Action::Help);
    assert!(!state.apply(Action::Reload));
    state.apply(Action::Resize);
    state.apply(Action::None);
    state.apply(Action::Quit);
    assert!(state.quit);
}

#[test]
fn operation_identity_rejects_stale_responses_and_concurrent_requests() {
    let mut state = State::default();
    let first = state.begin().expect("operation");
    assert!(state.begin().is_none());
    assert!(!state.apply(Action::Reload));
    assert!(!state.complete(first + 1, Ok(Snapshot::default())));
    assert_eq!(state.pending, Some(first));
    assert!(state.complete(first, Err("read refused")));
    assert_eq!(state.snapshot.status, "Read failed: read refused");
    let second = state.begin().expect("next operation");
    assert!(!state.complete(first, Ok(Snapshot::default())));
    assert!(state.complete(second, Ok(Snapshot::default())));
    let reload = state.begin().expect("reload");
    assert!(state.complete(reload, Err("input changed")));
    assert!(
        state
            .snapshot
            .status
            .starts_with("Reload failed; previous snapshot retained:")
    );
    state.next_operation = u64::MAX;
    assert!(state.begin().is_none());
}

#[test]
fn every_untrusted_preview_is_escaped_and_boundaries_are_utf8_safe() {
    let hostile = "\u{1b}]52;secret\u{7}\r\u{202e}\t";
    let snapshot = Snapshot {
        source_label: hostile.into(),
        candidate_label: Some(hostile.into()),
        source: hostile.repeat(TEXT_LIMIT),
        candidate: Some("界".repeat(TEXT_LIMIT)),
        findings: vec![hostile.repeat(1000); FINDING_LIMIT + 1],
        status: hostile.into(),
    }
    .bounded();
    assert!(snapshot.source.len() <= TEXT_LIMIT);
    assert!(snapshot.candidate.as_ref().expect("candidate").len() <= TEXT_LIMIT);
    assert_eq!(snapshot.findings.len(), FINDING_LIMIT + 1);
    for text in [
        &snapshot.source_label,
        snapshot.candidate_label.as_ref().expect("label"),
        &snapshot.status,
    ] {
        assert!(!crate::render::contains_terminal_effect(text));
    }
    assert!(snapshot.source.ends_with("[Preview truncated]"));
    assert!(
        snapshot
            .findings
            .last()
            .expect("omission")
            .contains("omitted")
    );
    assert_eq!(safe_text("line\nnext", 100, true), "line\nnext");
    assert_eq!(safe_text("界", 2, false), " [");
}
