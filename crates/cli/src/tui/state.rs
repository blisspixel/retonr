//! Bounded, terminal-safe presentation state and deterministic keyboard reducer.

use super::event::Action;

pub(super) const TEXT_LIMIT: usize = 65_536;
pub(super) const FINDING_LIMIT: usize = 256;

#[derive(Default)]
pub(super) struct Snapshot {
    pub source_label: String,
    pub candidate_label: Option<String>,
    pub source: String,
    pub candidate: Option<String>,
    pub findings: Vec<String>,
    pub status: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Pane {
    Findings,
    Source,
    Candidate,
}

pub(super) struct State {
    pub snapshot: Snapshot,
    pub pane: Pane,
    pub scroll: [u16; 3],
    pub horizontal: [u16; 3],
    pub help: bool,
    pub quit: bool,
    pub pending: Option<u64>,
    next_operation: u64,
    loaded: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            snapshot: Snapshot::default(),
            pane: Pane::Findings,
            scroll: [0; 3],
            horizontal: [0; 3],
            help: false,
            quit: false,
            pending: None,
            next_operation: 0,
            loaded: false,
        }
    }
}

impl State {
    pub fn begin(&mut self) -> Option<u64> {
        if self.pending.is_some() {
            return None;
        }
        self.next_operation = self.next_operation.checked_add(1)?;
        self.pending = Some(self.next_operation);
        Some(self.next_operation)
    }

    pub fn complete(&mut self, operation: u64, result: Result<Snapshot, &'static str>) -> bool {
        if self.pending != Some(operation) {
            return false;
        }
        self.pending = None;
        self.scroll = [0; 3];
        self.horizontal = [0; 3];
        match result {
            Ok(snapshot) => {
                self.snapshot = snapshot.bounded();
                self.loaded = true;
            }
            Err(message) => {
                let prefix = if self.loaded {
                    "Reload failed; previous snapshot retained: "
                } else {
                    "Read failed: "
                };
                self.snapshot.status = safe_text(&format!("{prefix}{message}"), 1024, false);
            }
        }
        true
    }

    pub fn apply(&mut self, action: Action) -> bool {
        match action {
            Action::Quit | Action::Interrupt => self.quit = true,
            Action::Help => self.help = !self.help,
            Action::Next => {
                self.pane = match self.pane {
                    Pane::Findings => Pane::Source,
                    Pane::Source => Pane::Candidate,
                    Pane::Candidate => Pane::Findings,
                }
            }
            Action::Previous => {
                self.pane = match self.pane {
                    Pane::Findings => Pane::Candidate,
                    Pane::Source => Pane::Findings,
                    Pane::Candidate => Pane::Source,
                }
            }
            Action::Scroll(amount) => {
                let index = self.pane.index();
                self.scroll[index] = self.scroll[index].saturating_add_signed(amount);
            }
            Action::Horizontal(amount) => {
                let index = self.pane.index();
                self.horizontal[index] = self.horizontal[index].saturating_add_signed(amount);
            }
            Action::Home => {
                self.scroll[self.pane.index()] = 0;
                self.horizontal[self.pane.index()] = 0;
            }
            Action::Reload => return !self.help && self.pending.is_none(),
            Action::Resize | Action::None => {}
        }
        false
    }
}

impl Pane {
    pub fn index(self) -> usize {
        match self {
            Self::Findings => 0,
            Self::Source => 1,
            Self::Candidate => 2,
        }
    }
}

impl Snapshot {
    pub(super) fn bounded(self) -> Self {
        let omitted = self.findings.len() > FINDING_LIMIT;
        let mut findings: Vec<_> = self
            .findings
            .into_iter()
            .take(FINDING_LIMIT)
            .map(|finding| safe_text(&finding, 512, false))
            .collect();
        if omitted {
            findings.push("Additional findings omitted from this preview.".into());
        }
        Self {
            source_label: safe_text(&self.source_label, 512, false),
            candidate_label: self
                .candidate_label
                .map(|label| safe_text(&label, 512, false)),
            source: safe_text(&self.source, TEXT_LIMIT, true),
            candidate: self
                .candidate
                .map(|text| safe_text(&text, TEXT_LIMIT, true)),
            findings,
            status: safe_text(&self.status, 1024, false),
        }
    }
}

fn prefix(value: &str, limit: usize) -> &str {
    let mut end = value.len().min(limit);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

fn safe_text(value: &str, limit: usize, multiline: bool) -> String {
    let initial = prefix(value, limit);
    let escaped = if multiline {
        crate::render::escape_for_display(initial)
    } else {
        crate::render::escape_inline_for_display(initial)
    };
    if initial.len() < value.len() || escaped.len() > limit {
        let marker = if multiline {
            "\n[Preview truncated]"
        } else {
            " [Preview truncated]"
        };
        if limit < marker.len() {
            return prefix(marker, limit).to_owned();
        }
        let mut result = prefix(&escaped, limit.saturating_sub(marker.len())).to_owned();
        result.push_str(marker);
        result
    } else {
        escaped
    }
}

#[cfg(test)]
mod tests;
