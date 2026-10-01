//! Bounded, model-free document review shared by presentation layers.

use rewrite_types::{
    CancellationToken, EditorialComparison, EditorialFinding, ReasonCode, RewriteRecord,
};

use crate::{
    AppError, CandidateCheckRequest, CandidateCheckService, EditorialLintService,
    PlainTextInventory, TextEncoding, inspect_plain_text,
};

mod contract;
pub use contract::{
    DocumentReviewError, DocumentReviewRequest, DocumentReviewResult, EditorialReview,
};

/// Maximum combined source and candidate findings retained by a review.
pub const MAX_DOCUMENT_REVIEW_FINDINGS: usize = 4096;
/// Maximum UTF-8 bytes in each review preview, including its truncation marker.
pub const MAX_DOCUMENT_REVIEW_PREVIEW_BYTES: usize = 65_536;

/// Shared read-only review orchestration without filesystem or model authority.
#[derive(Clone, Copy, Debug, Default)]
pub struct DocumentReviewService;

impl DocumentReviewService {
    /// Inventories complete inputs, checks a supplied candidate, and reports lint.
    ///
    /// Previews remain untrusted text. Callers must retain their filesystem and
    /// metadata decision boundaries and sanitize text for their presentation.
    /// This operation never writes, generates text, or grants runtime authority.
    /// Cancellation during noncooperative lint discards the result afterward.
    ///
    /// # Errors
    ///
    /// Refuses unsupported input, invalid protected terms, excessive findings,
    /// application failures, and cancellation. Candidate rejection is a review
    /// result containing the original application abstention record.
    pub fn review(
        request: DocumentReviewRequest,
        cancellation: &CancellationToken,
    ) -> Result<DocumentReviewResult, DocumentReviewError> {
        review_with_post_lint(request, cancellation, || {})
    }
}

fn review_with_post_lint(
    request: DocumentReviewRequest,
    cancellation: &CancellationToken,
    after_lint: impl FnOnce(),
) -> Result<DocumentReviewResult, DocumentReviewError> {
    ensure_active(cancellation)?;
    if request.candidate.is_none() && !request.protected_terms.is_empty() {
        return Err(DocumentReviewError::CandidateRequired);
    }
    let (source, inspection) = inspect(request.source, cancellation)?;
    let candidate = request
        .candidate
        .map(|bytes| inspect(bytes, cancellation))
        .transpose()?;
    let (check, editorial) = if let Some((text, _)) = &candidate {
        let result = CandidateCheckService::check_with_cancellation(
            CandidateCheckRequest::new(
                source.as_bytes().to_vec(),
                text.clone(),
                request.protected_terms,
            ),
            cancellation,
        )?;
        if result.record.reason == Some(ReasonCode::Cancelled) {
            return Err(DocumentReviewError::Cancelled);
        }
        ensure_active(cancellation)?;
        (
            Some(result.record),
            EditorialLintService::compare_bounded(&source, text, MAX_DOCUMENT_REVIEW_FINDINGS)
                .map(EditorialReview::Comparison),
        )
    } else {
        (
            None,
            EditorialLintService::lint_bounded(&source, MAX_DOCUMENT_REVIEW_FINDINGS)
                .map(EditorialReview::Document),
        )
    };
    after_lint();
    ensure_active(cancellation)?;
    let editorial = editorial.map_err(|_| DocumentReviewError::FindingLimitExceeded)?;
    let result = DocumentReviewResult {
        source_preview: preview(&source),
        candidate_preview: candidate.as_ref().map(|(text, _)| preview(text)),
        inspection,
        candidate_inspection: candidate.map(|(_, inventory)| inventory),
        check,
        editorial,
    };
    ensure_active(cancellation)?;
    Ok(result)
}

fn inspect(
    bytes: Vec<u8>,
    cancellation: &CancellationToken,
) -> Result<(String, PlainTextInventory), DocumentReviewError> {
    ensure_active(cancellation)?;
    let inventory = inspect_plain_text(&bytes)?;
    if inventory.encoding != TextEncoding::Utf8 {
        return Err(DocumentReviewError::UnsupportedEncoding);
    }
    let text = String::from_utf8(bytes).map_err(|_| DocumentReviewError::UnsupportedEncoding)?;
    ensure_active(cancellation)?;
    Ok((text, inventory))
}

fn ensure_active(cancellation: &CancellationToken) -> Result<(), DocumentReviewError> {
    if cancellation.is_cancelled() {
        Err(DocumentReviewError::Cancelled)
    } else {
        Ok(())
    }
}

fn preview(text: &str) -> String {
    if text.len() <= MAX_DOCUMENT_REVIEW_PREVIEW_BYTES {
        return text.to_owned();
    }
    let marker = "\n[Document preview truncated; validation used the complete input.]";
    let mut end = MAX_DOCUMENT_REVIEW_PREVIEW_BYTES.saturating_sub(marker.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let mut result = text[..end].to_owned();
    result.push_str(marker);
    result
}

#[cfg(test)]
mod tests;
