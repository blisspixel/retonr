//! Bounded locations for inventoried invisible characters. No source text is retained.

use std::fmt::Write as _;

use rewrite_types::{MAX_SPAN_FAILURES, TextPoint};

use crate::inventory::{ControlClass, control_class};

/// One contiguous invisible run, named by class and escaped code points.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvisibleLocation {
    /// Stable class name shared with the inventory counts.
    pub class: &'static str,
    /// One-based line of the first character in the run.
    pub line: u32,
    /// One-based byte column of the first character in the run.
    pub column: u32,
    /// Escaped code points. A contiguous scalar run uses `U+AAAA..U+BBBB`.
    pub code_points: String,
}

/// ASCII mapping of one tag run. This is data, not an instruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedTagMapping {
    /// One-based line of the first tag character.
    pub line: u32,
    /// One-based byte column of the first tag character.
    pub column: u32,
    /// Escaped mapping. Printable ASCII stays visible; every other tag uses `\\u{...}`.
    pub ascii: String,
}

/// Opt-in invisible detail. Empty vectors mean the request found nothing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvisibleInventoryDetail {
    /// Class runs, at most [`MAX_SPAN_FAILURES`].
    pub locations: Vec<InvisibleLocation>,
    /// True when at least one later location was omitted.
    pub locations_truncated: bool,
    /// Tag mappings, at most [`MAX_SPAN_FAILURES`], and only when decoding was requested.
    pub decoded_tags: Vec<DecodedTagMapping>,
    /// True when at least one later tag run was omitted.
    pub decoded_tags_truncated: bool,
}

/// Locates invisible runs and, when asked, maps tag characters to escaped ASCII.
///
/// Tag decoding is independent of the location budget, and both lists stop at
/// [`MAX_SPAN_FAILURES`]. The mapping covers U+E0020..=U+E007E as ASCII. The
/// language tag and the cancel tag stay escaped.
#[must_use]
pub fn locate_invisible(text: &str, decode_tags: bool) -> InvisibleInventoryDetail {
    let mut detail = InvisibleInventoryDetail {
        locations: Vec::new(),
        locations_truncated: false,
        decoded_tags: Vec::new(),
        decoded_tags_truncated: false,
    };
    let mut run: Option<PendingRun> = None;
    for (offset, character) in text.char_indices() {
        let Some(class) = control_class(character) else {
            finish(run.take(), text, decode_tags, &mut detail);
            continue;
        };
        match run.as_mut() {
            Some(current) if current.class == class => current.chars.push(character),
            Some(_) => {
                finish(run.take(), text, decode_tags, &mut detail);
                run = Some(PendingRun::new(class, offset, character));
            }
            None => run = Some(PendingRun::new(class, offset, character)),
        }
    }
    finish(run.take(), text, decode_tags, &mut detail);
    detail
}

struct PendingRun {
    class: ControlClass,
    offset: usize,
    chars: Vec<char>,
}

impl PendingRun {
    fn new(class: ControlClass, offset: usize, character: char) -> Self {
        Self {
            class,
            offset,
            chars: vec![character],
        }
    }
}

fn finish(
    run: Option<PendingRun>,
    text: &str,
    decode_tags: bool,
    detail: &mut InvisibleInventoryDetail,
) {
    let Some(run) = run else {
        return;
    };
    let Some(point) = TextPoint::from_byte_index(text, run.offset) else {
        return;
    };
    if detail.locations.len() < MAX_SPAN_FAILURES {
        detail.locations.push(InvisibleLocation {
            class: run.class.as_str(),
            line: point.line,
            column: point.column,
            code_points: format_code_points(&run.chars),
        });
    } else {
        detail.locations_truncated = true;
    }
    if !decode_tags || run.class != ControlClass::Tag {
        return;
    }
    if detail.decoded_tags.len() < MAX_SPAN_FAILURES {
        detail.decoded_tags.push(DecodedTagMapping {
            line: point.line,
            column: point.column,
            ascii: decode_tag_run(&run.chars),
        });
    } else {
        detail.decoded_tags_truncated = true;
    }
}

fn format_code_points(chars: &[char]) -> String {
    let Some(first) = chars.first().copied() else {
        return String::new();
    };
    if chars.len() == 1 {
        return format_code_point(first);
    }
    let codes: Vec<u32> = chars.iter().copied().map(u32::from).collect();
    let contiguous = codes
        .windows(2)
        .all(|pair| pair[1] == pair[0].saturating_add(1))
        || codes
            .windows(2)
            .all(|pair| pair[0] == pair[1].saturating_add(1));
    if contiguous {
        let last = codes[codes.len() - 1];
        return format!("U+{start:04X}..U+{last:04X}", start = u32::from(first));
    }
    let shown = codes.len().min(MAX_SPAN_FAILURES);
    let mut encoded = String::new();
    for (index, code) in codes.iter().take(shown).enumerate() {
        if index > 0 {
            encoded.push(' ');
        }
        let _ = write!(encoded, "U+{code:04X}");
    }
    if shown < codes.len() {
        let extra = codes.len() - shown;
        let _ = write!(encoded, " (+{extra})");
    }
    encoded
}

fn format_code_point(character: char) -> String {
    format!("U+{code:04X}", code = u32::from(character))
}

fn decode_tag_run(chars: &[char]) -> String {
    let mut decoded = String::new();
    for character in chars.iter().take(MAX_SPAN_FAILURES) {
        decoded.push_str(&decode_tag_character(*character));
    }
    if chars.len() > MAX_SPAN_FAILURES {
        let extra = chars.len() - MAX_SPAN_FAILURES;
        let _ = write!(decoded, " (+{extra})");
    }
    decoded
}

fn decode_tag_character(character: char) -> String {
    let code = u32::from(character);
    let ascii = (0xE0020..=0xE007E)
        .contains(&code)
        .then_some(code - 0xE0000)
        .and_then(char::from_u32);
    if let Some(ascii) = ascii
        && (ascii.is_ascii_graphic() || ascii == ' ')
    {
        return ascii.to_string();
    }
    format!("\\u{{{code:x}}}")
}

#[cfg(test)]
mod tests {
    use super::locate_invisible;

    #[test]
    fn each_class_is_located_without_source_text_and_tags_decode_only_on_request() {
        let text =
            "A\u{1b}B\u{9b}C\u{202e}D\u{fe0f}E\u{200b}F\u{E0067}\u{E0062}\u{E007F}G\u{00ad}H";
        let located = locate_invisible(text, false);
        assert!(!located.locations_truncated);
        assert!(located.decoded_tags.is_empty());
        let classes: Vec<&str> = located.locations.iter().map(|item| item.class).collect();
        assert_eq!(
            classes,
            [
                "c0",
                "c1",
                "bidi",
                "variation_selectors",
                "zero_width",
                "tag",
                "other_format",
            ]
        );
        let tags = located
            .locations
            .iter()
            .find(|item| item.class == "tag")
            .expect("tag run");
        assert_eq!(tags.code_points, "U+E0067 U+E0062 U+E007F");
        assert!(!tags.code_points.contains('G'));
        assert_eq!(tags.line, 1);
        let decoded = locate_invisible(text, true);
        assert_eq!(decoded.decoded_tags.len(), 1);
        assert_eq!(decoded.decoded_tags[0].ascii, "gb\\u{e007f}");
        assert!(!decoded.decoded_tags[0].ascii.contains("instruction"));
        let second_line = locate_invisible("keep\n\u{E0068}\u{E0069}", true);
        assert_eq!(second_line.decoded_tags[0].line, 2);
        assert_eq!(second_line.decoded_tags[0].column, 1);
        assert_eq!(second_line.decoded_tags[0].ascii, "hi");
        assert_eq!(second_line.locations[0].code_points, "U+E0068..U+E0069");
    }

    #[test]
    fn location_and_decode_lists_stop_at_the_span_budget() {
        let mut text = String::new();
        for _ in 0..40 {
            text.push('\u{1b}');
            text.push('x');
        }
        let located = locate_invisible(&text, false);
        assert_eq!(located.locations.len(), 32);
        assert!(located.locations_truncated);
        let mut tags = String::new();
        for _ in 0..40 {
            tags.push('\u{E0061}');
            tags.push(' ');
        }
        let decoded = locate_invisible(&tags, true);
        assert_eq!(decoded.decoded_tags.len(), 32);
        assert!(decoded.decoded_tags_truncated);
    }
}
