//! Safe rendering for terminal text and structured JSON reports.

use std::fmt::Write as _;

use serde::Serialize;

/// Escapes untrusted document text so a terminal cannot interpret it as control state.
///
/// Line feeds remain line feeds so multiline document output stays readable. Every
/// other C0, C1, bidi, format, and invisible character becomes a visible escape.
#[must_use]
pub(crate) fn escape_for_display(value: &str) -> String {
    escape_text(value, true)
}

/// Escapes one untrusted report field without permitting embedded line breaks.
#[must_use]
pub(crate) fn escape_inline_for_display(value: &str) -> String {
    escape_text(value, false)
}

/// Returns whether text contains a character that can affect terminal presentation.
#[must_use]
pub(crate) fn contains_terminal_effect(value: &str) -> bool {
    value
        .chars()
        .any(|ch| ch == '\n' || must_escape_for_terminal(ch))
}

/// Serializes pretty JSON while escaping terminal-affecting Unicode inside strings.
///
/// The additional escapes preserve the decoded JSON values. Applying this to every
/// structured report keeps explicit `--format json` output safe on a terminal while
/// retaining the same machine-readable contract through pipes and files.
pub(crate) fn to_safe_pretty_json<T: Serialize>(value: &T) -> serde_json::Result<Vec<u8>> {
    serde_json::to_string_pretty(value)
        .map(|encoded| escape_json_for_display(&encoded).into_bytes())
}

/// Renders accepted document bytes for an interactive terminal.
///
/// # Errors
///
/// Returns [`std::str::Utf8Error`] when the accepted bytes are not UTF-8. The
/// candidate-check path only produces UTF-8, so that case is fail-closed.
pub(crate) fn render_document_for_terminal(bytes: &[u8]) -> Result<Vec<u8>, std::str::Utf8Error> {
    let text = std::str::from_utf8(bytes)?;
    Ok(escape_for_display(text).into_bytes())
}

fn escape_text(value: &str, preserve_line_feeds: bool) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\n' if preserve_line_feeds => escaped.push('\n'),
            '\n' => escaped.push_str("\\n"),
            '\t' => escaped.push_str("\\t"),
            '\r' => escaped.push_str("\\r"),
            '\u{1b}' => escaped.push_str("\\e"),
            '\u{7f}' => escaped.push_str("\\x7f"),
            ch if must_escape_for_terminal(ch) => {
                let _ = write!(escaped, "\\u{{{:x}}}", u32::from(ch));
            }
            ch => escaped.push(ch),
        }
    }
    escaped
}

fn escape_json_for_display(value: &str) -> String {
    let mut escaped_output = String::with_capacity(value.len());
    let mut in_string = false;
    let mut escaped_input = false;
    for ch in value.chars() {
        if !in_string {
            escaped_output.push(ch);
            if ch == '"' {
                in_string = true;
            }
            continue;
        }
        if escaped_input {
            escaped_output.push(ch);
            escaped_input = false;
            continue;
        }
        match ch {
            '\\' => {
                escaped_output.push(ch);
                escaped_input = true;
            }
            '"' => {
                escaped_output.push(ch);
                in_string = false;
            }
            ch if must_escape_for_terminal(ch) => {
                append_json_unicode_escape(&mut escaped_output, ch);
            }
            ch => escaped_output.push(ch),
        }
    }
    escaped_output
}

fn append_json_unicode_escape(output: &mut String, ch: char) {
    let mut encoded = [0_u16; 2];
    for unit in ch.encode_utf16(&mut encoded) {
        let _ = write!(output, "\\u{unit:04x}");
    }
}

fn must_escape_for_terminal(ch: char) -> bool {
    if ch == '\n' {
        return false;
    }
    if ch.is_control() {
        return true;
    }
    matches!(
        ch,
        '\u{00ad}'
            | '\u{034f}'
            | '\u{061c}'
            | '\u{115f}'
            | '\u{1160}'
            | '\u{17b4}'
            | '\u{17b5}'
            | '\u{180b}'..='\u{180e}'
            | '\u{200b}'..='\u{200f}'
            | '\u{2028}'..='\u{202f}'
            | '\u{2060}'..='\u{206f}'
            | '\u{3164}'
            | '\u{fe00}'..='\u{fe0f}'
            | '\u{feff}'
            | '\u{ffa0}'
            | '\u{fff9}'..='\u{fffb}'
            | '\u{e0001}'
            | '\u{e0020}'..='\u{e007f}'
            | '\u{e0100}'..='\u{e01ef}'
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        contains_terminal_effect, escape_for_display, escape_inline_for_display,
        must_escape_for_terminal, render_document_for_terminal, to_safe_pretty_json,
    };

    #[test]
    fn ansi_osc_c0_c1_and_carriage_return_cannot_reach_a_terminal() {
        let rendered = escape_for_display("\u{1b}[31mred\r\u{9b}C1\nnext");
        assert_eq!(rendered, "\\e[31mred\\r\\u{9b}C1\nnext");
        assert!(!rendered.contains('\u{1b}'));
        assert!(!rendered.contains('\r'));
        assert!(!rendered.contains('\u{9b}'));
        assert!(rendered.contains('\n'));
    }

    #[test]
    fn bidi_hyperlink_clipboard_and_invisible_marks_are_neutralized() {
        let rendered =
            escape_for_display("\u{202e}dlrow\u{200b}\u{feff}\u{2066}in\u{00ad}vis\u{e0001}ible");
        assert_eq!(
            rendered,
            "\\u{202e}dlrow\\u{200b}\\u{feff}\\u{2066}in\\u{ad}vis\\u{e0001}ible"
        );
        assert!(!rendered.chars().any(must_escape_for_terminal));
        assert!(contains_terminal_effect("safe\u{202e}"));
        assert!(contains_terminal_effect("line\nbreak"));
        assert!(!contains_terminal_effect("ordinary Unicode: cafe\u{301}"));
    }

    #[test]
    fn inline_fields_cannot_inject_report_lines() {
        assert_eq!(
            escape_inline_for_display("name\nnext\t\u{202e}"),
            "name\\nnext\\t\\u{202e}"
        );
    }

    #[test]
    fn structured_json_escapes_effects_without_changing_values() {
        let value = json!({
            "path": "draft\u{202e}.txt",
            "tag": "hidden\u{e0001}",
            "ordinary": "cafe\u{301}",
            "escaped": "quote=\" slash=\\ newline=\n"
        });
        let encoded = to_safe_pretty_json(&value).expect("serialize safe JSON");
        let text = std::str::from_utf8(&encoded).expect("JSON is UTF-8");
        assert!(!text.contains('\u{202e}'));
        assert!(!text.contains('\u{e0001}'));
        assert!(text.contains("\\u202e"));
        assert!(text.contains("\\udb40\\udc01"));
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&encoded).expect("decode safe JSON"),
            value
        );
    }

    #[test]
    fn ordinary_text_and_final_newline_are_preserved() {
        assert_eq!(escape_for_display("Hello, world!\n"), "Hello, world!\n");
        assert_eq!(
            render_document_for_terminal(b"Hello, world!\n").expect("UTF-8"),
            b"Hello, world!\n"
        );
    }

    #[test]
    fn invalid_utf8_is_rejected_before_terminal_render() {
        assert!(render_document_for_terminal(b"a\xffb").is_err());
    }
}
