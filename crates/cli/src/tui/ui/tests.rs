use super::*;
use crate::tui::state::Snapshot;
use ratatui::{Terminal, backend::TestBackend};

fn render(width: u16, height: u16, state: &State) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test terminal");
    terminal.draw(|frame| draw(frame, state)).expect("draw");
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect()
}

#[test]
fn hostile_text_cannot_reach_terminal_cells_and_all_three_panes_render() {
    let mut state = State::default();
    let operation = state.begin().expect("operation");
    state.complete(
        operation,
        Ok(Snapshot {
            source_label: "private\u{202e}.txt".into(),
            candidate_label: Some("draft.txt".into()),
            source: "hello\u{1b}[31m\nnext".into(),
            candidate: Some("candidate\rtext".into()),
            findings: vec!["finding\u{7}".into()],
            status: "ready\u{1b}]52;secret".into(),
        }),
    );
    let rendered = render(120, 24, &state);
    assert!(rendered.contains("Findings"));
    assert!(rendered.contains("Source:"));
    assert!(rendered.contains("Candidate:"));
    assert!(rendered.contains("\\e[31m"));
    assert!(rendered.contains("\\rtext"));
    assert!(!crate::render::contains_terminal_effect(&rendered));
}

#[test]
fn help_loading_empty_candidate_and_small_viewports_are_renderable() {
    let mut state = State::default();
    assert!(render(80, 16, &state).contains("not supplied"));
    state.begin();
    assert!(render(80, 16, &state).contains("Loading"));
    state.help = true;
    assert!(render(80, 24, &state).contains("Shift+Tab"));
    for (width, height) in [(0, 0), (1, 1), (29, 9), (30, 10)] {
        let _ = render(width, height, &state);
    }
    state.help = false;
    state.pane = Pane::Candidate;
    state.scroll = [u16::MAX; 3];
    let _ = render(120, 24, &state);
}
