//! App-owned compilation of one exact prelaunch candidate-attempt precursor.

use std::fmt;

use rewrite_inference::{
    GenerationRequest, SingleCandidateRequestMappingError, StructuredCompletionRequest,
    derive_single_candidate_structured_request,
};
use rewrite_model::{
    CandidateGenerationAttemptPrecursorId, CandidateGenerationAttemptPrecursorV1,
    CandidateOutputCeilingsV1, EffectiveRuntimeState, GenerationQualificationContractError,
    GenerationQualificationOperationPolicyV1, GenerationQualificationPlanV1,
    GenerationQualificationRequestProjectionEntryV1, GenerationSystemRecordV1,
    PlannedCandidateAttemptV1, RuntimeBuildIdentity, RuntimePackageManifest,
};
use rewrite_runtime_attestor::VerifiedFrozenExternalNativeComponentSet;
use rewrite_types::CancellationToken;
use thiserror::Error;

use crate::{
    ManagedOllamaModelAuthorityError, PackageAttestationError,
    ReleasedGenerationEffectivePackageV2, RuntimePackageLease, StaticModelInterpretationV1,
    VerifiedAdmittedRuntime, VerifiedGenerationSystemPolicy, VerifiedManagedGenerationPath,
    VerifiedManagedOllamaLaunchPlan,
};

mod validation;

/// Named exact inputs for one app-owned prelaunch relationship join.
pub struct CandidateAttemptPrecursorCompilationInput<'records, 'model, 'runtime, 'characterized> {
    /// Selected immutable qualification plan.
    pub qualification_plan: &'records GenerationQualificationPlanV1,
    /// Exact selected planned attempt.
    pub planned_attempt: &'records PlannedCandidateAttemptV1,
    /// Exact stable generation-system record named by the attempt.
    pub generation_system: &'records GenerationSystemRecordV1,
    /// Current semantic runtime-package manifest.
    pub runtime_manifest: &'records RuntimePackageManifest,
    /// Exact live runtime-package lease retained through the later launch.
    pub runtime_package: &'runtime mut RuntimePackageLease,
    /// Exact runtime build derived from the current runtime manifest.
    pub runtime_build: &'records RuntimeBuildIdentity,
    /// Opaque admitted-runtime authority.
    pub admitted_runtime: &'records VerifiedAdmittedRuntime,
    /// Opaque reviewed managed-generation path.
    pub generation_path: &'records VerifiedManagedGenerationPath,
    /// Independently verified external native-component closure.
    pub frozen_components: &'records VerifiedFrozenExternalNativeComponentSet,
    /// Current model input and exact local-generation license launch authority.
    pub launch_plan: VerifiedManagedOllamaLaunchPlan<'model>,
    /// Prior cleanup-gated portable package characterization.
    pub characterized_package: &'characterized ReleasedGenerationEffectivePackageV2,
    /// Complete stable runtime-state expectation, not only its ID.
    pub expected_runtime_state: &'records EffectiveRuntimeState,
    /// Complete static interpretation derived from the current model launch facts.
    pub static_model: &'records StaticModelInterpretationV1,
    /// Exact provider-neutral request, moved into the resulting capability.
    pub generation_request: GenerationRequest,
    /// Exact predeclared output ceilings from the selected plan entry.
    pub output_ceilings: CandidateOutputCeilingsV1,
    /// Independently approved exact construction policy.
    pub generation_policy: &'records VerifiedGenerationSystemPolicy,
}

/// Compiler for the exact app-owned candidate-attempt precursor join.
#[derive(Clone, Copy, Debug, Default)]
pub struct CandidateAttemptPrecursorCompiler;

impl CandidateAttemptPrecursorCompiler {
    /// Revalidates both retained packages and compiles one prelaunch capability.
    ///
    /// No caller-supplied structured request, typed owner ID, or installation
    /// generation is accepted. Those values are derived from the exact inputs.
    ///
    /// # Errors
    ///
    /// Returns [`CandidateAttemptPrecursorCompilationError`] for package drift,
    /// model-license drift, unsupported request mapping, or any substituted
    /// portable or opaque relationship.
    pub fn compile<'model, 'runtime, 'characterized>(
        input: CandidateAttemptPrecursorCompilationInput<'_, 'model, 'runtime, 'characterized>,
        cancellation: &CancellationToken,
    ) -> Result<
        VerifiedManagedCandidateAttemptPrecursor<'model, 'runtime, 'characterized>,
        CandidateAttemptPrecursorCompilationError,
    > {
        input
            .runtime_package
            .revalidate(cancellation)
            .map_err(|error| {
                CandidateAttemptPrecursorCompilationError::RuntimeRevalidation(Box::new(error))
            })?;
        input
            .launch_plan
            .revalidate_for_candidate_precursor(cancellation)
            .map_err(|error| {
                CandidateAttemptPrecursorCompilationError::ModelRevalidation(Box::new(error))
            })?;

        let output_policy = validation::output_policy(input.output_ceilings)?;
        let structured_request =
            derive_single_candidate_structured_request(&input.generation_request, output_policy)?;
        validation::validate(&input, &structured_request)?;

        let precursor = CandidateGenerationAttemptPrecursorV1::new(
            input.qualification_plan,
            input.planned_attempt,
            input.generation_system,
            rewrite_model::CandidateGenerationAttemptPrecursorV1Input {
                runtime_installation_generation: input
                    .runtime_package
                    .installation_key()
                    .installation_generation(),
                model_installation_generation: input.launch_plan.model_installation_generation(),
                structured_request_binding_id: structured_request.structured_request_binding_id(),
            },
        )?;

        Ok(VerifiedManagedCandidateAttemptPrecursor {
            runtime_package: input.runtime_package,
            launch_plan: input.launch_plan,
            characterized_package: input.characterized_package,
            planned_attempt: input.planned_attempt.clone(),
            generation_system: input.generation_system.clone(),
            expected_runtime_state: input.expected_runtime_state.clone(),
            generation_request: input.generation_request,
            structured_request,
            precursor,
        })
    }
}

/// Noncloneable prelaunch capability retaining every exact consuming input.
///
/// It grants no qualification, model-use, response, or cleanup claim.
///
/// ```compile_fail
/// use rewrite_app::VerifiedManagedCandidateAttemptPrecursor;
///
/// fn clone_capability(value: &VerifiedManagedCandidateAttemptPrecursor<'_, '_, '_>) {
///     let _forged: VerifiedManagedCandidateAttemptPrecursor<'_, '_, '_> = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_app::VerifiedManagedCandidateAttemptPrecursor;
///
/// fn serialize_capability(value: &VerifiedManagedCandidateAttemptPrecursor<'_, '_, '_>) {
///     let _bytes = serde_json::to_vec(value).expect("capability must not serialize");
/// }
/// ```
pub struct VerifiedManagedCandidateAttemptPrecursor<'model, 'runtime, 'characterized> {
    runtime_package: &'runtime mut RuntimePackageLease,
    launch_plan: VerifiedManagedOllamaLaunchPlan<'model>,
    characterized_package: &'characterized ReleasedGenerationEffectivePackageV2,
    planned_attempt: PlannedCandidateAttemptV1,
    generation_system: GenerationSystemRecordV1,
    expected_runtime_state: EffectiveRuntimeState,
    generation_request: GenerationRequest,
    structured_request: StructuredCompletionRequest,
    precursor: CandidateGenerationAttemptPrecursorV1,
}

impl VerifiedManagedCandidateAttemptPrecursor<'_, '_, '_> {
    /// Returns the inert portable precursor.
    #[must_use]
    pub fn precursor(&self) -> &CandidateGenerationAttemptPrecursorV1 {
        &self.precursor
    }

    /// Returns the exact provider-neutral request retained for the later runner.
    #[must_use]
    pub fn generation_request(&self) -> &GenerationRequest {
        &self.generation_request
    }

    /// Returns the internally derived single-candidate wire request.
    #[must_use]
    pub fn structured_request(&self) -> &StructuredCompletionRequest {
        &self.structured_request
    }

    /// Returns the cleanup-gated portable package characterization.
    #[must_use]
    pub const fn characterized_package(&self) -> &ReleasedGenerationEffectivePackageV2 {
        self.characterized_package
    }

    /// Returns a read-only view of the exact current launch authority.
    #[must_use]
    pub const fn launch_plan(&self) -> &VerifiedManagedOllamaLaunchPlan<'_> {
        &self.launch_plan
    }

    /// Returns a read-only view of the exact retained runtime package.
    #[must_use]
    pub const fn runtime_package(&self) -> &RuntimePackageLease {
        self.runtime_package
    }
}

impl fmt::Debug for VerifiedManagedCandidateAttemptPrecursor<'_, '_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedManagedCandidateAttemptPrecursor")
            .field("precursor_id", self.precursor.precursor_id())
            .field(
                "runtime_installation_generation",
                &self.precursor.runtime_installation_generation(),
            )
            .field(
                "model_installation_generation",
                &self.precursor.model_installation_generation(),
            )
            .finish_non_exhaustive()
    }
}

/// Nonforgeable consuming seam into one retained managed execution bracket.
///
/// The handoff contains no caller-selected typed IDs. It can only be obtained by
/// consuming a successfully compiled precursor capability.
pub struct CandidateAttemptPrecursorRunnerHandoff<'model, 'runtime, 'characterized> {
    runtime_package: &'runtime mut RuntimePackageLease,
    launch_plan: VerifiedManagedOllamaLaunchPlan<'model>,
    characterized_package: &'characterized ReleasedGenerationEffectivePackageV2,
    planned_attempt: PlannedCandidateAttemptV1,
    generation_system: GenerationSystemRecordV1,
    expected_runtime_state: EffectiveRuntimeState,
    generation_request: GenerationRequest,
    structured_request: StructuredCompletionRequest,
    precursor: CandidateGenerationAttemptPrecursorV1,
}

/// Content-redacted failure while binding a candidate handoff to one operation request.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum CandidateAttemptOperationScopeError {
    /// Cancellation was observed while checking the retained handoff.
    #[error("candidate-attempt operation-scope validation was cancelled")]
    Cancelled,
    /// The handoff did not match the exact operation, attempt, and projected request.
    #[error("candidate-attempt operation scope does not match")]
    Mismatch,
}

#[derive(Clone, Copy)]
struct CandidateAttemptOperationScopeFacts {
    relationships: [bool; 4],
}

impl CandidateAttemptOperationScopeFacts {
    const fn is_exact(self) -> bool {
        let [operation, attempt, projection, precursor] = self.relationships;
        operation && attempt && projection && precursor
    }
}

impl<'model, 'runtime, 'characterized>
    CandidateAttemptPrecursorRunnerHandoff<'model, 'runtime, 'characterized>
{
    /// Returns the exact inert precursor identity retained for execution.
    #[must_use]
    pub const fn precursor_id(&self) -> &CandidateGenerationAttemptPrecursorId {
        self.precursor.precursor_id()
    }

    /// Returns the exact inert precursor retained for execution.
    ///
    /// This record grants no launch, runtime, model, or live-session authority.
    #[must_use]
    pub const fn precursor(&self) -> &CandidateGenerationAttemptPrecursorV1 {
        &self.precursor
    }

    /// Returns the exact specialized model package retained by the launch plan.
    #[must_use]
    pub const fn model_package(&self) -> &'model crate::VerifiedManagedOllamaModelPackageLease {
        self.launch_plan.package()
    }

    /// Revalidates this handoff against one exact preregistered operation request.
    ///
    /// # Errors
    ///
    /// Returns a content-redacted error for cancellation or any substituted
    /// operation, planned attempt, system, provider-neutral request, or wire request.
    pub fn revalidate_operation_request_scope(
        &self,
        operation_policy: &GenerationQualificationOperationPolicyV1,
        planned_attempt: &PlannedCandidateAttemptV1,
        projection: &GenerationQualificationRequestProjectionEntryV1,
        cancellation: &CancellationToken,
    ) -> Result<(), CandidateAttemptOperationScopeError> {
        if cancellation.is_cancelled() {
            return Err(CandidateAttemptOperationScopeError::Cancelled);
        }
        let system_id = self.generation_system.generation_system_id();
        let system_is_in_operation = system_id == operation_policy.target_generation_system_id()
            || system_id == operation_policy.baseline_generation_system_id();
        let matches = CandidateAttemptOperationScopeFacts {
            relationships: [
                system_is_in_operation
                    && operation_policy.generation_qualification_plan_id()
                        == self.precursor.qualification_plan_id()
                    && operation_policy.suite_manifest_id()
                        == self.planned_attempt.suite_manifest_id(),
                planned_attempt == &self.planned_attempt
                    && planned_attempt.generation_system_id() == system_id,
                projection.planned_attempt_id() == planned_attempt.planned_attempt_id()
                    && projection.generation_system_id() == system_id
                    && projection.generation_request_binding_id()
                        == &self.generation_request.generation_request_binding_id()
                    && projection.structured_completion_request_binding_id()
                        == &self.structured_request.structured_request_binding_id(),
                self.precursor.planned_attempt_id() == planned_attempt.planned_attempt_id()
                    && self.precursor.structured_request_binding_id()
                        == projection.structured_completion_request_binding_id(),
            ],
        }
        .is_exact();
        if cancellation.is_cancelled() {
            return Err(CandidateAttemptOperationScopeError::Cancelled);
        }
        if matches {
            Ok(())
        } else {
            Err(CandidateAttemptOperationScopeError::Mismatch)
        }
    }

    /// Consumes the handoff into the exact raw inputs needed by the eval-owned runner.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        &'runtime mut RuntimePackageLease,
        VerifiedManagedOllamaLaunchPlan<'model>,
        &'characterized ReleasedGenerationEffectivePackageV2,
        PlannedCandidateAttemptV1,
        GenerationSystemRecordV1,
        EffectiveRuntimeState,
        GenerationRequest,
        StructuredCompletionRequest,
        CandidateGenerationAttemptPrecursorV1,
    ) {
        (
            self.runtime_package,
            self.launch_plan,
            self.characterized_package,
            self.planned_attempt,
            self.generation_system,
            self.expected_runtime_state,
            self.generation_request,
            self.structured_request,
            self.precursor,
        )
    }
}

impl<'model, 'runtime, 'characterized>
    VerifiedManagedCandidateAttemptPrecursor<'model, 'runtime, 'characterized>
{
    /// Consumes this capability into the only managed-runner handoff.
    #[must_use]
    pub fn into_runner_handoff(
        self,
    ) -> CandidateAttemptPrecursorRunnerHandoff<'model, 'runtime, 'characterized> {
        CandidateAttemptPrecursorRunnerHandoff {
            runtime_package: self.runtime_package,
            launch_plan: self.launch_plan,
            characterized_package: self.characterized_package,
            planned_attempt: self.planned_attempt,
            generation_system: self.generation_system,
            expected_runtime_state: self.expected_runtime_state,
            generation_request: self.generation_request,
            structured_request: self.structured_request,
            precursor: self.precursor,
        }
    }
}

/// Failure to compile one exact app-owned candidate-attempt precursor.
#[derive(Debug, Error)]
pub enum CandidateAttemptPrecursorCompilationError {
    /// Current retained runtime bytes or managed-tree identity changed.
    #[error("candidate-attempt runtime package revalidation failed")]
    RuntimeRevalidation(#[source] Box<PackageAttestationError>),
    /// Current specialized model foundation or exact license authority changed.
    #[error("candidate-attempt model authority revalidation failed")]
    ModelRevalidation(#[source] Box<ManagedOllamaModelAuthorityError>),
    /// The provider-neutral request cannot map to the one supported wire request.
    #[error("candidate-attempt request mapping failed")]
    RequestMapping(#[from] SingleCandidateRequestMappingError),
    /// A stable or opaque input relationship was substituted.
    #[error("candidate-attempt precursor relationship is invalid")]
    RelationshipMismatch,
    /// The inert model-layer precursor rejected the selected plan closure.
    #[error("candidate-attempt precursor contract rejected the input")]
    Precursor(#[from] GenerationQualificationContractError),
}

#[cfg(test)]
#[path = "candidate_attempt_precursor/tests.rs"]
pub(crate) mod tests;

#[cfg(all(feature = "test-support", not(test)))]
#[expect(
    dead_code,
    unused_imports,
    reason = "the synthetic integration seam reuses a strict subset of app test support"
)]
#[path = "candidate_attempt_precursor/tests/support.rs"]
pub(crate) mod synthetic_test_support;
