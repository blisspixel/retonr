//! Request-projected operation state.

use rewrite_app::{
    GenerationQualificationLicenseAssessmentInput,
    GenerationQualificationPlatformPortableRelations, ProductionModelLicenseApprovalPolicy,
    VerifiedGenerationQualificationLicenseAssessment,
    VerifiedGenerationQualificationLicenseAssessmentPolicy,
    VerifiedGenerationQualificationPlatformAssessment,
    VerifiedGenerationQualificationRequestProjectionV1, VerifiedManagedOllamaModelPackageLease,
    VerifiedModelLicenseControl,
};
use rewrite_model::GenerationQualificationRequestProjectionV1Relations;
use rewrite_model_store::GenerationQualificationPreregistrationFoundationV1Input;
use rewrite_types::CancellationToken;

use super::{
    GenerationQualificationPreparationError, PreparedGenerationQualificationOperation,
    repository::GenerationQualificationPreregistrationRepository,
};

/// Exact request-projected operation awaiting app assessment authorities.
///
/// This state owns the app request-stream authority. Its public views are inert
/// portable relationship records used only to compile the two app assessments.
pub struct ProjectedGenerationQualificationOperation<'records, 'store> {
    pub(super) stream: VerifiedGenerationQualificationRequestProjectionV1<'records, 'store>,
}

impl<'records, 'store> ProjectedGenerationQualificationOperation<'records, 'store> {
    pub(super) const fn from_verified_stream(
        stream: VerifiedGenerationQualificationRequestProjectionV1<'records, 'store>,
    ) -> Self {
        Self { stream }
    }

    /// Returns the inert portable closure needed by the platform assessor.
    #[must_use]
    pub fn platform_portable_relations(
        &self,
    ) -> GenerationQualificationPlatformPortableRelations<'_> {
        GenerationQualificationPlatformPortableRelations {
            operation_policy: self.stream.operation_policy(),
            operation_policy_relations: self.stream.operation_policy_relations(),
            operation_policy_input: self.stream.operation_policy_input(),
            request_projection: self.stream.projection(),
            request_projection_relations: self.projection_relations(),
            request_projection_entry_inputs: self.stream.entry_inputs(),
        }
    }

    /// Forms the exact borrowed input needed by the license assessor.
    ///
    /// The returned value is inert. The app compiler independently validates all
    /// retained proof, package, policy, operation, and projection relationships.
    #[must_use]
    pub fn license_assessment_input<'view, 'proof, 'lease>(
        &'view self,
        assessment_policy: &'view VerifiedGenerationQualificationLicenseAssessmentPolicy,
        model_license_control: &'proof VerifiedModelLicenseControl<'lease>,
        selected_model_package_lease: &'lease VerifiedManagedOllamaModelPackageLease,
        production_approval_policy: &'view ProductionModelLicenseApprovalPolicy,
    ) -> GenerationQualificationLicenseAssessmentInput<'view, 'proof, 'lease> {
        let operation_relations = self.stream.operation_policy_relations();
        GenerationQualificationLicenseAssessmentInput {
            operation_policy: self.stream.operation_policy(),
            operation_policy_relations: operation_relations,
            operation_policy_input: self.stream.operation_policy_input(),
            request_projection: self.stream.projection(),
            request_projection_relations: self.projection_relations(),
            request_projection_entry_inputs: self.stream.entry_inputs(),
            target_generation_system: operation_relations.target_system.generation_system,
            target_generation_system_relations: operation_relations.target_system.relations,
            assessment_policy,
            model_license_control,
            selected_model_package_lease,
            production_approval_policy,
        }
    }

    /// Persists the exact foundation and atomically preregisters both records.
    ///
    /// The schema 8 generation-system and schema 9 plan foundations are materialized
    /// before the schema 7 preregistration transaction cold-validates that exact plan.
    /// Negative platform or license assessments are valid prepared outcomes. This
    /// method performs no launch, runtime, network, or model traffic operation.
    ///
    /// # Errors
    ///
    /// Returns a content-redacted failure for expiry, cancellation, immutable
    /// collision, readback drift, or any app-authority substitution or drift.
    #[expect(
        clippy::too_many_arguments,
        reason = "the state transition consumes one complete pretraffic authority closure"
    )]
    pub fn finish<'platform, 'proof, 'lease>(
        self,
        repository: &mut GenerationQualificationPreregistrationRepository,
        foundation: GenerationQualificationPreregistrationFoundationV1Input<'_>,
        platform: VerifiedGenerationQualificationPlatformAssessment<'platform>,
        license: VerifiedGenerationQualificationLicenseAssessment<'proof, 'lease>,
        license_assessment_policy: VerifiedGenerationQualificationLicenseAssessmentPolicy,
        production_approval_policy: ProductionModelLicenseApprovalPolicy,
        cancellation: &CancellationToken,
    ) -> Result<
        PreparedGenerationQualificationOperation<'records, 'store, 'platform, 'proof, 'lease>,
        GenerationQualificationPreparationError,
    > {
        PreparedGenerationQualificationOperation::finish_projected(
            self,
            repository,
            foundation,
            platform,
            license,
            license_assessment_policy,
            production_approval_policy,
            cancellation,
        )
    }

    fn projection_relations(&self) -> GenerationQualificationRequestProjectionV1Relations<'_> {
        let operation_relations = self.stream.operation_policy_relations();
        GenerationQualificationRequestProjectionV1Relations {
            operation_policy: self.stream.operation_policy(),
            qualification_plan: operation_relations.plan,
            suite: operation_relations.suite,
            planned_attempts: operation_relations.planned_attempts,
        }
    }
}
