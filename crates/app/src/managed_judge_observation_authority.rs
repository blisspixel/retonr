//! App-owned schedule-wide managed judge observation authority.

use std::fmt;

use rewrite_model::{
    CandidateJudgePlanId, CandidateJudgePlanV1, CandidateJudgeScheduleId, CandidateJudgeScheduleV1,
    GenerationQualificationContractError, ManagedOllamaEffectiveRuntimeStateJoinId,
};
use rewrite_types::{CancellationToken, Digest};
use thiserror::Error;

use crate::effective_runtime_state_observation::CompletedManagedJudgeObservationSequence;
use crate::effective_runtime_state_observation::ManagedJudgePreflightObservation;

mod framing;
#[cfg(test)]
#[path = "managed_judge_observation_authority/test_support.rs"]
mod test_support;
mod validation;

/// Domain for the exact typed preflight evidence retained by this app authority.
pub const MANAGED_JUDGE_PREFLIGHT_OBSERVER_BINDING_DOMAIN: &[u8] =
    b"retonr:managed-local-judge-preflight-observer-binding:v1\0";

/// Derives the content-free provenance binding for one unsealed preflight token.
///
/// This helper lets eval bind its managed preflight report to the exact app token
/// before the token is consumed by the observer sequence. It grants no authority
/// and does not claim to encode the complete managed preflight report.
#[must_use]
pub fn derive_managed_judge_preflight_observer_binding_digest(
    schedule_id: &CandidateJudgeScheduleId,
    observation: &ManagedJudgePreflightObservation,
) -> Digest {
    framing::preflight_observer_binding(
        schedule_id,
        observation.initial_process().evidence_digest(),
        observation.post_preflight_process().evidence_digest(),
        observation.final_process().evidence_digest(),
        observation
            .native_load()
            .native_load_observation_id()
            .digest(),
        observation.connection_binding_digest(),
    )
}

/// Complete owned inputs for one schedule-wide observer join.
pub struct ManagedJudgeObservationAuthorityInput {
    /// Exact portable judge plan.
    pub judge_plan: CandidateJudgePlanV1,
    /// Exact deterministic schedule derived from the plan.
    pub judge_schedule: CandidateJudgeScheduleV1,
    /// Completed owning-observer authority for every schedule entry.
    pub completed_sequence: CompletedManagedJudgeObservationSequence,
}

/// Closed relationship rejected by the schedule-wide compiler.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagedJudgeObservationAuthorityRelationship {
    /// The supplied schedule was not the exact plan-derived schedule.
    PlanSchedule,
    /// The completed observer sequence named a different schedule.
    SequenceSchedule,
    /// The schedule and completed observer entry counts differed.
    AttemptCount,
    /// An observer binding was not at its exact schedule index.
    ScheduleCursor,
    /// Response or residency ordinals did not match the reviewed profile.
    ResponseSpan,
    /// The retained residency leaf was not its complete owner-derived receipt binding.
    ResidencyReceipt,
    /// Attempt receipts did not share one retained-session preflight binding.
    RetainedPreflight,
    /// A retained aggregate or schedule-wide join did not rederive exactly.
    DerivedAggregate,
}

/// Compiler for one exact schedule-wide managed judge observation authority.
#[derive(Clone, Copy, Debug, Default)]
pub struct ManagedJudgeObservationAuthorityCompiler;

impl ManagedJudgeObservationAuthorityCompiler {
    /// Validates and consumes one complete observer sequence.
    ///
    /// Leaf and aggregate digests are derived internally from the retained owning
    /// observer bindings. No caller-supplied digest participates in construction.
    ///
    /// # Errors
    ///
    /// Returns an error for cancellation, a noncanonical portable relationship,
    /// or any schedule, cursor, ordinal, count, or preflight substitution.
    pub fn compile(
        input: ManagedJudgeObservationAuthorityInput,
        cancellation: &CancellationToken,
    ) -> Result<VerifiedManagedJudgeObservationAuthority, ManagedJudgeObservationAuthorityError>
    {
        ensure_active(cancellation)?;
        validation::validate(&input, cancellation)?;
        ensure_active(cancellation)?;
        let aggregates = framing::derive(&input, cancellation)?;
        ensure_active(cancellation)?;
        Ok(VerifiedManagedJudgeObservationAuthority {
            judge_plan: input.judge_plan,
            judge_schedule: input.judge_schedule,
            completed_sequence: input.completed_sequence,
            attempt_count: aggregates.attempt_count,
            preflight_observer_binding_digest: aggregates.preflight_observer,
            retained_session_preflight_digest: aggregates.retained_preflight,
            residency_receipt_aggregate_digest: aggregates.residency,
            process_observation_aggregate_digest: aggregates.process,
            native_load_observation_aggregate_digest: aggregates.native_load,
            connection_observation_aggregate_digest: aggregates.connection,
            effective_runtime_state_observation_aggregate_digest: aggregates.effective_state,
            effective_runtime_state_join_id: aggregates.effective_state_join,
        })
    }
}

/// Noncloneable, nonserializable schedule-wide owning-observer authority.
///
/// The authority proves exact equality closure over the retained observer outputs.
/// It does not prove judge correctness, candidate qualification, or eval readiness.
///
/// ```compile_fail
/// use rewrite_app::VerifiedManagedJudgeObservationAuthority;
///
/// fn clone_authority(value: &VerifiedManagedJudgeObservationAuthority) {
///     let _forged: VerifiedManagedJudgeObservationAuthority = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_app::VerifiedManagedJudgeObservationAuthority;
///
/// fn serialize_authority(value: &VerifiedManagedJudgeObservationAuthority) {
///     let _bytes = serde_json::to_vec(value).expect("authority must not serialize");
/// }
/// ```
pub struct VerifiedManagedJudgeObservationAuthority {
    judge_plan: CandidateJudgePlanV1,
    judge_schedule: CandidateJudgeScheduleV1,
    completed_sequence: CompletedManagedJudgeObservationSequence,
    attempt_count: u64,
    preflight_observer_binding_digest: Digest,
    retained_session_preflight_digest: Digest,
    residency_receipt_aggregate_digest: Digest,
    process_observation_aggregate_digest: Digest,
    native_load_observation_aggregate_digest: Digest,
    connection_observation_aggregate_digest: Digest,
    effective_runtime_state_observation_aggregate_digest: Digest,
    effective_runtime_state_join_id: ManagedOllamaEffectiveRuntimeStateJoinId,
}

impl VerifiedManagedJudgeObservationAuthority {
    pub(crate) fn revalidate_retained_bindings(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), ManagedJudgeObservationAuthorityError> {
        validation::validate_retained(
            &self.judge_plan,
            &self.judge_schedule,
            &self.completed_sequence,
            cancellation,
        )?;
        let derived = framing::derive_retained(
            &self.judge_plan,
            &self.judge_schedule,
            &self.completed_sequence,
            cancellation,
        )?;
        if derived.attempt_count != self.attempt_count
            || derived.preflight_observer != self.preflight_observer_binding_digest
            || derived.retained_preflight != self.retained_session_preflight_digest
            || derived.residency != self.residency_receipt_aggregate_digest
            || derived.process != self.process_observation_aggregate_digest
            || derived.native_load != self.native_load_observation_aggregate_digest
            || derived.connection != self.connection_observation_aggregate_digest
            || derived.effective_state != self.effective_runtime_state_observation_aggregate_digest
            || derived.effective_state_join != self.effective_runtime_state_join_id
        {
            return Err(ManagedJudgeObservationAuthorityError::Relationship(
                ManagedJudgeObservationAuthorityRelationship::DerivedAggregate,
            ));
        }
        Ok(())
    }

    /// Returns the exact judge-plan identity.
    #[must_use]
    pub const fn judge_plan_id(&self) -> &CandidateJudgePlanId {
        self.judge_plan.candidate_judge_plan_id()
    }

    /// Returns the exact judge-schedule identity.
    #[must_use]
    pub const fn judge_schedule_id(&self) -> &CandidateJudgeScheduleId {
        self.judge_schedule.candidate_judge_schedule_id()
    }

    /// Returns the exact number of schedule entries bound by every aggregate.
    #[must_use]
    pub const fn attempt_count(&self) -> u64 {
        self.attempt_count
    }

    /// Returns the equality binding for the exact retained preflight observer token.
    ///
    /// This digest binds the token's process, native-load, and connection evidence.
    /// It is not the complete managed preflight report and does not replace the
    /// separately compiled portable `managed_preflight_digest`.
    #[must_use]
    pub const fn preflight_observer_binding_digest(&self) -> &Digest {
        &self.preflight_observer_binding_digest
    }

    /// Returns the owner-derived retained-session preflight response binding.
    #[must_use]
    pub const fn retained_session_preflight_digest(&self) -> &Digest {
        &self.retained_session_preflight_digest
    }

    /// Returns the ordered aggregate of complete resident-receipt bindings.
    #[must_use]
    pub const fn residency_receipt_aggregate_digest(&self) -> &Digest {
        &self.residency_receipt_aggregate_digest
    }

    /// Returns the ordered aggregate of server-process observation bindings.
    #[must_use]
    pub const fn process_observation_aggregate_digest(&self) -> &Digest {
        &self.process_observation_aggregate_digest
    }

    /// Returns the ordered aggregate of native-load observation bindings.
    #[must_use]
    pub const fn native_load_observation_aggregate_digest(&self) -> &Digest {
        &self.native_load_observation_aggregate_digest
    }

    /// Returns the ordered aggregate of connection-span observation bindings.
    #[must_use]
    pub const fn connection_observation_aggregate_digest(&self) -> &Digest {
        &self.connection_observation_aggregate_digest
    }

    /// Returns the ordered aggregate of effective-runtime-state bindings.
    #[must_use]
    pub const fn effective_runtime_state_observation_aggregate_digest(&self) -> &Digest {
        &self.effective_runtime_state_observation_aggregate_digest
    }

    /// Returns the dedicated schedule-wide effective-runtime-state join identity.
    #[must_use]
    pub const fn effective_runtime_state_join_id(
        &self,
    ) -> &ManagedOllamaEffectiveRuntimeStateJoinId {
        &self.effective_runtime_state_join_id
    }

    pub(crate) const fn judge_plan(&self) -> &CandidateJudgePlanV1 {
        &self.judge_plan
    }

    pub(crate) const fn judge_schedule(&self) -> &CandidateJudgeScheduleV1 {
        &self.judge_schedule
    }

    pub(crate) const fn completed_sequence(&self) -> &CompletedManagedJudgeObservationSequence {
        &self.completed_sequence
    }
}

impl fmt::Debug for VerifiedManagedJudgeObservationAuthority {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedManagedJudgeObservationAuthority")
            .field("judge_plan_id", self.judge_plan_id())
            .field("judge_schedule_id", self.judge_schedule_id())
            .field("attempt_count", &self.attempt_count)
            .finish_non_exhaustive()
    }
}

/// Failure to compile one schedule-wide managed judge observation authority.
#[derive(Error)]
pub enum ManagedJudgeObservationAuthorityError {
    /// Work was cancelled before authority release.
    #[error("managed judge observation authority compilation was cancelled")]
    Cancelled,
    /// A portable plan or schedule constructor rejected the supplied relationship.
    #[error("managed judge observation portable contract rejected the input")]
    PortableContract(#[source] GenerationQualificationContractError),
    /// One exact observer or schedule relationship was substituted.
    #[error("managed judge observation relationship is invalid: {0:?}")]
    Relationship(ManagedJudgeObservationAuthorityRelationship),
}

impl fmt::Debug for ManagedJudgeObservationAuthorityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut value = formatter.debug_struct("ManagedJudgeObservationAuthorityError");
        match self {
            Self::Cancelled => value.field("kind", &"cancelled"),
            Self::PortableContract(_) => value.field("kind", &"portable_contract"),
            Self::Relationship(relationship) => value
                .field("kind", &"relationship")
                .field("relationship", relationship),
        };
        value.finish_non_exhaustive()
    }
}

fn ensure_active(
    cancellation: &CancellationToken,
) -> Result<(), ManagedJudgeObservationAuthorityError> {
    if cancellation.is_cancelled() {
        Err(ManagedJudgeObservationAuthorityError::Cancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "managed_judge_observation_authority/tests.rs"]
mod tests;
