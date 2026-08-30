use thiserror::Error;

/// Opaque failure returned by a caller-supplied source-build input opener.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("runtime source-build input is unavailable")]
pub struct RuntimeSourceBuildInputOpenError;

/// Controlled runtime source-build input validation failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RuntimeSourceBuildInputError {
    /// Cooperative cancellation was requested.
    #[error("runtime source-build input verification was cancelled")]
    Cancelled,
    /// A configured or declared input ceiling is invalid or exceeded.
    #[error("runtime source-build input limit was exceeded")]
    LimitExceeded,
    /// JSON is malformed, ambiguous, duplicated, or contains unknown fields.
    #[error("runtime source-build input encoding is invalid")]
    InvalidEncoding,
    /// JSON is valid but not in the deterministic canonical representation.
    #[error("runtime source-build input encoding is not canonical")]
    NoncanonicalEncoding,
    /// The manifest schema version is unsupported.
    #[error("runtime source-build input schema is unsupported")]
    UnsupportedSchema,
    /// The first controlled-build target or policy is unsupported.
    #[error("runtime source-build policy is unsupported")]
    UnsupportedPolicy,
    /// Component declarations are incomplete, invalid, or noncanonical.
    #[error("runtime source-build component declaration is invalid")]
    InvalidComponent,
    /// Required closure roles are missing or have invalid cardinality.
    #[error("runtime source-build input closure is incomplete")]
    IncompleteClosure,
    /// The declared artifact-set identity does not match the component bytes.
    #[error("runtime source-build artifact-set identity is invalid")]
    InvalidArtifactSet,
    /// A declared component stream could not be opened.
    #[error("runtime source-build input component is unavailable")]
    ComponentUnavailable,
    /// A declared component stream could not be read.
    #[error("runtime source-build input component could not be read")]
    ComponentRead,
    /// A component did not have its exact declared byte length.
    #[error("runtime source-build input component size does not match")]
    ComponentSizeMismatch,
    /// A component did not have its exact declared byte digest.
    #[error("runtime source-build input component digest does not match")]
    ComponentDigestMismatch,
    /// Retained-program lineage or its canonical recipe is malformed or excessive.
    #[error("runtime source-build retained-program lineage is invalid")]
    InvalidProgramLineage,
    /// Retained-program lineage does not match the exact source-input members.
    #[error("runtime source-build retained-program lineage binding does not match")]
    ProgramLineageMismatch,
    /// The retained build program or isolation helper requires an ambient ELF interpreter.
    #[error("runtime source-build executable is not self-contained Linux x86-64 ELF")]
    NonSelfContainedExecutable,
}
