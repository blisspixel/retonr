use thiserror::Error;

use crate::RuntimeReconstructionError;

/// Opaque failure returned by a caller-owned report evidence or member opener.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("controlled-build report input is unavailable")]
pub struct RuntimeSourceBuildReportOpenError;

/// Controlled source-build report verification failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeSourceBuildReportError {
    /// Cooperative cancellation was requested.
    #[error("controlled-build report verification was cancelled")]
    Cancelled,
    /// A configured or declared report ceiling is invalid or exceeded.
    #[error("controlled-build report limit was exceeded")]
    LimitExceeded,
    /// JSON is malformed, ambiguous, duplicated, or contains unknown fields.
    #[error("controlled-build report encoding is invalid")]
    InvalidEncoding,
    /// JSON is valid but not in deterministic canonical form.
    #[error("controlled-build report encoding is not canonical")]
    NoncanonicalEncoding,
    /// The report schema version is unsupported.
    #[error("controlled-build report schema is unsupported")]
    UnsupportedSchema,
    /// A typed execution receipt is malformed, incomplete, failed, or false.
    #[error("controlled-build execution receipt is invalid")]
    InvalidExecutionReceipt,
    /// A typed execution receipt is valid JSON but not canonically encoded.
    #[error("controlled-build execution receipt is not canonical")]
    NoncanonicalExecutionReceipt,
    /// The report does not bind the supplied frozen input set and build plan.
    #[error("controlled-build report plan binding is invalid")]
    InvalidPlanBinding,
    /// A portable output-tree sidecar is malformed, unsupported, or inconsistent.
    #[error("controlled-build report output tree is invalid")]
    InvalidOutputTree,
    /// A portable output-tree sidecar is valid but not canonically encoded.
    #[error("controlled-build report output tree is not canonical")]
    NoncanonicalOutputTree,
    /// Attempt or referenced evidence declarations are incomplete or invalid.
    #[error("controlled-build report attempt is invalid")]
    InvalidAttempt,
    /// The declared rebuild comparison does not match reconstructed output bytes.
    #[error("controlled-build report comparison is invalid")]
    InvalidComparison,
    /// A referenced metadata record could not be opened.
    #[error("controlled-build report evidence is unavailable")]
    EvidenceUnavailable,
    /// A referenced metadata record could not be read completely.
    #[error("controlled-build report evidence could not be read")]
    EvidenceRead,
    /// A referenced metadata record has the wrong byte length.
    #[error("controlled-build report evidence size does not match")]
    EvidenceSizeMismatch,
    /// A referenced metadata record has the wrong byte digest.
    #[error("controlled-build report evidence digest does not match")]
    EvidenceDigestMismatch,
    /// A runtime output layout or member tree failed reconstruction.
    #[error("controlled-build runtime output reconstruction failed")]
    RuntimeReconstruction(#[source] RuntimeReconstructionError),
    /// A reconstructed output is not bound to the supplied inputs and policy.
    #[error("controlled-build runtime output binding is invalid")]
    InvalidRuntimeBinding,
}
