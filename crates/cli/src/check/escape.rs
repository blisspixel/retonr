//! Neutralize terminal-control effects in untrusted document text.

use std::fmt::Write;

/// Escapes untrusted text so a terminal cannot interpret it as a control sequence.
///
/// Line feeds remain line feeds so wrapped output stays readable. Every other C0,
/// C1, bidi, format, and invisible character is replaced by a visible escape.
#[must_use]
pub(crate) fn escape_for_display(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\n' => escaped.push('\n'),
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
    use super::{escape_for_display, must_escape_for_terminal, render_document_for_terminal};

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
