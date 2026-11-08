use super::*;
use crate::tui::event::Action;
use crate::tui::state::Snapshot;
use ratatui::{
    Terminal,
    backend::TestBackend,
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier},
};

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
            highlight_line: None,
            directory: None,
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
    let help = render(80, 24, &state);
    assert!(help.contains("Shift+Tab"));
    assert!(help.contains("close help"));
    for (width, height) in [(0, 0), (1, 1), (29, 9), (30, 10)] {
        let _ = render(width, height, &state);
    }
    state.help = false;
    state.pane = Pane::Candidate;
    state.scroll = [u16::MAX; 3];
    let _ = render(120, 24, &state);
}

#[test]
fn footer_and_notice_fit_sixty_columns() {
    assert!(FOOTER_HINT.chars().count() <= 60);
    assert!(FOLDER_HINT.chars().count() <= 60);
    assert!(READ_ONLY_NOTICE.chars().count() <= 60);
    let mut state = State::default();
    state.snapshot.directory = Some(crate::tui::state::DirectoryNavigation {
        selected: 1,
        total: 5,
        source_skipped: 0,
        candidate_skipped: 0,
        unmatched_candidates: 0,
        selected_relative: None,
    });
    let buffer = buffer_of(60, 24, &state);
    assert!(row_text(&buffer, 0, 0, 60).contains("2/5"));
    assert!(row_text(&buffer, 0, 1, 60).starts_with(READ_ONLY_NOTICE));
    assert!(row_text(&buffer, 0, 23, 60).starts_with(FOLDER_HINT));
}

#[test]
fn escape_on_help_returns_to_the_panes() {
    let mut state = State::default();
    state.help = true;
    assert!(render(80, 24, &state).contains("close help"));
    state.apply(Action::Dismiss);
    assert!(!state.help);
    assert!(!state.quit);
    let returned = render(80, 24, &state);
    assert!(!returned.contains("close help"));
    assert!(returned.contains("Findings"));
    assert!(returned.contains(READ_ONLY_NOTICE));
}

#[test]
fn verdict_words_use_role_colors_and_borders_stay_on_the_terminal_foreground() {
    for (word, color) in [
        ("accepted", Color::Green),
        ("abstained", Color::Yellow),
        ("unchanged", Color::Yellow),
        ("failed", Color::Red),
    ] {
        let mut state = State::default();
        state.snapshot.status = format!("{word}  line 1 col 2  number altered");
        let buffer = buffer_of(60, 24, &state);
        assert_only_role_colors(&buffer);
        assert_eq!(buffer.cell((0, 22)).expect("verdict").fg, color);
        let gap = u16::try_from(word.len()).expect("verdict width");
        assert_eq!(buffer.cell((gap, 22)).expect("gap").fg, Color::Reset);
        let areas = panes(screen(Rect::new(0, 0, 60, 24))[1]);
        assert_visible_border(&buffer, areas[0], true);
        assert_visible_border(&buffer, areas[1], false);
        assert_visible_border(&buffer, areas[2], false);
        assert!(row_text(&buffer, areas[0].x, areas[0].y, areas[0].width).contains('>'));
        assert!(row_text(&buffer, areas[1].x, areas[1].y, areas[1].width).contains("Source"));
        assert!(!row_text(&buffer, areas[1].x, areas[1].y, areas[1].width).contains('>'));
    }
}

#[test]
fn abstaining_candidate_highlights_the_located_line_in_both_panes() {
    let directory = tempfile::tempdir().expect("fixture");
    let source = directory.path().join("source.txt");
    let candidate = directory.path().join("candidate.txt");
    std::fs::write(&source, b"Keep Acme 42 exactly.\n").expect("source");
    std::fs::write(&candidate, b"Keep Elsewhere 43 exactly.\n").expect("candidate");
    let request = crate::tui::TuiArgs {
        source,
        candidate: Some(candidate),
        protected_terms: vec!["Acme".to_owned()],
        plain: true,
        recursive: false,
        document: None,
    };
    let presentation =
        crate::tui::service::load(&request, &rewrite_types::CancellationToken::new())
            .expect("review")
            .into_presentation();
    assert!(presentation.status.starts_with("abstained  line "));
    assert!(presentation.status.contains(" col "));
    assert!(presentation.highlight_line.is_some());
    let mut state = State::default();
    let operation = state.begin().expect("operation");
    state.complete(operation, Ok(presentation));
    for width in [60_u16, 100] {
        let buffer = buffer_of(width, 24, &state);
        assert_only_role_colors(&buffer);
        let status = row_text(&buffer, 0, 22, width);
        assert!(status.starts_with("abstained"));
        assert_eq!(buffer.cell((0, 22)).expect("verdict").fg, Color::Yellow);
        assert_eq!(buffer.cell((9, 22)).expect("gap").fg, Color::Reset);
        if state.snapshot.status.chars().count() <= usize::from(width) {
            assert!(status.starts_with(&state.snapshot.status));
        }
        assert!(row_text(&buffer, 0, 1, width).starts_with(READ_ONLY_NOTICE));
        assert!(row_text(&buffer, 0, 23, width).starts_with(FOOTER_HINT));
        let areas = panes(screen(Rect::new(0, 0, width, 24))[1]);
        assert!(pane_has_reversed_line(&buffer, areas[1], "Keep"));
        assert!(pane_has_reversed_line(&buffer, areas[2], "Keep"));
        assert!(!pane_content_reversed(&buffer, areas[0]));
    }
}

#[test]
fn wrapped_panes_keep_whole_words_at_sixty_and_one_hundred_columns() {
    let sentence =
        "You should not, under any circumstances, replace the protected value at the end.";
    let mut state = State::default();
    let operation = state.begin().expect("operation");
    state.complete(
        operation,
        Ok(Snapshot {
            source_label: "source.txt".into(),
            candidate_label: Some("candidate.txt".into()),
            source: sentence.into(),
            candidate: Some(sentence.into()),
            findings: vec![sentence.into()],
            status: "accepted".into(),
            highlight_line: Some(1),
            directory: None,
        }),
    );
    for width in [60_u16, 100] {
        let buffer = buffer_of(width, 24, &state);
        let areas = panes(screen(Rect::new(0, 0, width, 24))[1]);
        for pane in areas.iter() {
            let inner = usize::from(pane.width.saturating_sub(2));
            let rows = pane_content(&buffer, *pane);
            for (text, _) in &rows {
                assert!(
                    text.trim_end().chars().count() <= inner,
                    "clipped row {text:?} inner {inner}"
                );
            }
            for word in sentence.split_whitespace() {
                if word.chars().count() <= inner {
                    assert!(
                        rows.iter().any(|(text, _)| text.contains(word)),
                        "{word} missing at {width} columns"
                    );
                }
            }
        }
        assert!(pane_has_reversed_line(&buffer, areas[1], "circumstances,"));
        assert!(pane_has_reversed_line(&buffer, areas[2], "circumstances,"));
        assert!(!pane_content_reversed(&buffer, areas[0]));
        assert_eq!(buffer.cell((0, 22)).expect("verdict").fg, Color::Green);
    }
}

fn buffer_of(width: u16, height: u16, state: &State) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test terminal");
    terminal.draw(|frame| draw(frame, state)).expect("draw");
    terminal.backend().buffer().clone()
}

fn row_text(buffer: &Buffer, x: u16, y: u16, width: u16) -> String {
    (x..x.saturating_add(width))
        .map(|column| buffer.cell((column, y)).expect("cell").symbol().to_owned())
        .collect()
}

fn pane_content(buffer: &Buffer, pane: Rect) -> Vec<(String, bool)> {
    let mut rows = Vec::new();
    if pane.width < 3 || pane.height < 3 {
        return rows;
    }
    for y in (pane.y + 1)..(pane.y + pane.height - 1) {
        let mut text = String::new();
        let mut reversed = false;
        for x in (pane.x + 1)..(pane.x + pane.width - 1) {
            let cell = buffer.cell((x, y)).expect("cell");
            text.push_str(cell.symbol());
            reversed |= cell.modifier.contains(Modifier::REVERSED);
        }
        rows.push((text, reversed));
    }
    rows
}

fn pane_has_reversed_line(buffer: &Buffer, pane: Rect, needle: &str) -> bool {
    pane_content(buffer, pane)
        .iter()
        .any(|(text, reversed)| *reversed && text.contains(needle))
}

fn pane_content_reversed(buffer: &Buffer, pane: Rect) -> bool {
    pane_content(buffer, pane)
        .iter()
        .any(|(_, reversed)| *reversed)
}

fn assert_visible_border(buffer: &Buffer, pane: Rect, selected: bool) {
    let corners = [
        (pane.x, pane.y),
        (pane.x + pane.width - 1, pane.y),
        (pane.x, pane.y + pane.height - 1),
        (pane.x + pane.width - 1, pane.y + pane.height - 1),
    ];
    for (x, y) in corners {
        let cell = buffer.cell((x, y)).expect("corner");
        assert_ne!(cell.symbol(), " ", "border missing at {x},{y}");
        assert_eq!(cell.fg, Color::Reset);
        assert_eq!(cell.bg, Color::Reset);
        if selected {
            assert!(cell.modifier.contains(Modifier::BOLD));
            assert!(!cell.modifier.contains(Modifier::DIM));
        } else {
            assert!(cell.modifier.contains(Modifier::DIM));
            assert!(!cell.modifier.contains(Modifier::BOLD));
        }
    }
}

fn assert_only_role_colors(buffer: &Buffer) {
    for cell in &buffer.content {
        assert!(
            matches!(
                cell.fg,
                Color::Reset | Color::Green | Color::Yellow | Color::Red
            ),
            "foreground {:?}",
            cell.fg
        );
        assert_eq!(cell.bg, Color::Reset);
    }
}
