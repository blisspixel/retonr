//! Escaped inspection views for model-free candidate checks.

use std::{
    fs::OpenOptions,
    io::{self, Write},
    path::Path,
};

use rewrite_types::RewriteRecord;
use serde::Serialize;

use super::ReportTarget;
use super::escape::escape_for_display;
use crate::{
    contract::{CommandName, SuccessEnvelope},
    failure::RunFailure,
};

const RESYNC_WINDOW: usize = 32;

/// One escaped linear change between source and accepted output.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct DiffHunk {
    kind: &'static str,
    source_line: String,
    output_line: String,
    source: String,
    output: String,
}

/// Escaped linear comparison of source bytes and accepted output bytes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct SafeDiff {
    changed: bool,
    hunks: Vec<DiffHunk>,
}

impl SafeDiff {
    pub(crate) fn compare(source: &[u8], output: &[u8]) -> Self {
        if source == output {
            return Self {
                changed: false,
                hunks: Vec::new(),
            };
        }
        let source_lines = split_display_lines(source);
        let output_lines = split_display_lines(output);
        Self {
            changed: true,
            hunks: greedy_line_hunks(&source_lines, &output_lines),
        }
    }

    pub(crate) fn render_text(&self) -> String {
        use std::fmt::Write as _;
        let mut text = format!(
            "diff: {}\nhunks: {}\n",
            if self.changed { "changed" } else { "unchanged" },
            self.hunks.len()
        );
        for hunk in &self.hunks {
            let _ = writeln!(
                text,
                "{} source_line={} output_line={} source={} output={}",
                hunk.kind, hunk.source_line, hunk.output_line, hunk.source, hunk.output
            );
        }
        text
    }
}

pub(crate) fn write_trace(
    path: &Path,
    record: &RewriteRecord,
    command: CommandName,
) -> Result<(), RunFailure> {
    let mut bytes = serde_json::to_vec_pretty(&SuccessEnvelope::new(command, record))
        .map_err(|_| RunFailure::operational(command))?;
    bytes.push(b'\n');
    write_new_file(path, &bytes, command)
}

pub(crate) fn write_diff(
    diff: &SafeDiff,
    target: ReportTarget,
    command: CommandName,
) -> Result<(), RunFailure> {
    let bytes = diff.render_text().into_bytes();
    match target {
        ReportTarget::Data => write_stdout(&bytes, command),
        ReportTarget::Diagnostic => write_stderr(&bytes, command),
    }
}

fn write_new_file(path: &Path, bytes: &[u8], command: CommandName) -> Result<(), RunFailure> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| {
            if error.kind() == io::ErrorKind::AlreadyExists {
                RunFailure::output_exists_for(command)
            } else {
                RunFailure::operational(command)
            }
        })?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| RunFailure::operational(command))
}

fn write_stdout(bytes: &[u8], command: CommandName) -> Result<(), RunFailure> {
    let mut stream = io::stdout().lock();
    stream
        .write_all(bytes)
        .and_then(|()| stream.flush())
        .map_err(|_| RunFailure::operational(command))
}

fn write_stderr(bytes: &[u8], command: CommandName) -> Result<(), RunFailure> {
    let mut stream = io::stderr().lock();
    stream
        .write_all(bytes)
        .and_then(|()| stream.flush())
        .map_err(|_| RunFailure::operational(command))
}

fn greedy_line_hunks(source: &[&str], output: &[&str]) -> Vec<DiffHunk> {
    let mut hunks = Vec::new();
    let mut source_index = 0;
    let mut output_index = 0;
    while source_index < source.len() || output_index < output.len() {
        if source_index < source.len()
            && output_index < output.len()
            && source[source_index] == output[output_index]
        {
            source_index += 1;
            output_index += 1;
            continue;
        }
        match next_sync(source, output, source_index, output_index) {
            Sync::Replace => {
                hunks.push(hunk(
                    "replace",
                    source_index,
                    output_index,
                    Some(source[source_index]),
                    Some(output[output_index]),
                ));
                source_index += 1;
                output_index += 1;
            }
            Sync::Delete => {
                hunks.push(hunk(
                    "delete",
                    source_index,
                    output_index,
                    Some(source[source_index]),
                    None,
                ));
                source_index += 1;
            }
            Sync::Insert => {
                hunks.push(hunk(
                    "insert",
                    source_index,
                    output_index,
                    None,
                    Some(output[output_index]),
                ));
                output_index += 1;
            }
        }
    }
    hunks
}

enum Sync {
    Replace,
    Delete,
    Insert,
}

fn next_sync(source: &[&str], output: &[&str], source_index: usize, output_index: usize) -> Sync {
    if source_index >= source.len() {
        return Sync::Insert;
    }
    if output_index >= output.len() {
        return Sync::Delete;
    }
    let source_resync = find_line(output, output_index, source[source_index]);
    let output_resync = find_line(source, source_index, output[output_index]);
    match (source_resync, output_resync) {
        (None, None) => Sync::Replace,
        (Some(source_hit), Some(output_hit)) if source_hit <= output_hit => Sync::Insert,
        (Some(_) | None, Some(_)) => Sync::Delete,
        (Some(_), None) => Sync::Insert,
    }
}

fn find_line(lines: &[&str], start: usize, needle: &str) -> Option<usize> {
    lines
        .iter()
        .skip(start)
        .take(RESYNC_WINDOW)
        .position(|line| *line == needle)
}

fn hunk(
    kind: &'static str,
    source_index: usize,
    output_index: usize,
    source: Option<&str>,
    output: Option<&str>,
) -> DiffHunk {
    DiffHunk {
        kind,
        source_line: (source_index + 1).to_string(),
        output_line: (output_index + 1).to_string(),
        source: source.map_or_else(String::new, escape_for_display),
        output: output.map_or_else(String::new, escape_for_display),
    }
}

fn split_display_lines(bytes: &[u8]) -> Vec<&str> {
    let text = std::str::from_utf8(bytes).unwrap_or("");
    if text.is_empty() {
        return Vec::new();
    }
    text.split('\n').collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_documents_are_unchanged() {
        let diff = SafeDiff::compare(b"Hello world\n", b"Hello world\n");
        assert!(!diff.changed);
        assert!(diff.hunks.is_empty());
        assert!(diff.render_text().contains("diff: unchanged"));
    }

    #[test]
    fn punctuation_change_is_one_escaped_replace() {
        let diff = SafeDiff::compare(b"Hello world\n", b"Hello, world!\n");
        assert!(diff.changed);
        assert_eq!(diff.hunks.len(), 1);
        assert_eq!(diff.hunks[0].kind, "replace");
        assert_eq!(diff.hunks[0].source, "Hello world");
        assert_eq!(diff.hunks[0].output, "Hello, world!");
    }

    #[test]
    fn control_characters_cannot_reach_the_rendered_diff() {
        let diff = SafeDiff::compare(b"safe\n", b"\x1b[31mdanger\r\n");
        assert_eq!(diff.hunks[0].output, "\\e[31mdanger\\r");
        assert!(!diff.render_text().contains('\u{1b}'));
        assert!(!diff.render_text().contains('\r'));
    }
}
