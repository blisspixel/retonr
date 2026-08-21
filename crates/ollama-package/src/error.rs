use thiserror::Error;

/// Opaque failure returned by a caller-supplied blob opener.
///
/// The unit type prevents filesystem paths or other sensitive details from
/// entering reconstruction diagnostics.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("model package blob is unavailable")]
pub struct BlobOpenError;

/// Offline Ollama package reconstruction failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ReconstructionError {
    /// Cooperative cancellation was requested.
    #[error("model package reconstruction was cancelled")]
    Cancelled,
    /// A caller-supplied stream failed without exposing its diagnostic text.
    #[error("model package input could not be read")]
    InputRead,
    /// A required descriptor could not be opened.
    #[error("model package blob is unavailable")]
    BlobUnavailable,
    /// An encoded manifest exceeded its fixed byte ceiling.
    #[error("Ollama manifest exceeds its limit")]
    ManifestTooLarge,
    /// Manifest JSON was malformed, ambiguous, or contained unknown fields.
    #[error("Ollama manifest encoding is invalid")]
    InvalidManifest,
    /// The manifest contract or exact layer shape is unsupported.
    #[error("Ollama manifest shape is unsupported")]
    UnsupportedManifest,
    /// A descriptor digest, size, or media type was invalid.
    #[error("Ollama descriptor is invalid")]
    InvalidDescriptor,
    /// A blob did not have its exact declared length.
    #[error("Ollama blob size does not match its descriptor")]
    BlobSizeMismatch,
    /// A blob did not have its exact declared SHA-256 digest.
    #[error("Ollama blob digest does not match its descriptor")]
    BlobDigestMismatch,
    /// A supported-size JSON blob was malformed or ambiguous.
    #[error("Ollama package JSON is invalid")]
    InvalidJson,
    /// Ollama configuration fields required by the narrow contract were invalid.
    #[error("Ollama model configuration is unsupported")]
    UnsupportedConfiguration,
    /// A retained textual layer was empty or not UTF-8.
    #[error("Ollama text layer is invalid")]
    InvalidTextLayer,
    /// The GGUF header or structural table was malformed.
    #[error("GGUF structure is invalid")]
    InvalidGguf,
    /// The GGUF version, value type, or narrow structural shape is unsupported.
    #[error("GGUF structure is unsupported")]
    UnsupportedGguf,
    /// A fixed parsing count or byte budget was exceeded.
    #[error("model package parsing limit exceeded")]
    LimitExceeded,
    /// A GGUF string was not valid UTF-8.
    #[error("GGUF string encoding is invalid")]
    InvalidUtf8,
    /// GGUF metadata or tensor names were duplicated.
    #[error("GGUF table contains a duplicate name")]
    DuplicateName,
    /// A canonical model package contract rejected the reconstructed state.
    #[error("reconstructed model package contract is invalid")]
    ModelContract,
}

/// Result type for offline package reconstruction.
pub type ReconstructionResult<T> = Result<T, ReconstructionError>;
