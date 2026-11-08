//! Read-only preview, findings, and help presentation.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph},
};

use super::state::{Pane, State};
use crate::presentation;

/// Said once in the header. The status line stays a verdict.
pub(super) const READ_ONLY_NOTICE: &str = "Read-only; local generation unavailable in this view.";

const FOOTER_HINT: &str = "Tab pane | s sync | w wrap | r reload | ? help | q quit";
const FOLDER_HINT: &str = "[ ] doc | s sync | w wrap | r reload | ? help | q quit";

const HELP: &str = "\
Read-only source and supplied candidate review.\n\
\n\
Tab / Shift+Tab: select pane\n\
Up / Down / j / k: scroll lines\n\
Left / Right / h / l: scroll columns\n\
s: lock or unlock source and candidate\n\
w: wrap lines, or show one line\n\
PageUp / PageDown: scroll ten lines\n\
Home: return to preview start\n\
[ / ]: previous or next folder document\n\
r: reload the selected document\n\
? / F1: toggle help\n\
Escape: close help, then quit\n\
q: quit from anywhere\n\
Ctrl+C: interrupt\n\
\n\
Previews and finding lists are bounded.\n\
No model generation, saves, or writes.";

pub(super) fn draw(frame: &mut Frame<'_>, state: &State) {
    let area = frame.area();
    if area.width < 30 || area.height < 10 {
        frame.render_widget(Paragraph::new("Enlarge terminal. q quits."), area);
        return;
    }
    let rows = screen(area);
    frame.render_widget(Paragraph::new(header(state)), rows[0]);
    let columns = panes(rows[1]);
    let findings = if state.snapshot.findings.is_empty() {
        "No findings to display.".to_owned()
    } else {
        state.snapshot.findings.join("\n")
    };
    render_pane(
        frame,
        columns[0],
        "Findings",
        &findings,
        state,
        Pane::Findings,
    );
    let source_title = format!("Source: {}", state.snapshot.source_label);
    render_pane(
        frame,
        columns[1],
        &source_title,
        &state.snapshot.source,
        state,
        Pane::Source,
    );
    let candidate_title = format!(
        "Candidate: {}",
        state
            .snapshot
            .candidate_label
            .as_deref()
            .unwrap_or("not supplied")
    );
    render_pane(
        frame,
        columns[2],
        &candidate_title,
        state
            .snapshot
            .candidate
            .as_deref()
            .unwrap_or("Supply --candidate to compare an existing draft."),
        state,
        Pane::Candidate,
    );
    let status = if state.pending.is_some() {
        Line::from("Loading bounded read-only snapshot...")
    } else {
        status_line(&state.snapshot.status)
    };
    frame.render_widget(
        Paragraph::new(Text::from(vec![status, Line::from(footer_hint(state))])),
        rows[2],
    );
    if state.help {
        help(frame, area);
    }
}

fn screen(area: Rect) -> std::rc::Rc<[Rect]> {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(2),
        ])
        .split(area)
}

fn panes(area: Rect) -> std::rc::Rc<[Rect]> {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(35),
            Constraint::Percentage(35),
        ])
        .split(area)
}

fn footer_hint(state: &State) -> &'static str {
    if state.snapshot.directory.is_some() {
        FOLDER_HINT
    } else {
        FOOTER_HINT
    }
}

fn header(state: &State) -> String {
    let title = match &state.snapshot.directory {
        Some(directory) => {
            let current = directory.selected.saturating_add(1).min(directory.total);
            format!("retonr: folder review {current}/{}", directory.total)
        }
        None => "retonr: experimental read-only review".to_owned(),
    };
    format!("{title}\n{READ_ONLY_NOTICE}")
}

fn status_line(status: &str) -> Line<'static> {
    let Some((word, rest)) = status.split_once(' ') else {
        return Line::styled(status.to_owned(), verdict_style(status));
    };
    Line::from(vec![
        Span::styled(word.to_owned(), verdict_style(word)),
        Span::raw(format!(" {rest}")),
    ])
}

fn verdict_style(word: &str) -> Style {
    match word {
        "accepted" => Style::default().fg(Color::Green),
        "abstained" | "unchanged" => Style::default().fg(Color::Yellow),
        "failed" => Style::default().fg(Color::Red),
        _ => Style::default(),
    }
}

fn render_pane(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    text: &str,
    state: &State,
    pane: Pane,
) {
    let selected = state.pane == pane;
    // Default foreground. Bright black is the background in Solarized Dark.
    let border = if selected {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default().add_modifier(Modifier::DIM)
    };
    let title = if selected {
        format!("> {title}")
    } else {
        title.to_owned()
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border)
        .title(title)
        .title_style(border);
    let highlight = match pane {
        Pane::Findings => None,
        Pane::Source | Pane::Candidate => state.snapshot.highlight_line,
    };
    let inner = usize::from(area.width.saturating_sub(2));
    let lines = pane_lines(text, inner, state.view.wrap, highlight);
    let horizontal = if state.view.wrap {
        0
    } else {
        state.horizontal[pane.index()]
    };
    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .scroll((state.scroll[pane.index()], horizontal)),
        area,
    );
}

/// Wraps each source line, then reverses every visual row of the located line.
fn pane_lines(text: &str, inner: usize, wrap: bool, highlight: Option<u32>) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for (index, source_line) in text.split('\n').enumerate() {
        let line_number = u32::try_from(index.saturating_add(1)).unwrap_or(u32::MAX);
        let reversed = highlight == Some(line_number);
        let pieces = if wrap {
            presentation::wrap_line(source_line, inner)
        } else {
            vec![source_line.to_owned()]
        };
        for piece in pieces {
            let mut line = Line::from(piece);
            if reversed {
                line = line.style(Style::default().add_modifier(Modifier::REVERSED));
            }
            lines.push(line);
        }
    }
    lines
}

fn help(frame: &mut Frame<'_>, area: Rect) {
    let popup = Rect::new(
        area.x + 2,
        area.y + 2,
        area.width.saturating_sub(4),
        area.height.saturating_sub(4),
    );
    let inner = usize::from(popup.width.saturating_sub(2));
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(presentation::wrap(HELP, Some(inner))).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().add_modifier(Modifier::DIM))
                .title("Help"),
        ),
        popup,
    );
}

#[cfg(test)]
mod tests;
