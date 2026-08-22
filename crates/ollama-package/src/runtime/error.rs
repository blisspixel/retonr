use thiserror::Error;

/// Opaque failure returned by a caller-supplied runtime member opener.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("runtime package member is unavailable")]
pub struct MemberOpenError;

/// Offline Ollama runtime-package reconstruction failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RuntimeReconstructionError {
    /// Cooperative cancellation was requested.
    #[error("runtime package reconstruction was cancelled")]
    Cancelled,
    /// A caller-supplied stream failed without exposing its diagnostic text.
    #[error("runtime package member could not be read")]
    InputRead,
    /// A declared member could not be opened.
    #[error("runtime package member is unavailable")]
    MemberUnavailable,
    /// The reviewed layout exceeded its fixed byte ceiling.
    #[error("runtime package layout exceeds its limit")]
    LayoutTooLarge,
    /// Layout JSON was malformed, ambiguous, duplicated, or contained unknown fields.
    #[error("runtime package layout encoding is invalid")]
    InvalidLayout,
    /// The layout contract, family, transformation, or member shape is unsupported.
    #[error("runtime package layout is unsupported")]
    UnsupportedLayout,
    /// The native target is outside the first admitted Linux managed subset.
    #[error("runtime package target is unsupported")]
    UnsupportedTarget,
    /// A declared member path, role, policy, size, or digest was invalid.
    #[error("runtime package member declaration is invalid")]
    InvalidMember,
    /// A member did not have its exact declared length.
    #[error("runtime package member size does not match its declaration")]
    MemberSizeMismatch,
    /// A member did not have its exact declared SHA-256 digest.
    #[error("runtime package member digest does not match its declaration")]
    MemberDigestMismatch,
    /// Observed regular-file paths were not exactly the declared member set.
    #[error("runtime package observed tree does not match the layout")]
    ObservedTreeMismatch,
    /// A fixed member or byte budget was exceeded.
    #[error("runtime package reconstruction limit exceeded")]
    LimitExceeded,
    /// The reconstructed runtime-package contract rejected the complete overlay.
    #[error("reconstructed runtime package contract is invalid")]
    RuntimeContract,
}

/// Result type for offline runtime-package reconstruction.
pub type RuntimeReconstructionResult<T> = Result<T, RuntimeReconstructionError>;
