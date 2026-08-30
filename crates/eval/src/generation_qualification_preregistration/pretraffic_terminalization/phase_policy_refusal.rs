//! Fail-fast closure for a source-denied resource or human phase policy.

use std::fmt;

use rewrite_app::{
    VerifiedGenerationQualificationHumanAdjudicationPolicy,
    VerifiedGenerationQualificationResourcePolicy,
};
use rewrite_model::{
    GenerationAttemptLedgerManifestV1, GenerationHumanAdjudicationEvidenceManifestV1,
    GenerationHumanAdjudicationPolicyDenialRecordV1, GenerationQualificationOperationReceiptV1,
    GenerationRepeatabilityEvidenceManifestV1, GenerationResourceEvidenceManifestV1,
    GenerationResourcePolicyDenialRecordV1,
};
use rewrite_types::CancellationToken;
use thiserror::Error;

use crate::generation_qualification_preregistration::{
    GenerationQualificationPreparationError, PreparedGenerationQualificationOperation,
    prepared::PreparedGenerationQualificationValidationError,
};

mod evidence;

use evidence::{PhasePolicyRefusalEvidence, derive_refusal_evidence, validate_refusal_evidence};

/// Content-redacted failure while closing a source-denied phase policy.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum GenerationQualificationPhasePolicyRefusalError {
    /// Fresh validation of the retained Prepared authority failed.
    #[error("generation qualification prepared authority does not match")]
    PreparedAuthority,
    /// Neither exact phase-policy source denied execution.
    #[error("generation qualification phase policies permit execution")]
    BothPoliciesApproved,
    /// The exact resource-policy authority did not match the operation.
    #[error("generation qualification resource phase policy does not match")]
    ResourcePolicyMismatch,
    /// The exact human-adjudication-policy authority did not match the operation.
    #[error("generation qualification human phase policy does not match")]
    HumanPolicyMismatch,
    /// One required typed source-denial record did not match.
    #[error("generation qualification phase policy denial record does not match")]
    DenialRecordMismatch,
    /// One exact empty Skipped phase manifest did not match.
    #[error("generation qualification skipped phase closure does not match")]
    PhaseClosureMismatch,
    /// The exact failed operation receipt did not match.
    #[error("generation qualification operation receipt does not match")]
    ReceiptMismatch,
    /// The monotonic elapsed interval could not be represented exactly.
    #[error("generation qualification operation elapsed time is unavailable")]
    ElapsedUnavailable,
    /// Multiple independently retained validation stages failed in one pass.
    #[error("generation qualification terminal validation failures were aggregated")]
    ValidationAggregation,
    /// The original absolute operation deadline was reached.
    #[error("generation qualification preparation deadline was reached")]
    DeadlineExceeded,
    /// Cancellation was observed before the original deadline.
    #[error("generation qualification preparation was cancelled")]
    Cancelled,
}

/// Closed, noncloneable pretraffic phase-policy refusal.
///
/// This value retains the exact Prepared closure, both consumed phase-policy
/// authorities, every required denial record, four empty Skipped manifests,
/// and the failed receipt. It grants no launch, generation, traffic,
/// qualification-verification, activation, or live-use authority.
/// The denial records preserve only source disposition. They are not items in
/// the Skipped manifests and do not claim that either phase was reached.
///
/// ```compile_fail
/// use rewrite_eval::FinalizedPhasePolicyRefusedPretrafficGenerationQualification;
/// fn cannot_clone(
///     value: &FinalizedPhasePolicyRefusedPretrafficGenerationQualification<'_, '_, '_, '_, '_>,
/// ) {
///     let _forged = (*value).clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_eval::FinalizedPhasePolicyRefusedPretrafficGenerationQualification;
/// fn no_qualification_record(
///     value: &FinalizedPhasePolicyRefusedPretrafficGenerationQualification<'_, '_, '_, '_, '_>,
/// ) {
///     let _record = value.record();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_eval::FinalizedPhasePolicyRefusedPretrafficGenerationQualification;
/// fn require_serialize<T: serde::Serialize>(_: &T) {}
/// fn cannot_serialize(
///     value: &FinalizedPhasePolicyRefusedPretrafficGenerationQualification<'_, '_, '_, '_, '_>,
/// ) {
///     require_serialize(value);
/// }
/// ```
pub struct FinalizedPhasePolicyRefusedPretrafficGenerationQualification<
    'records,
    'store,
    'platform,
    'proof,
    'lease,
> {
    prepared: PreparedGenerationQualificationOperation<'records, 'store, 'platform, 'proof, 'lease>,
    resource_policy: VerifiedGenerationQualificationResourcePolicy,
    human_policy: VerifiedGenerationQualificationHumanAdjudicationPolicy,
    evidence: PhasePolicyRefusalEvidence,
}

impl<'records, 'store, 'platform, 'proof, 'lease>
    PreparedGenerationQualificationOperation<'records, 'store, 'platform, 'proof, 'lease>
{
    /// Consumes Prepared and both exact phase-policy authorities, then fails fast.
    ///
    /// At least one source disposition must be `Denied`. This transition does
    /// not acquire managed authority and performs no runtime or model operation.
    ///
    /// # Errors
    ///
    /// Returns a content-redacted error for two approved policies, expiry,
    /// cancellation, monotonic clock failure, or any substituted relationship.
    pub fn finalize_phase_policy_refused_pretraffic(
        self,
        resource_policy: VerifiedGenerationQualificationResourcePolicy,
        human_policy: VerifiedGenerationQualificationHumanAdjudicationPolicy,
        cancellation: &CancellationToken,
    ) -> Result<
        FinalizedPhasePolicyRefusedPretrafficGenerationQualification<
            'records,
            'store,
            'platform,
            'proof,
            'lease,
        >,
        GenerationQualificationPhasePolicyRefusalError,
    > {
        FinalizedPhasePolicyRefusedPretrafficGenerationQualification::from_prepared(
            self,
            resource_policy,
            human_policy,
            cancellation,
        )
    }
}

impl<'records, 'store, 'platform, 'proof, 'lease>
    FinalizedPhasePolicyRefusedPretrafficGenerationQualification<
        'records,
        'store,
        'platform,
        'proof,
        'lease,
    >
{
    fn from_prepared(
        mut prepared: PreparedGenerationQualificationOperation<
            'records,
            'store,
            'platform,
            'proof,
            'lease,
        >,
        resource_policy: VerifiedGenerationQualificationResourcePolicy,
        human_policy: VerifiedGenerationQualificationHumanAdjudicationPolicy,
        cancellation: &CancellationToken,
    ) -> Result<Self, GenerationQualificationPhasePolicyRefusalError> {
        let evidence = prepared
            .with_validated_view(cancellation, |view| {
                derive_refusal_evidence(&view, &resource_policy, &human_policy, cancellation)
            })
            .map_err(map_validated_view_error)?;
        Ok(Self {
            prepared,
            resource_policy,
            human_policy,
            evidence,
        })
    }

    /// Returns the exact failed inert operation receipt.
    #[must_use]
    pub const fn receipt(&self) -> &GenerationQualificationOperationReceiptV1 {
        &self.evidence.receipt
    }

    /// Returns the standalone resource-source denial record when denied.
    ///
    /// This record is not a resource-manifest item and does not mean that the
    /// resource phase was reached.
    #[must_use]
    pub const fn resource_policy_denial_record(
        &self,
    ) -> Option<&GenerationResourcePolicyDenialRecordV1> {
        self.evidence.resource_denial_record.as_ref()
    }

    /// Returns the standalone human-source denial record when denied.
    ///
    /// This record is not a human-manifest item and does not mean that the
    /// adjudication phase was reached.
    #[must_use]
    pub const fn human_policy_denial_record(
        &self,
    ) -> Option<&GenerationHumanAdjudicationPolicyDenialRecordV1> {
        self.evidence.human_denial_record.as_ref()
    }

    /// Returns the exact skipped attempt-ledger manifest.
    #[must_use]
    pub const fn attempt_ledger_manifest(&self) -> &GenerationAttemptLedgerManifestV1 {
        &self.evidence.attempt_ledger_manifest
    }

    /// Returns the exact skipped repeatability manifest.
    #[must_use]
    pub const fn repeatability_manifest(&self) -> &GenerationRepeatabilityEvidenceManifestV1 {
        &self.evidence.repeatability_manifest
    }

    /// Returns the exact skipped resource manifest.
    #[must_use]
    pub const fn resource_manifest(&self) -> &GenerationResourceEvidenceManifestV1 {
        &self.evidence.resource_manifest
    }

    /// Returns the exact skipped human-adjudication manifest.
    #[must_use]
    pub const fn human_adjudication_manifest(
        &self,
    ) -> &GenerationHumanAdjudicationEvidenceManifestV1 {
        &self.evidence.human_adjudication_manifest
    }

    /// Freshly revalidates both policies, Prepared, denial records, and receipt.
    ///
    /// # Errors
    ///
    /// Returns a content-redacted error for expiry, cancellation, retained
    /// authority drift, or any changed exact relationship.
    pub fn revalidate(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<(), GenerationQualificationPhasePolicyRefusalError> {
        let Self {
            prepared,
            resource_policy,
            human_policy,
            evidence,
        } = self;
        prepared
            .with_validated_view(cancellation, |view| {
                validate_refusal_evidence(
                    &view,
                    resource_policy,
                    human_policy,
                    evidence,
                    cancellation,
                )
            })
            .map_err(map_validated_view_error)
    }
}

impl fmt::Debug
    for FinalizedPhasePolicyRefusedPretrafficGenerationQualification<'_, '_, '_, '_, '_>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FinalizedPhasePolicyRefusedPretrafficGenerationQualification")
            .field(
                "resource_policy_denied",
                &self.evidence.resource_denial_record.is_some(),
            )
            .field(
                "human_policy_denied",
                &self.evidence.human_denial_record.is_some(),
            )
            .field("terminal_status", &self.evidence.receipt.terminal_status())
            .finish_non_exhaustive()
    }
}

fn map_validated_view_error(
    error: PreparedGenerationQualificationValidationError<
        GenerationQualificationPhasePolicyRefusalError,
    >,
) -> GenerationQualificationPhasePolicyRefusalError {
    match error {
        PreparedGenerationQualificationValidationError::Initial(error)
        | PreparedGenerationQualificationValidationError::Final(error) => {
            map_preparation_error(error)
        }
        PreparedGenerationQualificationValidationError::Callback(error) => error,
        PreparedGenerationQualificationValidationError::InitialAndFinal { .. }
        | PreparedGenerationQualificationValidationError::CallbackAndFinal { .. } => {
            GenerationQualificationPhasePolicyRefusalError::ValidationAggregation
        }
    }
}

const fn map_preparation_error(
    error: GenerationQualificationPreparationError,
) -> GenerationQualificationPhasePolicyRefusalError {
    match error {
        GenerationQualificationPreparationError::DeadlineExceeded => {
            GenerationQualificationPhasePolicyRefusalError::DeadlineExceeded
        }
        GenerationQualificationPreparationError::Cancelled => {
            GenerationQualificationPhasePolicyRefusalError::Cancelled
        }
        _ => GenerationQualificationPhasePolicyRefusalError::PreparedAuthority,
    }
}

#[cfg(test)]
#[path = "phase_policy_refusal/tests.rs"]
mod tests;
