use crate::{
    DocumentReviewError, DocumentReviewRequest, DocumentReviewResult, MAX_CANDIDATE_CHECK_BYTES,
};
use rewrite_engine::{
    MAX_PROTECTED_TERM_BYTES, MAX_PROTECTED_TERM_TOTAL_BYTES, MAX_PROTECTED_TERMS,
};

/// Bridge-local monotonic identity of an accepted review operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DocumentReviewOperationId(pub(super) u64);
impl DocumentReviewOperationId {
    /// Returns the numeric operation identity, without granting any authority.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Owned complete review payload with bounded retained allocation.
pub struct DocumentReviewOperationRequest(pub(super) DocumentReviewRequest);
impl DocumentReviewOperationRequest {
    /// Returns the complete source byte count, without exposing document text.
    #[must_use]
    pub fn source_bytes(&self) -> usize {
        self.0.source.len()
    }

    /// Returns the complete candidate byte count, distinguishing absence from an empty candidate.
    #[must_use]
    pub fn candidate_bytes(&self) -> Option<usize> {
        self.0.candidate.as_ref().map(Vec::len)
    }

    /// Bounds complete inputs and exact protected-term transport without parsing.
    /// Validation still uses the existing shared review service over all bytes.
    ///
    /// # Errors
    /// Returns `InputLimitExceeded` for a document or term transport budget breach.
    pub fn new(request: DocumentReviewRequest) -> Result<Self, DocumentReviewOperationError> {
        if request.source.len() > MAX_CANDIDATE_CHECK_BYTES
            || request
                .candidate
                .as_ref()
                .is_some_and(|candidate| candidate.len() > MAX_CANDIDATE_CHECK_BYTES)
            || request.protected_terms.len() > MAX_PROTECTED_TERMS
        {
            return Err(DocumentReviewOperationError::InputLimitExceeded);
        }
        let retained_bytes = request
            .source
            .len()
            .checked_add(request.candidate.as_ref().map_or(0, Vec::len))
            .and_then(|bytes| {
                request
                    .protected_terms
                    .iter()
                    .try_fold(bytes, |bytes, term| bytes.checked_add(term.len()))
            });
        let ceiling = MAX_CANDIDATE_CHECK_BYTES
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(MAX_PROTECTED_TERM_TOTAL_BYTES));
        if retained_bytes
            .zip(ceiling)
            .is_none_or(|(bytes, ceiling)| bytes > ceiling)
            || request
                .protected_terms
                .iter()
                .any(|term| term.len() > MAX_PROTECTED_TERM_BYTES)
            || request
                .protected_terms
                .iter()
                .try_fold(0_usize, |count, term| count.checked_add(term.len()))
                .is_none_or(|count| count > MAX_PROTECTED_TERM_TOTAL_BYTES)
        {
            return Err(DocumentReviewOperationError::InputLimitExceeded);
        }
        Ok(Self(DocumentReviewRequest {
            source: request.source.into_boxed_slice().into_vec(),
            candidate: request
                .candidate
                .map(|candidate| candidate.into_boxed_slice().into_vec()),
            protected_terms: request
                .protected_terms
                .into_iter()
                .map(|term| term.into_boxed_str().into_string())
                .collect::<Vec<_>>()
                .into_boxed_slice()
                .into_vec(),
        }))
    }
}
impl std::fmt::Debug for DocumentReviewOperationRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Latest-only inert service completion; text remains presentation-untrusted.
#[derive(Debug)]
pub struct DocumentReviewOperationCompletion {
    /// Identity assigned when this complete input was accepted.
    pub operation_id: DocumentReviewOperationId,
    /// Exact shared service result or content-redacted failure.
    pub result: Result<DocumentReviewResult, DocumentReviewOperationError>,
}

/// Content-redacted background orchestration failure.
#[derive(Debug, thiserror::Error)]
#[error("{error}")]
pub struct DocumentReviewOperationSubmissionFailure {
    /// Original bounded request retained for an explicit retry.
    pub request: DocumentReviewOperationRequest,
    /// Reason no operation was accepted or cancelled.
    #[source]
    pub error: DocumentReviewOperationError,
}

/// Content-redacted background orchestration failure.
#[derive(Debug, thiserror::Error)]
pub enum DocumentReviewOperationError {
    /// The cleanup supervisor could not be created; no worker was started.
    #[error("document review operation worker unavailable")]
    WorkerUnavailable,
    /// Owned transport exceeds the existing document or term budget.
    #[error("document review operation input limit exceeded")]
    InputLimitExceeded,
    /// Mailbox ownership is temporarily unavailable; no blocking is performed.
    #[error("document review operation mailbox is busy")]
    Busy,
    /// Shutdown, worker failure, or mailbox poisoning stopped the operation.
    #[error("document review operation worker stopped")]
    Stopped,
    /// Checked monotonic identities cannot advance further.
    #[error("document review operation identifiers exhausted")]
    IdentifierExhausted,
    /// The operation's original cancellation token was cancelled.
    #[error("document review operation cancelled")]
    Cancelled,
    /// Existing shared service validation or review failed.
    #[error(transparent)]
    Review(#[from] DocumentReviewError),
}
