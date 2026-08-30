use thiserror::Error;

/// Content-free failure while validating qualification-operation evidence.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum GenerationQualificationOperationContractError {
    /// Encoded JSON exceeds its fixed predecode ceiling.
    #[error("generation qualification operation evidence exceeds its limit")]
    EncodedRecordTooLarge,
    /// JSON is malformed, duplicated, trailing, or contains an unknown field.
    #[error("generation qualification operation evidence encoding is invalid")]
    InvalidEncoding,
    /// JSON differs from the one canonical encoding.
    #[error("generation qualification operation evidence encoding is noncanonical")]
    NonCanonicalEncoding,
    /// The schema version is unsupported.
    #[error("generation qualification operation evidence schema is unsupported")]
    UnsupportedSchema,
    /// Canonical identity bytes exceed their fixed ceiling.
    #[error("generation qualification operation identity exceeds its limit")]
    CanonicalEncodingTooLarge,
    /// Checked count or identity arithmetic overflowed.
    #[error("generation qualification operation encoding overflowed")]
    EncodingOverflow,
    /// A policy encoding is empty or exceeds its fixed ceiling.
    #[error("generation qualification assessment policy encoding is invalid")]
    InvalidPolicyEncoding,
    /// One or more common operation limits are invalid or inconsistent.
    #[error("generation qualification operation limits are invalid")]
    InvalidLimits,
    /// An entry or evidence count is empty, excessive, or inconsistent.
    #[error("generation qualification operation count is invalid")]
    InvalidCount,
    /// A semantic collection contains a duplicate identity.
    #[error("generation qualification operation collection contains a duplicate")]
    DuplicateEntry,
    /// A semantic collection is reordered.
    #[error("generation qualification operation order is invalid")]
    NonCanonicalOrder,
    /// The target, baseline, plan, suite, or system scope differs.
    #[error("generation qualification operation scope does not match")]
    ScopeMismatch,
    /// A typed dependency or derived field was substituted.
    #[error("generation qualification operation relationship does not match")]
    RelationshipMismatch,
    /// The closed decision rule or failure-policy digest differs.
    #[error("generation qualification operation decision rule does not match")]
    InvalidDecisionRule,
    /// Platform status, reason, or exact system facts disagree.
    #[error("generation qualification platform evidence does not match")]
    InvalidPlatformClosure,
    /// License decision, reason, permission, or exact package facts disagree.
    #[error("generation qualification license evidence does not match")]
    InvalidLicenseClosure,
    /// Phase progression or the terminal status is inconsistent.
    #[error("generation qualification operation terminal closure is invalid")]
    InvalidTerminalClosure,
    /// Elapsed time and the policy deadline disagree.
    #[error("generation qualification operation deadline does not match")]
    DeadlineMismatch,
    /// Finalization status and acquired-operation facts disagree.
    #[error("generation qualification operation finalization does not match")]
    FinalizationMismatch,
    /// Phase, checkpoint, reason, attempt, and terminal facts do not close.
    #[error("generation qualification phase interruption does not match")]
    InvalidInterruptionClosure,
}
