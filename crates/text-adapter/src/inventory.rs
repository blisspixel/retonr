//! Pre-model inventory of encoding, BOM, controls, and possible C2PA wrappers.

use rewrite_types::Digest;
use serde::Serialize;

use super::{
    LineEndingKind, MAX_PLAIN_TEXT_BYTES, TextAdapter, TextAdapterError, UTF8_BOM, UTF16_BE_BOM,
    UTF16_LE_BOM, metadata,
};

/// Observable encoding of one inspected byte buffer.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TextEncoding {
    /// Valid UTF-8, optionally with a UTF-8 BOM.
    Utf8,
    /// UTF-16 little-endian BOM. Decoding is unsupported.
    Utf16Le,
    /// UTF-16 big-endian BOM. Decoding is unsupported.
    Utf16Be,
    /// Bytes that are not UTF-8 after an optional UTF-8 BOM.
    InvalidUtf8,
}

/// Presence of a locally recognized carrier without validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CarrierPresence {
    /// No recognized wrapper pattern was observed.
    Absent,
    /// A UTF-8 BOM is followed by at least one variation selector.
    Possible,
    /// The buffer could not be decoded as UTF-8, so the wrapper was not checked.
    NotDecoded,
}

/// Disjoint counts of non-textual Unicode classes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct ControlCounts {
    /// C0 controls other than tab, LF, and CR, plus DEL.
    pub c0: u64,
    /// C1 controls in U+0080..=U+009F.
    pub c1: u64,
    /// Explicit bidirectional formatting controls.
    pub bidi: u64,
    /// Variation selectors, including supplementary-plane selectors.
    pub variation_selectors: u64,
    /// Zero-width and word-joiner characters, including ZWNBSP.
    pub zero_width: u64,
    /// Other format and invisible marks that are not in the classes above.
    pub other_format: u64,
}

/// Content-redacted pre-model inventory of one plain-text buffer.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PlainTextInventory {
    /// Detected encoding family.
    pub encoding: TextEncoding,
    /// First invalid UTF-8 offset, when encoding is [`TextEncoding::InvalidUtf8`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_up_to: Option<u64>,
    /// Whether the buffer starts with a UTF-8 BOM.
    pub utf8_bom: bool,
    /// Exact source length in bytes.
    pub byte_size: u64,
    /// SHA-256 of the exact source bytes.
    pub digest: Digest,
    /// Newline classification when the body is valid UTF-8.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_endings: Option<LineEndingKind>,
    /// Whether a valid UTF-8 body ends with LF, CRLF, or CR.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_newline: Option<bool>,
    /// Control-class counts. Empty when the body is not decoded.
    pub controls: ControlCounts,
    /// Unstructured-text Content Credential wrapper observation only.
    pub c2pa_unstructured_text: CarrierPresence,
}

impl TextAdapter {
    /// Inventories one buffer without normalizing, stripping, or validating a credential.
    ///
    /// UTF-16 and invalid UTF-8 are inventory facts, not rewrite eligibility.
    ///
    /// # Errors
    ///
    /// Returns [`TextAdapterError::InputTooLarge`] when the buffer exceeds the
    /// plain-text byte ceiling.
    pub fn inventory(input: &[u8]) -> Result<PlainTextInventory, TextAdapterError> {
        if input.len() > MAX_PLAIN_TEXT_BYTES {
            return Err(TextAdapterError::InputTooLarge {
                actual: input.len(),
                maximum: MAX_PLAIN_TEXT_BYTES,
            });
        }
        let digest = Digest::sha256(input);
        let byte_size = u64::try_from(input.len()).unwrap_or(u64::MAX);
        if input.starts_with(UTF16_LE_BOM) {
            return Ok(undecoded(TextEncoding::Utf16Le, false, byte_size, digest));
        }
        if input.starts_with(UTF16_BE_BOM) {
            return Ok(undecoded(TextEncoding::Utf16Be, false, byte_size, digest));
        }
        let utf8_bom = input.starts_with(UTF8_BOM);
        let body_offset = usize::from(utf8_bom) * UTF8_BOM.len();
        let body = &input[body_offset..];
        let text = match core::str::from_utf8(body) {
            Ok(text) => text,
            Err(error) => {
                return Ok(PlainTextInventory {
                    encoding: TextEncoding::InvalidUtf8,
                    valid_up_to: Some(
                        u64::try_from(body_offset + error.valid_up_to()).unwrap_or(u64::MAX),
                    ),
                    utf8_bom,
                    byte_size,
                    digest,
                    line_endings: None,
                    final_newline: None,
                    controls: ControlCounts::default(),
                    c2pa_unstructured_text: CarrierPresence::NotDecoded,
                });
            }
        };
        let meta = metadata(input, text, utf8_bom);
        let controls = count_controls(text);
        let c2pa_unstructured_text = if utf8_bom && controls.variation_selectors > 0 {
            CarrierPresence::Possible
        } else {
            CarrierPresence::Absent
        };
        Ok(PlainTextInventory {
            encoding: TextEncoding::Utf8,
            valid_up_to: None,
            utf8_bom,
            byte_size,
            digest,
            line_endings: Some(meta.line_endings),
            final_newline: Some(meta.has_final_newline),
            controls,
            c2pa_unstructured_text,
        })
    }
}

fn undecoded(
    encoding: TextEncoding,
    utf8_bom: bool,
    byte_size: u64,
    digest: Digest,
) -> PlainTextInventory {
    PlainTextInventory {
        encoding,
        valid_up_to: None,
        utf8_bom,
        byte_size,
        digest,
        line_endings: None,
        final_newline: None,
        controls: ControlCounts::default(),
        c2pa_unstructured_text: CarrierPresence::NotDecoded,
    }
}

fn count_controls(text: &str) -> ControlCounts {
    let mut counts = ControlCounts::default();
    for character in text.chars() {
        match classify(character) {
            Some(ControlClass::C0) => counts.c0 += 1,
            Some(ControlClass::C1) => counts.c1 += 1,
            Some(ControlClass::Bidi) => counts.bidi += 1,
            Some(ControlClass::VariationSelector) => counts.variation_selectors += 1,
            Some(ControlClass::ZeroWidth) => counts.zero_width += 1,
            Some(ControlClass::OtherFormat) => counts.other_format += 1,
            None => {}
        }
    }
    counts
}

enum ControlClass {
    C0,
    C1,
    Bidi,
    VariationSelector,
    ZeroWidth,
    OtherFormat,
}

fn classify(character: char) -> Option<ControlClass> {
    if matches!(character, '\t' | '\n' | '\r') {
        return None;
    }
    let code = u32::from(character);
    if matches!(
        code,
        0x061C | 0x200E | 0x200F | 0x202A..=0x202E | 0x2066..=0x2069
    ) {
        return Some(ControlClass::Bidi);
    }
    if matches!(code, 0xFE00..=0xFE0F | 0xE0100..=0xE01EF) {
        return Some(ControlClass::VariationSelector);
    }
    if matches!(code, 0x200B..=0x200D | 0x2060 | 0xFEFF | 0x180E) {
        return Some(ControlClass::ZeroWidth);
    }
    if (0x80..=0x9F).contains(&code) {
        return Some(ControlClass::C1);
    }
    if character.is_control() || character == '\u{7f}' {
        return Some(ControlClass::C0);
    }
    if matches!(
        code,
        0x00AD
            | 0x034F
            | 0x115F
            | 0x1160
            | 0x17B4
            | 0x17B5
            | 0x180B..=0x180D
            | 0x2028
            | 0x2029
            | 0x202F
            | 0x2061..=0x2064
            | 0x206A..=0x206F
            | 0x3164
            | 0xFFA0
            | 0xFFF9..=0xFFFB
            | 0xE0001
            | 0xE0020..=0xE007F
    ) {
        return Some(ControlClass::OtherFormat);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{CarrierPresence, TextAdapter, TextEncoding};
    use crate::{MAX_PLAIN_TEXT_BYTES, UTF8_BOM};

    #[test]
    fn inventories_utf8_without_claiming_a_credential() {
        let report = TextAdapter::inventory(b"Hello world\n").expect("utf-8");
        assert_eq!(report.encoding, TextEncoding::Utf8);
        assert!(!report.utf8_bom);
        assert_eq!(report.c2pa_unstructured_text, CarrierPresence::Absent);
        assert_eq!(report.controls.variation_selectors, 0);
        let encoded = serde_json::to_string(&report).expect("serialize");
        assert!(!encoded.contains("Hello"));
    }

    #[test]
    fn bom_and_variation_selector_are_a_possible_wrapper_only() {
        let mut source = UTF8_BOM.to_vec();
        source.extend_from_slice("\u{fe00}plain".as_bytes());
        let report = TextAdapter::inventory(&source).expect("utf-8");
        assert!(report.utf8_bom);
        assert_eq!(report.controls.variation_selectors, 1);
        assert_eq!(report.c2pa_unstructured_text, CarrierPresence::Possible);
        let encoded = serde_json::to_string(&report).expect("serialize");
        assert!(!encoded.contains("plain"));
        assert!(!encoded.contains("valid"));
    }

    #[test]
    fn utf16_and_invalid_utf8_are_inventory_facts() {
        let le = TextAdapter::inventory(b"\xFF\xFEa\0").expect("utf-16 le");
        assert_eq!(le.encoding, TextEncoding::Utf16Le);
        assert_eq!(le.c2pa_unstructured_text, CarrierPresence::NotDecoded);
        let be = TextAdapter::inventory(b"\xFE\xFF\0a").expect("utf-16 be");
        assert_eq!(be.encoding, TextEncoding::Utf16Be);
        let invalid = TextAdapter::inventory(b"a\xFF").expect("invalid utf-8");
        assert_eq!(invalid.encoding, TextEncoding::InvalidUtf8);
        assert_eq!(invalid.valid_up_to, Some(1));
        assert_eq!(invalid.c2pa_unstructured_text, CarrierPresence::NotDecoded);
    }

    #[test]
    fn control_classes_are_disjoint_counts() {
        let report = TextAdapter::inventory("\u{1b}\u{9b}\u{202e}\u{200b}\u{00ad}".as_bytes())
            .expect("utf-8");
        assert_eq!(report.controls.c0, 1);
        assert_eq!(report.controls.c1, 1);
        assert_eq!(report.controls.bidi, 1);
        assert_eq!(report.controls.zero_width, 1);
        assert_eq!(report.controls.other_format, 1);
        assert_eq!(report.controls.variation_selectors, 0);
    }

    #[test]
    fn oversized_input_is_still_a_hard_error() {
        let source = vec![b'a'; MAX_PLAIN_TEXT_BYTES + 1];
        assert!(TextAdapter::inventory(&source).is_err());
    }
}
