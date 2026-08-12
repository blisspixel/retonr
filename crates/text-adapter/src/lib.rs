//! Byte-aware plain-text parsing, edit application, and verification.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use rewrite_types::{
    AcceptedEdit, Digest, DocumentId, DocumentIr, MediaType, RewriteUnit, RewriteUnitId,
    SourceSpan, StructuralFingerprint,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

const UTF8_BOM: &[u8] = b"\xEF\xBB\xBF";
const UTF16_LE_BOM: &[u8] = b"\xFF\xFE";
const UTF16_BE_BOM: &[u8] = b"\xFE\xFF";

/// Maximum accepted plain-text source size in bytes.
pub const MAX_PLAIN_TEXT_BYTES: usize = 16 * 1024 * 1024;

/// Plain-text newline classification.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineEndingKind {
    /// The document contains no newline bytes.
    None,
    /// Every newline is LF.
    Lf,
    /// Every newline is CRLF.
    CrLf,
    /// Every newline is a lone CR.
    Cr,
    /// The document contains more than one newline representation.
    Mixed,
}

/// Observable source properties retained by the adapter.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct TextMetadata {
    /// Whether the source starts with a UTF-8 byte order mark.
    pub has_utf8_bom: bool,
    /// Newline representation found in the source.
    pub line_endings: LineEndingKind,
    /// Whether the decoded source ends with LF, CRLF, or CR.
    pub has_final_newline: bool,
    /// Complete source length in bytes.
    pub source_byte_len: usize,
}

/// Parsed document plus adapter reconstruction state.
#[derive(Clone, Debug)]
pub struct ParsedTextDocument {
    document: DocumentIr,
    original: Vec<u8>,
    body_offset: usize,
    metadata: TextMetadata,
}

impl ParsedTextDocument {
    /// Returns the format-neutral document representation.
    #[must_use]
    pub fn document(&self) -> &DocumentIr {
        &self.document
    }

    /// Returns observable source metadata.
    #[must_use]
    pub const fn metadata(&self) -> &TextMetadata {
        &self.metadata
    }

    /// Returns the exact original bytes.
    #[must_use]
    pub fn original(&self) -> &[u8] {
        &self.original
    }
}

/// Result of reparsing and checking a completed document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationReport {
    /// Whether the output meets every adapter obligation.
    pub valid: bool,
    /// Stable redacted diagnostics.
    pub diagnostics: Vec<String>,
}

/// Plain-text adapter failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum TextAdapterError {
    /// Source exceeds the bounded plain-text parser limit.
    #[error("plain text is {actual} bytes; the supported maximum is {maximum}")]
    InputTooLarge {
        /// Observed input length.
        actual: usize,
        /// Configured parser limit.
        maximum: usize,
    },
    /// UTF-16 is recognized but intentionally unsupported.
    #[error("UTF-16 plain text is unsupported; convert the source to UTF-8 explicitly")]
    UnsupportedUtf16,
    /// Source bytes are not valid UTF-8.
    #[error("plain text is not valid UTF-8 at byte {valid_up_to}")]
    InvalidUtf8 {
        /// First byte after the valid UTF-8 prefix.
        valid_up_to: usize,
    },
    /// An edit targets a unit that does not belong to this parsed document.
    #[error("edit targets an unknown rewrite unit")]
    UnknownUnit,
    /// More than one edit targets the same whole-document unit.
    #[error("plain-text prototype accepts at most one edit")]
    TooManyEdits,
    /// A source span could not be represented.
    #[error("plain-text source span is invalid")]
    InvalidSpan,
}

/// Stateless adapter for UTF-8 plain text.
#[derive(Clone, Copy, Debug, Default)]
pub struct TextAdapter;

impl TextAdapter {
    /// Parses UTF-8 source without normalizing any bytes.
    ///
    /// # Errors
    ///
    /// Returns [`TextAdapterError`] for UTF-16 or invalid UTF-8 input.
    pub fn parse(input: &[u8]) -> Result<ParsedTextDocument, TextAdapterError> {
        if input.len() > MAX_PLAIN_TEXT_BYTES {
            return Err(TextAdapterError::InputTooLarge {
                actual: input.len(),
                maximum: MAX_PLAIN_TEXT_BYTES,
            });
        }
        if input.starts_with(UTF16_LE_BOM) || input.starts_with(UTF16_BE_BOM) {
            return Err(TextAdapterError::UnsupportedUtf16);
        }

        let has_utf8_bom = input.starts_with(UTF8_BOM);
        let body_offset = usize::from(has_utf8_bom) * UTF8_BOM.len();
        let body = &input[body_offset..];
        let text = core::str::from_utf8(body).map_err(|error| TextAdapterError::InvalidUtf8 {
            valid_up_to: body_offset + error.valid_up_to(),
        })?;

        let source_digest = Digest::sha256(input);
        let document_id = DocumentId::from_digest(&source_digest);
        let rewrite_units = if text.is_empty() {
            Vec::new()
        } else {
            let source_span =
                SourceSpan::new(0, body.len()).map_err(|_error| TextAdapterError::InvalidSpan)?;
            vec![RewriteUnit {
                id: RewriteUnitId::new(&document_id, 0),
                source_span,
                text: text.to_owned(),
            }]
        };

        let metadata = metadata(input, text, has_utf8_bom);
        let structure = structural_fingerprint(text, has_utf8_bom);
        let document = DocumentIr::new(
            source_digest,
            MediaType::PlainText,
            rewrite_units,
            structure,
        )
        .map_err(|_error| TextAdapterError::InvalidSpan)?;

        Ok(ParsedTextDocument {
            document,
            original: input.to_vec(),
            body_offset,
            metadata,
        })
    }

    /// Applies an accepted whole-document edit while retaining the original BOM.
    ///
    /// An empty edit set returns the original bytes exactly.
    ///
    /// # Errors
    ///
    /// Returns [`TextAdapterError`] for duplicate or unknown unit edits.
    pub fn apply(
        parsed: &ParsedTextDocument,
        edits: &[AcceptedEdit],
    ) -> Result<Vec<u8>, TextAdapterError> {
        if edits.is_empty() {
            return Ok(parsed.original.clone());
        }
        if edits.len() > 1 {
            return Err(TextAdapterError::TooManyEdits);
        }

        let expected = parsed
            .document
            .rewrite_units
            .first()
            .ok_or(TextAdapterError::UnknownUnit)?;
        let edit = &edits[0];
        if edit.unit_id != expected.id {
            return Err(TextAdapterError::UnknownUnit);
        }

        let mut output = Vec::with_capacity(parsed.body_offset + edit.replacement.len());
        output.extend_from_slice(&parsed.original[..parsed.body_offset]);
        output.extend_from_slice(edit.replacement.as_bytes());
        Ok(output)
    }

    /// Reparses output and checks structural and byte-preservation obligations.
    #[must_use]
    pub fn verify(
        before: &ParsedTextDocument,
        output: &[u8],
        edits: &[AcceptedEdit],
    ) -> VerificationReport {
        if edits.is_empty() {
            return if output == before.original() {
                VerificationReport {
                    valid: true,
                    diagnostics: Vec::new(),
                }
            } else {
                VerificationReport {
                    valid: false,
                    diagnostics: vec!["original_bytes_changed_without_edit".to_owned()],
                }
            };
        }

        let Ok(after) = Self::parse(output) else {
            return VerificationReport {
                valid: false,
                diagnostics: vec!["output_reparse_failed".to_owned()],
            };
        };

        let mut diagnostics = Vec::new();
        if before.metadata.has_utf8_bom != after.metadata.has_utf8_bom {
            diagnostics.push("utf8_bom_changed".to_owned());
        }
        if before.document.structure != after.document.structure {
            diagnostics.push("text_structure_changed".to_owned());
        }

        VerificationReport {
            valid: diagnostics.is_empty(),
            diagnostics,
        }
    }

    /// Checks whether a replacement retains the source BOM and newline skeleton.
    #[must_use]
    pub fn replacement_preserves_structure(parsed: &ParsedTextDocument, replacement: &str) -> bool {
        structural_fingerprint(replacement, parsed.metadata.has_utf8_bom)
            == parsed.document.structure
    }

    /// Rejects newly introduced terminal controls, invisible separators, and
    /// bidirectional formatting characters.
    ///
    /// A source that already contains one of these characters is eligible only
    /// for byte-identical output in the current adapter capability.
    #[must_use]
    pub fn replacement_preserves_text_safety(source: &str, replacement: &str) -> bool {
        if source.chars().any(is_unsafe_text_character) {
            source == replacement
        } else {
            !replacement.chars().any(is_unsafe_text_character)
        }
    }
}

fn metadata(input: &[u8], text: &str, has_utf8_bom: bool) -> TextMetadata {
    let signature = newline_signature(text);
    let mut saw_lf = false;
    let mut saw_crlf = false;
    let mut saw_cr = false;
    for kind in &signature {
        match kind {
            b'L' => saw_lf = true,
            b'C' => saw_crlf = true,
            b'R' => saw_cr = true,
            _ => {}
        }
    }
    let kinds = u8::from(saw_lf) + u8::from(saw_crlf) + u8::from(saw_cr);
    let line_endings = match (kinds, saw_lf, saw_crlf, saw_cr) {
        (0, _, _, _) => LineEndingKind::None,
        (1, true, _, _) => LineEndingKind::Lf,
        (1, _, true, _) => LineEndingKind::CrLf,
        (1, _, _, true) => LineEndingKind::Cr,
        _ => LineEndingKind::Mixed,
    };

    TextMetadata {
        has_utf8_bom,
        line_endings,
        has_final_newline: text.ends_with('\n') || text.ends_with('\r'),
        source_byte_len: input.len(),
    }
}

fn structural_fingerprint(text: &str, has_utf8_bom: bool) -> StructuralFingerprint {
    let newline_kinds = newline_signature(text);
    let mut bytes = Vec::with_capacity(newline_kinds.len() + 1);
    bytes.push(u8::from(has_utf8_bom));
    bytes.extend_from_slice(&newline_kinds);
    StructuralFingerprint {
        kind: "plain-text-newline-skeleton-v1".to_owned(),
        digest: Digest::sha256(&bytes),
    }
}

fn is_unsafe_text_character(character: char) -> bool {
    let codepoint = u32::from(character);
    (character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
        || matches!(
            codepoint,
            0x061C
                | 0x200B..=0x200F
                | 0x2028..=0x202E
                | 0x2060..=0x206F
                | 0xFEFF
        )
}

fn newline_signature(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut result = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => {
                result.push(b'C');
                index += 2;
            }
            b'\r' => {
                result.push(b'R');
                index += 1;
            }
            b'\n' => {
                result.push(b'L');
                index += 1;
            }
            _ => index += 1,
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use rewrite_types::AcceptedEdit;

    use super::{LineEndingKind, MAX_PLAIN_TEXT_BYTES, TextAdapter, TextAdapterError, UTF8_BOM};

    #[test]
    fn parses_utf8_bom_and_crlf() {
        let mut source = UTF8_BOM.to_vec();
        source.extend_from_slice(b"one\r\ntwo\r\n");
        let parsed = TextAdapter::parse(&source).expect("valid UTF-8 fixture");
        assert!(parsed.metadata().has_utf8_bom);
        assert_eq!(parsed.metadata().line_endings, LineEndingKind::CrLf);
        assert!(parsed.metadata().has_final_newline);
        assert_eq!(parsed.document().rewrite_units.len(), 1);
    }

    #[test]
    fn rejects_utf16_and_invalid_utf8() {
        assert!(matches!(
            TextAdapter::parse(b"\xFF\xFEa\0"),
            Err(TextAdapterError::UnsupportedUtf16)
        ));
        assert!(matches!(
            TextAdapter::parse(b"a\xFF"),
            Err(TextAdapterError::InvalidUtf8 { valid_up_to: 1 })
        ));
    }

    #[test]
    fn applies_edit_and_preserves_bom() {
        let parsed = TextAdapter::parse(b"\xEF\xBB\xBFhello\n").expect("valid fixture");
        let unit = parsed.document().rewrite_units[0].id.clone();
        let edits = vec![AcceptedEdit {
            unit_id: unit,
            replacement: "Hello.\n".to_owned(),
        }];
        let output = TextAdapter::apply(&parsed, &edits).expect("known unit");
        assert_eq!(output, b"\xEF\xBB\xBFHello.\n");
        assert!(TextAdapter::verify(&parsed, &output, &edits).valid);
    }

    #[test]
    fn rejects_structure_change_during_verification() {
        let parsed = TextAdapter::parse(b"one\ntwo\n").expect("valid fixture");
        let edits = vec![AcceptedEdit {
            unit_id: parsed.document().rewrite_units[0].id.clone(),
            replacement: "one two\n".to_owned(),
        }];
        let output = TextAdapter::apply(&parsed, &edits).expect("known unit");
        assert!(!TextAdapter::verify(&parsed, &output, &edits).valid);
    }

    #[test]
    fn rejects_introduced_terminal_and_directionality_controls() {
        assert!(!TextAdapter::replacement_preserves_text_safety(
            "safe text",
            "safe\u{1b} text"
        ));
        assert!(!TextAdapter::replacement_preserves_text_safety(
            "safe text",
            "safe\u{202e} text"
        ));
        assert!(TextAdapter::replacement_preserves_text_safety(
            "safe\ttext",
            "safe\ttext."
        ));
    }

    #[test]
    fn rejects_oversized_input_before_decoding() {
        let source = vec![b'a'; MAX_PLAIN_TEXT_BYTES + 1];
        assert!(matches!(
            TextAdapter::parse(&source),
            Err(TextAdapterError::InputTooLarge { .. })
        ));
    }

    #[test]
    fn empty_document_has_no_units() {
        let parsed = TextAdapter::parse(b"").expect("empty UTF-8 is valid");
        assert!(parsed.document().rewrite_units.is_empty());
        assert_eq!(TextAdapter::apply(&parsed, &[]).expect("no edit"), b"");
    }

    proptest! {
        #[test]
        fn no_edit_is_always_byte_identical(source in proptest::collection::vec(any::<u8>(), 0..128)) {
            if let Ok(parsed) = TextAdapter::parse(&source) {
                let output = TextAdapter::apply(&parsed, &[]).expect("empty edit set is valid");
                prop_assert_eq!(output.as_slice(), source.as_slice());
                prop_assert!(TextAdapter::verify(&parsed, &output, &[]).valid);
            }
        }
    }
}
