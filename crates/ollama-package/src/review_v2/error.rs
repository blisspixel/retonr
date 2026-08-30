use thiserror::Error;

use crate::{RuntimeReconstructionError, RuntimeSourceBuildInputError};

/// Opaque failure returned by a caller-supplied review-evidence opener.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("runtime-package review evidence is unavailable")]
pub struct RuntimePackageReviewEvidenceOpenError;

/// Controlled source-build review validation failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RuntimePackageReviewV2Error {
    /// Cooperative cancellation was requested.
    #[error("runtime-package review verification was cancelled")]
    Cancelled,
    /// A configured or declared review ceiling is invalid or exceeded.
    #[error("runtime-package review limit was exceeded")]
    LimitExceeded,
    /// The encoded review exceeds its configured byte ceiling.
    #[error("runtime-package review exceeds its byte limit")]
    ReviewTooLarge,
    /// JSON is malformed, ambiguous, duplicated, or contains unknown fields.
    #[error("runtime-package review encoding is invalid")]
    InvalidEncoding,
    /// JSON is valid but not in the deterministic canonical representation.
    #[error("runtime-package review encoding is not canonical")]
    NoncanonicalEncoding,
    /// The review schema version is unsupported.
    #[error("runtime-package review schema is unsupported")]
    UnsupportedSchema,
    /// Runtime family, version, or source revision is invalid.
    #[error("runtime-package review identity is invalid")]
    InvalidIdentity,
    /// The target is outside the first controlled source-build subset.
    #[error("runtime-package review target is unsupported")]
    UnsupportedTarget,
    /// Evidence declarations are missing, invalid, excessive, or noncanonical.
    #[error("runtime-package review evidence declaration is invalid")]
    InvalidEvidence,
    /// A referenced evidence stream could not be opened.
    #[error("runtime-package review evidence is unavailable")]
    EvidenceUnavailable,
    /// A referenced evidence stream could not be read.
    #[error("runtime-package review evidence could not be read")]
    EvidenceRead,
    /// Referenced evidence bytes do not have their exact declared length.
    #[error("runtime-package review evidence size does not match")]
    EvidenceSizeMismatch,
    /// Referenced evidence bytes do not have their exact declared digest.
    #[error("runtime-package review evidence digest does not match")]
    EvidenceDigestMismatch,
    /// The frozen source-build input manifest is invalid or has the wrong identity.
    #[error("runtime-package review source-build inputs are invalid")]
    InvalidSourceBuildInputs,
    /// Frozen source-build input byte verification failed.
    #[error("runtime-package review source-build input verification failed: {0}")]
    SourceBuildInputs(RuntimeSourceBuildInputError),
    /// Required checks are incomplete, reordered, duplicated, or weakly evidenced.
    #[error("runtime-package review checks are invalid")]
    InvalidChecks,
    /// Admission status conflicts with check results, layout evidence, or identity.
    #[error("runtime-package review disposition is invalid")]
    InvalidDisposition,
    /// Final runtime reconstruction failed.
    #[error("runtime-package review runtime reconstruction failed: {0}")]
    RuntimeReconstruction(RuntimeReconstructionError),
    /// Reconstructed runtime identity does not match the reviewed source build.
    #[error("runtime-package review runtime binding is invalid")]
    InvalidRuntimeBinding,
}
