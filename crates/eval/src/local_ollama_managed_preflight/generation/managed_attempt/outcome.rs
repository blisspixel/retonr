use std::fmt;

use rewrite_app::{
    ReleasedGenerationEffectivePackageV2, VerifiedGenerationQualificationResourceAttemptObservation,
};
use rewrite_inference::{
    GenerationCandidate, GenerationRequest, StructuredCompletionRequest,
    StructuredCompletionResponse,
};
use rewrite_model::{
    CandidateGenerationAttemptCleanupDispositionV1, CandidateGenerationAttemptFailureCategoryV1,
    CandidateGenerationAttemptFailurePhaseV1, CandidateGenerationAttemptPrecursorV1,
    CandidateGenerationAttemptRecordV1, CandidateGenerationCleanupRecordV1,
    CandidateGenerationCleanupRecordV1Input, CandidateGenerationPackageRevalidationStatusV1,
    CandidateGenerationProcessCleanupStatusV1, EffectiveRuntimeState,
    GenerationQualificationContractError, GenerationSystemRecordV1,
    ManagedOllamaCandidateGenerationEvidenceV2, ManagedOllamaCandidateGenerationEvidenceV2Input,
    ManagedOllamaCandidateGenerationEvidenceV2Relations, ManagedOllamaEffectiveRuntimeStateJoinId,
    PlannedCandidateAttemptV1,
};
use rewrite_ollama::OllamaResidentSessionExecutionReceipt;
use rewrite_runtime_attestor::{
    ManagedGenerationWorkerEvidence, ManagedGenerationWorkerResourceObservation,
};

use crate::{
    LocalOllamaManagedBuildBinding, LocalOllamaManagedGenerationEvidence,
    LocalOllamaManagedPreflightReport, ManagedOllamaGenerationBracketObservationV1,
    active_generation_qualification_subject::{
        ActiveGenerationQualificationBinding, ActiveGenerationQualificationSubject,
    },
};

use rewrite_app::effective_runtime_state_observation::ManagedOllamaEffectiveRuntimeState;

use super::super::{
    PendingLocalOllamaManagedGenerationOutcome, PendingManagedGenerationCompletion,
};
use super::error::{
    ManagedCandidateAttemptCleanupFailures, ManagedCandidateAttemptFailureFacts,
    ManagedCandidateAttemptFailureRecordError, ManagedCandidateAttemptPrimaryFailure,
    RetainedBracketCleanupFailures, cleanup_failures,
};

pub(super) struct PendingAttemptRecords {
    completion: Option<PendingManagedGenerationCompletion>,
    managed_preflight: LocalOllamaManagedPreflightReport,
    managed_build: LocalOllamaManagedBuildBinding,
    legacy_evidence: LocalOllamaManagedGenerationEvidence,
    bracket_observation: ManagedOllamaGenerationBracketObservationV1,
}

pub(super) fn split_pending_attempt(
    pending: PendingLocalOllamaManagedGenerationOutcome,
) -> (ManagedOllamaEffectiveRuntimeState, PendingAttemptRecords) {
    (
        pending.effective_runtime_state,
        PendingAttemptRecords {
            completion: Some(pending.completion),
            managed_preflight: pending.managed_preflight,
            managed_build: pending.managed_build,
            legacy_evidence: pending.evidence,
            bracket_observation: pending.bracket_observation,
        },
    )
}

impl PendingAttemptRecords {
    pub(super) fn take_completion(&mut self) -> Option<PendingManagedGenerationCompletion> {
        self.completion.take()
    }
}

pub(crate) struct ResourceObservedManagedCandidateAttemptClosure {
    pub(crate) observation: VerifiedGenerationQualificationResourceAttemptObservation,
    pub(crate) worker_observation: ManagedGenerationWorkerResourceObservation,
    pub(crate) worker_evidence: ManagedGenerationWorkerEvidence,
}

pub(super) enum VerifiedManagedCandidateCompletion {
    Compatibility {
        response: Box<StructuredCompletionResponse>,
        residency_receipt: Box<OllamaResidentSessionExecutionReceipt>,
    },
    ResourceObserved(Box<ResourceObservedManagedCandidateAttemptClosure>),
}

impl VerifiedManagedCandidateCompletion {
    const fn response(&self) -> &StructuredCompletionResponse {
        match self {
            Self::Compatibility { response, .. } => response,
            Self::ResourceObserved(resource) => resource.observation.response(),
        }
    }

    const fn residency_receipt(&self) -> &OllamaResidentSessionExecutionReceipt {
        match self {
            Self::Compatibility {
                residency_receipt, ..
            } => residency_receipt,
            Self::ResourceObserved(resource) => resource.observation.resident_execution_receipt(),
        }
    }
}

/// Completed authority or an exact portable failed-attempt closure.
pub enum ManagedCandidateAttemptExecutionOutcome {
    /// Cleanup-gated successful managed candidate authority.
    Completed(Box<VerifiedCompletedManagedCandidateAttempt>),
    /// Non-success closure retaining exact diagnostic and cause accounting.
    Failed(Box<FailedManagedCandidateAttempt>),
}

impl ManagedCandidateAttemptExecutionOutcome {
    pub(crate) fn bind_active_subject(
        mut self,
        binding: ActiveGenerationQualificationBinding,
    ) -> Self {
        match &mut self {
            Self::Completed(completed) => completed.active_binding = Some(binding),
            Self::Failed(failed) => failed.active_binding = Some(binding),
        }
        self
    }

    pub(crate) fn matches_active_subject(
        &self,
        subject: &ActiveGenerationQualificationSubject,
    ) -> bool {
        match self {
            Self::Completed(completed) => completed.matches_active_subject(subject),
            Self::Failed(failed) => failed.matches_active_subject(subject),
        }
    }

    /// Returns completed authority when the attempt succeeded.
    #[must_use]
    pub fn completed(&self) -> Option<&VerifiedCompletedManagedCandidateAttempt> {
        match self {
            Self::Completed(completed) => Some(completed.as_ref()),
            Self::Failed(_) => None,
        }
    }

    /// Returns the exact non-success closure when the attempt failed.
    #[must_use]
    pub fn failure(&self) -> Option<&FailedManagedCandidateAttempt> {
        match self {
            Self::Completed(_) => None,
            Self::Failed(failed) => Some(failed.as_ref()),
        }
    }

    /// Consumes the outcome and returns completed authority or the exact failure.
    ///
    /// # Errors
    ///
    /// Returns the boxed exact failed-attempt closure for a non-success outcome.
    pub fn into_completed(
        self,
    ) -> Result<VerifiedCompletedManagedCandidateAttempt, Box<FailedManagedCandidateAttempt>> {
        match self {
            Self::Completed(completed) => Ok(*completed),
            Self::Failed(failed) => Err(failed),
        }
    }
}

impl fmt::Debug for ManagedCandidateAttemptExecutionOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Completed(completed) => formatter
                .debug_tuple("ManagedCandidateAttemptExecutionOutcome::Completed")
                .field(completed.as_ref())
                .finish(),
            Self::Failed(failed) => formatter
                .debug_tuple("ManagedCandidateAttemptExecutionOutcome::Failed")
                .field(failed.as_ref())
                .finish(),
        }
    }
}

/// Exact non-success closure for one consumed managed candidate precursor.
///
/// The portable attempt record is cloneable and serializable. The retained causes
/// remain typed, bounded, redacted, and nonserializable.
pub struct FailedManagedCandidateAttempt {
    active_binding: Option<ActiveGenerationQualificationBinding>,
    precursor: CandidateGenerationAttemptPrecursorV1,
    attempt_record: CandidateGenerationAttemptRecordV1,
    primary: ManagedCandidateAttemptPrimaryFailure,
    retained_cleanup: Option<RetainedBracketCleanupFailures>,
    facts: ManagedCandidateAttemptFailureFacts,
}

impl FailedManagedCandidateAttempt {
    pub(super) fn new(
        precursor: CandidateGenerationAttemptPrecursorV1,
        attempt_record: CandidateGenerationAttemptRecordV1,
        primary: ManagedCandidateAttemptPrimaryFailure,
        retained_cleanup: Option<RetainedBracketCleanupFailures>,
        facts: ManagedCandidateAttemptFailureFacts,
    ) -> Self {
        Self {
            active_binding: None,
            precursor,
            attempt_record,
            primary,
            retained_cleanup,
            facts,
        }
    }

    /// Returns the exact prelaunch attempt precursor consumed by this failure.
    #[must_use]
    pub const fn precursor(&self) -> &CandidateGenerationAttemptPrecursorV1 {
        &self.precursor
    }

    /// Returns the portable failed-attempt record.
    #[must_use]
    pub const fn attempt_record(&self) -> &CandidateGenerationAttemptRecordV1 {
        &self.attempt_record
    }

    /// Returns the original primary failure.
    #[must_use]
    pub const fn primary_failure(&self) -> &ManagedCandidateAttemptPrimaryFailure {
        &self.primary
    }

    /// Returns every independent cleanup and final revalidation failure.
    #[must_use]
    pub fn cleanup_failures(&self) -> Option<ManagedCandidateAttemptCleanupFailures<'_>> {
        cleanup_failures(&self.primary, self.retained_cleanup.as_ref())
    }

    /// Returns the exact primary failure phase encoded in the portable record.
    #[must_use]
    pub const fn failure_phase(&self) -> CandidateGenerationAttemptFailurePhaseV1 {
        self.facts.failure_phase
    }

    /// Returns the exact primary failure category encoded in the portable record.
    #[must_use]
    pub const fn failure_category(&self) -> CandidateGenerationAttemptFailureCategoryV1 {
        self.facts.failure_category
    }

    /// Returns whether candidate-generation traffic was directly observed.
    #[must_use]
    pub const fn traffic_observed(&self) -> bool {
        self.facts.traffic_observed
    }

    /// Returns whether generated candidate output was directly observed.
    #[must_use]
    pub const fn output_observed(&self) -> bool {
        self.facts.output_observed
    }

    /// Returns the exact final cleanup disposition encoded in the portable record.
    #[must_use]
    pub const fn cleanup_disposition(&self) -> CandidateGenerationAttemptCleanupDispositionV1 {
        self.facts.cleanup_disposition
    }

    pub(crate) fn matches_active_subject(
        &self,
        subject: &ActiveGenerationQualificationSubject,
    ) -> bool {
        self.active_binding
            .as_ref()
            .is_some_and(|binding| subject.accepts(binding))
    }
}

impl fmt::Debug for FailedManagedCandidateAttempt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FailedManagedCandidateAttempt")
            .field("attempt_record_id", self.attempt_record.attempt_record_id())
            .field("failure_phase", &self.facts.failure_phase)
            .field("failure_category", &self.facts.failure_category)
            .field("traffic_observed", &self.facts.traffic_observed)
            .field("output_observed", &self.facts.output_observed)
            .field("cleanup_disposition", &self.facts.cleanup_disposition)
            .finish_non_exhaustive()
    }
}

pub(super) fn failed_managed_attempt(
    planned_attempt: &PlannedCandidateAttemptV1,
    precursor: &CandidateGenerationAttemptPrecursorV1,
    primary: ManagedCandidateAttemptPrimaryFailure,
    retained_cleanup: Option<RetainedBracketCleanupFailures>,
    facts: ManagedCandidateAttemptFailureFacts,
) -> Result<ManagedCandidateAttemptExecutionOutcome, Box<ManagedCandidateAttemptFailureRecordError>>
{
    let attempt_record = match CandidateGenerationAttemptRecordV1::failed(
        planned_attempt,
        Some(precursor),
        facts.input(),
    ) {
        Ok(record) => record,
        Err(source) => {
            return Err(Box::new(ManagedCandidateAttemptFailureRecordError::new(
                source,
                primary,
                retained_cleanup,
            )));
        }
    };
    Ok(ManagedCandidateAttemptExecutionOutcome::Failed(Box::new(
        FailedManagedCandidateAttempt::new(
            precursor.clone(),
            attempt_record,
            primary,
            retained_cleanup,
            facts,
        ),
    )))
}

/// Completed nonforgeable managed attempt released after mandatory cleanup.
///
/// This capability is deliberately noncloneable and nonserializable. Its response
/// and ordered candidates remain untrusted content. They grant no authority over
/// the retained runtime, effective package, or later qualification decision.
///
/// ```compile_fail
/// use rewrite_eval::VerifiedCompletedManagedCandidateAttempt;
///
/// fn clone_capability(value: &VerifiedCompletedManagedCandidateAttempt) {
///     let _forged: VerifiedCompletedManagedCandidateAttempt = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_eval::VerifiedCompletedManagedCandidateAttempt;
///
/// fn serialize_capability(value: &VerifiedCompletedManagedCandidateAttempt) {
///     let _bytes = serde_json::to_vec(value).expect("capability must not serialize");
/// }
/// ```
pub struct VerifiedCompletedManagedCandidateAttempt {
    active_binding: Option<ActiveGenerationQualificationBinding>,
    completion: VerifiedManagedCandidateCompletion,
    candidates: Vec<GenerationCandidate>,
    managed_preflight: LocalOllamaManagedPreflightReport,
    managed_build: LocalOllamaManagedBuildBinding,
    legacy_evidence: LocalOllamaManagedGenerationEvidence,
    bracket_observation: ManagedOllamaGenerationBracketObservationV1,
    effective_runtime_state: EffectiveRuntimeState,
    effective_package: ReleasedGenerationEffectivePackageV2,
    planned_attempt: PlannedCandidateAttemptV1,
    generation_system: GenerationSystemRecordV1,
    generation_request: GenerationRequest,
    structured_request: StructuredCompletionRequest,
    precursor: CandidateGenerationAttemptPrecursorV1,
    managed_evidence: ManagedOllamaCandidateGenerationEvidenceV2,
    cleanup: CandidateGenerationCleanupRecordV1,
}

impl VerifiedCompletedManagedCandidateAttempt {
    pub(crate) fn active_binding(&self) -> Option<&ActiveGenerationQualificationBinding> {
        self.active_binding.as_ref()
    }

    pub(crate) fn matches_active_subject(
        &self,
        subject: &ActiveGenerationQualificationSubject,
    ) -> bool {
        self.active_binding
            .as_ref()
            .is_some_and(|binding| subject.accepts(binding))
    }

    /// Returns the bounded but untrusted structured response.
    #[must_use]
    pub const fn response(&self) -> &StructuredCompletionResponse {
        self.completion.response()
    }

    /// Returns ordered candidates parsed internally under the planned ceilings.
    #[must_use]
    pub fn candidates(&self) -> &[GenerationCandidate] {
        &self.candidates
    }

    /// Returns the exact retained-session execution and residency receipt.
    #[must_use]
    pub const fn residency_receipt(&self) -> &OllamaResidentSessionExecutionReceipt {
        self.completion.residency_receipt()
    }

    pub(crate) const fn resource_closure(
        &self,
    ) -> Option<&ResourceObservedManagedCandidateAttemptClosure> {
        match &self.completion {
            VerifiedManagedCandidateCompletion::Compatibility { .. } => None,
            VerifiedManagedCandidateCompletion::ResourceObserved(resource) => Some(resource),
        }
    }

    /// Returns the exact managed preflight retained by the live bracket.
    #[must_use]
    pub const fn managed_preflight(&self) -> &LocalOllamaManagedPreflightReport {
        &self.managed_preflight
    }

    /// Returns the exact runtime-build binding retained by the live bracket.
    #[must_use]
    pub const fn managed_build(&self) -> &LocalOllamaManagedBuildBinding {
        &self.managed_build
    }

    /// Returns the compatibility-preserved managed generation evidence V1.
    #[must_use]
    pub const fn legacy_evidence(&self) -> &LocalOllamaManagedGenerationEvidence {
        &self.legacy_evidence
    }

    /// Returns the exact retained-bracket observation V1.
    #[must_use]
    pub const fn bracket_observation(&self) -> &ManagedOllamaGenerationBracketObservationV1 {
        &self.bracket_observation
    }

    /// Returns the complete observed portable effective runtime state.
    #[must_use]
    pub const fn effective_runtime_state(&self) -> &EffectiveRuntimeState {
        &self.effective_runtime_state
    }

    /// Returns cleanup-gated effective-package V2 evidence and model authorities.
    #[must_use]
    pub const fn effective_package(&self) -> &ReleasedGenerationEffectivePackageV2 {
        &self.effective_package
    }

    /// Returns the exact selected planned attempt.
    #[must_use]
    pub const fn planned_attempt(&self) -> &PlannedCandidateAttemptV1 {
        &self.planned_attempt
    }

    /// Returns the exact stable generation-system record.
    #[must_use]
    pub const fn generation_system(&self) -> &GenerationSystemRecordV1 {
        &self.generation_system
    }

    /// Returns the exact provider-neutral request retained by the precursor.
    #[must_use]
    pub const fn generation_request(&self) -> &GenerationRequest {
        &self.generation_request
    }

    /// Returns the internally derived exact structured request.
    #[must_use]
    pub const fn structured_request(&self) -> &StructuredCompletionRequest {
        &self.structured_request
    }

    /// Returns the exact prelaunch attempt precursor.
    #[must_use]
    pub const fn precursor(&self) -> &CandidateGenerationAttemptPrecursorV1 {
        &self.precursor
    }

    /// Returns managed candidate-generation evidence V2.
    #[must_use]
    pub const fn managed_evidence(&self) -> &ManagedOllamaCandidateGenerationEvidenceV2 {
        &self.managed_evidence
    }

    /// Returns the all-pass cleanup and final package-revalidation record.
    #[must_use]
    pub const fn cleanup(&self) -> &CandidateGenerationCleanupRecordV1 {
        &self.cleanup
    }
}

impl fmt::Debug for VerifiedCompletedManagedCandidateAttempt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedCompletedManagedCandidateAttempt")
            .field("precursor_id", self.precursor.precursor_id())
            .field(
                "managed_evidence_id",
                self.managed_evidence.managed_evidence_v2_id(),
            )
            .field("cleanup_id", self.cleanup.cleanup_id())
            .field("candidate_count", &self.candidates.len())
            .finish_non_exhaustive()
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "each exact inert record remains visible at the final authority join"
)]
pub(super) fn complete_managed_attempt(
    pending: PendingAttemptRecords,
    completion: VerifiedManagedCandidateCompletion,
    effective_runtime_state: EffectiveRuntimeState,
    effective_state_join_id: ManagedOllamaEffectiveRuntimeStateJoinId,
    effective_package: ReleasedGenerationEffectivePackageV2,
    planned_attempt: PlannedCandidateAttemptV1,
    generation_system: GenerationSystemRecordV1,
    generation_request: GenerationRequest,
    structured_request: StructuredCompletionRequest,
    precursor: CandidateGenerationAttemptPrecursorV1,
    candidates: Vec<GenerationCandidate>,
) -> Result<
    VerifiedCompletedManagedCandidateAttempt,
    Box<(
        GenerationQualificationContractError,
        PlannedCandidateAttemptV1,
        CandidateGenerationAttemptPrecursorV1,
    )>,
> {
    let bracket_observation_v1_id = pending.bracket_observation.bracket_observation_v1_id();
    let response_id = completion
        .residency_receipt()
        .execution()
        .retained_response_id();
    let managed_evidence = match ManagedOllamaCandidateGenerationEvidenceV2::new(
        ManagedOllamaCandidateGenerationEvidenceV2Relations {
            precursor: &precursor,
            planned_attempt: &planned_attempt,
            generation_system: &generation_system,
            effective_package_evidence_v2: effective_package.evidence(),
        },
        ManagedOllamaCandidateGenerationEvidenceV2Input {
            bracket_observation_v1_id,
            effective_runtime_state_join_id: effective_state_join_id,
            response_id,
        },
    ) {
        Ok(evidence) => evidence,
        Err(error) => return Err(Box::new((error, planned_attempt, precursor))),
    };
    let cleanup = match CandidateGenerationCleanupRecordV1::new(
        &precursor,
        &managed_evidence,
        CandidateGenerationCleanupRecordV1Input {
            process_cleanup_status: CandidateGenerationProcessCleanupStatusV1::Succeeded,
            runtime_package_revalidation_status:
                CandidateGenerationPackageRevalidationStatusV1::Verified,
            model_package_revalidation_status:
                CandidateGenerationPackageRevalidationStatusV1::Verified,
        },
    ) {
        Ok(cleanup) => cleanup,
        Err(error) => return Err(Box::new((error, planned_attempt, precursor))),
    };

    Ok(VerifiedCompletedManagedCandidateAttempt {
        active_binding: None,
        completion,
        candidates,
        managed_preflight: pending.managed_preflight,
        managed_build: pending.managed_build,
        legacy_evidence: pending.legacy_evidence,
        bracket_observation: pending.bracket_observation,
        effective_runtime_state,
        effective_package,
        planned_attempt,
        generation_system,
        generation_request,
        structured_request,
        precursor,
        managed_evidence,
        cleanup,
    })
}
