//! Batch-specific managed judge effective-package derivation and release.

use std::fmt;

use rewrite_model::{
    ArtifactSetManifest, CandidateJudgeRequestAggregateV1, EffectivePackageEvidenceV2,
    EffectiveRuntimeState, GenerationSystemRecordV1, RuntimeBuildIdentity, RuntimePackageManifest,
};
use rewrite_runtime_attestor::VerifiedFrozenExternalNativeComponentSet;
use rewrite_runtime_isolation::PreparedIsolation;
use rewrite_types::CancellationToken;

use crate::{
    ManagedOllamaIsolationLease, ModelLicenseControlId, ModelPackageFoundationId,
    ReleasedGenerationEffectivePackageV2, RuntimePackageLease, StaticModelInterpretationV1,
    VerifiedAdmittedRuntime, VerifiedManagedGenerationPath,
    VerifiedManagedJudgeObservationAuthority, VerifiedManagedOllamaModelPackageLease,
};

#[path = "managed_judge_effective_package/error.rs"]
mod error;
#[path = "managed_judge_effective_package/release.rs"]
mod release;
#[path = "managed_judge_effective_package/released.rs"]
mod released;
#[path = "managed_judge_effective_package/validation.rs"]
mod validation;

pub use error::{
    ManagedJudgeEffectivePackageDerivationError, ManagedJudgeEffectivePackageFinalValidationError,
    ManagedJudgeEffectivePackagePlanError, ManagedJudgeEffectivePackageReleaseError,
};
pub use released::ReleasedManagedJudgeEffectivePackageV2;

use crate::generation_effective_package::{ManagedJudgeBatchInputs, derive_managed_judge_batch};
use release::finalize;

/// Exact named inputs for one batch-specific managed judge package derivation.
pub struct ManagedJudgeEffectivePackagePlanInput<'records, 'model, 'runtime, 'characterized> {
    /// Complete schedule-wide observer authority, consumed by this plan.
    pub observation_authority: VerifiedManagedJudgeObservationAuthority,
    /// Exact schedule-order request identities from the precursor and eval handoff.
    pub request_aggregate: CandidateJudgeRequestAggregateV1,
    /// Exact separately loaded judge generation system.
    pub judge_system: GenerationSystemRecordV1,
    /// Complete expected portable runtime state from the precursor.
    pub expected_runtime_state: EffectiveRuntimeState,
    /// Exact current runtime manifest.
    pub runtime_manifest: &'records RuntimePackageManifest,
    /// Exact retained runtime package authority.
    pub runtime_package: &'runtime mut RuntimePackageLease,
    /// Exact current runtime build.
    pub runtime_build: &'records RuntimeBuildIdentity,
    /// Opaque admitted-runtime authority retained by the precursor.
    pub admitted_runtime: &'records VerifiedAdmittedRuntime,
    /// Opaque managed generation-path authority retained by the precursor.
    pub generation_path: &'records VerifiedManagedGenerationPath,
    /// Exact frozen external native-component closure.
    pub frozen_components: &'records VerifiedFrozenExternalNativeComponentSet,
    /// Exact precursor-selected prepared isolation authority.
    pub prepared_isolation: &'records PreparedIsolation,
    /// Exact specialized model package retained by the managed bracket.
    pub model_package: &'model VerifiedManagedOllamaModelPackageLease,
    /// Complete precursor-derived static model interpretation.
    pub static_model: &'records StaticModelInterpretationV1,
    /// Prior cleanup-gated package characterization named by the judge system.
    pub characterized_package: &'characterized ReleasedGenerationEffectivePackageV2,
    /// Exact runtime installation generation captured by the precursor.
    pub runtime_installation_generation: u64,
    /// Exact model installation generation captured by the precursor.
    pub model_installation_generation: u64,
    /// Still-live managed isolation bracket covering every scheduled attempt.
    pub managed_ollama: ManagedOllamaIsolationLease<'model>,
}

/// Exact relationship rejected by batch-specific derivation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagedJudgeEffectivePackageRelationship {
    /// Plan, schedule, request, or judge-system lineage differed.
    PortableLineage,
    /// Runtime manifest, build, admission, path, or frozen closure differed.
    Runtime,
    /// Prepared isolation or the live managed bracket differed.
    Isolation,
    /// Model package, static binding, foundation, or license differed.
    Model,
    /// A retained attempt cursor, request, response, receipt, or ordinal differed.
    Attempt,
    /// A retained attempt did not contain a real managed effective state.
    RealEffectiveState,
    /// A retained effective state or schedule aggregate differed.
    EffectiveState,
    /// Freshly derived package evidence differed from characterized or system evidence.
    Evidence,
}

/// Noncloneable pending cleanup-gated batch package capability.
///
/// ```compile_fail
/// use rewrite_app::VerifiedManagedJudgeEffectivePackagePlan;
/// fn require_clone<T: Clone>() {}
/// require_clone::<VerifiedManagedJudgeEffectivePackagePlan<'static, 'static>>();
/// ```
///
/// ```compile_fail
/// use rewrite_app::VerifiedManagedJudgeEffectivePackagePlan;
/// fn serialize_plan(value: &VerifiedManagedJudgeEffectivePackagePlan<'_, '_>) {
///     let _bytes = serde_json::to_vec(value).unwrap();
/// }
/// ```
pub struct VerifiedManagedJudgeEffectivePackagePlan<'model, 'runtime> {
    managed_ollama: ManagedOllamaIsolationLease<'model>,
    model_package: &'model VerifiedManagedOllamaModelPackageLease,
    runtime_package: &'runtime mut RuntimePackageLease,
    artifact_set: ArtifactSetManifest,
    runtime_build: RuntimeBuildIdentity,
    expected_runtime_state: EffectiveRuntimeState,
    evidence: EffectivePackageEvidenceV2,
    observation_authority: VerifiedManagedJudgeObservationAuthority,
    request_aggregate: CandidateJudgeRequestAggregateV1,
    judge_system: GenerationSystemRecordV1,
    runtime_installation_generation: u64,
    model_installation_generation: u64,
    foundation_id: ModelPackageFoundationId,
    license_control_id: ModelLicenseControlId,
    first_response_ordinal: u64,
    last_response_ordinal: u64,
}

impl<'model, 'runtime> VerifiedManagedJudgeEffectivePackagePlan<'model, 'runtime> {
    /// Validates and derives one pending batch package result.
    ///
    /// Any error after acquisition closes the managed bracket and independently
    /// runs model, runtime, and final evidence/authority validation.
    ///
    /// # Errors
    ///
    /// Returns the primary failure plus every mandatory finalization failure.
    pub fn prepare(
        input: ManagedJudgeEffectivePackagePlanInput<'_, 'model, 'runtime, '_>,
        cancellation: &CancellationToken,
    ) -> Result<Self, ManagedJudgeEffectivePackagePlanError> {
        Self::prepare_with_post_derivation(input, cancellation, || {})
    }

    fn prepare_with_post_derivation(
        input: ManagedJudgeEffectivePackagePlanInput<'_, 'model, 'runtime, '_>,
        cancellation: &CancellationToken,
        post_derivation: impl FnOnce(),
    ) -> Result<Self, ManagedJudgeEffectivePackagePlanError> {
        match Self::prepare_inner(input, cancellation, post_derivation) {
            Ok(plan) => Ok(plan),
            Err(failure) => {
                let PrepareFailure {
                    primary,
                    input,
                    evidence,
                } = *failure;
                let finalization = finalize(
                    input.managed_ollama,
                    input.model_package,
                    input.runtime_package,
                    &input.observation_authority,
                    input.characterized_package.evidence(),
                    evidence.as_ref(),
                    input.model_package.private_view().artifact_set_manifest(),
                    input.runtime_build,
                    &input.expected_runtime_state,
                );
                Err(ManagedJudgeEffectivePackagePlanError {
                    primary,
                    finalization: finalization.into_error(),
                })
            }
        }
    }

    fn prepare_inner<'records, 'characterized>(
        input: ManagedJudgeEffectivePackagePlanInput<'records, 'model, 'runtime, 'characterized>,
        cancellation: &CancellationToken,
        post_derivation: impl FnOnce(),
    ) -> Result<Self, Box<PrepareFailure<'records, 'model, 'runtime, 'characterized>>> {
        let operation = (|| {
            ensure_active(cancellation)?;
            input
                .managed_ollama
                .revalidate_model_package(cancellation)
                .map_err(|error| {
                    ManagedJudgeEffectivePackageDerivationError::ModelRevalidation(Box::new(error))
                })?;
            input
                .runtime_package
                .revalidate(cancellation)
                .map_err(|error| {
                    ManagedJudgeEffectivePackageDerivationError::RuntimeRevalidation(Box::new(
                        error,
                    ))
                })?;
            validation::validate_input(&input, cancellation)?;
            let derived = derive_managed_judge_batch(
                &ManagedJudgeBatchInputs {
                    runtime_manifest: input.runtime_manifest,
                    model_package: input.model_package,
                    runtime_build: input.runtime_build,
                    effective_state: &input.expected_runtime_state,
                    admitted_runtime: input.admitted_runtime,
                    generation_path: input.generation_path,
                    frozen_components: input.frozen_components,
                    prepared_isolation: input.prepared_isolation,
                    license_control_id: input.managed_ollama.model_license_control_id(),
                },
                cancellation,
            )
            .map_err(|error| {
                ManagedJudgeEffectivePackageDerivationError::Evidence(Box::new(error))
            })?;
            Ok(derived)
        })();
        let derived = match operation {
            Ok(derived) => derived,
            Err(primary) => {
                return Err(Box::new(PrepareFailure {
                    primary,
                    input,
                    evidence: None,
                }));
            }
        };
        post_derivation();
        if let Err(error) =
            validation::validate_derived(&input, &derived.artifact_set, &derived.evidence)
        {
            return Err(Box::new(PrepareFailure {
                primary: error,
                input,
                evidence: Some(derived.evidence),
            }));
        }
        if let Err(error) = ensure_active(cancellation) {
            return Err(Box::new(PrepareFailure {
                primary: error,
                input,
                evidence: Some(derived.evidence),
            }));
        }
        let bindings = input.observation_authority.completed_sequence().bindings();
        let first_response_ordinal = bindings
            .first()
            .expect("validated judge schedule is nonempty")
            .first_response_ordinal();
        let last_response_ordinal = bindings
            .last()
            .expect("validated judge schedule is nonempty")
            .last_response_ordinal();
        Ok(Self {
            foundation_id: input.model_package.foundation_id().clone(),
            license_control_id: input.managed_ollama.model_license_control_id().clone(),
            managed_ollama: input.managed_ollama,
            model_package: input.model_package,
            runtime_package: input.runtime_package,
            artifact_set: derived.artifact_set,
            runtime_build: input.runtime_build.clone(),
            expected_runtime_state: input.expected_runtime_state,
            evidence: derived.evidence,
            observation_authority: input.observation_authority,
            request_aggregate: input.request_aggregate,
            judge_system: input.judge_system,
            runtime_installation_generation: input.runtime_installation_generation,
            model_installation_generation: input.model_installation_generation,
            first_response_ordinal,
            last_response_ordinal,
        })
    }

    /// Closes the bracket and releases the distinct schedule-specific capability.
    ///
    /// # Errors
    ///
    /// Returns every independently observed cleanup, model, runtime, and final
    /// evidence/authority validation failure.
    pub fn release(
        self,
    ) -> Result<ReleasedManagedJudgeEffectivePackageV2, ManagedJudgeEffectivePackageReleaseError>
    {
        let Self {
            managed_ollama,
            model_package,
            runtime_package,
            artifact_set,
            runtime_build,
            expected_runtime_state,
            evidence,
            observation_authority,
            request_aggregate,
            judge_system,
            runtime_installation_generation,
            model_installation_generation,
            foundation_id,
            license_control_id,
            first_response_ordinal,
            last_response_ordinal,
        } = self;
        let failures = finalize(
            managed_ollama,
            model_package,
            runtime_package,
            &observation_authority,
            &evidence,
            Some(&evidence),
            &artifact_set,
            &runtime_build,
            &expected_runtime_state,
        );
        if let Some(error) = failures.into_error() {
            return Err(error);
        }
        Ok(ReleasedManagedJudgeEffectivePackageV2 {
            evidence,
            artifact_set,
            runtime_build,
            observation_authority,
            request_aggregate,
            judge_system,
            expected_runtime_state,
            runtime_installation_generation,
            model_installation_generation,
            foundation_id,
            license_control_id,
            first_response_ordinal,
            last_response_ordinal,
        })
    }
}

struct PrepareFailure<'records, 'model, 'runtime, 'characterized> {
    primary: ManagedJudgeEffectivePackageDerivationError,
    input: ManagedJudgeEffectivePackagePlanInput<'records, 'model, 'runtime, 'characterized>,
    evidence: Option<EffectivePackageEvidenceV2>,
}

impl fmt::Debug for VerifiedManagedJudgeEffectivePackagePlan<'_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedManagedJudgeEffectivePackagePlan")
            .field("judge_plan_id", self.observation_authority.judge_plan_id())
            .field(
                "judge_schedule_id",
                self.observation_authority.judge_schedule_id(),
            )
            .field("attempt_count", &self.observation_authority.attempt_count())
            .finish_non_exhaustive()
    }
}

fn ensure_active(
    cancellation: &CancellationToken,
) -> Result<(), ManagedJudgeEffectivePackageDerivationError> {
    if cancellation.is_cancelled() {
        Err(ManagedJudgeEffectivePackageDerivationError::Cancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "managed_judge_effective_package/tests.rs"]
mod tests;
