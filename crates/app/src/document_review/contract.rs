use super::{AppError, EditorialComparison, EditorialFinding, PlainTextInventory, RewriteRecord};

/// Complete byte inputs for an explicitly requested model-free review.
pub struct DocumentReviewRequest {
    /// Complete source bytes, never a clipped preview.
    pub source: Vec<u8>,
    /// Optional complete supplied candidate bytes.
    pub candidate: Option<Vec<u8>>,
    /// Exact protected terms for candidate checking; requires a candidate.
    pub protected_terms: Vec<String>,
}

impl std::fmt::Debug for DocumentReviewRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentReviewRequest")
            .field("source_bytes", &self.source.len())
            .field("candidate_bytes", &self.candidate.as_ref().map(Vec::len))
            .field("protected_term_count", &self.protected_terms.len())
            .finish_non_exhaustive()
    }
}

/// Exact bounded editorial evidence, independent of presentation clipping.
pub enum EditorialReview {
    /// Findings for a source with no candidate.
    Document(Vec<EditorialFinding>),
    /// Comparison of complete source and candidate inputs.
    Comparison(EditorialComparison),
}

impl std::fmt::Debug for EditorialReview {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Document(findings) => f
                .debug_struct("Document")
                .field("finding_count", &findings.len())
                .finish_non_exhaustive(),
            Self::Comparison(comparison) => f
                .debug_struct("Comparison")
                .field("source_finding_count", &comparison.source_findings.len())
                .field(
                    "candidate_finding_count",
                    &comparison.candidate_findings.len(),
                )
                .finish_non_exhaustive(),
        }
    }
}

/// Review data with complete-input identities and bounded untrusted previews.
pub struct DocumentReviewResult {
    /// Bounded source preview; presentation must sanitize it appropriately.
    pub source_preview: String,
    /// Bounded supplied candidate preview, including rejected candidates.
    pub candidate_preview: Option<String>,
    /// Inventory and digest of the complete source input.
    pub inspection: PlainTextInventory,
    /// Inventory and digest of the complete candidate input.
    pub candidate_inspection: Option<PlainTextInventory>,
    /// Exact application check record, including abstention.
    pub check: Option<RewriteRecord>,
    /// Editorial findings over complete inputs, before display clipping.
    pub editorial: EditorialReview,
}

impl std::fmt::Debug for DocumentReviewResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentReviewResult")
            .field("source_digest", &self.inspection.digest)
            .field("candidate_check_present", &self.check.is_some())
            .finish_non_exhaustive()
    }
}

/// Content-redacted operational review failure.
#[derive(Debug, thiserror::Error)]
pub enum DocumentReviewError {
    /// Protected terms are meaningful only for a candidate check.
    #[error("protected terms require a supplied candidate")]
    CandidateRequired,
    /// Either input is not supported UTF-8 text.
    #[error("review requires supported UTF-8 text")]
    UnsupportedEncoding,
    /// Original cancellation discards the complete review result.
    #[error("document review cancelled")]
    Cancelled,
    /// Combined source and candidate findings exceed the review ceiling.
    #[error("document review finding limit exceeded")]
    FindingLimitExceeded,
    /// The underlying shared application operation failed.
    #[error(transparent)]
    Application(#[from] AppError),
}
