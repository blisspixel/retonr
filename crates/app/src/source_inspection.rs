//! Pre-model plain-text inventory without rewrite or credential validation.

use rewrite_text_adapter::{MAX_PLAIN_TEXT_BYTES, PlainTextInventory, TextAdapter};

use crate::AppError;

/// Maximum accepted source or candidate size for the plain-text check service.
pub const MAX_CANDIDATE_CHECK_BYTES: usize = MAX_PLAIN_TEXT_BYTES;

/// Inventories one bounded source buffer before model work.
///
/// The report does not strip bytes, parse a Content Credential, or follow an
/// external reference.
///
/// # Errors
///
/// Returns [`AppError::TextAdapter`] when the buffer exceeds the plain-text
/// ceiling.
pub fn inspect_plain_text(source: &[u8]) -> Result<PlainTextInventory, AppError> {
    Ok(TextAdapter::inventory(source)?)
}

#[cfg(test)]
mod tests {
    use rewrite_text_adapter::TextEncoding;

    use super::inspect_plain_text;

    #[test]
    fn inspect_does_not_require_a_candidate_or_model() {
        let report = inspect_plain_text(b"Hello world\n").expect("utf-8");
        assert_eq!(report.encoding, TextEncoding::Utf8);
        assert!(!report.utf8_bom);
    }
}
