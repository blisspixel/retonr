//! Nonforgeable live authority over one complete Qualified generation system.

use std::fmt;

use rewrite_model::{
    GenerationHumanAdjudicationEvidenceManifestV1, GenerationQualificationLicenseEvidenceV1,
    GenerationQualificationOperationFinalizationStatusV1,
    GenerationQualificationOperationReceiptV1, GenerationQualificationOperationTerminalStatusV1,
    GenerationQualificationPhaseStatusV1, GenerationQualificationPlanV1,
    GenerationQualificationPlatformEvidenceV1, GenerationQualificationRecordV1,
    GenerationQualificationStatusV1, GenerationRepeatabilityEvidenceManifestV1,
    GenerationResourceEvidenceManifestV1, GenerationSuiteManifestV1, GenerationSystemRecordV1,
};
use rewrite_types::CancellationToken;
use thiserror::Error;

use crate::{
    VerifiedGenerationQualificationResourcePhase,
    generation_qualification_preregistration::GenerationQualificationPreregistrationRepository,
};

#[cfg(test)]
mod tests;

/// Content-free failure during generation qualification verification.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum VerifiedGenerationQualificationError {
    /// Cooperative cancellation was observed.
    #[error("verified generation qualification was cancelled")]
    Cancelled,
    /// Qualification record status is not Qualified.
    #[error("generation qualification record status is not qualified")]
    NotQualified,
    /// Human adjudication manifest status was not Passed.
    #[error("generation qualification human adjudication did not pass")]
    HumanAdjudicationNotPassed,
    /// Resource phase manifest status was not Passed.
    #[error("generation qualification resource phase did not pass")]
    ResourcePhaseNotPassed,
    /// Repeatability phase manifest status was not Passed.
    #[error("generation qualification repeatability phase did not pass")]
    RepeatabilityNotPassed,
    /// Operation receipt terminal status is not Completed or finalization status is not Passed.
    #[error("generation qualification operation receipt is not completed")]
    ReceiptNotCompleted,
    /// Cross-phase scopes, plan, suite, or generation system identities do not match.
    #[error("generation qualification scope does not match")]
    ScopeMismatch,
    /// Live join authorities or mandatory settlement finalizers failed revalidation.
    #[error("generation qualification live authority failed")]
    JoinAuthority,
    /// Durable record readback failed or differs from the expected qualification.
    #[error("generation qualification durable verification failed")]
    DurableVerification,
}

/// Noncloneable and nonserializable live authority over one complete Qualified generation system.
///
/// This authority proves that the complete stored evidence and every required
/// live repetition join were reloaded and accepted by the current operation policy,
/// yielding an exact Qualified record. It grants no launch authority.
///
/// ```compile_fail
/// use rewrite_eval::VerifiedGenerationQualification;
///
/// fn clone_authority(value: VerifiedGenerationQualification<'_, '_, '_, '_>) {
///     let _forged = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_eval::VerifiedGenerationQualification;
///
/// fn serialize_authority(value: &VerifiedGenerationQualification<'_, '_, '_, '_>) {
///     let _bytes = serde_json::to_vec(value).expect("authority must not serialize");
/// }
/// ```
pub struct VerifiedGenerationQualification<'store, 'records, 'model, 'runtime> {
    resource_phase:
        VerifiedGenerationQualificationResourcePhase<'store, 'records, 'model, 'runtime>,
    operation_receipt: GenerationQualificationOperationReceiptV1,
    qualification_record: GenerationQualificationRecordV1,
    human_manifest: GenerationHumanAdjudicationEvidenceManifestV1,
    platform_evidence: GenerationQualificationPlatformEvidenceV1,
    license_evidence: GenerationQualificationLicenseEvidenceV1,
}

impl<'store, 'records, 'model, 'runtime>
    VerifiedGenerationQualification<'store, 'records, 'model, 'runtime>
{
    /// Returns the exact terminal qualification record.
    #[must_use]
    pub const fn qualification_record(&self) -> &GenerationQualificationRecordV1 {
        &self.qualification_record
    }

    /// Returns the exact completed operation receipt.
    #[must_use]
    pub const fn operation_receipt(&self) -> &GenerationQualificationOperationReceiptV1 {
        &self.operation_receipt
    }

    /// Returns the exact passed human-adjudication manifest.
    #[must_use]
    pub const fn human_manifest(&self) -> &GenerationHumanAdjudicationEvidenceManifestV1 {
        &self.human_manifest
    }

    /// Returns the exact platform assessment evidence.
    #[must_use]
    pub const fn platform_evidence(&self) -> &GenerationQualificationPlatformEvidenceV1 {
        &self.platform_evidence
    }

    /// Returns the exact license assessment evidence.
    #[must_use]
    pub const fn license_evidence(&self) -> &GenerationQualificationLicenseEvidenceV1 {
        &self.license_evidence
    }

    /// Returns the retained verified resource phase authority.
    #[must_use]
    pub const fn resource_phase(
        &self,
    ) -> &VerifiedGenerationQualificationResourcePhase<'store, 'records, 'model, 'runtime> {
        &self.resource_phase
    }

    /// Returns the passed resource manifest.
    #[must_use]
    pub const fn resource_manifest(&self) -> &GenerationResourceEvidenceManifestV1 {
        self.resource_phase.resource_manifest()
    }

    /// Returns the passed repeatability manifest.
    #[must_use]
    pub const fn repeatability_manifest(&self) -> &GenerationRepeatabilityEvidenceManifestV1 {
        self.resource_phase.repeatability_manifest()
    }

    /// Returns the target generation-system record.
    #[must_use]
    pub const fn target_generation_system(&self) -> &GenerationSystemRecordV1 {
        self.resource_phase.target_generation_system()
    }

    /// Returns the qualification plan record.
    #[must_use]
    pub const fn qualification_plan(&self) -> &GenerationQualificationPlanV1 {
        self.resource_phase.qualification_plan()
    }

    /// Returns the suite manifest.
    #[must_use]
    pub const fn suite_manifest(&self) -> &GenerationSuiteManifestV1 {
        self.resource_phase.suite_manifest()
    }

    /// Freshly revalidates live join authorities and qualification state.
    ///
    /// # Errors
    /// Returns an error if cancellation is observed or live join revalidation fails.
    pub fn revalidate(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<(), VerifiedGenerationQualificationError> {
        if cancellation.is_cancelled() {
            return Err(VerifiedGenerationQualificationError::Cancelled);
        }
        self.resource_phase
            .revalidate(cancellation)
            .map_err(|_| VerifiedGenerationQualificationError::JoinAuthority)?;
        if self.qualification_record.status() != GenerationQualificationStatusV1::Qualified {
            return Err(VerifiedGenerationQualificationError::NotQualified);
        }
        Ok(())
    }

    /// Freshly revalidates mandatory settlement finalizers.
    ///
    /// # Errors
    /// Returns an error if mandatory settlement finalization revalidation fails.
    pub fn revalidate_for_mandatory_settlement_finalization(
        &mut self,
    ) -> Result<(), VerifiedGenerationQualificationError> {
        self.resource_phase
            .revalidate_for_mandatory_settlement_finalization()
            .map_err(|()| VerifiedGenerationQualificationError::JoinAuthority)
    }
}

impl fmt::Debug for VerifiedGenerationQualification<'_, '_, '_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedGenerationQualification")
            .field(
                "qualification_record_id",
                self.qualification_record.generation_qualification_id(),
            )
            .field("status", &self.qualification_record.status())
            .field(
                "target_generation_system_id",
                self.target_generation_system().generation_system_id(),
            )
            .finish_non_exhaustive()
    }
}

/// Typed input bundle for compiling verified generation qualification authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedGenerationQualificationCompilerInput {
    /// Exact completed operation receipt.
    pub receipt: GenerationQualificationOperationReceiptV1,
    /// Exact terminal qualification record.
    pub record: GenerationQualificationRecordV1,
    /// Exact passed human adjudication phase manifest.
    pub human_manifest: GenerationHumanAdjudicationEvidenceManifestV1,
    /// Exact platform assessment evidence.
    pub platform_evidence: GenerationQualificationPlatformEvidenceV1,
    /// Exact license assessment evidence.
    pub license_evidence: GenerationQualificationLicenseEvidenceV1,
}

/// Stateless compiler for one verified generation qualification authority.
#[derive(Clone, Copy, Debug, Default)]
pub struct VerifiedGenerationQualificationCompiler;

impl VerifiedGenerationQualificationCompiler {
    /// Compiles and freshly revalidates one Qualified terminal qualification authority.
    ///
    /// # Errors
    /// Returns a content-free error if the qualification record is not Qualified,
    /// if human adjudication or resource phases did not pass, if live authority
    /// revalidation fails, if durable readback fails, or upon cancellation.
    pub fn compile<'store, 'records, 'model, 'runtime>(
        repository: &GenerationQualificationPreregistrationRepository,
        mut phase: VerifiedGenerationQualificationResourcePhase<'store, 'records, 'model, 'runtime>,
        input: VerifiedGenerationQualificationCompilerInput,
        cancellation: &CancellationToken,
    ) -> Result<
        VerifiedGenerationQualification<'store, 'records, 'model, 'runtime>,
        VerifiedGenerationQualificationError,
    > {
        let VerifiedGenerationQualificationCompilerInput {
            receipt,
            record,
            human_manifest,
            platform_evidence,
            license_evidence,
        } = input;
        if cancellation.is_cancelled() {
            return Err(VerifiedGenerationQualificationError::Cancelled);
        }
        if record.status() != GenerationQualificationStatusV1::Qualified {
            return Err(VerifiedGenerationQualificationError::NotQualified);
        }
        if human_manifest.status() != GenerationQualificationPhaseStatusV1::Passed {
            return Err(VerifiedGenerationQualificationError::HumanAdjudicationNotPassed);
        }
        if phase.resource_manifest().status() != GenerationQualificationPhaseStatusV1::Passed {
            return Err(VerifiedGenerationQualificationError::ResourcePhaseNotPassed);
        }
        if phase.repeatability_manifest().status() != GenerationQualificationPhaseStatusV1::Passed {
            return Err(VerifiedGenerationQualificationError::RepeatabilityNotPassed);
        }
        if receipt.terminal_status() != GenerationQualificationOperationTerminalStatusV1::Completed
            || receipt.finalization_status()
                != GenerationQualificationOperationFinalizationStatusV1::Passed
        {
            return Err(VerifiedGenerationQualificationError::ReceiptNotCompleted);
        }

        let target_id = phase.target_generation_system().generation_system_id();
        let plan_id = phase.qualification_plan().qualification_plan_id();
        let suite_id = phase.suite_manifest().suite_manifest_id();
        let policy_id = phase.operation_policy().operation_policy_id();

        if record.target_generation_system_id() != target_id
            || record.operation_policy_id() != policy_id
            || record.operation_receipt_id() != receipt.operation_receipt_id()
            || record.platform_evidence_id() != platform_evidence.platform_evidence_id()
            || record.license_evidence_id() != license_evidence.license_evidence_id()
            || receipt.target_generation_system_id() != target_id
            || receipt.operation_policy_id() != policy_id
            || receipt.platform_evidence_id() != platform_evidence.platform_evidence_id()
            || receipt.license_evidence_id() != license_evidence.license_evidence_id()
            || receipt.resource_evidence_manifest().0
                != phase.resource_manifest().resource_evidence_manifest_id()
            || receipt.resource_evidence_manifest().1
                != phase.resource_manifest().evidence_root_digest()
            || receipt.repeatability_evidence_manifest().0
                != phase
                    .repeatability_manifest()
                    .repeatability_evidence_manifest_id()
            || receipt.repeatability_evidence_manifest().1
                != phase.repeatability_manifest().evidence_root_digest()
            || receipt.human_adjudication_evidence_manifest().0
                != human_manifest.human_adjudication_evidence_manifest_id()
            || receipt.human_adjudication_evidence_manifest().1
                != human_manifest.evidence_root_digest()
            || human_manifest.generation_system_id() != target_id
            || human_manifest.generation_qualification_plan_id() != plan_id
            || human_manifest.suite_manifest_id() != suite_id
            || phase.resource_manifest().generation_system_id() != target_id
            || phase.resource_manifest().generation_qualification_plan_id() != plan_id
            || phase.resource_manifest().suite_manifest_id() != suite_id
            || phase.repeatability_manifest().generation_system_id() != target_id
            || phase
                .repeatability_manifest()
                .generation_qualification_plan_id()
                != plan_id
            || phase.repeatability_manifest().suite_manifest_id() != suite_id
        {
            return Err(VerifiedGenerationQualificationError::ScopeMismatch);
        }

        phase
            .revalidate(cancellation)
            .map_err(|_| VerifiedGenerationQualificationError::JoinAuthority)?;
        phase
            .revalidate_for_mandatory_settlement_finalization()
            .map_err(|()| VerifiedGenerationQualificationError::JoinAuthority)?;

        let stored_record = repository
            .read_qualification_record(&record)
            .map_err(|_| VerifiedGenerationQualificationError::DurableVerification)?;
        if stored_record.as_ref() != Some(&record) {
            return Err(VerifiedGenerationQualificationError::DurableVerification);
        }

        if cancellation.is_cancelled() {
            return Err(VerifiedGenerationQualificationError::Cancelled);
        }

        Ok(VerifiedGenerationQualification {
            resource_phase: phase,
            operation_receipt: receipt,
            qualification_record: record,
            human_manifest,
            platform_evidence,
            license_evidence,
        })
    }
}
