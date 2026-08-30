//! Inert app authorities for resource and human policies denied before phase execution.
//!
//! These compilers are pure, bounded, in-memory derivations with no I/O. The outer
//! evaluation state-machine bracket owns cancellation and the absolute operation
//! deadline before and after this work; adding a second clock here would weaken that
//! single-deadline relationship.

use std::fmt;

use rewrite_model::{
    GenerationHumanAdjudicationEvidenceManifestV1,
    GenerationHumanAdjudicationEvidenceManifestV1Relations,
    GenerationHumanAdjudicationPolicyDenialRecordV1,
    GenerationHumanAdjudicationPolicyDenialRecordV1Relations,
    GenerationQualificationOperationPolicyV1, GenerationQualificationPhaseEvidenceError,
    GenerationQualificationPhaseScopeV1, GenerationQualificationPhaseStatusV1,
    GenerationResourceEvidenceManifestV1, GenerationResourceEvidenceManifestV1Relations,
    GenerationResourcePolicyDenialRecordV1, GenerationResourcePolicyDenialRecordV1Relations,
};
use rewrite_types::Digest;
use thiserror::Error;

use super::{
    GenerationQualificationPhasePolicyError, GenerationQualificationPhasePolicySourceDisposition,
    VerifiedGenerationQualificationHumanAdjudicationPolicy,
    VerifiedGenerationQualificationResourcePolicy,
};

/// Content-free failure while compiling or revalidating a denied phase authority.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum GenerationQualificationPhaseDenialError {
    /// The phase policy was source-approved and therefore cannot produce denial evidence.
    #[error("generation qualification phase policy was not denied")]
    PolicyNotDenied,
    /// The phase policy did not match the operation's exact phase-policy digest.
    #[error("generation qualification denied phase policy does not match the operation")]
    Policy(#[source] GenerationQualificationPhasePolicyError),
    /// The supplied target, plan, or suite did not match the operation policy.
    #[error("generation qualification denied phase scope does not match the operation")]
    ScopeMismatch,
    /// The inert denial record or phase manifest could not be derived or revalidated.
    #[error("generation qualification denied phase evidence does not match")]
    Evidence(#[source] GenerationQualificationPhaseEvidenceError),
}

/// Compiler for one exact resource-policy-denied phase authority.
#[derive(Clone, Copy, Debug, Default)]
pub struct GenerationQualificationResourcePolicyDeniedCompiler;

impl GenerationQualificationResourcePolicyDeniedCompiler {
    /// Derives an inert failed resource manifest from one exact source-denied policy.
    ///
    /// This compiler performs no launch, traffic, generation, measurement, approval,
    /// persistence, or qualification work.
    ///
    /// # Errors
    ///
    /// Returns a content-free error for an approved policy or any policy, operation,
    /// target, plan, suite, denial-record, or manifest substitution.
    pub fn compile<'records>(
        operation_policy: &GenerationQualificationOperationPolicyV1,
        scope: GenerationQualificationPhaseScopeV1<'records>,
        phase_policy: VerifiedGenerationQualificationResourcePolicy,
    ) -> Result<
        VerifiedGenerationQualificationResourcePolicyDenied<'records>,
        GenerationQualificationPhaseDenialError,
    > {
        validate_resource_policy(operation_policy, &phase_policy)?;
        validate_scope(operation_policy, scope)?;
        let denial_record = GenerationResourcePolicyDenialRecordV1::new(
            GenerationResourcePolicyDenialRecordV1Relations {
                scope,
                phase_policy_digest: phase_policy.policy_digest(),
            },
        )
        .map_err(GenerationQualificationPhaseDenialError::Evidence)?;
        let manifest = resource_manifest(scope, phase_policy.policy_digest(), &denial_record)?;
        let authority = VerifiedGenerationQualificationResourcePolicyDenied {
            denial_record,
            manifest,
            operation_policy: operation_policy.clone(),
            scope,
            phase_policy,
        };
        authority.revalidate()?;
        Ok(authority)
    }
}

/// Noncloneable and nonserializable authority for exact resource-policy denial.
///
/// The authority is inert. It grants no execution, traffic, evidence-collection,
/// persistence, positive qualification, or model-use capability.
///
/// ```compile_fail
/// use rewrite_app::VerifiedGenerationQualificationResourcePolicyDenied;
///
/// fn clone_authority(value: &VerifiedGenerationQualificationResourcePolicyDenied<'_>) {
///     let _forged: VerifiedGenerationQualificationResourcePolicyDenied<'_> = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_app::VerifiedGenerationQualificationResourcePolicyDenied;
///
/// fn serialize_authority(value: &VerifiedGenerationQualificationResourcePolicyDenied<'_>) {
///     let _bytes = serde_json::to_vec(value).expect("authority must not serialize");
/// }
/// ```
pub struct VerifiedGenerationQualificationResourcePolicyDenied<'records> {
    denial_record: GenerationResourcePolicyDenialRecordV1,
    manifest: GenerationResourceEvidenceManifestV1,
    operation_policy: GenerationQualificationOperationPolicyV1,
    scope: GenerationQualificationPhaseScopeV1<'records>,
    phase_policy: VerifiedGenerationQualificationResourcePolicy,
}

impl VerifiedGenerationQualificationResourcePolicyDenied<'_> {
    /// Returns the exact inert source-denial record.
    #[must_use]
    pub const fn denial_record(&self) -> &GenerationResourcePolicyDenialRecordV1 {
        &self.denial_record
    }

    /// Returns the exact one-item failed resource manifest.
    #[must_use]
    pub const fn manifest(&self) -> &GenerationResourceEvidenceManifestV1 {
        &self.manifest
    }

    /// Returns the exact preregistered resource-policy digest.
    #[must_use]
    pub const fn policy_digest(&self) -> &Digest {
        self.denial_record.phase_policy_digest()
    }

    /// Freshly revalidates the retained policy, operation, scope, record, and manifest.
    ///
    /// # Errors
    ///
    /// Returns a content-free error if any retained relationship is no longer exact.
    pub fn revalidate(&self) -> Result<(), GenerationQualificationPhaseDenialError> {
        let scope = self.scope;
        validate_resource_policy(&self.operation_policy, &self.phase_policy)?;
        validate_scope(&self.operation_policy, scope)?;
        self.denial_record
            .validate_against(GenerationResourcePolicyDenialRecordV1Relations {
                scope,
                phase_policy_digest: self.phase_policy.policy_digest(),
            })
            .map_err(GenerationQualificationPhaseDenialError::Evidence)?;
        validate_resource_manifest(
            &self.manifest,
            scope,
            self.phase_policy.policy_digest(),
            &self.denial_record,
        )
    }
}

impl fmt::Debug for VerifiedGenerationQualificationResourcePolicyDenied<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedGenerationQualificationResourcePolicyDenied")
            .field("status", &self.manifest.status())
            .finish_non_exhaustive()
    }
}

/// Compiler for one exact human-adjudication-policy-denied phase authority.
#[derive(Clone, Copy, Debug, Default)]
pub struct GenerationQualificationHumanAdjudicationPolicyDeniedCompiler;

impl GenerationQualificationHumanAdjudicationPolicyDeniedCompiler {
    /// Derives an inert failed human manifest from one exact source-denied policy.
    ///
    /// This compiler performs no launch, traffic, generation, review, approval,
    /// persistence, or qualification work.
    ///
    /// # Errors
    ///
    /// Returns a content-free error for an approved policy or any policy, operation,
    /// target, plan, suite, denial-record, or manifest substitution.
    pub fn compile<'records>(
        operation_policy: &GenerationQualificationOperationPolicyV1,
        scope: GenerationQualificationPhaseScopeV1<'records>,
        phase_policy: VerifiedGenerationQualificationHumanAdjudicationPolicy,
    ) -> Result<
        VerifiedGenerationQualificationHumanAdjudicationPolicyDenied<'records>,
        GenerationQualificationPhaseDenialError,
    > {
        validate_human_policy(operation_policy, &phase_policy)?;
        validate_scope(operation_policy, scope)?;
        let denial_record = GenerationHumanAdjudicationPolicyDenialRecordV1::new(
            GenerationHumanAdjudicationPolicyDenialRecordV1Relations {
                scope,
                phase_policy_digest: phase_policy.policy_digest(),
            },
        )
        .map_err(GenerationQualificationPhaseDenialError::Evidence)?;
        let manifest = human_manifest(scope, phase_policy.policy_digest(), &denial_record)?;
        let authority = VerifiedGenerationQualificationHumanAdjudicationPolicyDenied {
            denial_record,
            manifest,
            operation_policy: operation_policy.clone(),
            scope,
            phase_policy,
        };
        authority.revalidate()?;
        Ok(authority)
    }
}

/// Noncloneable and nonserializable authority for exact human-policy denial.
///
/// The authority is inert. It grants no execution, traffic, review, persistence,
/// positive qualification, or model-use capability.
///
/// ```compile_fail
/// use rewrite_app::VerifiedGenerationQualificationHumanAdjudicationPolicyDenied;
///
/// fn clone_authority(value: &VerifiedGenerationQualificationHumanAdjudicationPolicyDenied<'_>) {
///     let _forged: VerifiedGenerationQualificationHumanAdjudicationPolicyDenied<'_> =
///         value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_app::VerifiedGenerationQualificationHumanAdjudicationPolicyDenied;
///
/// fn serialize_authority(value: &VerifiedGenerationQualificationHumanAdjudicationPolicyDenied<'_>) {
///     let _bytes = serde_json::to_vec(value).expect("authority must not serialize");
/// }
/// ```
pub struct VerifiedGenerationQualificationHumanAdjudicationPolicyDenied<'records> {
    denial_record: GenerationHumanAdjudicationPolicyDenialRecordV1,
    manifest: GenerationHumanAdjudicationEvidenceManifestV1,
    operation_policy: GenerationQualificationOperationPolicyV1,
    scope: GenerationQualificationPhaseScopeV1<'records>,
    phase_policy: VerifiedGenerationQualificationHumanAdjudicationPolicy,
}

impl VerifiedGenerationQualificationHumanAdjudicationPolicyDenied<'_> {
    /// Returns the exact inert source-denial record.
    #[must_use]
    pub const fn denial_record(&self) -> &GenerationHumanAdjudicationPolicyDenialRecordV1 {
        &self.denial_record
    }

    /// Returns the exact one-item failed human-adjudication manifest.
    #[must_use]
    pub const fn manifest(&self) -> &GenerationHumanAdjudicationEvidenceManifestV1 {
        &self.manifest
    }

    /// Returns the exact preregistered human-adjudication-policy digest.
    #[must_use]
    pub const fn policy_digest(&self) -> &Digest {
        self.denial_record.phase_policy_digest()
    }

    /// Freshly revalidates the retained policy, operation, scope, record, and manifest.
    ///
    /// # Errors
    ///
    /// Returns a content-free error if any retained relationship is no longer exact.
    pub fn revalidate(&self) -> Result<(), GenerationQualificationPhaseDenialError> {
        let scope = self.scope;
        validate_human_policy(&self.operation_policy, &self.phase_policy)?;
        validate_scope(&self.operation_policy, scope)?;
        self.denial_record
            .validate_against(GenerationHumanAdjudicationPolicyDenialRecordV1Relations {
                scope,
                phase_policy_digest: self.phase_policy.policy_digest(),
            })
            .map_err(GenerationQualificationPhaseDenialError::Evidence)?;
        validate_human_manifest(
            &self.manifest,
            scope,
            self.phase_policy.policy_digest(),
            &self.denial_record,
        )
    }
}

impl fmt::Debug for VerifiedGenerationQualificationHumanAdjudicationPolicyDenied<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedGenerationQualificationHumanAdjudicationPolicyDenied")
            .field("status", &self.manifest.status())
            .finish_non_exhaustive()
    }
}

fn validate_resource_policy(
    operation_policy: &GenerationQualificationOperationPolicyV1,
    phase_policy: &VerifiedGenerationQualificationResourcePolicy,
) -> Result<(), GenerationQualificationPhaseDenialError> {
    require_denied(phase_policy.source_disposition())?;
    phase_policy
        .revalidate_operation_policy(operation_policy)
        .map_err(GenerationQualificationPhaseDenialError::Policy)
}

fn validate_human_policy(
    operation_policy: &GenerationQualificationOperationPolicyV1,
    phase_policy: &VerifiedGenerationQualificationHumanAdjudicationPolicy,
) -> Result<(), GenerationQualificationPhaseDenialError> {
    require_denied(phase_policy.source_disposition())?;
    phase_policy
        .revalidate_operation_policy(operation_policy)
        .map_err(GenerationQualificationPhaseDenialError::Policy)
}

fn require_denied(
    disposition: GenerationQualificationPhasePolicySourceDisposition,
) -> Result<(), GenerationQualificationPhaseDenialError> {
    match disposition {
        GenerationQualificationPhasePolicySourceDisposition::Denied => Ok(()),
        GenerationQualificationPhasePolicySourceDisposition::Approved => {
            Err(GenerationQualificationPhaseDenialError::PolicyNotDenied)
        }
    }
}

fn validate_scope(
    operation_policy: &GenerationQualificationOperationPolicyV1,
    scope: GenerationQualificationPhaseScopeV1<'_>,
) -> Result<(), GenerationQualificationPhaseDenialError> {
    if operation_policy.target_generation_system_id()
        != scope.generation_system.generation_system_id()
        || operation_policy.generation_qualification_plan_id()
            != scope.qualification_plan.qualification_plan_id()
        || operation_policy.suite_manifest_id() != scope.suite.suite_manifest_id()
    {
        Err(GenerationQualificationPhaseDenialError::ScopeMismatch)
    } else {
        Ok(())
    }
}

fn resource_manifest(
    scope: GenerationQualificationPhaseScopeV1<'_>,
    policy_digest: &Digest,
    record: &GenerationResourcePolicyDenialRecordV1,
) -> Result<GenerationResourceEvidenceManifestV1, GenerationQualificationPhaseDenialError> {
    let record_digest = record.resource_policy_denial_record_id().digest().clone();
    GenerationResourceEvidenceManifestV1::new(GenerationResourceEvidenceManifestV1Relations {
        scope,
        phase_policy_digest: policy_digest,
        evidence_record_digests: std::slice::from_ref(&record_digest),
        status: GenerationQualificationPhaseStatusV1::Failed,
    })
    .map_err(GenerationQualificationPhaseDenialError::Evidence)
}

fn human_manifest(
    scope: GenerationQualificationPhaseScopeV1<'_>,
    policy_digest: &Digest,
    record: &GenerationHumanAdjudicationPolicyDenialRecordV1,
) -> Result<GenerationHumanAdjudicationEvidenceManifestV1, GenerationQualificationPhaseDenialError>
{
    let record_digest = record
        .human_adjudication_policy_denial_record_id()
        .digest()
        .clone();
    GenerationHumanAdjudicationEvidenceManifestV1::new(
        GenerationHumanAdjudicationEvidenceManifestV1Relations {
            scope,
            phase_policy_digest: policy_digest,
            evidence_record_digests: std::slice::from_ref(&record_digest),
            status: GenerationQualificationPhaseStatusV1::Failed,
        },
    )
    .map_err(GenerationQualificationPhaseDenialError::Evidence)
}

fn validate_resource_manifest(
    manifest: &GenerationResourceEvidenceManifestV1,
    scope: GenerationQualificationPhaseScopeV1<'_>,
    policy_digest: &Digest,
    record: &GenerationResourcePolicyDenialRecordV1,
) -> Result<(), GenerationQualificationPhaseDenialError> {
    let record_digest = record.resource_policy_denial_record_id().digest().clone();
    let expected =
        GenerationResourceEvidenceManifestV1::new(GenerationResourceEvidenceManifestV1Relations {
            scope,
            phase_policy_digest: policy_digest,
            evidence_record_digests: std::slice::from_ref(&record_digest),
            status: GenerationQualificationPhaseStatusV1::Failed,
        })
        .map_err(GenerationQualificationPhaseDenialError::Evidence)?;
    if manifest == &expected {
        Ok(())
    } else {
        Err(GenerationQualificationPhaseDenialError::Evidence(
            GenerationQualificationPhaseEvidenceError::RelationshipMismatch,
        ))
    }
}

fn validate_human_manifest(
    manifest: &GenerationHumanAdjudicationEvidenceManifestV1,
    scope: GenerationQualificationPhaseScopeV1<'_>,
    policy_digest: &Digest,
    record: &GenerationHumanAdjudicationPolicyDenialRecordV1,
) -> Result<(), GenerationQualificationPhaseDenialError> {
    let record_digest = record
        .human_adjudication_policy_denial_record_id()
        .digest()
        .clone();
    let expected = GenerationHumanAdjudicationEvidenceManifestV1::new(
        GenerationHumanAdjudicationEvidenceManifestV1Relations {
            scope,
            phase_policy_digest: policy_digest,
            evidence_record_digests: std::slice::from_ref(&record_digest),
            status: GenerationQualificationPhaseStatusV1::Failed,
        },
    )
    .map_err(GenerationQualificationPhaseDenialError::Evidence)?;
    if manifest == &expected {
        Ok(())
    } else {
        Err(GenerationQualificationPhaseDenialError::Evidence(
            GenerationQualificationPhaseEvidenceError::RelationshipMismatch,
        ))
    }
}

#[cfg(test)]
pub(crate) mod tests;
