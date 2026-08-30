//! App-owned native-observation authority for one complete managed judge schedule.
//!
//! ```compile_fail
//! use rewrite_app::effective_runtime_state_observation::ManagedJudgeObservationSequence;
//!
//! fn require_clone<T: Clone>() {}
//! require_clone::<ManagedJudgeObservationSequence>();
//! ```
//!
//! ```compile_fail
//! use rewrite_app::effective_runtime_state_observation::ManagedJudgeObservationSequence;
//!
//! fn require_serialize<T: serde::Serialize>() {}
//! require_serialize::<ManagedJudgeObservationSequence>();
//! ```

mod contract;
mod error;
mod framing;
mod process;
mod sequence;
mod state;
mod worker;

pub use contract::{
    CompletedManagedJudgeObservationSequence, MANAGED_JUDGE_ATTEMPT_RESPONSE_COUNT,
    MANAGED_JUDGE_FIRST_RESIDENCY_RESPONSE_OFFSET, MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT,
    MANAGED_JUDGE_WORKER_RESPONSE_OFFSET, ManagedJudgeAttemptObservation,
    ManagedJudgeAttemptObserverBinding, ManagedJudgePreflightObservation,
    ManagedJudgePreflightObserverBinding,
};
pub use error::ManagedJudgeObservationError;
pub use sequence::ManagedJudgeObservationSequence;

#[cfg(test)]
pub(crate) use sequence::tests::{
    CompletedSequenceMutation, completed_sequence_for_effective_package,
    completed_sequence_for_observation_authority,
    completed_sequence_with_unsealed_preflight_binding, mutate_completed_sequence,
};
