//! Read-only preview, findings, and help presentation.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Clear, Paragraph},
};

use super::state::{Pane, State};

pub(super) fn draw(frame: &mut Frame<'_>, state: &State) {
    let area = frame.area();
    if area.width < 30 || area.height < 10 {
        frame.render_widget(Paragraph::new("Enlarge terminal. q quits."), area);
        return;
    }
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(2),
        ])
        .split(area);
    frame.render_widget(
        Paragraph::new(
            "retonr: experimental read-only review\nFindings and supplied candidate validation.",
        ),
        rows[0],
    );
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(35),
            Constraint::Percentage(35),
        ])
        .split(rows[1]);
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
        "Loading bounded read-only snapshot..."
    } else {
        &state.snapshot.status
    };
    frame.render_widget(
        Paragraph::new(format!(
            "{status}\nTab pane | arrows scroll | r reload | ? help | q quit"
        )),
        rows[2],
    );
    if state.help {
        help(frame, area);
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
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(if selected {
            Color::Cyan
        } else {
            Color::DarkGray
        }));
    frame.render_widget(
        Paragraph::new(text)
            .block(block)
            .scroll((state.scroll[pane.index()], state.horizontal[pane.index()])),
        area,
    );
}

fn help(frame: &mut Frame<'_>, area: Rect) {
    let popup = Rect::new(
        area.x + 2,
        area.y + 2,
        area.width.saturating_sub(4),
        area.height.saturating_sub(4),
    );
    frame.render_widget(Clear, popup);
    frame.render_widget(Paragraph::new(
        "Read-only source and supplied candidate review.\n\nTab / Shift+Tab: select pane\nUp / Down / j / k: scroll lines\nLeft / Right / h / l: scroll columns\nPageUp / PageDown: scroll ten lines\nHome: return to preview start\nr: reload when no operation is running\n? / F1: toggle help\nq / Escape: quit; Ctrl+C: interrupt\n\nDocument previews and finding lists are bounded.\nNo model generation, saves, or document writes."
    ).block(Block::default().borders(Borders::ALL).title("Help")), popup);
}

#[cfg(test)]
mod tests;
