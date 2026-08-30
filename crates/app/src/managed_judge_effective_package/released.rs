use std::fmt;

use rewrite_model::{
    ArtifactSetManifest, CandidateJudgePlanId, CandidateJudgeRequestAggregateId,
    CandidateJudgeRequestAggregateV1, CandidateJudgeScheduleId, EffectivePackageEvidenceV2,
    EffectivePackageEvidenceV2Id, EffectiveRuntimeState, EffectiveRuntimeStateId,
    GenerationSystemId, GenerationSystemRecordV1, ManagedOllamaEffectiveRuntimeStateJoinId,
    RuntimeBuildIdentity,
};
use rewrite_types::{CancellationToken, Digest};

use crate::{
    ModelLicenseControlId, ModelPackageFoundationId, VerifiedManagedJudgeObservationAuthority,
    effective_runtime_state_observation::ManagedJudgeAttemptObserverBinding,
};

use super::ManagedJudgeEffectivePackageFinalValidationError;

/// Cleanup-gated batch-specific managed judge package evidence and authority.
///
/// This is intentionally distinct from `ReleasedGenerationEffectivePackageV2`.
/// It retains the complete ordered schedule authority and cannot be reconstructed
/// from the singular final attempt or serialized evidence.
///
/// ```compile_fail
/// use rewrite_app::ReleasedManagedJudgeEffectivePackageV2;
/// fn require_clone<T: Clone>() {}
/// require_clone::<ReleasedManagedJudgeEffectivePackageV2>();
/// ```
///
/// ```compile_fail
/// use rewrite_app::ReleasedManagedJudgeEffectivePackageV2;
/// fn serialize_release(value: &ReleasedManagedJudgeEffectivePackageV2) {
///     let _bytes = serde_json::to_vec(value).unwrap();
/// }
/// ```
pub struct ReleasedManagedJudgeEffectivePackageV2 {
    pub(super) evidence: EffectivePackageEvidenceV2,
    pub(super) artifact_set: ArtifactSetManifest,
    pub(super) runtime_build: RuntimeBuildIdentity,
    pub(super) observation_authority: VerifiedManagedJudgeObservationAuthority,
    pub(super) request_aggregate: CandidateJudgeRequestAggregateV1,
    pub(super) judge_system: GenerationSystemRecordV1,
    pub(super) expected_runtime_state: EffectiveRuntimeState,
    pub(super) runtime_installation_generation: u64,
    pub(super) model_installation_generation: u64,
    pub(super) foundation_id: ModelPackageFoundationId,
    pub(super) license_control_id: ModelLicenseControlId,
    pub(super) first_response_ordinal: u64,
    pub(super) last_response_ordinal: u64,
}

impl ReleasedManagedJudgeEffectivePackageV2 {
    /// Revalidates the retained observation authority and inert package evidence.
    ///
    /// This narrow readback seam preserves the app-owned half of a later durable
    /// receipt bracket. It grants no launch, execution, or qualification authority.
    ///
    /// # Errors
    ///
    /// Returns an error for cancellation, retained observer drift, evidence drift,
    /// or a substituted portable lineage or response span.
    #[doc(hidden)]
    pub fn revalidate_retained_authority(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), ManagedJudgeEffectivePackageFinalValidationError> {
        self.observation_authority
            .revalidate_retained_bindings(cancellation)
            .map_err(ManagedJudgeEffectivePackageFinalValidationError::Authority)?;
        self.evidence
            .validate_against(
                &self.artifact_set,
                &self.runtime_build,
                &self.expected_runtime_state,
            )
            .map_err(ManagedJudgeEffectivePackageFinalValidationError::Evidence)?;
        let expected_requests = CandidateJudgeRequestAggregateV1::new(
            self.observation_authority.judge_plan(),
            self.observation_authority.judge_schedule(),
            self.request_aggregate
                .structured_request_binding_ids()
                .to_vec(),
        )
        .map_err(|_error| ManagedJudgeEffectivePackageFinalValidationError::Relationship)?;
        let bindings = self.observation_authority.completed_sequence().bindings();
        if expected_requests != self.request_aggregate
            || self.request_aggregate.candidate_judge_plan_id() != self.judge_plan_id()
            || self.request_aggregate.candidate_judge_schedule_id() != self.judge_schedule_id()
            || self
                .observation_authority
                .judge_plan()
                .judge_generation_system_id()
                != self.judge_system.generation_system_id()
            || self
                .observation_authority
                .judge_plan()
                .prompt_contract_digest()
                != self.judge_system.prompt_digest()
            || self
                .observation_authority
                .judge_plan()
                .output_schema_digest()
                != self.judge_system.output_schema_digest()
            || self.judge_system.runtime_build_id() != &self.runtime_build.runtime_build_id()
            || self.judge_system.effective_runtime_state_id()
                != &self.expected_runtime_state.effective_runtime_state_id()
            || self.judge_system.model_artifact_set_id() != self.evidence.artifact_set_id()
            || self.judge_system.effective_package_evidence_v2_id()
                != &self.evidence.effective_package_evidence_v2_id()
            || bindings
                .first()
                .map(ManagedJudgeAttemptObserverBinding::first_response_ordinal)
                != Some(self.first_response_ordinal)
            || bindings
                .last()
                .map(ManagedJudgeAttemptObserverBinding::last_response_ordinal)
                != Some(self.last_response_ordinal)
            || self.runtime_installation_generation == 0
            || self.model_installation_generation == 0
        {
            return Err(ManagedJudgeEffectivePackageFinalValidationError::Relationship);
        }
        if cancellation.is_cancelled() {
            return Err(ManagedJudgeEffectivePackageFinalValidationError::Authority(
                crate::ManagedJudgeObservationAuthorityError::Cancelled,
            ));
        }
        Ok(())
    }

    /// Returns the cleanup-gated inert V2 package evidence.
    #[must_use]
    pub const fn evidence(&self) -> &EffectivePackageEvidenceV2 {
        &self.evidence
    }
    /// Returns the exact package evidence identity.
    #[must_use]
    pub fn effective_package_evidence_v2_id(&self) -> EffectivePackageEvidenceV2Id {
        self.evidence.effective_package_evidence_v2_id()
    }
    /// Returns the exact judge-plan identity.
    #[must_use]
    pub const fn judge_plan_id(&self) -> &CandidateJudgePlanId {
        self.observation_authority.judge_plan_id()
    }
    /// Returns the exact judge-schedule identity.
    #[must_use]
    pub const fn judge_schedule_id(&self) -> &CandidateJudgeScheduleId {
        self.observation_authority.judge_schedule_id()
    }
    /// Returns the exact request aggregate identity.
    #[must_use]
    pub const fn request_aggregate_id(&self) -> &CandidateJudgeRequestAggregateId {
        self.request_aggregate.request_aggregate_id()
    }
    /// Returns the exact judge generation-system identity.
    #[must_use]
    pub const fn judge_generation_system_id(&self) -> &GenerationSystemId {
        self.judge_system.generation_system_id()
    }
    /// Returns the exact inert judge generation-system record.
    #[must_use]
    pub const fn judge_system(&self) -> &GenerationSystemRecordV1 {
        &self.judge_system
    }
    /// Returns the common exact effective-runtime-state identity.
    #[must_use]
    pub fn effective_runtime_state_id(&self) -> EffectiveRuntimeStateId {
        self.expected_runtime_state.effective_runtime_state_id()
    }
    /// Returns the exact number of retained schedule entries.
    #[must_use]
    pub const fn attempt_count(&self) -> u64 {
        self.observation_authority.attempt_count()
    }
    /// Returns the first response ordinal derived from the first retained receipt.
    #[must_use]
    pub const fn first_response_ordinal(&self) -> u64 {
        self.first_response_ordinal
    }
    /// Returns the last response ordinal derived from the final retained receipt.
    #[must_use]
    pub const fn last_response_ordinal(&self) -> u64 {
        self.last_response_ordinal
    }
    /// Returns the precursor-captured runtime installation generation.
    #[must_use]
    pub const fn runtime_installation_generation(&self) -> u64 {
        self.runtime_installation_generation
    }
    /// Returns the precursor-captured model installation generation.
    #[must_use]
    pub const fn model_installation_generation(&self) -> u64 {
        self.model_installation_generation
    }
    /// Returns the exact model foundation identity.
    #[must_use]
    pub const fn foundation_id(&self) -> &ModelPackageFoundationId {
        &self.foundation_id
    }
    /// Returns the exact consumed license-control identity.
    #[must_use]
    pub const fn license_control_id(&self) -> &ModelLicenseControlId {
        &self.license_control_id
    }
    /// Returns the app-owned retained preflight observer provenance binding.
    #[must_use]
    pub const fn preflight_observer_binding_digest(&self) -> &Digest {
        self.observation_authority
            .preflight_observer_binding_digest()
    }
    /// Returns the preflight response binding derived from retained owner receipts.
    #[must_use]
    pub const fn retained_session_preflight_digest(&self) -> &Digest {
        self.observation_authority
            .retained_session_preflight_digest()
    }
    /// Returns the schedule-order complete residency-receipt aggregate.
    #[must_use]
    pub const fn residency_receipt_aggregate_digest(&self) -> &Digest {
        self.observation_authority
            .residency_receipt_aggregate_digest()
    }
    /// Returns the schedule-order process-observation aggregate.
    #[must_use]
    pub const fn process_observation_aggregate_digest(&self) -> &Digest {
        self.observation_authority
            .process_observation_aggregate_digest()
    }
    /// Returns the schedule-order native-load observation aggregate.
    #[must_use]
    pub const fn native_load_observation_aggregate_digest(&self) -> &Digest {
        self.observation_authority
            .native_load_observation_aggregate_digest()
    }
    /// Returns the schedule-order connection observation aggregate.
    #[must_use]
    pub const fn connection_observation_aggregate_digest(&self) -> &Digest {
        self.observation_authority
            .connection_observation_aggregate_digest()
    }
    /// Returns the schedule-order effective-state observation aggregate.
    #[must_use]
    pub const fn effective_runtime_state_observation_aggregate_digest(&self) -> &Digest {
        self.observation_authority
            .effective_runtime_state_observation_aggregate_digest()
    }
    /// Returns the dedicated schedule-wide effective-state join identity.
    #[must_use]
    pub const fn effective_runtime_state_join_id(
        &self,
    ) -> &ManagedOllamaEffectiveRuntimeStateJoinId {
        self.observation_authority.effective_runtime_state_join_id()
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the following app-owned receipt compiler consumes this exact authority view"
        )
    )]
    pub(crate) fn into_receipt_parts(self) -> ReleasedManagedJudgeEffectivePackageParts {
        ReleasedManagedJudgeEffectivePackageParts {
            evidence: self.evidence,
            artifact_set: self.artifact_set,
            runtime_build: self.runtime_build,
            observation_authority: self.observation_authority,
            request_aggregate: self.request_aggregate,
            judge_system: self.judge_system,
            expected_runtime_state: self.expected_runtime_state,
            runtime_installation_generation: self.runtime_installation_generation,
            model_installation_generation: self.model_installation_generation,
            foundation_id: self.foundation_id,
            license_control_id: self.license_control_id,
            first_response_ordinal: self.first_response_ordinal,
            last_response_ordinal: self.last_response_ordinal,
        }
    }
}

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the following app-owned receipt compiler consumes this exact authority view"
    )
)]
pub(crate) struct ReleasedManagedJudgeEffectivePackageParts {
    pub(crate) evidence: EffectivePackageEvidenceV2,
    pub(crate) artifact_set: ArtifactSetManifest,
    pub(crate) runtime_build: RuntimeBuildIdentity,
    pub(crate) observation_authority: VerifiedManagedJudgeObservationAuthority,
    pub(crate) request_aggregate: CandidateJudgeRequestAggregateV1,
    pub(crate) judge_system: GenerationSystemRecordV1,
    pub(crate) expected_runtime_state: EffectiveRuntimeState,
    pub(crate) runtime_installation_generation: u64,
    pub(crate) model_installation_generation: u64,
    pub(crate) foundation_id: ModelPackageFoundationId,
    pub(crate) license_control_id: ModelLicenseControlId,
    pub(crate) first_response_ordinal: u64,
    pub(crate) last_response_ordinal: u64,
}

impl fmt::Debug for ReleasedManagedJudgeEffectivePackageV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReleasedManagedJudgeEffectivePackageV2")
            .field("judge_plan_id", self.judge_plan_id())
            .field("judge_schedule_id", self.judge_schedule_id())
            .field("attempt_count", &self.attempt_count())
            .field(
                "effective_package_id",
                &self.effective_package_evidence_v2_id(),
            )
            .finish_non_exhaustive()
    }
}
