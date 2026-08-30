use thiserror::Error;

/// Content-free failure from deterministic qualification request construction.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum GenerationQualificationRequestBuildError {
    /// The typed case profile or its canonical preimage did not match.
    #[error("generation qualification request profile does not match")]
    ProfileMismatch,
    /// The approved managed-candidate policy did not match the generation system.
    #[error("generation qualification request policy does not match")]
    PolicyMismatch,
    /// A builder-owned component binding did not match the generation system.
    #[error("generation qualification request system does not match")]
    SystemMismatch,
    /// The case and deterministic contract relationship did not match.
    #[error("generation qualification request case does not match")]
    CaseMismatch,
    /// The retained source capability failed validation or changed.
    #[error("generation qualification request source does not match")]
    SourceMismatch,
    /// The caller cancelled source validation or request construction.
    #[error("generation qualification request construction was cancelled")]
    Cancelled,
    /// The source is not supported plain UTF-8 text.
    #[error("generation qualification request source encoding is unsupported")]
    SourceEncodingUnsupported,
    /// The text adapter did not produce exactly one whole-document rewrite unit.
    #[error("generation qualification request unit policy does not match")]
    UnitPolicyMismatch,
    /// Exact deterministic protection could not be planned.
    #[error("generation qualification request protection planning failed")]
    ProtectionFailed,
    /// Canonical grounded prompt rendering failed or exceeded its exact ceiling.
    #[error("generation qualification request prompt rendering failed")]
    PromptFailed,
    /// The selected operation policy or its common limits did not match.
    #[error("generation qualification request operation does not match")]
    OperationMismatch,
    /// The selected frozen plan did not bind the exact attempt position.
    #[error("generation qualification request plan does not match")]
    PlanMismatch,
    /// The selected planned attempt or its exact facts did not match.
    #[error("generation qualification request attempt does not match")]
    AttemptMismatch,
    /// The derived provider-neutral or structured request did not match.
    #[error("generation qualification request derivation does not match")]
    RequestMismatch,
}
