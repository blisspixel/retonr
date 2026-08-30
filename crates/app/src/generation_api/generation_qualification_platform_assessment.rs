//! App-owned pretraffic platform assessment for generation qualification.

use std::fmt;

use rewrite_model::{
    GenerationQualificationOperationContractError, GenerationQualificationOperationPolicyV1,
    GenerationQualificationOperationPolicyV1Input,
    GenerationQualificationOperationPolicyV1Relations, GenerationQualificationPlatformEvidenceV1,
    GenerationQualificationPlatformEvidenceV1Input,
    GenerationQualificationPlatformEvidenceV1Relations, GenerationQualificationPlatformReasonV1,
    GenerationQualificationPlatformStatusV1, GenerationQualificationRequestProjectionEntryV1Input,
    GenerationQualificationRequestProjectionV1,
    GenerationQualificationRequestProjectionV1Relations, RuntimeAbi, RuntimeArchitecture,
    RuntimeOperatingSystem, RuntimeTarget,
};
use rewrite_runtime_attestor::VerifiedFrozenExternalNativeComponentSet;
use rewrite_types::CancellationToken;
use thiserror::Error;

use crate::{
    CurrentHostEnvironmentError, GenerationQualificationAssessmentPolicySourceDisposition,
    GenerationSystemPolicyPermission, GenerationSystemPolicyPurpose, VerifiedAdmittedRuntime,
    VerifiedCurrentHostEnvironment, VerifiedGenerationQualificationPlatformAssessmentPolicy,
    VerifiedGenerationSystemPolicy, VerifiedManagedGenerationPath,
};

#[path = "generation_qualification_platform_assessment/current_host.rs"]
mod current_host;
use current_host::{current_host_input, validate_current_host_static};

/// Complete portable relationship closure for one platform assessment.
#[derive(Clone, Copy)]
pub struct GenerationQualificationPlatformPortableRelations<'a> {
    /// Exact preregistered operation policy.
    pub operation_policy: &'a GenerationQualificationOperationPolicyV1,
    /// Complete typed relations used to rederive the operation policy.
    pub operation_policy_relations: GenerationQualificationOperationPolicyV1Relations<'a>,
    /// Independently retained operation-policy input.
    pub operation_policy_input: &'a GenerationQualificationOperationPolicyV1Input,
    /// Exact complete request projection.
    pub request_projection: &'a GenerationQualificationRequestProjectionV1,
    /// Complete typed relations used to rederive the request projection.
    pub request_projection_relations: GenerationQualificationRequestProjectionV1Relations<'a>,
    /// Independently derived request-projection inputs in exact plan order.
    pub request_projection_entry_inputs:
        &'a [GenerationQualificationRequestProjectionEntryV1Input],
}

impl GenerationQualificationPlatformPortableRelations<'_> {
    fn evidence_relations(&self) -> GenerationQualificationPlatformEvidenceV1Relations<'_> {
        GenerationQualificationPlatformEvidenceV1Relations {
            operation_policy: self.operation_policy,
            request_projection: self.request_projection,
            target_generation_system: self
                .operation_policy_relations
                .target_system
                .generation_system,
            target_generation_system_relations: self
                .operation_policy_relations
                .target_system
                .relations,
        }
    }

    fn target(&self) -> RuntimeTarget {
        self.operation_policy_relations
            .target_system
            .relations
            .runtime_package_manifest
            .target()
    }
}

/// Exact app-owned authorities required for the reviewed native profile.
#[derive(Clone, Copy)]
pub struct GenerationQualificationReviewedPlatformAuthorities<'a> {
    /// Exact admitted runtime authority.
    pub admitted_runtime: &'a VerifiedAdmittedRuntime,
    /// Exact separately reviewed managed generation path.
    pub generation_path: &'a VerifiedManagedGenerationPath,
    /// Exact independently verified external native-component closure.
    pub frozen_components: &'a VerifiedFrozenExternalNativeComponentSet,
    /// Exact approved generation-system policy for managed candidate generation.
    pub generation_system_policy: &'a VerifiedGenerationSystemPolicy,
    /// Structurally verified platform assessment policy.
    pub assessment_policy: &'a VerifiedGenerationQualificationPlatformAssessmentPolicy,
}

/// Noncloneable app-owned result of one exact pretraffic platform assessment.
///
/// The contained portable evidence is inert. This authority grants no launch,
/// request, generation, live-use, or qualification authority.
///
/// ```compile_fail
/// use rewrite_app::VerifiedGenerationQualificationPlatformAssessment;
///
/// fn clone_authority(value: &VerifiedGenerationQualificationPlatformAssessment<'_>) {
///     let _forged: VerifiedGenerationQualificationPlatformAssessment<'_> = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_app::VerifiedGenerationQualificationPlatformAssessment;
///
/// fn serialize_authority(value: &VerifiedGenerationQualificationPlatformAssessment<'_>) {
///     let _bytes = serde_json::to_vec(value).expect("authority must not serialize");
/// }
/// ```
pub struct VerifiedGenerationQualificationPlatformAssessment<'authority> {
    evidence: GenerationQualificationPlatformEvidenceV1,
    input: GenerationQualificationPlatformEvidenceV1Input,
    retained: RetainedPlatformAssessment<'authority>,
}

enum RetainedPlatformAssessment<'authority> {
    Intrinsic,
    Reviewed {
        authorities: GenerationQualificationReviewedPlatformAuthorities<'authority>,
        current_host: &'authority VerifiedCurrentHostEnvironment,
    },
    PolicyDenied {
        authorities: GenerationQualificationReviewedPlatformAuthorities<'authority>,
    },
}

impl VerifiedGenerationQualificationPlatformAssessment<'_> {
    /// Returns the inert portable evidence owned by this authority.
    #[must_use]
    pub const fn evidence(&self) -> &GenerationQualificationPlatformEvidenceV1 {
        &self.evidence
    }

    /// Returns the independently derived closed assessment input.
    #[must_use]
    pub const fn assessment_input(&self) -> GenerationQualificationPlatformEvidenceV1Input {
        self.input
    }

    /// Freshly revalidates the portable closure and every retained reviewed owner.
    ///
    /// The reviewed path re-observes the current host and requires the newly derived
    /// closed assessment input to equal the retained evidence exactly. Intrinsic and
    /// source-policy-denied results recheck their deterministic rejection path.
    ///
    /// # Errors
    ///
    /// Returns a content-free error for cancellation, current-host drift, or any
    /// changed portable, policy, runtime, path, component, or system relationship.
    pub fn revalidate(
        &self,
        portable: GenerationQualificationPlatformPortableRelations<'_>,
        cancellation: &CancellationToken,
    ) -> Result<(), GenerationQualificationPlatformAssessmentError> {
        ensure_assessment_active(cancellation)?;
        validate_portable(portable)?;
        match self.retained {
            RetainedPlatformAssessment::Intrinsic => {
                if intrinsic_rejection_reason(portable.target()) != Some(self.input.reason)
                    || self.input.status != GenerationQualificationPlatformStatusV1::Rejected
                {
                    return Err(
                        GenerationQualificationPlatformAssessmentError::InvalidPortableClosure,
                    );
                }
            }
            RetainedPlatformAssessment::Reviewed {
                authorities,
                current_host,
            } => {
                validate_reviewed_authorities(portable, authorities)?;
                if authorities.assessment_policy.source_disposition()
                    != GenerationQualificationAssessmentPolicySourceDisposition::Approved
                {
                    return Err(GenerationQualificationPlatformAssessmentError::AssessmentPolicyDispositionMismatch);
                }
                current_host
                    .revalidate_current(cancellation)
                    .map_err(map_current_host_error)?;
                validate_current_host_static(portable, current_host)?;
                if current_host_input(portable, current_host) != self.input {
                    return Err(GenerationQualificationPlatformAssessmentError::CurrentHostDrift);
                }
            }
            RetainedPlatformAssessment::PolicyDenied { authorities } => {
                validate_reviewed_authorities(portable, authorities)?;
                if authorities.assessment_policy.source_disposition()
                    != GenerationQualificationAssessmentPolicySourceDisposition::Denied
                    || self.input.status != GenerationQualificationPlatformStatusV1::Rejected
                    || self.input.reason
                        != GenerationQualificationPlatformReasonV1::AssessmentPolicyDenied
                {
                    return Err(GenerationQualificationPlatformAssessmentError::AssessmentPolicyDispositionMismatch);
                }
            }
        }
        self.evidence
            .validate_against(portable.evidence_relations(), self.input)
            .map_err(map_portable_error)?;
        validate_portable(portable)?;
        ensure_assessment_active(cancellation)
    }
}

impl fmt::Debug for VerifiedGenerationQualificationPlatformAssessment<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedGenerationQualificationPlatformAssessment")
            .field("platform_evidence_id", self.evidence.platform_evidence_id())
            .field("status", &self.input.status)
            .field("reason", &self.input.reason)
            .finish_non_exhaustive()
    }
}

/// App-owned compiler for deterministic pretraffic platform evidence.
#[derive(Clone, Copy, Debug, Default)]
pub struct GenerationQualificationPlatformAssessmentCompiler;

impl GenerationQualificationPlatformAssessmentCompiler {
    /// Assesses an intrinsically unsupported target without acquiring managed owners.
    ///
    /// Rejection precedence is operating system, architecture, then ABI.
    ///
    /// # Errors
    ///
    /// Returns an error when the portable closure is invalid or the target is the
    /// reviewed Linux x86-64 GNU-libc tuple and therefore needs the reviewed path.
    pub fn assess_intrinsic(
        portable: GenerationQualificationPlatformPortableRelations<'_>,
    ) -> Result<
        VerifiedGenerationQualificationPlatformAssessment<'static>,
        GenerationQualificationPlatformAssessmentError,
    > {
        validate_portable(portable)?;
        let reason = intrinsic_rejection_reason(portable.target())
            .ok_or(GenerationQualificationPlatformAssessmentError::ReviewedProfileRequired)?;
        derive_and_revalidate_intrinsic(
            portable,
            GenerationQualificationPlatformEvidenceV1Input {
                status: GenerationQualificationPlatformStatusV1::Rejected,
                reason,
            },
        )
    }

    /// Assesses the reviewed Linux x86-64 GNU-libc profile.
    ///
    /// This positive-capable path requires the exact retained current-host
    /// authority and an approved assessment policy.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid portable or owner relationships, an intrinsic
    /// target, current-host unavailability, mismatch, or drift.
    pub fn assess_reviewed<'authority>(
        portable: GenerationQualificationPlatformPortableRelations<'_>,
        authorities: GenerationQualificationReviewedPlatformAuthorities<'authority>,
        current_host: &'authority VerifiedCurrentHostEnvironment,
        cancellation: &CancellationToken,
    ) -> Result<
        VerifiedGenerationQualificationPlatformAssessment<'authority>,
        GenerationQualificationPlatformAssessmentError,
    > {
        ensure_assessment_active(cancellation)?;
        validate_portable(portable)?;
        if intrinsic_rejection_reason(portable.target()).is_some() {
            return Err(GenerationQualificationPlatformAssessmentError::IntrinsicTarget);
        }
        validate_reviewed_authorities(portable, authorities)?;
        if authorities.assessment_policy.source_disposition()
            == GenerationQualificationAssessmentPolicySourceDisposition::Denied
        {
            return Err(
                GenerationQualificationPlatformAssessmentError::AssessmentPolicyDispositionMismatch,
            );
        }

        current_host
            .revalidate_current(cancellation)
            .map_err(map_current_host_error)?;
        validate_current_host_static(portable, current_host)?;
        let input = current_host_input(portable, current_host);
        let authority = derive_evidence(
            portable,
            input,
            RetainedPlatformAssessment::Reviewed {
                authorities,
                current_host,
            },
        )?;

        validate_portable(portable)?;
        validate_reviewed_authorities(portable, authorities)?;
        current_host
            .revalidate_current(cancellation)
            .map_err(map_current_host_error)?;
        validate_current_host_static(portable, current_host)?;
        authority
            .evidence
            .validate_against(portable.evidence_relations(), input)
            .map_err(map_portable_error)?;
        ensure_assessment_active(cancellation)?;
        Ok(authority)
    }

    /// Derives deterministic negative evidence for an exact source-denied policy.
    ///
    /// This path deliberately acquires no current-host authority.
    ///
    /// # Errors
    ///
    /// Returns an error for cancellation, invalid portable or owner relations,
    /// an intrinsic target, or an assessment policy that is not source-denied.
    pub fn assess_policy_denied<'authority>(
        portable: GenerationQualificationPlatformPortableRelations<'_>,
        authorities: GenerationQualificationReviewedPlatformAuthorities<'authority>,
        cancellation: &CancellationToken,
    ) -> Result<
        VerifiedGenerationQualificationPlatformAssessment<'authority>,
        GenerationQualificationPlatformAssessmentError,
    > {
        ensure_assessment_active(cancellation)?;
        validate_portable(portable)?;
        if intrinsic_rejection_reason(portable.target()).is_some() {
            return Err(GenerationQualificationPlatformAssessmentError::IntrinsicTarget);
        }
        validate_reviewed_authorities(portable, authorities)?;
        if authorities.assessment_policy.source_disposition()
            != GenerationQualificationAssessmentPolicySourceDisposition::Denied
        {
            return Err(
                GenerationQualificationPlatformAssessmentError::AssessmentPolicyDispositionMismatch,
            );
        }
        let input = GenerationQualificationPlatformEvidenceV1Input {
            status: GenerationQualificationPlatformStatusV1::Rejected,
            reason: GenerationQualificationPlatformReasonV1::AssessmentPolicyDenied,
        };
        let authority = derive_evidence(
            portable,
            input,
            RetainedPlatformAssessment::PolicyDenied { authorities },
        )?;
        validate_portable(portable)?;
        validate_reviewed_authorities(portable, authorities)?;
        authority
            .evidence
            .validate_against(portable.evidence_relations(), input)
            .map_err(map_portable_error)?;
        ensure_assessment_active(cancellation)?;
        Ok(authority)
    }
}

/// Failure while deriving one app-owned platform assessment authority.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum GenerationQualificationPlatformAssessmentError {
    /// The operation policy, projection, target system, or exact relation input failed.
    #[error("generation qualification portable platform closure is invalid")]
    InvalidPortableClosure,
    /// The reviewed path was requested for an intrinsically unsupported target.
    #[error("generation qualification target requires intrinsic platform rejection")]
    IntrinsicTarget,
    /// The intrinsic path was requested for the reviewed native target.
    #[error("generation qualification target requires reviewed platform assessment")]
    ReviewedProfileRequired,
    /// One reviewed runtime, path, component, system-policy, or assessment-policy owner differed.
    #[error("generation qualification reviewed platform authority binding is invalid")]
    InvalidReviewedAuthorityBinding,
    /// The caller selected the wrong reviewed path for the policy disposition.
    #[error(
        "generation qualification platform policy disposition does not match the selected path"
    )]
    AssessmentPolicyDispositionMismatch,
    /// The current-host observer failed operationally or could not form exact evidence.
    #[error("generation qualification current-host platform observation is unavailable")]
    CurrentHostObservationUnavailable,
    /// The observed runtime target, operating system, or architecture differed.
    #[error("generation qualification current-host platform does not match the target")]
    CurrentHostMismatch,
    /// The current-host observation changed during evidence derivation.
    #[error("generation qualification current-host platform observation drifted")]
    CurrentHostDrift,
    /// Assessment was cancelled at a mandatory checkpoint.
    #[error("generation qualification platform assessment was cancelled")]
    Cancelled,
}

fn validate_portable(
    portable: GenerationQualificationPlatformPortableRelations<'_>,
) -> Result<(), GenerationQualificationPlatformAssessmentError> {
    portable
        .operation_policy
        .validate_against(
            portable.operation_policy_relations,
            portable.operation_policy_input,
        )
        .map_err(map_portable_error)?;
    portable
        .request_projection
        .validate_against(
            portable.request_projection_relations,
            portable.request_projection_entry_inputs,
        )
        .map_err(map_portable_error)?;
    if portable.request_projection_relations.operation_policy != portable.operation_policy
        || portable.request_projection_relations.qualification_plan
            != portable.operation_policy_relations.plan
        || portable.request_projection_relations.suite != portable.operation_policy_relations.suite
        || portable.request_projection_relations.planned_attempts
            != portable.operation_policy_relations.planned_attempts
    {
        return Err(GenerationQualificationPlatformAssessmentError::InvalidPortableClosure);
    }
    Ok(())
}

fn validate_reviewed_authorities(
    portable: GenerationQualificationPlatformPortableRelations<'_>,
    authorities: GenerationQualificationReviewedPlatformAuthorities<'_>,
) -> Result<(), GenerationQualificationPlatformAssessmentError> {
    let system = portable
        .operation_policy_relations
        .target_system
        .generation_system;
    let relations = portable.operation_policy_relations.target_system.relations;
    let package = relations.runtime_package_manifest;
    let admitted = authorities.admitted_runtime;
    let path = authorities.generation_path;
    let frozen = authorities.frozen_components;
    let policy = authorities.generation_system_policy;
    let platform_policy = authorities.assessment_policy;

    let owners_match = admitted.runtime_admission_join_id() == *system.runtime_admission_join_id()
        && admitted.runtime_package_manifest_id() == system.runtime_package_manifest_id()
        && admitted.runtime_package_manifest_id() == &package.runtime_package_manifest_id()
        && admitted.frozen_external_component_set_id() == frozen.frozen_set_id()
        && path.managed_generation_path_id() == *system.managed_generation_path_id()
        && path.matches_runtime(
            admitted,
            package,
            path.runtime_version(),
            frozen.frozen_set_id(),
        )
        && frozen.runtime_package_manifest_id() == system.runtime_package_manifest_id()
        && frozen.frozen_external_component_set_id() == *system.frozen_external_component_set_id();
    if !owners_match
        || policy.permission() != GenerationSystemPolicyPermission::ConstructGenerationSystem
        || policy.purpose() != GenerationSystemPolicyPurpose::ManagedCandidateGeneration
        || !system_policy_matches(policy, system)
        || platform_policy.policy_id() != portable.operation_policy.platform_assessment_policy_id()
        || platform_policy.bindings().runtime_target != portable.target()
        || platform_policy.bindings().operating_system_digest != *system.operating_system_digest()
        || platform_policy.bindings().architecture_digest != *system.architecture_digest()
        || platform_policy.bindings().execution_class_digest != *system.execution_class_digest()
        || platform_policy.bindings().hardware_envelope_digest != *system.hardware_envelope_digest()
    {
        return Err(
            GenerationQualificationPlatformAssessmentError::InvalidReviewedAuthorityBinding,
        );
    }
    Ok(())
}

fn system_policy_matches(
    policy: &VerifiedGenerationSystemPolicy,
    system: &rewrite_model::GenerationSystemRecordV1,
) -> bool {
    let bindings = policy.bindings();
    bindings.strategy_digest() == system.strategy_digest()
        && bindings.planner_digest() == system.planner_digest()
        && bindings.validator_digest() == system.validator_digest()
        && bindings.adapter_digest() == system.adapter_digest()
        && bindings.prompt_digest() == system.prompt_digest()
        && bindings.output_schema_digest() == system.output_schema_digest()
        && bindings.request_policy_digest() == system.request_policy_digest()
        && bindings.language_digest() == system.language_digest()
        && bindings.mode_digest() == system.mode_digest()
        && bindings.format_digest() == system.format_digest()
        && bindings.operating_system_digest() == system.operating_system_digest()
        && bindings.architecture_digest() == system.architecture_digest()
        && bindings.execution_class_digest() == system.execution_class_digest()
        && bindings.hardware_envelope_digest() == system.hardware_envelope_digest()
}

fn intrinsic_rejection_reason(
    target: RuntimeTarget,
) -> Option<GenerationQualificationPlatformReasonV1> {
    if target.operating_system() != RuntimeOperatingSystem::Linux {
        Some(GenerationQualificationPlatformReasonV1::UnsupportedOperatingSystem)
    } else if target.architecture() != RuntimeArchitecture::X86_64 {
        Some(GenerationQualificationPlatformReasonV1::UnsupportedArchitecture)
    } else if target.abi() != RuntimeAbi::LinuxGnuLibc {
        Some(GenerationQualificationPlatformReasonV1::UnsupportedAbi)
    } else {
        None
    }
}

fn derive_and_revalidate_intrinsic(
    portable: GenerationQualificationPlatformPortableRelations<'_>,
    input: GenerationQualificationPlatformEvidenceV1Input,
) -> Result<
    VerifiedGenerationQualificationPlatformAssessment<'static>,
    GenerationQualificationPlatformAssessmentError,
> {
    let authority = derive_evidence(portable, input, RetainedPlatformAssessment::Intrinsic)?;
    validate_portable(portable)?;
    authority
        .evidence
        .validate_against(portable.evidence_relations(), input)
        .map_err(map_portable_error)?;
    Ok(authority)
}

fn derive_evidence<'authority>(
    portable: GenerationQualificationPlatformPortableRelations<'_>,
    input: GenerationQualificationPlatformEvidenceV1Input,
    retained: RetainedPlatformAssessment<'authority>,
) -> Result<
    VerifiedGenerationQualificationPlatformAssessment<'authority>,
    GenerationQualificationPlatformAssessmentError,
> {
    let evidence =
        GenerationQualificationPlatformEvidenceV1::new(portable.evidence_relations(), input)
            .map_err(map_portable_error)?;
    Ok(VerifiedGenerationQualificationPlatformAssessment {
        evidence,
        input,
        retained,
    })
}

const fn map_portable_error(
    _error: GenerationQualificationOperationContractError,
) -> GenerationQualificationPlatformAssessmentError {
    GenerationQualificationPlatformAssessmentError::InvalidPortableClosure
}

const fn map_current_host_error(
    error: CurrentHostEnvironmentError,
) -> GenerationQualificationPlatformAssessmentError {
    match error {
        CurrentHostEnvironmentError::Cancelled => {
            GenerationQualificationPlatformAssessmentError::Cancelled
        }
        CurrentHostEnvironmentError::ObservationDrift => {
            GenerationQualificationPlatformAssessmentError::CurrentHostDrift
        }
        CurrentHostEnvironmentError::UnsupportedPlatform
        | CurrentHostEnvironmentError::ObservationUnavailable
        | CurrentHostEnvironmentError::ConstrainedEnvironment
        | CurrentHostEnvironmentError::InvalidObservation => {
            GenerationQualificationPlatformAssessmentError::CurrentHostObservationUnavailable
        }
    }
}

fn ensure_assessment_active(
    cancellation: &CancellationToken,
) -> Result<(), GenerationQualificationPlatformAssessmentError> {
    if cancellation.is_cancelled() {
        Err(GenerationQualificationPlatformAssessmentError::Cancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "generation_qualification_platform_assessment/tests.rs"]
mod tests;
