use thiserror::Error;

/// Redacted failure from managed generation-worker observation.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ManagedGenerationWorkerError {
    /// One or more caller-selected ceilings are invalid.
    #[error("managed generation-worker limits are invalid")]
    InvalidLimits,
    /// Package, profile, worker, or weight input is invalid.
    #[error("managed generation-worker request is invalid")]
    InvalidRequest,
    /// Native-load package or allowlist input is invalid.
    #[error("managed generation-worker native-load request is invalid")]
    InvalidNativeLoadRequest,
    /// Observation was cancelled.
    #[error("managed generation-worker observation was cancelled")]
    Cancelled,
    /// Observation exceeded its elapsed-time ceiling.
    #[error("managed generation-worker observation deadline was exceeded")]
    DeadlineExceeded,
    /// The platform has no admitted worker observation mechanism.
    #[error("managed generation-worker observation is unsupported")]
    Unsupported,
    /// The retained server exited or changed incarnation.
    #[error("managed generation-worker retained server changed")]
    ServerChanged,
    /// Process visibility or pidfd acquisition was insufficient.
    #[error("managed generation-worker process visibility is insufficient")]
    ProcessVisibilityInsufficient,
    /// Exactly one matching live descendant worker was not present.
    #[error("managed generation-worker count does not equal one")]
    WorkerCountMismatch,
    /// The worker exited, was reused, or changed incarnation.
    #[error("managed generation-worker process changed")]
    WorkerChanged,
    /// The worker parent chain did not stably terminate at the retained server.
    #[error("managed generation-worker parent chain is invalid")]
    ParentChainMismatch,
    /// Worker namespaces differ from the retained server namespaces.
    #[error("managed generation-worker namespaces do not match the server")]
    NamespaceMismatch,
    /// Worker effective UID or privilege state violates the closed policy.
    #[error("managed generation-worker privilege state is invalid")]
    PrivilegeMismatch,
    /// Worker executable object differs from the exact retained worker.
    #[error("managed generation-worker executable does not match")]
    ExecutableMismatch,
    /// Worker command bytes are malformed or violate the version policy.
    #[error("managed generation-worker command does not match")]
    CommandMismatch,
    /// Worker executable mappings were anonymous, deleted, or unavailable.
    #[error("managed generation-worker executable mapping is unverifiable")]
    UnverifiableExecutableMapping,
    /// Worker native code did not equal the exact admitted closure.
    #[error("managed generation-worker native closure does not match")]
    NativeClosureMismatch,
    /// Listener, helper, or utility code was mapped into the worker.
    #[error("managed generation-worker mapped forbidden server code")]
    ForbiddenServerCodeMapped,
    /// A known accelerator library was mapped by the CPU-only worker.
    #[error("managed generation-worker mapped an accelerator library")]
    AcceleratorLibraryMapped,
    /// The exact expected GGUF file object was not mapped.
    #[error("managed generation-worker model mapping does not match")]
    ModelMappingMismatch,
    /// An observation phase required for complete reobservation is absent.
    #[error("managed generation-worker observation phases are incomplete")]
    IncompleteObservation,
    /// A bounded process, parent, row, byte, component, or hash ceiling was exhausted.
    #[error("managed generation-worker observation exceeded a resource limit")]
    ResourceLimit,
    /// Evidence changed across an observation bracket.
    #[error("managed generation-worker observation changed during its bracket")]
    ObservationChanged,
    /// A redacted operating-system observation failed.
    #[error("managed generation-worker native observation failed")]
    PlatformObservationFailed,
}
