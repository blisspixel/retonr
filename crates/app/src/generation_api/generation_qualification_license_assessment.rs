//! App-owned pretraffic model-license assessment for generation qualification.

use std::fmt;

use rewrite_model::{
    GenerationQualificationContractError, GenerationQualificationLicenseDecisionV1,
    GenerationQualificationLicenseEvidenceV1, GenerationQualificationLicenseEvidenceV1Input,
    GenerationQualificationLicenseEvidenceV1Relations, GenerationQualificationLicensePermissionV1,
    GenerationQualificationLicenseReasonV1, GenerationQualificationOperationContractError,
    GenerationQualificationOperationPolicyV1, GenerationQualificationOperationPolicyV1Input,
    GenerationQualificationOperationPolicyV1Relations,
    GenerationQualificationRequestProjectionEntryV1Input,
    GenerationQualificationRequestProjectionV1,
    GenerationQualificationRequestProjectionV1Relations, GenerationSystemRecordV1,
    GenerationSystemRecordV1Relations, ModelPackageMemberRole,
};
use rewrite_types::CancellationToken;
use thiserror::Error;

use super::{
    GenerationQualificationAssessmentPolicySourceDisposition,
    ModelLicenseControlAssessmentDisposition, ModelLicenseControlError, ModelLicensePermission,
    ProductionModelLicenseApprovalPolicy, VerifiedGenerationQualificationLicenseAssessmentPolicy,
    VerifiedModelLicenseControl,
};
use crate::VerifiedManagedOllamaModelPackageLease;

/// Exact records and live authorities required for one license assessment.
#[derive(Clone, Copy)]
pub struct GenerationQualificationLicenseAssessmentInput<'records, 'proof, 'lease> {
    /// Preregistered operation policy.
    pub operation_policy: &'records GenerationQualificationOperationPolicyV1,
    /// Complete exact relationships of the operation policy.
    pub operation_policy_relations: GenerationQualificationOperationPolicyV1Relations<'records>,
    /// Independently retained expected operation-policy input.
    pub operation_policy_input: &'records GenerationQualificationOperationPolicyV1Input,
    /// Complete preregistered request projection.
    pub request_projection: &'records GenerationQualificationRequestProjectionV1,
    /// Independently retained exact relationships of the request projection.
    pub request_projection_relations: GenerationQualificationRequestProjectionV1Relations<'records>,
    /// Independently retained exact request-projection entry inputs.
    pub request_projection_entry_inputs:
        &'records [GenerationQualificationRequestProjectionEntryV1Input],
    /// Target generation system selected by the operation policy.
    pub target_generation_system: &'records GenerationSystemRecordV1,
    /// Complete exact target generation-system closure.
    pub target_generation_system_relations: GenerationSystemRecordV1Relations<'records>,
    /// Structurally verified app-owned assessment policy.
    pub assessment_policy: &'records VerifiedGenerationQualificationLicenseAssessmentPolicy,
    /// Live structural proof for the exact portable model-license control.
    pub model_license_control: &'proof VerifiedModelLicenseControl<'lease>,
    /// Exact live model-package lease selected by the structural proof.
    pub selected_model_package_lease: &'lease VerifiedManagedOllamaModelPackageLease,
    /// Immutable repository-owned production approval root.
    pub production_approval_policy: &'records ProductionModelLicenseApprovalPolicy,
}

/// Portable records and immutable policies needed to freshly revalidate one
/// retained license assessment without exposing its live structural proof.
#[derive(Clone, Copy)]
pub struct GenerationQualificationLicensePortableRelations<'records> {
    /// Preregistered operation policy.
    pub operation_policy: &'records GenerationQualificationOperationPolicyV1,
    /// Complete exact relationships of the operation policy.
    pub operation_policy_relations: GenerationQualificationOperationPolicyV1Relations<'records>,
    /// Independently retained expected operation-policy input.
    pub operation_policy_input: &'records GenerationQualificationOperationPolicyV1Input,
    /// Complete preregistered request projection.
    pub request_projection: &'records GenerationQualificationRequestProjectionV1,
    /// Independently retained exact relationships of the request projection.
    pub request_projection_relations: GenerationQualificationRequestProjectionV1Relations<'records>,
    /// Independently retained exact request-projection entry inputs.
    pub request_projection_entry_inputs:
        &'records [GenerationQualificationRequestProjectionEntryV1Input],
    /// Target generation system selected by the operation policy.
    pub target_generation_system: &'records GenerationSystemRecordV1,
    /// Complete exact target generation-system closure.
    pub target_generation_system_relations: GenerationSystemRecordV1Relations<'records>,
    /// Structurally verified app-owned assessment policy.
    pub assessment_policy: &'records VerifiedGenerationQualificationLicenseAssessmentPolicy,
    /// Immutable repository-owned production approval root.
    pub production_approval_policy: &'records ProductionModelLicenseApprovalPolicy,
}

impl<'records> GenerationQualificationLicenseAssessmentInput<'records, '_, '_> {
    /// Returns the exact portable closure without exposing either live owner.
    #[must_use]
    pub const fn portable_relations(
        &self,
    ) -> GenerationQualificationLicensePortableRelations<'records> {
        GenerationQualificationLicensePortableRelations {
            operation_policy: self.operation_policy,
            operation_policy_relations: self.operation_policy_relations,
            operation_policy_input: self.operation_policy_input,
            request_projection: self.request_projection,
            request_projection_relations: self.request_projection_relations,
            request_projection_entry_inputs: self.request_projection_entry_inputs,
            target_generation_system: self.target_generation_system,
            target_generation_system_relations: self.target_generation_system_relations,
            assessment_policy: self.assessment_policy,
            production_approval_policy: self.production_approval_policy,
        }
    }
}

/// Closed relationship rejected by the license assessment compiler.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GenerationQualificationLicenseAssessmentRelationship {
    /// The separately supplied target was not the operation policy's exact target.
    OperationTarget,
    /// The request projection named another operation policy.
    ProjectionOperation,
    /// The request projection named another target generation system.
    ProjectionTarget,
    /// The projection and operation closures named different plans or suites.
    ProjectionScope,
    /// The projection and operation closures retained different planned attempts.
    ProjectionAttempts,
    /// The operation policy named another license assessment policy.
    AssessmentPolicy,
    /// The target system and selected lease named different model artifact sets.
    ModelArtifactSet,
    /// The target system and selected lease named different model packages.
    ModelPackage,
    /// The target model was not an exact model-weight member of the selected package.
    ModelArtifact,
    /// The structural control and selected lease named different foundations.
    ModelFoundation,
    /// The structural control and assessment policy named different controls.
    ModelLicenseControl,
    /// The operation, assessment policy, and structural control permissions differed.
    Permission,
    /// The freshly supplied live proof or package lease was not the retained owner.
    RetainedAuthority,
}

/// Failure while deriving exact pretraffic model-license assessment evidence.
#[derive(Debug, Error)]
pub enum GenerationQualificationLicenseAssessmentError {
    /// Work was cancelled before a stable result could be derived.
    #[error("generation qualification license assessment was cancelled")]
    Cancelled,
    /// The preregistered operation policy failed exact relationship validation.
    #[error("generation qualification operation policy is invalid")]
    OperationPolicy(#[source] GenerationQualificationOperationContractError),
    /// The preregistered request projection failed exact relationship validation.
    #[error("generation qualification request projection is invalid")]
    RequestProjection(#[source] GenerationQualificationOperationContractError),
    /// The target generation-system closure failed exact validation.
    #[error("generation qualification target system is invalid")]
    TargetSystem(#[source] GenerationQualificationContractError),
    /// A required exact assessment relationship did not match.
    #[error("generation qualification license assessment relationship does not match")]
    Relationship(GenerationQualificationLicenseAssessmentRelationship),
    /// The live structural model-license proof failed revalidation or assessment.
    #[error("generation qualification model-license control is invalid")]
    ModelLicenseControl(#[source] ModelLicenseControlError),
    /// Portable license evidence could not be derived from the exact closure.
    #[error("generation qualification license evidence is invalid")]
    Evidence(#[source] GenerationQualificationOperationContractError),
}

/// Compiler for one exact app-owned pretraffic license assessment.
#[derive(Clone, Copy, Debug, Default)]
pub struct GenerationQualificationLicenseAssessmentCompiler;

impl GenerationQualificationLicenseAssessmentCompiler {
    /// Validates the complete static and live closure and derives portable evidence.
    ///
    /// Source-policy denial and production-control denial are valid negative
    /// assessment outcomes. Structural drift, substitution, or cancellation is an
    /// error and yields no evidence. This compiler never promotes a structural
    /// model-license proof and cannot mint launch authority.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationLicenseAssessmentError`] for cancellation,
    /// invalid static relationships, live proof drift, or any substitution.
    pub fn compile<'proof, 'lease>(
        input: &GenerationQualificationLicenseAssessmentInput<'_, 'proof, 'lease>,
        cancellation: &CancellationToken,
    ) -> Result<
        VerifiedGenerationQualificationLicenseAssessment<'proof, 'lease>,
        GenerationQualificationLicenseAssessmentError,
    > {
        compile_with_post_derivation(input, cancellation, || {})
    }
}

/// Noncloneable, nonserializable app authority for exact portable license evidence.
///
/// This authority retains the live structural proof and selected package lease. It
/// grants no model launch, traffic, generation, or qualification authority.
///
/// ```compile_fail
/// use rewrite_app::VerifiedGenerationQualificationLicenseAssessment;
///
/// fn clone_authority(value: &VerifiedGenerationQualificationLicenseAssessment<'_, '_>) {
///     let _forged: VerifiedGenerationQualificationLicenseAssessment<'_, '_> = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_app::VerifiedGenerationQualificationLicenseAssessment;
///
/// fn serialize_authority(value: &VerifiedGenerationQualificationLicenseAssessment<'_, '_>) {
///     let _bytes = serde_json::to_vec(value).expect("authority must not serialize");
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_app::{
///     VerifiedApprovedModelLicenseControl,
///     VerifiedGenerationQualificationLicenseAssessment,
/// };
///
/// fn require_launch_authority(_value: VerifiedApprovedModelLicenseControl<'_>) {}
///
/// fn negative_evidence_cannot_launch(
///     value: VerifiedGenerationQualificationLicenseAssessment<'_, '_>,
/// ) {
///     require_launch_authority(value);
/// }
/// ```
pub struct VerifiedGenerationQualificationLicenseAssessment<'proof, 'lease> {
    evidence: GenerationQualificationLicenseEvidenceV1,
    input: GenerationQualificationLicenseEvidenceV1Input,
    model_license_control: &'proof VerifiedModelLicenseControl<'lease>,
    selected_model_package_lease: &'lease VerifiedManagedOllamaModelPackageLease,
}

impl VerifiedGenerationQualificationLicenseAssessment<'_, '_> {
    /// Returns the exact inert portable evidence derived by this authority.
    #[must_use]
    pub const fn portable_evidence(&self) -> &GenerationQualificationLicenseEvidenceV1 {
        &self.evidence
    }

    /// Returns the independently derived closed assessment input.
    #[must_use]
    pub const fn assessment_input(&self) -> GenerationQualificationLicenseEvidenceV1Input {
        self.input
    }

    /// Freshly revalidates the retained structural proof and exact selected lease.
    ///
    /// This operation adds no production approval or launch authority.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationLicenseAssessmentError`] for cancellation,
    /// retained-state drift, or live lease substitution.
    pub fn revalidate_live_proof(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), GenerationQualificationLicenseAssessmentError> {
        self.model_license_control
            .revalidate(
                self.selected_model_package_lease,
                ModelLicensePermission::LocalGeneration,
                cancellation,
            )
            .map_err(map_license_error)
    }

    /// Freshly revalidates the complete static closure, retained live proof, and
    /// current production disposition against the independently derived input.
    ///
    /// # Errors
    ///
    /// Returns a content-free error for cancellation, owner substitution, drift,
    /// or any changed operation, projection, target, policy, or license relation.
    pub fn revalidate_against(
        &self,
        input: &GenerationQualificationLicenseAssessmentInput<'_, '_, '_>,
        cancellation: &CancellationToken,
    ) -> Result<(), GenerationQualificationLicenseAssessmentError> {
        ensure_active(cancellation)?;
        if !std::ptr::eq(self.model_license_control, input.model_license_control)
            || !std::ptr::eq(
                self.selected_model_package_lease,
                input.selected_model_package_lease,
            )
        {
            return Err(GenerationQualificationLicenseAssessmentError::Relationship(
                GenerationQualificationLicenseAssessmentRelationship::RetainedAuthority,
            ));
        }
        validate_static_closure(input)?;
        let control_disposition = input
            .production_approval_policy
            .assess(
                self.model_license_control,
                self.selected_model_package_lease,
                ModelLicensePermission::LocalGeneration,
                cancellation,
            )
            .map_err(map_license_error)?;
        let expected = assessment_input(
            input.assessment_policy.source_disposition(),
            control_disposition,
        );
        if expected != self.input {
            return Err(GenerationQualificationLicenseAssessmentError::Evidence(
                GenerationQualificationOperationContractError::RelationshipMismatch,
            ));
        }
        self.evidence
            .validate_against(evidence_relations(input), self.input)
            .map_err(GenerationQualificationLicenseAssessmentError::Evidence)?;
        self.revalidate_live_proof(cancellation)?;
        validate_static_closure(input)?;
        ensure_active(cancellation)
    }

    /// Freshly revalidates against a portable closure while keeping the retained
    /// structural proof and selected package lease opaque to the caller.
    ///
    /// # Errors
    ///
    /// Returns a content-free error for cancellation, live-proof drift, or any
    /// changed operation, projection, target, policy, approval, or license relation.
    pub fn revalidate_portable(
        &self,
        portable: GenerationQualificationLicensePortableRelations<'_>,
        cancellation: &CancellationToken,
    ) -> Result<(), GenerationQualificationLicenseAssessmentError> {
        self.revalidate_against(
            &GenerationQualificationLicenseAssessmentInput {
                operation_policy: portable.operation_policy,
                operation_policy_relations: portable.operation_policy_relations,
                operation_policy_input: portable.operation_policy_input,
                request_projection: portable.request_projection,
                request_projection_relations: portable.request_projection_relations,
                request_projection_entry_inputs: portable.request_projection_entry_inputs,
                target_generation_system: portable.target_generation_system,
                target_generation_system_relations: portable.target_generation_system_relations,
                assessment_policy: portable.assessment_policy,
                model_license_control: self.model_license_control,
                selected_model_package_lease: self.selected_model_package_lease,
                production_approval_policy: portable.production_approval_policy,
            },
            cancellation,
        )
    }
}

impl fmt::Debug for VerifiedGenerationQualificationLicenseAssessment<'_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedGenerationQualificationLicenseAssessment")
            .field("decision", &self.evidence.decision())
            .field("reason", &self.evidence.reason())
            .finish_non_exhaustive()
    }
}

fn compile_with_post_derivation<'proof, 'lease>(
    input: &GenerationQualificationLicenseAssessmentInput<'_, 'proof, 'lease>,
    cancellation: &CancellationToken,
    post_derivation: impl FnOnce(),
) -> Result<
    VerifiedGenerationQualificationLicenseAssessment<'proof, 'lease>,
    GenerationQualificationLicenseAssessmentError,
> {
    ensure_active(cancellation)?;
    validate_static_closure(input)?;
    let control_disposition = input
        .production_approval_policy
        .assess(
            input.model_license_control,
            input.selected_model_package_lease,
            ModelLicensePermission::LocalGeneration,
            cancellation,
        )
        .map_err(map_license_error)?;
    let assessment = assessment_input(
        input.assessment_policy.source_disposition(),
        control_disposition,
    );
    let evidence =
        GenerationQualificationLicenseEvidenceV1::new(evidence_relations(input), assessment)
            .map_err(GenerationQualificationLicenseAssessmentError::Evidence)?;
    post_derivation();
    input
        .model_license_control
        .revalidate(
            input.selected_model_package_lease,
            ModelLicensePermission::LocalGeneration,
            cancellation,
        )
        .map_err(map_license_error)?;
    evidence
        .validate_against(evidence_relations(input), assessment)
        .map_err(GenerationQualificationLicenseAssessmentError::Evidence)?;
    ensure_active(cancellation)?;
    Ok(VerifiedGenerationQualificationLicenseAssessment {
        evidence,
        input: assessment,
        model_license_control: input.model_license_control,
        selected_model_package_lease: input.selected_model_package_lease,
    })
}

fn validate_static_closure(
    input: &GenerationQualificationLicenseAssessmentInput<'_, '_, '_>,
) -> Result<(), GenerationQualificationLicenseAssessmentError> {
    input
        .operation_policy
        .validate_against(
            input.operation_policy_relations,
            input.operation_policy_input,
        )
        .map_err(GenerationQualificationLicenseAssessmentError::OperationPolicy)?;
    input
        .target_generation_system
        .validate_against(input.target_generation_system_relations)
        .map_err(GenerationQualificationLicenseAssessmentError::TargetSystem)?;

    input
        .request_projection
        .validate_against(
            input.request_projection_relations,
            input.request_projection_entry_inputs,
        )
        .map_err(GenerationQualificationLicenseAssessmentError::RequestProjection)?;

    validate_relationships(input)
}

fn validate_relationships(
    input: &GenerationQualificationLicenseAssessmentInput<'_, '_, '_>,
) -> Result<(), GenerationQualificationLicenseAssessmentError> {
    validate_operation_projection_relationships(input)?;
    validate_model_relationships(input)
}

fn validate_operation_projection_relationships(
    input: &GenerationQualificationLicenseAssessmentInput<'_, '_, '_>,
) -> Result<(), GenerationQualificationLicenseAssessmentError> {
    use GenerationQualificationLicenseAssessmentRelationship as Relationship;

    if input.operation_policy.target_generation_system_id()
        != input.target_generation_system.generation_system_id()
        || input
            .operation_policy_relations
            .target_system
            .generation_system
            .generation_system_id()
            != input.target_generation_system.generation_system_id()
    {
        return Err(GenerationQualificationLicenseAssessmentError::Relationship(
            Relationship::OperationTarget,
        ));
    }
    if input.request_projection.operation_policy_id()
        != input.operation_policy.operation_policy_id()
    {
        return Err(GenerationQualificationLicenseAssessmentError::Relationship(
            Relationship::ProjectionOperation,
        ));
    }
    if input.request_projection.target_generation_system_id()
        != input.target_generation_system.generation_system_id()
    {
        return Err(GenerationQualificationLicenseAssessmentError::Relationship(
            Relationship::ProjectionTarget,
        ));
    }
    if input
        .request_projection_relations
        .operation_policy
        .operation_policy_id()
        != input.operation_policy.operation_policy_id()
        || input
            .request_projection_relations
            .qualification_plan
            .qualification_plan_id()
            != input
                .operation_policy_relations
                .plan
                .qualification_plan_id()
        || input.request_projection_relations.suite.suite_manifest_id()
            != input.operation_policy_relations.suite.suite_manifest_id()
    {
        return Err(GenerationQualificationLicenseAssessmentError::Relationship(
            Relationship::ProjectionScope,
        ));
    }
    if input.request_projection_relations.planned_attempts
        != input.operation_policy_relations.planned_attempts
    {
        return Err(GenerationQualificationLicenseAssessmentError::Relationship(
            Relationship::ProjectionAttempts,
        ));
    }
    if input.operation_policy.license_assessment_policy_id() != input.assessment_policy.policy_id()
    {
        return Err(GenerationQualificationLicenseAssessmentError::Relationship(
            Relationship::AssessmentPolicy,
        ));
    }
    Ok(())
}

fn validate_model_relationships(
    input: &GenerationQualificationLicenseAssessmentInput<'_, '_, '_>,
) -> Result<(), GenerationQualificationLicenseAssessmentError> {
    use GenerationQualificationLicenseAssessmentRelationship as Relationship;

    if input.target_generation_system.model_artifact_set_id()
        != input.selected_model_package_lease.artifact_set_id()
    {
        return Err(GenerationQualificationLicenseAssessmentError::Relationship(
            Relationship::ModelArtifactSet,
        ));
    }
    if input.target_generation_system.model_package_manifest_id()
        != input
            .selected_model_package_lease
            .model_package_manifest_id()
    {
        return Err(GenerationQualificationLicenseAssessmentError::Relationship(
            Relationship::ModelPackage,
        ));
    }
    let selected_model = input.target_generation_system.model_artifact_id();
    if !input
        .selected_model_package_lease
        .private_view()
        .model_package_manifest()
        .members()
        .iter()
        .any(|member| {
            member.artifact_id() == selected_model
                && member.roles().iter().any(|role| {
                    matches!(
                        role,
                        ModelPackageMemberRole::ModelWeights
                            | ModelPackageMemberRole::ModelWeightShard
                    )
                })
        })
    {
        return Err(GenerationQualificationLicenseAssessmentError::Relationship(
            Relationship::ModelArtifact,
        ));
    }
    if input.model_license_control.foundation_id()
        != input.selected_model_package_lease.foundation_id()
    {
        return Err(GenerationQualificationLicenseAssessmentError::Relationship(
            Relationship::ModelFoundation,
        ));
    }
    if input.model_license_control.control_id()
        != input.assessment_policy.model_license_control_id()
    {
        return Err(GenerationQualificationLicenseAssessmentError::Relationship(
            Relationship::ModelLicenseControl,
        ));
    }
    if input.operation_policy.required_license_permission()
        != GenerationQualificationLicensePermissionV1::LocalGeneration
        || input.assessment_policy.permission()
            != GenerationQualificationLicensePermissionV1::LocalGeneration
        || input.model_license_control.permission() != ModelLicensePermission::LocalGeneration
    {
        return Err(GenerationQualificationLicenseAssessmentError::Relationship(
            Relationship::Permission,
        ));
    }
    Ok(())
}

const fn assessment_input(
    policy: GenerationQualificationAssessmentPolicySourceDisposition,
    control: ModelLicenseControlAssessmentDisposition,
) -> GenerationQualificationLicenseEvidenceV1Input {
    if matches!(
        (policy, control),
        (
            GenerationQualificationAssessmentPolicySourceDisposition::Approved,
            ModelLicenseControlAssessmentDisposition::Approved
        )
    ) {
        GenerationQualificationLicenseEvidenceV1Input {
            decision: GenerationQualificationLicenseDecisionV1::LocalUseOnly,
            reason: GenerationQualificationLicenseReasonV1::ApprovedLocalGeneration,
        }
    } else {
        GenerationQualificationLicenseEvidenceV1Input {
            decision: GenerationQualificationLicenseDecisionV1::Rejected,
            reason: GenerationQualificationLicenseReasonV1::ApprovalPolicyDenied,
        }
    }
}

fn evidence_relations<'a>(
    input: &'a GenerationQualificationLicenseAssessmentInput<'a, '_, '_>,
) -> GenerationQualificationLicenseEvidenceV1Relations<'a> {
    GenerationQualificationLicenseEvidenceV1Relations {
        operation_policy: input.operation_policy,
        request_projection: input.request_projection,
        target_generation_system: input.target_generation_system,
        target_generation_system_relations: input.target_generation_system_relations,
        model_package_foundation_id: input.model_license_control.foundation_id(),
        model_license_control_id: input.model_license_control.control_id(),
    }
}

fn map_license_error(
    error: ModelLicenseControlError,
) -> GenerationQualificationLicenseAssessmentError {
    if matches!(error, ModelLicenseControlError::Cancelled) {
        GenerationQualificationLicenseAssessmentError::Cancelled
    } else {
        GenerationQualificationLicenseAssessmentError::ModelLicenseControl(error)
    }
}

fn ensure_active(
    cancellation: &CancellationToken,
) -> Result<(), GenerationQualificationLicenseAssessmentError> {
    if cancellation.is_cancelled() {
        Err(GenerationQualificationLicenseAssessmentError::Cancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "generation_qualification_license_assessment/tests.rs"]
mod tests;
