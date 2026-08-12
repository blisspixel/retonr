use thiserror::Error;

/// Versioned request-contract validation failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ContractError {
    /// Request schema version is unsupported.
    #[error("unsupported generation request schema")]
    UnsupportedSchema,
    /// Requested artifact identity is internally inconsistent.
    #[error("generation artifact identifier and digest differ")]
    ArtifactMismatch,
    /// Source or output limits are zero or internally inconsistent.
    #[error("generation byte or context limits are invalid")]
    InvalidLimits,
    /// Candidate count is outside the supported contract.
    #[error("generation candidate count is invalid")]
    InvalidCandidateCount,
    /// Sampling values are non-finite or outside their supported range.
    #[error("generation sampling parameters are invalid")]
    InvalidSampling,
    /// Structured-output schema is empty, oversized, or has a mismatched digest.
    #[error("generation output contract is invalid")]
    InvalidOutputContract,
}

/// Stable inference failure category used for retry and abstention policy.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum InferenceErrorKind {
    /// Operation was cancelled cooperatively.
    #[error("cancelled")]
    Cancelled,
    /// Operation deadline expired.
    #[error("deadline exceeded")]
    Deadline,
    /// Bounded transient failure may be retried by explicit policy.
    #[error("retryable backend failure")]
    Retryable,
    /// Backend cannot satisfy the requested capability or version.
    #[error("incompatible backend")]
    Compatibility,
    /// Request violates local network, artifact, or inference policy.
    #[error("inference policy rejection")]
    Policy,
    /// Backend response violated the versioned contract.
    #[error("malformed backend response")]
    MalformedResponse,
    /// Permanent backend or runtime failure.
    #[error("permanent backend failure")]
    Permanent,
}

/// Redacted inference error containing no source or generated content.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{kind}: {code}")]
pub struct InferenceError {
    /// Stable error category.
    pub kind: InferenceErrorKind,
    /// Stable redacted detail code.
    pub code: String,
}

impl InferenceError {
    /// Creates an error from a stable category and safe detail code.
    #[must_use]
    pub fn new(kind: InferenceErrorKind, code: impl Into<String>) -> Self {
        Self {
            kind,
            code: code.into(),
        }
    }
}
