//! App-owned measurement authority for one managed qualification attempt.

use std::{fmt, time::Instant};

use rewrite_inference::StructuredCompletionResponse;
use rewrite_model::{
    EffectivePackageEvidenceV2Id, GenerationQualificationOperationPolicyId,
    GenerationQualificationOperationPolicyV1, GenerationQualificationPlanId,
    GenerationResourceExceededLimitV1, GenerationResourceObservationProfileV1,
    GenerationSuiteManifestId, GenerationSystemId, ModelPackageManifestId,
    OllamaRetainedSessionResponseId, RuntimePackageManifestId,
};
use rewrite_ollama::{
    OllamaResidentResourceObservedCompletion, OllamaResidentSessionExecutionReceipt,
    OllamaRetainedSessionSubjectToken,
};
use rewrite_runtime_attestor::{
    ManagedGenerationWorkerEvidence, ManagedGenerationWorkerResourceObservation,
};
use rewrite_types::{CancellationToken, Digest};
use thiserror::Error;

use crate::{
    ReleasedGenerationEffectivePackageV2, RuntimePackageLease,
    VerifiedGenerationQualificationResourcePolicy, VerifiedManagedOllamaModelPackageLease,
};

use super::GenerationQualificationResourcePolicyLimitsV1;

mod validation;

/// Caller-independent monotonic start authority for one resource-observed attempt.
///
/// Construction requires an exact source-approved resource policy and its bound
/// operation policy. The value grants no launch, traffic, persistence, phase,
/// or qualification authority. It cannot be cloned or serialized.
///
/// This app-owned clock controls the monotonic value, but does not prove where
/// its caller placed `start` relative to earlier attempt work. The production
/// runner owns and enforces that sequencing guarantee.
///
/// ```compile_fail
/// use rewrite_app::GenerationQualificationResourceAttemptClock;
///
/// fn clone_clock(value: &GenerationQualificationResourceAttemptClock) {
///     let _copy: GenerationQualificationResourceAttemptClock = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_app::GenerationQualificationResourceAttemptClock;
///
/// fn serialize_clock(value: &GenerationQualificationResourceAttemptClock) {
///     let _json = serde_json::to_string(value).unwrap();
/// }
/// ```
pub struct GenerationQualificationResourceAttemptClock {
    started: Instant,
    policy_digest: Digest,
    operation_policy_id: GenerationQualificationOperationPolicyId,
    qualification_plan_id: GenerationQualificationPlanId,
    suite_manifest_id: GenerationSuiteManifestId,
    target_generation_system_id: GenerationSystemId,
    limits: GenerationQualificationResourcePolicyLimitsV1,
}

impl GenerationQualificationResourceAttemptClock {
    /// Starts one app-owned monotonic measurement bracket.
    ///
    /// # Errors
    ///
    /// Returns a content-free error for cancellation, source denial, or an
    /// operation-policy substitution.
    pub fn start(
        policy: &VerifiedGenerationQualificationResourcePolicy,
        operation_policy: &GenerationQualificationOperationPolicyV1,
        cancellation: &CancellationToken,
    ) -> Result<Self, GenerationQualificationResourceAttemptObservationError> {
        validation::validate_policy(policy, operation_policy, cancellation)?;
        Ok(Self {
            started: Instant::now(),
            policy_digest: policy.policy_digest().clone(),
            operation_policy_id: operation_policy.operation_policy_id().clone(),
            qualification_plan_id: operation_policy.generation_qualification_plan_id().clone(),
            suite_manifest_id: operation_policy.suite_manifest_id().clone(),
            target_generation_system_id: operation_policy.target_generation_system_id().clone(),
            limits: policy.limits(),
        })
    }

    /// Consumes the start authority and joins exact retained attempt observations.
    ///
    /// The attempt endpoint is the private monotonic checkpoint immediately after
    /// managed isolation close. Later package and evidence revalidation time is
    /// deliberately excluded. No caller-supplied duration or byte total is used.
    ///
    /// # Errors
    ///
    /// Returns a content-free error for any substituted, stale, cancelled,
    /// inconsistent, reversed, or unrepresentable input.
    pub fn finish(
        self,
        input: GenerationQualificationResourceAttemptObservationInput<'_>,
    ) -> Result<
        VerifiedGenerationQualificationResourceAttemptObservation,
        GenerationQualificationResourceAttemptObservationError,
    > {
        validation::finish(self, input)
    }
}

impl fmt::Debug for GenerationQualificationResourceAttemptClock {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GenerationQualificationResourceAttemptClock")
            .field("content", &"redacted")
            .finish()
    }
}

/// Exact retained inputs joined at the resource-attempt measurement boundary.
pub struct GenerationQualificationResourceAttemptObservationInput<'a> {
    /// Fresh exact verified resource policy.
    pub policy: &'a VerifiedGenerationQualificationResourcePolicy,
    /// Fresh exact operation policy.
    pub operation_policy: &'a GenerationQualificationOperationPolicyV1,
    /// Complete owned response, resident receipt, and provider observation.
    ///
    /// The app authority consumes and retains this value without splitting it.
    pub completion: OllamaResidentResourceObservedCompletion,
    /// Exact retained worker resource observation.
    pub worker_observation: &'a ManagedGenerationWorkerResourceObservation,
    /// Exact worker evidence named by the resource observation.
    pub worker_evidence: &'a ManagedGenerationWorkerEvidence,
    /// Cleanup-gated package and private attempt-end checkpoint.
    pub released_package: &'a ReleasedGenerationEffectivePackageV2,
    /// Exact retained runtime-package lease, freshly revalidated by completion.
    pub runtime_package: &'a mut RuntimePackageLease,
    /// Exact retained model-package lease, freshly revalidated by completion.
    pub model_package: &'a VerifiedManagedOllamaModelPackageLease,
    /// Cooperative cancellation observed throughout final validation.
    pub cancellation: &'a CancellationToken,
}

/// Nonportable app-owned measurement authority for one completed target attempt.
///
/// The authority is noncloneable, nonserializable, and content-redacted. It is
/// the sole owner of the inseparable provider response, resident receipt, and
/// resource observation, exposing only borrowed response and receipt views. It is
/// neither a portable resource result nor evidence that a resource phase passed
/// or failed. Its observations do not provide formal resource guarantees.
///
/// ```compile_fail
/// use rewrite_app::VerifiedGenerationQualificationResourceAttemptObservation;
///
/// fn clone_observation(value: &VerifiedGenerationQualificationResourceAttemptObservation) {
///     let _copy: VerifiedGenerationQualificationResourceAttemptObservation = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_app::VerifiedGenerationQualificationResourceAttemptObservation;
///
/// fn serialize_observation(
///     value: &VerifiedGenerationQualificationResourceAttemptObservation,
/// ) {
///     let _json = serde_json::to_string(value).unwrap();
/// }
/// ```
pub struct VerifiedGenerationQualificationResourceAttemptObservation {
    pub(super) completion: OllamaResidentResourceObservedCompletion,
    pub(super) retained_session_subject: OllamaRetainedSessionSubjectToken,
    pub(super) policy_digest: Digest,
    pub(super) operation_policy_id: GenerationQualificationOperationPolicyId,
    pub(super) qualification_plan_id: GenerationQualificationPlanId,
    pub(super) suite_manifest_id: GenerationSuiteManifestId,
    pub(super) target_generation_system_id: GenerationSystemId,
    pub(super) request_binding_digest: Digest,
    pub(super) response_id: OllamaRetainedSessionResponseId,
    pub(super) response_ordinal: u64,
    pub(super) receipt_binding_digest: Digest,
    pub(super) worker_evidence_digest: Digest,
    pub(super) worker_observation_digest: Digest,
    pub(super) effective_package_id: EffectivePackageEvidenceV2Id,
    pub(super) runtime_package_manifest_id: RuntimePackageManifestId,
    pub(super) model_package_manifest_id: ModelPackageManifestId,
    pub(super) values: ResourceAttemptValues,
    pub(super) limits: GenerationQualificationResourcePolicyLimitsV1,
    pub(super) exceeded_limits: Vec<GenerationResourceExceededLimitV1>,
    pub(super) snapshot_digest: Digest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ResourceAttemptValues {
    pub(super) prompt_token_count: u64,
    pub(super) generated_token_count: u64,
    pub(super) total_duration_nanoseconds: u64,
    pub(super) load_duration_nanoseconds: u64,
    pub(super) prompt_evaluation_duration_nanoseconds: u64,
    pub(super) evaluation_duration_nanoseconds: u64,
    pub(super) attempt_elapsed_nanoseconds: u64,
    pub(super) first_response_elapsed_nanoseconds: u64,
    pub(super) cleanup_elapsed_nanoseconds: u64,
    pub(super) worker_high_water_resident_bytes: u64,
    pub(super) runtime_installed_payload_bytes: u64,
    pub(super) model_installed_payload_bytes: u64,
    pub(super) installed_footprint_bytes: u64,
}

impl VerifiedGenerationQualificationResourceAttemptObservation {
    /// Revalidates every retained snapshot identity and checked invariant.
    ///
    /// The consuming finish boundary already performed the final fresh policy,
    /// worker, and retained-package checks. The exact provider completion is
    /// retained and rechecked. Released OS and package capabilities are not
    /// retained, so this method cannot reobserve their external state.
    ///
    /// # Errors
    ///
    /// Returns a content-free error if a checked invariant or the complete
    /// domain-separated snapshot binding differs.
    pub fn revalidate(&self) -> Result<(), GenerationQualificationResourceAttemptObservationError> {
        validation::revalidate(self)
    }

    /// Borrows the exact structured response retained in the inseparable completion.
    #[must_use]
    pub const fn response(&self) -> &StructuredCompletionResponse {
        self.completion.response()
    }

    /// Borrows the exact resident receipt retained in the inseparable completion.
    #[must_use]
    pub const fn resident_execution_receipt(&self) -> &OllamaResidentSessionExecutionReceipt {
        self.completion.resident_execution_receipt()
    }

    /// Returns the exact resource-policy digest.
    #[must_use]
    pub const fn resource_policy_digest(&self) -> &Digest {
        &self.policy_digest
    }

    /// Returns the exact operation-policy identity.
    #[must_use]
    pub const fn operation_policy_id(&self) -> &GenerationQualificationOperationPolicyId {
        &self.operation_policy_id
    }

    /// Returns the exact frozen qualification-plan identity.
    #[must_use]
    pub const fn qualification_plan_id(&self) -> &GenerationQualificationPlanId {
        &self.qualification_plan_id
    }

    /// Returns the exact suite-manifest identity.
    #[must_use]
    pub const fn suite_manifest_id(&self) -> &GenerationSuiteManifestId {
        &self.suite_manifest_id
    }

    /// Returns the exact target generation-system identity.
    #[must_use]
    pub const fn target_generation_system_id(&self) -> &GenerationSystemId {
        &self.target_generation_system_id
    }

    /// Returns the exact structured request binding digest.
    #[must_use]
    pub const fn request_binding_digest(&self) -> &Digest {
        &self.request_binding_digest
    }

    /// Returns the exact final response identity.
    #[must_use]
    pub const fn response_id(&self) -> &OllamaRetainedSessionResponseId {
        &self.response_id
    }

    /// Returns the exact generate response-head ordinal.
    #[must_use]
    pub const fn response_ordinal(&self) -> u64 {
        self.response_ordinal
    }

    /// Returns the complete resident-session receipt binding digest.
    #[must_use]
    pub const fn receipt_binding_digest(&self) -> &Digest {
        &self.receipt_binding_digest
    }

    /// Returns the exact retained worker evidence digest.
    #[must_use]
    pub const fn worker_evidence_digest(&self) -> &Digest {
        &self.worker_evidence_digest
    }

    /// Returns the exact worker resource-observation digest.
    #[must_use]
    pub const fn worker_observation_digest(&self) -> &Digest {
        &self.worker_observation_digest
    }

    /// Returns the cleanup-gated effective-package identity.
    #[must_use]
    pub const fn effective_package_id(&self) -> &EffectivePackageEvidenceV2Id {
        &self.effective_package_id
    }

    /// Returns the exact retained runtime-package identity.
    #[must_use]
    pub const fn runtime_package_manifest_id(&self) -> &RuntimePackageManifestId {
        &self.runtime_package_manifest_id
    }

    /// Returns the exact retained model-package identity.
    #[must_use]
    pub const fn model_package_manifest_id(&self) -> &ModelPackageManifestId {
        &self.model_package_manifest_id
    }

    /// Returns the closed V1 observation profile.
    #[must_use]
    pub const fn observation_profile(&self) -> GenerationResourceObservationProfileV1 {
        GenerationResourceObservationProfileV1::ManagedOllamaV0_32_15LinuxV1
    }

    /// Returns observed prompt tokens.
    #[must_use]
    pub const fn prompt_token_count(&self) -> u64 {
        self.values.prompt_token_count
    }

    /// Returns observed generated tokens.
    #[must_use]
    pub const fn generated_token_count(&self) -> u64 {
        self.values.generated_token_count
    }

    /// Returns provider-reported total duration in nanoseconds.
    #[must_use]
    pub const fn total_duration_nanoseconds(&self) -> u64 {
        self.values.total_duration_nanoseconds
    }

    /// Returns provider-reported load duration in nanoseconds.
    #[must_use]
    pub const fn load_duration_nanoseconds(&self) -> u64 {
        self.values.load_duration_nanoseconds
    }

    /// Returns provider-reported prompt-evaluation duration in nanoseconds.
    #[must_use]
    pub const fn prompt_evaluation_duration_nanoseconds(&self) -> u64 {
        self.values.prompt_evaluation_duration_nanoseconds
    }

    /// Returns provider-reported generation duration in nanoseconds.
    #[must_use]
    pub const fn evaluation_duration_nanoseconds(&self) -> u64 {
        self.values.evaluation_duration_nanoseconds
    }

    /// Returns app-measured complete attempt duration in nanoseconds.
    #[must_use]
    pub const fn attempt_elapsed_nanoseconds(&self) -> u64 {
        self.values.attempt_elapsed_nanoseconds
    }

    /// Returns app-measured time to the exact generate response head.
    #[must_use]
    pub const fn first_response_elapsed_nanoseconds(&self) -> u64 {
        self.values.first_response_elapsed_nanoseconds
    }

    /// Returns app-measured managed close duration in nanoseconds.
    #[must_use]
    pub const fn cleanup_elapsed_nanoseconds(&self) -> u64 {
        self.values.cleanup_elapsed_nanoseconds
    }

    /// Returns kernel-reported worker high-water resident bytes.
    #[must_use]
    pub const fn worker_high_water_resident_bytes(&self) -> u64 {
        self.values.worker_high_water_resident_bytes
    }

    /// Returns exact verified runtime-package payload bytes.
    #[must_use]
    pub const fn runtime_installed_payload_bytes(&self) -> u64 {
        self.values.runtime_installed_payload_bytes
    }

    /// Returns exact verified model-package payload bytes.
    #[must_use]
    pub const fn model_installed_payload_bytes(&self) -> u64 {
        self.values.model_installed_payload_bytes
    }

    /// Returns the checked installed runtime-plus-model footprint.
    #[must_use]
    pub const fn installed_footprint_bytes(&self) -> u64 {
        self.values.installed_footprint_bytes
    }

    /// Returns exceeded limit identities in closed semantic order.
    #[must_use]
    pub fn exceeded_limits(&self) -> &[GenerationResourceExceededLimitV1] {
        &self.exceeded_limits
    }

    /// Returns the complete domain-separated app-owned snapshot binding.
    #[must_use]
    pub const fn snapshot_digest(&self) -> &Digest {
        &self.snapshot_digest
    }
}

impl fmt::Debug for VerifiedGenerationQualificationResourceAttemptObservation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedGenerationQualificationResourceAttemptObservation")
            .field("content", &"redacted")
            .finish()
    }
}

/// Content-free resource-attempt measurement failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum GenerationQualificationResourceAttemptObservationError {
    /// Cancellation was observed before fresh validation completed.
    #[error("generation qualification resource attempt measurement was cancelled")]
    Cancelled,
    /// The exact structurally verified policy was not source-approved.
    #[error("generation qualification resource policy is denied")]
    PolicyDenied,
    /// The policy or operation relationship was substituted.
    #[error("generation qualification resource policy binding is invalid")]
    PolicyBindingMismatch,
    /// The response, request, receipt, or response ordinal was substituted.
    #[error("generation qualification resource response binding is invalid")]
    ResponseBindingMismatch,
    /// Worker evidence or its exact resource observation was substituted.
    #[error("generation qualification resource worker binding is invalid")]
    WorkerBindingMismatch,
    /// The cleanup-gated package and retained package leases did not match.
    #[error("generation qualification resource package binding is invalid")]
    PackageBindingMismatch,
    /// Fresh retained runtime or model bytes did not revalidate.
    #[error("generation qualification resource package revalidation failed")]
    PackageRevalidationFailed,
    /// A duration, ordinal, or installed byte total was unrepresentable.
    #[error("generation qualification resource measurement overflowed")]
    MeasurementOverflow,
    /// Monotonic checkpoints were reversed or outside the attempt bracket.
    #[error("generation qualification resource measurement clock order is invalid")]
    ClockOrderInvalid,
    /// Provider and app observations were internally inconsistent.
    #[error("generation qualification resource observation is inconsistent")]
    ObservationInconsistent,
}

#[cfg(all(test, feature = "test-support"))]
mod tests;
