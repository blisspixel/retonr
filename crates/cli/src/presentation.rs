//! Terminal color and width for human reports.
//!
//! Color follows the stream, `--color`, `NO_COLOR`, and `TERM=dumb`.
//! Wrapping counts visible characters and happens before any color is added.

use clap::ValueEnum;
use std::io::IsTerminal;

/// Caller choice for report color.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum ColorChoice {
    /// Color when the report stream is a terminal.
    Auto,
    /// Color even when the stream is not a terminal.
    Always,
    /// Never color.
    Never,
}

/// Role colors from the interface rules.
#[derive(Clone, Copy)]
pub(crate) enum Role {
    /// Accepted or passed.
    Success,
    /// Abstained or kept.
    Warning,
    /// Errors and refusals.
    Error,
    /// Digests, locations, and other secondary fields.
    Muted,
}

/// Whether this report should emit ANSI color.
///
/// A nonempty `NO_COLOR` or `TERM=dumb` wins over `--color always`.
#[must_use]
pub(crate) fn color_enabled(choice: ColorChoice, stream_is_terminal: bool) -> bool {
    let blocked = std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty())
        || std::env::var_os("TERM").is_some_and(|value| value == "dumb");
    color_choice_enabled(choice, stream_is_terminal, blocked)
}

/// Color decision after the environment has already been read.
#[must_use]
pub(crate) fn color_choice_enabled(
    choice: ColorChoice,
    stream_is_terminal: bool,
    blocked: bool,
) -> bool {
    if blocked {
        return false;
    }
    match choice {
        ColorChoice::Always => true,
        ColorChoice::Never => false,
        ColorChoice::Auto => stream_is_terminal,
    }
}

/// Paints one already-visible word. The caller wraps before painting.
#[must_use]
pub(crate) fn paint(enabled: bool, role: Role, text: &str) -> String {
    if !enabled || text.is_empty() {
        return text.to_owned();
    }
    let code = match role {
        Role::Success => "32",
        Role::Warning => "33",
        Role::Error => "31",
        Role::Muted => "2",
    };
    format!("\u{1b}[{code}m{text}\u{1b}[0m")
}

/// Visible columns of a terminal stream. `COLUMNS` wins when it is set.
#[must_use]
pub(crate) fn terminal_width(stream_is_terminal: bool) -> Option<usize> {
    if !stream_is_terminal {
        return None;
    }
    if let Some(columns) = std::env::var("COLUMNS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|columns| *columns > 0)
    {
        return Some(columns);
    }
    crossterm::terminal::size()
        .ok()
        .map(|(width, _)| usize::from(width))
}

/// True when standard output is a terminal.
#[must_use]
pub(crate) fn stdout_is_terminal() -> bool {
    std::io::stdout().is_terminal()
}

/// True when standard error is a terminal.
#[must_use]
pub(crate) fn stderr_is_terminal() -> bool {
    std::io::stderr().is_terminal()
}

/// Wraps each line so no visible line exceeds `width`.
#[must_use]
pub(crate) fn wrap(text: &str, width: Option<usize>) -> String {
    let Some(width) = width.filter(|width| *width >= 8) else {
        return text.to_owned();
    };
    let mut wrapped = String::new();
    for line in text.split_inclusive('\n') {
        let (body, newline) = match line.strip_suffix('\n') {
            Some(body) => (body, true),
            None => (line, false),
        };
        push_wrapped(body, width, &mut wrapped);
        if newline {
            wrapped.push('\n');
        }
    }
    wrapped
}

fn push_wrapped(line: &str, width: usize, out: &mut String) {
    if line.chars().count() <= width {
        out.push_str(line);
        return;
    }
    let mut rest = line;
    let mut first = true;
    while rest.chars().count() > width {
        let split_at = split_index(rest, width);
        let (head, tail) = rest.split_at(split_at);
        let head = head.trim_end();
        if !first {
            out.push('\n');
        }
        out.push_str(head);
        rest = tail.trim_start();
        first = false;
        if rest.is_empty() {
            return;
        }
    }
    if !rest.is_empty() {
        out.push('\n');
        out.push_str(rest);
    }
}

fn split_index(line: &str, width: usize) -> usize {
    let mut last_space = None;
    for (count, (index, character)) in line.char_indices().enumerate() {
        if count == width {
            return last_space.unwrap_or(index);
        }
        if character == ' ' {
            last_space = Some(index);
        }
    }
    line.len()
}

/// Paints the verdict word, receipt word, locations, and short digests.
#[must_use]
pub(crate) fn paint_report(text: &str, enabled: bool) -> String {
    if !enabled {
        return text.to_owned();
    }
    let mut lines = text.lines();
    let Some(first) = lines.next() else {
        return text.to_owned();
    };
    let mut painted = paint_line(first, true);
    for line in lines {
        painted.push('\n');
        painted.push_str(&paint_line(line, false));
    }
    if text.ends_with('\n') {
        painted.push('\n');
    }
    painted
}

fn paint_line(line: &str, first: bool) -> String {
    let colored = if first {
        paint_leading_word(line)
    } else {
        paint_receipt_word(line)
    };
    dim_secondary(&colored)
}

fn dim_secondary(line: &str) -> String {
    let line = dim_after_marker(line, "sha256 ");
    let line = dim_after_marker(&line, "source_digest: ");
    let line = dim_after_marker(&line, "output_digest: ");
    dim_location(&line)
}

fn dim_location(line: &str) -> String {
    let Some(column_at) = line.find(" col ") else {
        return line.to_owned();
    };
    if !line.starts_with("line ") {
        return line.to_owned();
    }
    let after_column = &line[column_at + 5..];
    let digits = after_column
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(after_column.len());
    if digits == 0 {
        return line.to_owned();
    }
    let mut split = column_at + 5 + digits;
    if line[split..].starts_with(':') {
        split += 1;
    }
    format!(
        "{}{}",
        paint(true, Role::Muted, &line[..split]),
        &line[split..]
    )
}

fn paint_leading_word(line: &str) -> String {
    let Some(word) = line.split_whitespace().next() else {
        return line.to_owned();
    };
    let role = match word {
        "accepted" | "passed" | "clean" => Role::Success,
        "abstained" | "kept" | "unchanged" => Role::Warning,
        "failed" | "error" => Role::Error,
        _ => return line.to_owned(),
    };
    paint_prefix(line, word, role)
}

fn paint_receipt_word(line: &str) -> String {
    let Some(word) = line.split_whitespace().next() else {
        return line.to_owned();
    };
    let role = match word {
        "passed" => Role::Success,
        "kept" => Role::Warning,
        "failed" => Role::Error,
        _ => return line.to_owned(),
    };
    paint_prefix(line, word, role)
}

fn paint_prefix(line: &str, word: &str, role: Role) -> String {
    let Some(rest) = line.strip_prefix(word) else {
        return line.to_owned();
    };
    format!("{}{rest}", paint(true, role, word))
}

fn dim_after_marker(line: &str, marker: &str) -> String {
    let Some(index) = line.find(marker) else {
        return line.to_owned();
    };
    let start = index + marker.len();
    let rest = &line[start..];
    let end = rest
        .find(|character: char| !character.is_ascii_hexdigit())
        .unwrap_or(rest.len());
    if end == 0 {
        return line.to_owned();
    }
    format!(
        "{}{}{}",
        &line[..start],
        paint(true, Role::Muted, &rest[..end]),
        &rest[end..]
    )
}

#[cfg(test)]
mod tests {
    use super::{ColorChoice, Role, color_choice_enabled, paint, paint_report, wrap};

    #[test]
    fn color_obeys_no_color_term_and_the_flag() {
        assert!(!color_choice_enabled(ColorChoice::Never, true, false));
        assert!(color_choice_enabled(ColorChoice::Always, false, false));
        assert!(!color_choice_enabled(ColorChoice::Always, true, true));
        assert!(color_choice_enabled(ColorChoice::Auto, true, false));
        assert!(!color_choice_enabled(ColorChoice::Auto, false, false));
        assert!(!color_choice_enabled(ColorChoice::Auto, true, true));
    }

    #[test]
    fn wrap_keeps_short_lines_and_breaks_long_ones() {
        let text = "accepted    1 candidate passed; nothing written (use -o)\n";
        let wrapped = wrap(text, Some(40));
        assert!(wrapped.lines().all(|line| line.chars().count() <= 40));
        assert!(wrapped.contains("accepted"));
        assert_eq!(wrap("short\n", Some(40)), "short\n");
    }

    #[test]
    fn paint_is_a_no_op_when_disabled() {
        assert_eq!(paint(false, Role::Success, "accepted"), "accepted");
        assert!(paint(true, Role::Success, "accepted").contains("accepted"));
        assert_eq!(
            paint_report(
                "kept      original, byte-identical (sha256 abcdef123456)\n",
                false
            ),
            "kept      original, byte-identical (sha256 abcdef123456)\n"
        );
    }

    #[test]
    fn paint_dims_the_digest_and_the_location() {
        let painted = paint_report(
            "kept      original, byte-identical (sha256 abcdef123456)\nline 1 col 6: declared_term altered\n",
            true,
        );
        assert!(painted.contains("\u{1b}[33mkept\u{1b}[0m"));
        assert!(painted.contains("\u{1b}[2mabcdef123456\u{1b}[0m"));
        assert!(painted.contains("\u{1b}[2mline 1 col 6:\u{1b}[0m"));
        assert!(painted.contains("declared_term altered"));
    }
}
