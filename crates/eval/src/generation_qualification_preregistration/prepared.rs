//! Atomically read-back prepared operation authority.

mod mandatory_finalization;
mod persistence;

pub(crate) use mandatory_finalization::PreparedGenerationQualificationMandatoryFinalizationError;

use std::{fmt, time::Instant};

use rewrite_app::{
    GenerationQualificationLicensePortableRelations,
    GenerationQualificationPlatformPortableRelations, ProductionModelLicenseApprovalPolicy,
    VerifiedGenerationQualificationLicenseAssessment,
    VerifiedGenerationQualificationLicenseAssessmentPolicy,
    VerifiedGenerationQualificationPlatformAssessment,
    VerifiedGenerationQualificationRequestProjectionV1,
};
use rewrite_model::{
    GenerationQualificationLicenseDecisionV1, GenerationQualificationLicenseEvidenceV1,
    GenerationQualificationLicenseEvidenceV1Input,
    GenerationQualificationLicenseEvidenceV1Relations, GenerationQualificationOperationPolicyV1,
    GenerationQualificationOperationPolicyV1Input,
    GenerationQualificationOperationPolicyV1Relations, GenerationQualificationPlatformEvidenceV1,
    GenerationQualificationPlatformEvidenceV1Input,
    GenerationQualificationPlatformEvidenceV1Relations, GenerationQualificationPlatformStatusV1,
    GenerationQualificationRequestProjectionEntryV1Input,
    GenerationQualificationRequestProjectionV1,
    GenerationQualificationRequestProjectionV1Relations,
};
use rewrite_model_store::{
    GenerationQualificationPlanFoundationV1,
    GenerationQualificationPreregistrationFoundationV1Input,
    GenerationQualificationPreregistrationReadback,
};
use rewrite_types::CancellationToken;

use super::{
    GenerationQualificationPreparationError, ProjectedGenerationQualificationOperation, check_gate,
    map_license_error, map_platform_error, map_projection_error,
    repository::GenerationQualificationPreregistrationRepository,
};

/// Traffic disposition derived from both exact app assessments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GenerationQualificationPreparationDisposition {
    /// Both exact app assessments permit local qualification traffic.
    TrafficEligible,
    /// Platform assessment rejected the operation.
    PlatformRejected,
    /// Platform passed, but model-license assessment rejected the operation.
    LicenseRejected,
}

struct PreregistrationReadbacks {
    plan_foundation: GenerationQualificationPlanFoundationV1,
    operation_policy: GenerationQualificationOperationPolicyV1,
    request_projection: GenerationQualificationRequestProjectionV1,
}

#[derive(Clone, Copy)]
pub(crate) struct PreparedGenerationQualificationValidationView<'view> {
    pub(super) operation_policy: &'view GenerationQualificationOperationPolicyV1,
    pub(super) operation_policy_relations: GenerationQualificationOperationPolicyV1Relations<'view>,
    pub(super) operation_policy_input: &'view GenerationQualificationOperationPolicyV1Input,
    pub(super) request_projection: &'view GenerationQualificationRequestProjectionV1,
    pub(super) request_projection_relations:
        GenerationQualificationRequestProjectionV1Relations<'view>,
    pub(super) request_projection_entry_inputs:
        &'view [GenerationQualificationRequestProjectionEntryV1Input],
    pub(super) platform_evidence: &'view GenerationQualificationPlatformEvidenceV1,
    pub(super) platform_evidence_relations:
        GenerationQualificationPlatformEvidenceV1Relations<'view>,
    pub(super) platform_evidence_input: GenerationQualificationPlatformEvidenceV1Input,
    pub(super) license_evidence: &'view GenerationQualificationLicenseEvidenceV1,
    pub(super) license_evidence_relations: GenerationQualificationLicenseEvidenceV1Relations<'view>,
    pub(super) license_evidence_input: GenerationQualificationLicenseEvidenceV1Input,
    pub(super) started: Instant,
    pub(super) deadline: Instant,
    pub(super) disposition: GenerationQualificationPreparationDisposition,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum PreparedGenerationQualificationValidationError<E> {
    Initial(GenerationQualificationPreparationError),
    Callback(E),
    Final(GenerationQualificationPreparationError),
    InitialAndFinal {
        initial: GenerationQualificationPreparationError,
        final_validation: GenerationQualificationPreparationError,
    },
    CallbackAndFinal {
        callback: E,
        final_validation: GenerationQualificationPreparationError,
    },
}

/// Noncloneable, nonserializable pretraffic qualification authority.
///
/// This value owns the exact plan-foundation and preregistration readbacks plus all
/// three retained app authorities. It grants no runtime launch, network, generation,
/// or traffic API.
///
/// ```compile_fail
/// use rewrite_eval::PreparedGenerationQualificationOperation;
/// fn cannot_clone(value: &PreparedGenerationQualificationOperation<'_, '_, '_, '_, '_>) {
///     let _forged = (*value).clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_eval::PreparedGenerationQualificationOperation;
/// fn require_serialize<T: serde::Serialize>(_: &T) {}
/// fn cannot_serialize(value: &PreparedGenerationQualificationOperation<'_, '_, '_, '_, '_>) {
///     require_serialize(value);
/// }
/// ```
pub struct PreparedGenerationQualificationOperation<'records, 'store, 'platform, 'proof, 'lease> {
    readbacks: PreregistrationReadbacks,
    stream: VerifiedGenerationQualificationRequestProjectionV1<'records, 'store>,
    platform: VerifiedGenerationQualificationPlatformAssessment<'platform>,
    license: VerifiedGenerationQualificationLicenseAssessment<'proof, 'lease>,
    license_assessment_policy: VerifiedGenerationQualificationLicenseAssessmentPolicy,
    production_approval_policy: ProductionModelLicenseApprovalPolicy,
    disposition: GenerationQualificationPreparationDisposition,
    deadline: Instant,
}

impl<'records, 'store, 'platform, 'proof, 'lease>
    PreparedGenerationQualificationOperation<'records, 'store, 'platform, 'proof, 'lease>
{
    #[expect(
        clippy::too_many_arguments,
        reason = "the state transition consumes one complete pretraffic authority closure"
    )]
    pub(super) fn finish_projected(
        projected: ProjectedGenerationQualificationOperation<'records, 'store>,
        repository: &mut GenerationQualificationPreregistrationRepository,
        foundation: GenerationQualificationPreregistrationFoundationV1Input<'_>,
        platform: VerifiedGenerationQualificationPlatformAssessment<'platform>,
        license: VerifiedGenerationQualificationLicenseAssessment<'proof, 'lease>,
        license_assessment_policy: VerifiedGenerationQualificationLicenseAssessmentPolicy,
        production_approval_policy: ProductionModelLicenseApprovalPolicy,
        cancellation: &CancellationToken,
    ) -> Result<Self, GenerationQualificationPreparationError> {
        let ProjectedGenerationQualificationOperation { mut stream } = projected;
        let deadline = stream.deadline();
        check_gate(deadline, cancellation)?;
        revalidate_stream(&mut stream, deadline, cancellation)?;
        repository.persist_foundation(
            foundation,
            stream.operation_policy_relations(),
            deadline,
            cancellation,
        )?;

        let (readbacks, _write_disposition) = repository.transact(
            stream.operation_policy(),
            stream.operation_policy_relations(),
            stream.operation_policy_input(),
            stream.projection(),
            stream.entry_inputs(),
            deadline,
            cancellation,
            |readback| {
                let readbacks = retain_readbacks(&stream, &readback)?;
                validate_readback_closure(&readbacks, &stream)?;
                revalidate_platform(&platform, &readbacks, &stream, deadline, cancellation)?;
                revalidate_license(
                    &license,
                    &license_assessment_policy,
                    &production_approval_policy,
                    &readbacks,
                    &stream,
                    deadline,
                    cancellation,
                )?;
                check_gate(deadline, cancellation)?;
                Ok(readbacks)
            },
        )?;
        let disposition = disposition(&platform, &license);
        Ok(Self {
            readbacks,
            stream,
            platform,
            license,
            license_assessment_policy,
            production_approval_policy,
            disposition,
            deadline,
        })
    }

    /// Returns the exact traffic disposition, with platform rejection precedence.
    #[must_use]
    pub const fn disposition(&self) -> GenerationQualificationPreparationDisposition {
        self.disposition
    }

    /// Returns the exact operation-policy repository readback.
    #[must_use]
    pub const fn operation_policy(&self) -> &GenerationQualificationOperationPolicyV1 {
        &self.readbacks.operation_policy
    }

    /// Returns the complete inert plan foundation observed in the preregistration transaction.
    #[must_use]
    pub const fn plan_foundation(&self) -> &GenerationQualificationPlanFoundationV1 {
        &self.readbacks.plan_foundation
    }

    /// Returns the exact request-projection repository readback.
    #[must_use]
    pub const fn request_projection(&self) -> &GenerationQualificationRequestProjectionV1 {
        &self.readbacks.request_projection
    }

    pub(super) const fn operation_deadline(&self) -> Instant {
        self.deadline
    }

    /// Freshly revalidates the complete retained pretraffic authority closure.
    ///
    /// # Errors
    ///
    /// Returns a content-redacted error for original deadline expiry,
    /// cancellation, source or request drift, readback mismatch, or changed app
    /// platform or license authority relationships.
    pub fn revalidate(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<(), GenerationQualificationPreparationError> {
        check_gate(self.deadline, cancellation)?;
        revalidate_stream(&mut self.stream, self.deadline, cancellation)?;
        let readback_result = validate_readback_closure(&self.readbacks, &self.stream);
        check_gate(self.deadline, cancellation)?;
        readback_result?;
        revalidate_platform(
            &self.platform,
            &self.readbacks,
            &self.stream,
            self.deadline,
            cancellation,
        )?;
        revalidate_license(
            &self.license,
            &self.license_assessment_policy,
            &self.production_approval_policy,
            &self.readbacks,
            &self.stream,
            self.deadline,
            cancellation,
        )?;
        let disposition_matches = disposition(&self.platform, &self.license) == self.disposition;
        check_gate(self.deadline, cancellation)?;
        if disposition_matches {
            Ok(())
        } else {
            Err(GenerationQualificationPreparationError::ReadbackMismatch)
        }
    }

    pub(super) fn with_validated_view<T, E>(
        &mut self,
        cancellation: &CancellationToken,
        use_view: impl for<'view> FnOnce(
            PreparedGenerationQualificationValidationView<'view>,
        ) -> Result<T, E>,
    ) -> Result<T, PreparedGenerationQualificationValidationError<E>> {
        let initial = self.revalidate(cancellation);
        let callback = if initial.is_ok() {
            Some(use_view(self.validation_view()))
        } else {
            None
        };
        let final_validation = self.revalidate(cancellation);
        match (initial, callback, final_validation) {
            (Err(initial), None, Err(final_validation)) => Err(
                PreparedGenerationQualificationValidationError::InitialAndFinal {
                    initial,
                    final_validation,
                },
            ),
            (Err(initial), None, Ok(())) => Err(
                PreparedGenerationQualificationValidationError::Initial(initial),
            ),
            (Ok(()), Some(Err(callback)), Err(final_validation)) => Err(
                PreparedGenerationQualificationValidationError::CallbackAndFinal {
                    callback,
                    final_validation,
                },
            ),
            (Ok(()), Some(Err(callback)), Ok(())) => Err(
                PreparedGenerationQualificationValidationError::Callback(callback),
            ),
            (Ok(()), Some(Ok(_)), Err(final_validation)) => Err(
                PreparedGenerationQualificationValidationError::Final(final_validation),
            ),
            (Ok(()), Some(Ok(value)), Ok(())) => Ok(value),
            (Err(_), Some(_), _) | (Ok(()), None, _) => {
                unreachable!("callback presence is determined by initial validation")
            }
        }
    }

    fn validation_view(&self) -> PreparedGenerationQualificationValidationView<'_> {
        let operation_policy_relations = self.stream.operation_policy_relations();
        let request_projection_relations =
            projection_relations(&self.readbacks.operation_policy, &self.stream);
        let target_generation_system = operation_policy_relations.target_system.generation_system;
        let target_generation_system_relations = operation_policy_relations.target_system.relations;
        PreparedGenerationQualificationValidationView {
            operation_policy: &self.readbacks.operation_policy,
            operation_policy_relations,
            operation_policy_input: self.stream.operation_policy_input(),
            request_projection: &self.readbacks.request_projection,
            request_projection_relations,
            request_projection_entry_inputs: self.stream.entry_inputs(),
            platform_evidence: self.platform.evidence(),
            platform_evidence_relations: GenerationQualificationPlatformEvidenceV1Relations {
                operation_policy: &self.readbacks.operation_policy,
                request_projection: &self.readbacks.request_projection,
                target_generation_system,
                target_generation_system_relations,
            },
            platform_evidence_input: self.platform.assessment_input(),
            license_evidence: self.license.portable_evidence(),
            license_evidence_relations: GenerationQualificationLicenseEvidenceV1Relations {
                operation_policy: &self.readbacks.operation_policy,
                request_projection: &self.readbacks.request_projection,
                target_generation_system,
                target_generation_system_relations,
                model_package_foundation_id: self
                    .license
                    .portable_evidence()
                    .model_package_foundation_id(),
                model_license_control_id: self
                    .license
                    .portable_evidence()
                    .model_license_control_id(),
            },
            license_evidence_input: self.license.assessment_input(),
            started: self.stream.started(),
            deadline: self.deadline,
            disposition: self.disposition,
        }
    }
}

impl fmt::Debug for PreparedGenerationQualificationOperation<'_, '_, '_, '_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedGenerationQualificationOperation")
            .field(
                "operation_policy_id",
                self.readbacks.operation_policy.operation_policy_id(),
            )
            .field(
                "request_projection_id",
                self.readbacks.request_projection.request_projection_id(),
            )
            .field("disposition", &self.disposition)
            .finish_non_exhaustive()
    }
}

fn retain_readbacks(
    stream: &VerifiedGenerationQualificationRequestProjectionV1<'_, '_>,
    readback: &GenerationQualificationPreregistrationReadback<'_>,
) -> Result<PreregistrationReadbacks, GenerationQualificationPreparationError> {
    let plan_foundation = readback.plan_foundation().clone();
    let operation_relations = stream.operation_policy_relations();
    if plan_foundation.plan() != operation_relations.plan
        || plan_foundation.suite() != operation_relations.suite
        || plan_foundation.repetitions() != operation_relations.repetitions
        || plan_foundation.planned_attempts() != operation_relations.planned_attempts
    {
        return Err(GenerationQualificationPreparationError::ReadbackMismatch);
    }
    let operation_policy = readback.operation_policy().clone();
    if &operation_policy != stream.operation_policy()
        || operation_policy.operation_policy_id() != stream.operation_policy().operation_policy_id()
    {
        return Err(GenerationQualificationPreparationError::ReadbackMismatch);
    }
    let request_projection = readback.request_projection().clone();
    if &request_projection != stream.projection()
        || request_projection.request_projection_id() != stream.projection().request_projection_id()
    {
        return Err(GenerationQualificationPreparationError::ReadbackMismatch);
    }
    Ok(PreregistrationReadbacks {
        plan_foundation,
        operation_policy,
        request_projection,
    })
}

fn validate_readback_closure(
    readbacks: &PreregistrationReadbacks,
    stream: &VerifiedGenerationQualificationRequestProjectionV1<'_, '_>,
) -> Result<(), GenerationQualificationPreparationError> {
    readbacks
        .operation_policy
        .validate_against(
            stream.operation_policy_relations(),
            stream.operation_policy_input(),
        )
        .map_err(|_| GenerationQualificationPreparationError::ReadbackMismatch)?;
    readbacks
        .request_projection
        .validate_against(
            projection_relations(&readbacks.operation_policy, stream),
            stream.entry_inputs(),
        )
        .map_err(|_| GenerationQualificationPreparationError::ReadbackMismatch)?;
    if &readbacks.operation_policy == stream.operation_policy()
        && &readbacks.request_projection == stream.projection()
    {
        Ok(())
    } else {
        Err(GenerationQualificationPreparationError::ReadbackMismatch)
    }
}

fn revalidate_stream(
    stream: &mut VerifiedGenerationQualificationRequestProjectionV1<'_, '_>,
    deadline: Instant,
    cancellation: &CancellationToken,
) -> Result<(), GenerationQualificationPreparationError> {
    let result = stream.revalidate(cancellation);
    check_gate(deadline, cancellation)?;
    result.map_err(map_projection_error)
}

fn revalidate_platform(
    platform: &VerifiedGenerationQualificationPlatformAssessment<'_>,
    readbacks: &PreregistrationReadbacks,
    stream: &VerifiedGenerationQualificationRequestProjectionV1<'_, '_>,
    deadline: Instant,
    cancellation: &CancellationToken,
) -> Result<(), GenerationQualificationPreparationError> {
    let result = platform.revalidate(platform_relations(readbacks, stream), cancellation);
    check_gate(deadline, cancellation)?;
    result.map_err(map_platform_error)
}

fn revalidate_license(
    license: &VerifiedGenerationQualificationLicenseAssessment<'_, '_>,
    assessment_policy: &VerifiedGenerationQualificationLicenseAssessmentPolicy,
    production_policy: &ProductionModelLicenseApprovalPolicy,
    readbacks: &PreregistrationReadbacks,
    stream: &VerifiedGenerationQualificationRequestProjectionV1<'_, '_>,
    deadline: Instant,
    cancellation: &CancellationToken,
) -> Result<(), GenerationQualificationPreparationError> {
    let result = license.revalidate_portable(
        license_relations(readbacks, stream, assessment_policy, production_policy),
        cancellation,
    );
    check_gate(deadline, cancellation)?;
    result.map_err(|error| map_license_error(&error))
}

fn platform_relations<'a>(
    readbacks: &'a PreregistrationReadbacks,
    stream: &'a VerifiedGenerationQualificationRequestProjectionV1<'_, '_>,
) -> GenerationQualificationPlatformPortableRelations<'a> {
    GenerationQualificationPlatformPortableRelations {
        operation_policy: &readbacks.operation_policy,
        operation_policy_relations: stream.operation_policy_relations(),
        operation_policy_input: stream.operation_policy_input(),
        request_projection: &readbacks.request_projection,
        request_projection_relations: projection_relations(&readbacks.operation_policy, stream),
        request_projection_entry_inputs: stream.entry_inputs(),
    }
}

fn license_relations<'a>(
    readbacks: &'a PreregistrationReadbacks,
    stream: &'a VerifiedGenerationQualificationRequestProjectionV1<'_, '_>,
    assessment_policy: &'a VerifiedGenerationQualificationLicenseAssessmentPolicy,
    production_policy: &'a ProductionModelLicenseApprovalPolicy,
) -> GenerationQualificationLicensePortableRelations<'a> {
    let operation_relations = stream.operation_policy_relations();
    GenerationQualificationLicensePortableRelations {
        operation_policy: &readbacks.operation_policy,
        operation_policy_relations: operation_relations,
        operation_policy_input: stream.operation_policy_input(),
        request_projection: &readbacks.request_projection,
        request_projection_relations: projection_relations(&readbacks.operation_policy, stream),
        request_projection_entry_inputs: stream.entry_inputs(),
        target_generation_system: operation_relations.target_system.generation_system,
        target_generation_system_relations: operation_relations.target_system.relations,
        assessment_policy,
        production_approval_policy: production_policy,
    }
}

fn projection_relations<'a>(
    operation_policy: &'a GenerationQualificationOperationPolicyV1,
    stream: &'a VerifiedGenerationQualificationRequestProjectionV1<'_, '_>,
) -> GenerationQualificationRequestProjectionV1Relations<'a> {
    let operation_relations = stream.operation_policy_relations();
    GenerationQualificationRequestProjectionV1Relations {
        operation_policy,
        qualification_plan: operation_relations.plan,
        suite: operation_relations.suite,
        planned_attempts: operation_relations.planned_attempts,
    }
}

fn disposition(
    platform: &VerifiedGenerationQualificationPlatformAssessment<'_>,
    license: &VerifiedGenerationQualificationLicenseAssessment<'_, '_>,
) -> GenerationQualificationPreparationDisposition {
    disposition_from_evidence(
        platform.evidence().status(),
        license.portable_evidence().decision(),
    )
}

const fn disposition_from_evidence(
    platform: GenerationQualificationPlatformStatusV1,
    license: GenerationQualificationLicenseDecisionV1,
) -> GenerationQualificationPreparationDisposition {
    if matches!(platform, GenerationQualificationPlatformStatusV1::Rejected) {
        GenerationQualificationPreparationDisposition::PlatformRejected
    } else if matches!(license, GenerationQualificationLicenseDecisionV1::Rejected) {
        GenerationQualificationPreparationDisposition::LicenseRejected
    } else {
        GenerationQualificationPreparationDisposition::TrafficEligible
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_disposition_has_platform_precedence() {
        assert_eq!(
            disposition_from_evidence(
                GenerationQualificationPlatformStatusV1::Rejected,
                GenerationQualificationLicenseDecisionV1::Rejected,
            ),
            GenerationQualificationPreparationDisposition::PlatformRejected
        );
        assert_eq!(
            disposition_from_evidence(
                GenerationQualificationPlatformStatusV1::Supported,
                GenerationQualificationLicenseDecisionV1::Rejected,
            ),
            GenerationQualificationPreparationDisposition::LicenseRejected
        );
        assert_eq!(
            disposition_from_evidence(
                GenerationQualificationPlatformStatusV1::Supported,
                GenerationQualificationLicenseDecisionV1::LocalUseOnly,
            ),
            GenerationQualificationPreparationDisposition::TrafficEligible
        );
    }
}
