use std::{fmt, sync::Arc};

use rewrite_model::{
    CandidateJudgeScheduleId, NativeLoadObservation, OllamaRetainedSessionResponseId,
};
use rewrite_ollama::OllamaResidentSessionExecutionReceipt;
use rewrite_runtime_attestor::{
    AttachedProcessEvidence, ManagedGenerationWorkerEvidence,
    ManagedGenerationWorkerModelMappingEvidence, ManagedGenerationWorkerNativeLoadEvidence,
    RetainedTcpConnectionEvidence,
};
use rewrite_types::Digest;

use super::super::ManagedOllamaEffectiveRuntimeState;

/// Exact retained-session response count consumed by managed judge preflight.
pub const MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT: u32 = 7;
/// Exact retained-session response count consumed by each resident judge attempt.
pub const MANAGED_JUDGE_ATTEMPT_RESPONSE_COUNT: u32 = 9;
/// Exact response offset at which the managed generation worker is first observed.
pub const MANAGED_JUDGE_WORKER_RESPONSE_OFFSET: u32 = 4;
/// Zero-based offset of the first residency response within one attempt span.
pub const MANAGED_JUDGE_FIRST_RESIDENCY_RESPONSE_OFFSET: u32 = 4;

pub(super) struct ManagedJudgePreflightEvidence {
    pub(super) initial_process: AttachedProcessEvidence,
    pub(super) post_preflight_process: AttachedProcessEvidence,
    pub(super) final_process: AttachedProcessEvidence,
    pub(super) native_load: NativeLoadObservation,
    pub(super) connection_witness: RetainedTcpConnectionEvidence,
    pub(super) connection_observations: Vec<RetainedTcpConnectionEvidence>,
    pub(super) connection_binding_digest: Digest,
}

/// Sequence-bound preflight evidence awaiting sealing.
///
/// The token exposes the exact typed evidence expected by the existing managed
/// preflight report. It is noncloneable and nonserializable, and must be consumed
/// by [`super::ManagedJudgeObservationSequence::seal_preflight`].
///
/// ```compile_fail
/// use rewrite_app::effective_runtime_state_observation::ManagedJudgePreflightObservation;
///
/// fn require_clone<T: Clone>() {}
/// require_clone::<ManagedJudgePreflightObservation>();
/// ```
///
/// ```compile_fail
/// use rewrite_app::effective_runtime_state_observation::ManagedJudgePreflightObservation;
///
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<ManagedJudgePreflightObservation>();
/// ```
pub struct ManagedJudgePreflightObservation {
    pub(super) sequence_token: Arc<()>,
    pub(super) evidence: ManagedJudgePreflightEvidence,
}

/// Sealed exact preflight evidence retained by the completed schedule authority.
///
/// ```compile_fail
/// use rewrite_app::effective_runtime_state_observation::ManagedJudgePreflightObserverBinding;
///
/// fn require_clone<T: Clone>() {}
/// require_clone::<ManagedJudgePreflightObserverBinding>();
/// ```
///
/// ```compile_fail
/// use rewrite_app::effective_runtime_state_observation::ManagedJudgePreflightObserverBinding;
///
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<ManagedJudgePreflightObserverBinding>();
/// ```
pub struct ManagedJudgePreflightObserverBinding {
    pub(super) sequence_token: Arc<()>,
    pub(super) evidence: ManagedJudgePreflightEvidence,
}

macro_rules! preflight_accessors {
    ($type:ty) => {
        impl $type {
            /// Returns the process evidence captured before any retained response.
            #[must_use]
            pub const fn initial_process(&self) -> &AttachedProcessEvidence {
                &self.evidence.initial_process
            }

            /// Returns the process reobservation immediately after preflight traffic.
            #[must_use]
            pub const fn post_preflight_process(&self) -> &AttachedProcessEvidence {
                &self.evidence.post_preflight_process
            }

            /// Returns the process reobservation after preflight native-load observation.
            #[must_use]
            pub const fn final_process(&self) -> &AttachedProcessEvidence {
                &self.evidence.final_process
            }

            /// Returns the exact server native-load observation.
            #[must_use]
            pub const fn native_load(&self) -> &NativeLoadObservation {
                &self.evidence.native_load
            }

            /// Returns the initial connection observation followed by ordinals 1 through 7.
            #[must_use]
            pub fn connection_observations(&self) -> &[RetainedTcpConnectionEvidence] {
                &self.evidence.connection_observations
            }

            /// Returns the final connection witness from the preflight report sequence.
            #[must_use]
            pub const fn connection_witness(&self) -> &RetainedTcpConnectionEvidence {
                &self.evidence.connection_witness
            }

            /// Returns the internally derived complete preflight connection binding.
            #[must_use]
            pub const fn connection_binding_digest(&self) -> &Digest {
                &self.evidence.connection_binding_digest
            }
        }
    };
}

preflight_accessors!(ManagedJudgePreflightObservation);
preflight_accessors!(ManagedJudgePreflightObserverBinding);

impl fmt::Debug for ManagedJudgePreflightObservation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagedJudgePreflightObservation")
            .field(
                "connection_observation_count",
                &self.evidence.connection_observations.len(),
            )
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for ManagedJudgePreflightObserverBinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagedJudgePreflightObserverBinding")
            .field(
                "connection_observation_count",
                &self.evidence.connection_observations.len(),
            )
            .finish_non_exhaustive()
    }
}

/// One app-owned process, native-load, and connection observation awaiting sealing.
///
/// Construction is restricted to [`super::ManagedJudgeObservationSequence`]. The
/// type is noncloneable and nonserializable so copied observer fields cannot recreate
/// its sequence authority.
///
/// ```compile_fail
/// use rewrite_app::effective_runtime_state_observation::ManagedJudgeAttemptObservation;
///
/// fn require_clone<T: Clone>() {}
/// require_clone::<ManagedJudgeAttemptObservation>();
/// ```
///
/// ```compile_fail
/// use rewrite_app::effective_runtime_state_observation::ManagedJudgeAttemptObservation;
///
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<ManagedJudgeAttemptObservation>();
/// ```
pub struct ManagedJudgeAttemptObservation {
    pub(super) sequence_token: Arc<()>,
    pub(super) schedule_id: CandidateJudgeScheduleId,
    pub(super) schedule_cursor: u32,
    pub(super) first_response_ordinal: u64,
    pub(super) last_response_ordinal: u64,
    pub(super) process: AttachedProcessEvidence,
    pub(super) native_load: NativeLoadObservation,
    pub(super) initial_worker: ManagedGenerationWorkerEvidence,
    pub(super) final_worker: ManagedGenerationWorkerEvidence,
    pub(super) worker_native_load: ManagedGenerationWorkerNativeLoadEvidence,
    pub(super) model_mapping: ManagedGenerationWorkerModelMappingEvidence,
    pub(super) connection_binding_digest: Digest,
}

impl ManagedJudgeAttemptObservation {
    /// Returns the exact schedule cursor for this observation.
    #[must_use]
    pub const fn schedule_cursor(&self) -> u32 {
        self.schedule_cursor
    }

    /// Returns the final process observation derived by the retained authority.
    #[must_use]
    pub const fn process(&self) -> &AttachedProcessEvidence {
        &self.process
    }

    /// Returns the native-load observation derived by the retained authority.
    #[must_use]
    pub const fn native_load(&self) -> &NativeLoadObservation {
        &self.native_load
    }

    /// Returns the exact worker evidence captured at response offset 4.
    #[must_use]
    pub const fn initial_worker(&self) -> &ManagedGenerationWorkerEvidence {
        &self.initial_worker
    }

    /// Returns the mandatory worker reobservation after all nine responses.
    #[must_use]
    pub const fn final_worker(&self) -> &ManagedGenerationWorkerEvidence {
        &self.final_worker
    }

    /// Returns the worker native-code observation captured at response offset 4.
    #[must_use]
    pub const fn worker_native_load(&self) -> &ManagedGenerationWorkerNativeLoadEvidence {
        &self.worker_native_load
    }

    /// Returns the retained model-mapping observation captured at response offset 4.
    #[must_use]
    pub const fn model_mapping(&self) -> &ManagedGenerationWorkerModelMappingEvidence {
        &self.model_mapping
    }

    /// Returns the first response ordinal in this exact nine-response span.
    #[must_use]
    pub const fn first_response_ordinal(&self) -> u64 {
        self.first_response_ordinal
    }

    /// Returns the last response ordinal in this exact nine-response span.
    #[must_use]
    pub const fn last_response_ordinal(&self) -> u64 {
        self.last_response_ordinal
    }
}

impl fmt::Debug for ManagedJudgeAttemptObservation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagedJudgeAttemptObservation")
            .field("schedule_id", &self.schedule_id)
            .field("schedule_cursor", &self.schedule_cursor)
            .field("first_response_ordinal", &self.first_response_ordinal)
            .field("last_response_ordinal", &self.last_response_ordinal)
            .finish_non_exhaustive()
    }
}

/// Sealed app-owned observer binding for one exact schedule entry.
///
/// The binding is noncloneable and nonserializable. Its digest accessors support
/// later app-owned aggregate derivation but do not provide a constructor.
///
/// ```compile_fail
/// use rewrite_app::effective_runtime_state_observation::ManagedJudgeAttemptObserverBinding;
///
/// fn require_clone<T: Clone>() {}
/// require_clone::<ManagedJudgeAttemptObserverBinding>();
/// ```
///
/// ```compile_fail
/// use rewrite_app::effective_runtime_state_observation::ManagedJudgeAttemptObserverBinding;
///
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<ManagedJudgeAttemptObserverBinding>();
/// ```
pub struct ManagedJudgeAttemptObserverBinding {
    pub(super) sequence_token: Arc<()>,
    pub(super) schedule_id: CandidateJudgeScheduleId,
    pub(super) schedule_cursor: u32,
    pub(super) first_response_ordinal: u64,
    pub(super) last_response_ordinal: u64,
    pub(super) process_binding_digest: Digest,
    pub(super) native_load_binding_digest: Digest,
    pub(super) connection_binding_digest: Digest,
    pub(super) receipt_binding_digest: Digest,
    pub(super) receipt: OllamaResidentSessionExecutionReceipt,
    pub(super) effective_state: RetainedManagedJudgeEffectiveState,
}

pub(super) enum RetainedManagedJudgeEffectiveState {
    Managed(Box<ManagedOllamaEffectiveRuntimeState>),
    #[cfg(test)]
    Offline(Digest),
}

impl RetainedManagedJudgeEffectiveState {
    pub(super) fn binding_digest(&self) -> &Digest {
        match self {
            Self::Managed(state) => state.binding_digest(),
            #[cfg(test)]
            Self::Offline(digest) => digest,
        }
    }

    #[cfg_attr(
        not(test),
        expect(
            clippy::unnecessary_wraps,
            reason = "the test-only offline variant must fail the later real-state authority view"
        )
    )]
    pub(super) fn managed(&self) -> Option<&ManagedOllamaEffectiveRuntimeState> {
        match self {
            Self::Managed(state) => Some(state),
            #[cfg(test)]
            Self::Offline(_) => None,
        }
    }
}

impl ManagedJudgeAttemptObserverBinding {
    pub(crate) const fn receipt(&self) -> &OllamaResidentSessionExecutionReceipt {
        &self.receipt
    }

    pub(crate) fn effective_runtime_state(&self) -> Option<&ManagedOllamaEffectiveRuntimeState> {
        self.effective_state.managed()
    }

    /// Returns the exact schedule cursor.
    #[must_use]
    pub const fn schedule_cursor(&self) -> u32 {
        self.schedule_cursor
    }

    /// Returns the observer-derived process binding.
    #[must_use]
    pub const fn process_binding_digest(&self) -> &Digest {
        &self.process_binding_digest
    }

    /// Returns the observer-derived native-load binding.
    #[must_use]
    pub const fn native_load_binding_digest(&self) -> &Digest {
        &self.native_load_binding_digest
    }

    /// Returns the observer-derived complete connection-span binding.
    #[must_use]
    pub const fn connection_binding_digest(&self) -> &Digest {
        &self.connection_binding_digest
    }

    /// Returns the exact structured-request equality binding.
    #[must_use]
    pub fn request_binding_digest(&self) -> &Digest {
        self.receipt.execution().request_digest()
    }

    /// Returns the exact retained structured-response identity.
    #[must_use]
    pub fn retained_response_id(&self) -> OllamaRetainedSessionResponseId {
        self.receipt.execution().retained_response_id()
    }

    /// Returns the preflight binding shared by this retained session.
    #[must_use]
    pub fn retained_preflight_digest(&self) -> &Digest {
        self.receipt.execution().preflight_digest()
    }

    /// Returns the owner-derived binding over every retained receipt field.
    #[must_use]
    pub const fn complete_receipt_binding_digest(&self) -> &Digest {
        &self.receipt_binding_digest
    }

    /// Returns the first residency response ordinal inside this attempt span.
    #[must_use]
    pub const fn first_residency_ordinal(&self) -> usize {
        self.receipt.first_residency_ordinal()
    }

    /// Returns the confirming residency response ordinal inside this attempt span.
    #[must_use]
    pub const fn last_residency_ordinal(&self) -> usize {
        self.receipt.last_residency_ordinal()
    }

    /// Tests whether another inert receipt is the exact retained receipt.
    #[must_use]
    pub fn binds_receipt(&self, receipt: &OllamaResidentSessionExecutionReceipt) -> bool {
        self.receipt_binding_digest == receipt.complete_binding_digest()
    }

    /// Returns the exact effective-state live-join binding sealed to this attempt.
    #[must_use]
    pub fn effective_state_binding_digest(&self) -> &Digest {
        self.effective_state.binding_digest()
    }

    /// Returns the first retained response ordinal in this attempt.
    #[must_use]
    pub const fn first_response_ordinal(&self) -> u64 {
        self.first_response_ordinal
    }

    /// Returns the last retained response ordinal in this attempt.
    #[must_use]
    pub const fn last_response_ordinal(&self) -> u64 {
        self.last_response_ordinal
    }

    /// Tests whether this binding names the exact completed live-state join.
    #[must_use]
    pub fn binds_effective_runtime_state(
        &self,
        state: &ManagedOllamaEffectiveRuntimeState,
    ) -> bool {
        self.effective_state.binding_digest() == state.binding_digest()
    }
}

impl fmt::Debug for ManagedJudgeAttemptObserverBinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagedJudgeAttemptObserverBinding")
            .field("schedule_id", &self.schedule_id)
            .field("schedule_cursor", &self.schedule_cursor)
            .field("first_response_ordinal", &self.first_response_ordinal)
            .field("last_response_ordinal", &self.last_response_ordinal)
            .finish_non_exhaustive()
    }
}

/// Complete app-owned native-observation authority for every schedule entry.
///
/// This capability is noncloneable and nonserializable. It can be constructed only
/// by finishing one exact [`super::ManagedJudgeObservationSequence`].
///
/// ```compile_fail
/// use rewrite_app::effective_runtime_state_observation::CompletedManagedJudgeObservationSequence;
///
/// fn require_clone<T: Clone>() {}
/// require_clone::<CompletedManagedJudgeObservationSequence>();
/// ```
///
/// ```compile_fail
/// use rewrite_app::effective_runtime_state_observation::CompletedManagedJudgeObservationSequence;
///
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<CompletedManagedJudgeObservationSequence>();
/// ```
pub struct CompletedManagedJudgeObservationSequence {
    pub(super) schedule_id: CandidateJudgeScheduleId,
    pub(super) preflight: ManagedJudgePreflightObserverBinding,
    pub(super) bindings: Vec<ManagedJudgeAttemptObserverBinding>,
}

impl CompletedManagedJudgeObservationSequence {
    /// Returns the exact complete schedule identity.
    #[must_use]
    pub const fn schedule_id(&self) -> &CandidateJudgeScheduleId {
        &self.schedule_id
    }

    /// Returns the internally derived complete preflight connection binding.
    #[must_use]
    pub const fn preflight_connection_binding_digest(&self) -> &Digest {
        self.preflight.connection_binding_digest()
    }

    /// Returns the exact typed preflight evidence retained by this sequence.
    #[must_use]
    pub const fn preflight(&self) -> &ManagedJudgePreflightObserverBinding {
        &self.preflight
    }

    /// Returns sealed bindings in exact schedule order.
    #[must_use]
    pub fn bindings(&self) -> &[ManagedJudgeAttemptObserverBinding] {
        &self.bindings
    }
}

impl fmt::Debug for CompletedManagedJudgeObservationSequence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompletedManagedJudgeObservationSequence")
            .field("schedule_id", &self.schedule_id)
            .field("attempt_count", &self.bindings.len())
            .finish_non_exhaustive()
    }
}
