use thiserror::Error;

/// Failure to establish complete retained-program license evidence.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RetainedProgramLicenseEvidenceError {
    /// One configured resource ceiling is zero or exceeds the hard limit.
    #[error("retained-program license-evidence limits are invalid")]
    InvalidLimits,
    /// The supplied strong closures do not describe the same source-input set.
    #[error("retained-program license-evidence closure bindings do not match")]
    ClosureBindingMismatch,
    /// A required evidence or lockfile role is missing, duplicated, or aliased.
    #[error("retained-program license-evidence component roles are invalid")]
    InvalidComponentRoles,
    /// A required retained component could not be opened.
    #[error("retained-program license-evidence component is unavailable")]
    ComponentUnavailable,
    /// A required retained component could not be read.
    #[error("retained-program license-evidence component could not be read")]
    ComponentRead,
    /// Reopened component bytes differ from their verified measurement.
    #[error("retained-program license-evidence component measurement changed")]
    ComponentMeasurementMismatch,
    /// Cooperative cancellation was requested.
    #[error("retained-program license-evidence verification was cancelled")]
    Cancelled,
    /// The legacy eight-entry inventory lacks exact subjects and material bytes.
    #[error("legacy retained-program license evidence is incomplete")]
    LegacyEvidenceIncomplete,
    /// The inventory schema is not the supported fail-closed evidence schema.
    #[error("retained-program license-evidence schema is unsupported")]
    UnsupportedSchema,
    /// The inventory is malformed or uses unknown fields or values.
    #[error("retained-program license evidence is invalid")]
    InvalidInventory,
    /// The inventory bytes are not the unique canonical JSON encoding.
    #[error("retained-program license evidence is not canonical")]
    NoncanonicalInventory,
    /// A fixed parse, subject, material, string, or byte ceiling was exceeded.
    #[error("retained-program license evidence exceeds a fixed limit")]
    QuotaExceeded,
    /// Exact component or Cargo package subjects are absent or duplicated.
    #[error("retained-program license evidence has an incomplete subject set")]
    IncompleteSubjectSet,
    /// Evidence refers to a subject outside the bound source and Cargo closures.
    #[error("retained-program license evidence refers to an unknown subject")]
    UnknownSubject,
    /// A material record is duplicated, unreferenced, or multiply referenced.
    #[error("retained-program license evidence has an invalid material set")]
    InvalidMaterialSet,
    /// Retained license or notice bytes do not match their declared measurement.
    #[error("retained-program license material measurement does not match")]
    MaterialMeasurementMismatch,
    /// A declared expression or reviewer-retained origin path is invalid.
    #[error("retained-program license evidence contains invalid reviewer facts")]
    InvalidReviewerFact,
}
