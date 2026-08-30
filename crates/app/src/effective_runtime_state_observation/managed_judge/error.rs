use std::fmt;

use rewrite_runtime_attestor::{
    AttachedProcessWitnessError, ManagedGenerationWorkerError, NativeLoadObserverError,
};
use thiserror::Error;

/// Failure from the app-owned managed-judge native observation sequence.
#[derive(Error)]
pub enum ManagedJudgeObservationError {
    /// The current platform cannot construct the concrete managed Linux observer.
    #[error("managed judge observation is unsupported on this platform")]
    UnsupportedPlatform,
    /// Work was cancelled before an exact observation boundary completed.
    #[error("managed judge observation was cancelled")]
    Cancelled,
    /// The retained absolute operation deadline was reached.
    #[error("managed judge observation deadline was reached")]
    DeadlineExceeded,
    /// Checked schedule or response-count arithmetic overflowed.
    #[error("managed judge observation count is invalid")]
    InvalidCount,
    /// The retained callback sequence was reordered, repeated, or otherwise invalid.
    #[error("managed judge response observation sequence is invalid")]
    InvalidResponseSequence,
    /// A failed response attempt permanently invalidated the retained sequence.
    #[error("managed judge response attempt failed before completing its span")]
    FailedResponseAttempt,
    /// Preflight did not contain the exact reviewed seven-response sequence.
    #[error("managed judge preflight response span is invalid")]
    InvalidPreflightSpan,
    /// One attempt did not contain exactly nine contiguous responses.
    #[error("managed judge attempt response span is invalid")]
    InvalidAttemptSpan,
    /// The supplied schedule cursor was not the next exact cursor.
    #[error("managed judge schedule cursor is invalid")]
    InvalidScheduleCursor,
    /// Another attempt observation must be sealed before progress can continue.
    #[error("managed judge attempt observation is awaiting sealing")]
    AwaitingSeal,
    /// Preflight evidence must be sealed before attempt traffic can begin.
    #[error("managed judge preflight observation is awaiting sealing")]
    AwaitingPreflightSeal,
    /// Worker observation did not occur at exact attempt response offset 4.
    #[error("managed judge worker observation phase is invalid")]
    InvalidWorkerObservationPhase,
    /// Attempt completion did not include a complete worker observation bracket.
    #[error("managed judge worker observation is incomplete")]
    IncompleteWorkerObservation,
    /// A supplied observation came from another sequence or schedule entry.
    #[error("managed judge attempt observation authority was substituted")]
    ObservationSubstitution,
    /// Request, response, receipt, preflight, or ordinal closure was substituted.
    #[error("managed judge attempt request and receipt closure is invalid")]
    AttemptClosureMismatch,
    /// The effective state did not bind the exact process, native, and worker evidence.
    #[error("managed judge effective-state observation binding is invalid")]
    EffectiveStateMismatch,
    /// The sequence did not close over every schedule entry.
    #[error("managed judge observation sequence is incomplete")]
    IncompleteSequence,
    /// The concrete process or connection observer failed.
    #[error("managed judge process observation failed")]
    Process(#[source] AttachedProcessWitnessError),
    /// The concrete native-load observer failed.
    #[error("managed judge native-load observation failed")]
    NativeLoad(#[source] NativeLoadObserverError),
    /// The concrete managed generation-worker observer failed.
    #[error("managed judge generation-worker observation failed")]
    Worker(#[source] ManagedGenerationWorkerError),
}

impl fmt::Debug for ManagedJudgeObservationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedPlatform => "ManagedJudgeObservationError::UnsupportedPlatform",
            Self::Cancelled => "ManagedJudgeObservationError::Cancelled",
            Self::DeadlineExceeded => "ManagedJudgeObservationError::DeadlineExceeded",
            Self::InvalidCount => "ManagedJudgeObservationError::InvalidCount",
            Self::InvalidResponseSequence => {
                "ManagedJudgeObservationError::InvalidResponseSequence"
            }
            Self::FailedResponseAttempt => "ManagedJudgeObservationError::FailedResponseAttempt",
            Self::InvalidPreflightSpan => "ManagedJudgeObservationError::InvalidPreflightSpan",
            Self::InvalidAttemptSpan => "ManagedJudgeObservationError::InvalidAttemptSpan",
            Self::InvalidScheduleCursor => "ManagedJudgeObservationError::InvalidScheduleCursor",
            Self::AwaitingSeal => "ManagedJudgeObservationError::AwaitingSeal",
            Self::AwaitingPreflightSeal => "ManagedJudgeObservationError::AwaitingPreflightSeal",
            Self::InvalidWorkerObservationPhase => {
                "ManagedJudgeObservationError::InvalidWorkerObservationPhase"
            }
            Self::IncompleteWorkerObservation => {
                "ManagedJudgeObservationError::IncompleteWorkerObservation"
            }
            Self::ObservationSubstitution => {
                "ManagedJudgeObservationError::ObservationSubstitution"
            }
            Self::AttemptClosureMismatch => "ManagedJudgeObservationError::AttemptClosureMismatch",
            Self::EffectiveStateMismatch => "ManagedJudgeObservationError::EffectiveStateMismatch",
            Self::IncompleteSequence => "ManagedJudgeObservationError::IncompleteSequence",
            Self::Process(_) => "ManagedJudgeObservationError::Process",
            Self::NativeLoad(_) => "ManagedJudgeObservationError::NativeLoad",
            Self::Worker(_) => "ManagedJudgeObservationError::Worker",
        })
    }
}
