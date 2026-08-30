//! App-owned compilation of one exact managed candidate-judge precursor.

use std::fmt;

use rewrite_model::{
    CandidateJudgePlanV1, CandidateJudgeRequestAggregateV1, CandidateJudgeScheduleV1,
    EffectiveRuntimeState, GenerationSystemRecordV1, RuntimeBuildIdentity, RuntimePackageManifest,
};
use rewrite_runtime_attestor::VerifiedFrozenExternalNativeComponentSet;
use rewrite_runtime_isolation::PreparedIsolation;
use rewrite_types::CancellationToken;

use crate::{
    ReleasedGenerationEffectivePackageV2, RuntimePackageLease, StaticModelInterpretationV1,
    VerifiedAdmittedRuntime, VerifiedGenerationSystemPolicy, VerifiedManagedGenerationPath,
    VerifiedManagedOllamaLaunchPlan,
};

#[path = "managed_judge_precursor/error.rs"]
mod error;
mod validation;

pub use error::ManagedJudgePrecursorCompilationError;

/// Named exact inputs for one app-owned managed-judge prelaunch join.
pub struct ManagedJudgePrecursorCompilationInput<'records, 'model, 'runtime, 'characterized> {
    /// Exact portable judge plan produced before runtime acquisition.
    pub judge_plan: &'records CandidateJudgePlanV1,
    /// Exact complete two-presentation schedule.
    pub judge_schedule: &'records CandidateJudgeScheduleV1,
    /// Exact content-free request identities in schedule order.
    ///
    /// The app compiler rederives this aggregate from its supplied ordered IDs. It
    /// does not prove those IDs came from eligible candidate material; the supported
    /// runner compares it with the eval-owned ready handoff.
    pub request_aggregate: &'records CandidateJudgeRequestAggregateV1,
    /// Separately loaded stable judge generation system.
    pub judge_system: &'records GenerationSystemRecordV1,
    /// Current semantic runtime-package manifest.
    pub runtime_manifest: &'records RuntimePackageManifest,
    /// Exact live runtime-package lease retained through the later run.
    pub runtime_package: &'runtime mut RuntimePackageLease,
    /// Runtime build derived from the current runtime manifest.
    pub runtime_build: &'records RuntimeBuildIdentity,
    /// Opaque all-pass admitted-runtime authority.
    pub admitted_runtime: &'records VerifiedAdmittedRuntime,
    /// Opaque reviewed managed-generation path.
    pub generation_path: &'records VerifiedManagedGenerationPath,
    /// Independently verified external native-component closure.
    pub frozen_components: &'records VerifiedFrozenExternalNativeComponentSet,
    /// Exact prepared isolation authority selected before the managed launch.
    pub prepared_isolation: &'records PreparedIsolation,
    /// Current model input and exact local-generation license launch authority.
    pub launch_plan: VerifiedManagedOllamaLaunchPlan<'model>,
    /// Prior cleanup-gated stable package characterization.
    pub characterized_package: &'characterized ReleasedGenerationEffectivePackageV2,
    /// Complete stable runtime-state expectation, not only its ID.
    pub expected_runtime_state: &'records EffectiveRuntimeState,
    /// Complete static interpretation derived from current model launch facts.
    pub static_model: &'records StaticModelInterpretationV1,
    /// Independently approved exact construction policy.
    pub generation_policy: &'records VerifiedGenerationSystemPolicy,
}

/// Exact portable or live relationship checked by the precursor compiler.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagedJudgePrecursorRelationship {
    /// The portable plan did not name the supplied judge system.
    JudgePlan,
    /// The schedule was not the exact plan-derived schedule.
    JudgeSchedule,
    /// The request aggregate was not the exact schedule-order aggregate.
    RequestAggregate,
    /// Runtime records or opaque runtime authorities were substituted.
    Runtime,
    /// The prepared isolation policy was not the exact expected runtime-state policy.
    Isolation,
    /// Model records, launch authority, or prior characterization were substituted.
    Model,
    /// The reviewed generation-system policy was not the exact managed-judge policy.
    GenerationPolicy,
    /// A retained package installation generation was invalid.
    InstallationGeneration,
}

/// Compiler for the exact app-owned managed-judge precursor join.
#[derive(Clone, Copy, Debug, Default)]
pub struct ManagedJudgePrecursorCompiler;

impl ManagedJudgePrecursorCompiler {
    /// Compiles a precursor without launching after initial and final revalidation.
    ///
    /// The policy purpose is fixed internally to managed judge generation. The
    /// compiler accepts no caller-selected purpose, typed identity outside the
    /// supplied portable records, schedule entry, or installation generation.
    ///
    /// # Errors
    ///
    /// Returns [`ManagedJudgePrecursorCompilationError`] for cancellation, package
    /// drift, an invalid portable contract, or any substituted relationship.
    pub fn compile<'records, 'model, 'runtime, 'characterized>(
        mut input: ManagedJudgePrecursorCompilationInput<
            'records,
            'model,
            'runtime,
            'characterized,
        >,
        cancellation: &CancellationToken,
    ) -> Result<
        VerifiedManagedJudgePrecursor<'records, 'model, 'runtime, 'characterized>,
        ManagedJudgePrecursorCompilationError,
    > {
        ensure_not_cancelled(cancellation)?;
        revalidate_runtime(&mut input, cancellation)?;
        revalidate_model(&input, cancellation)?;
        ensure_not_cancelled(cancellation)?;
        validation::validate(&input)?;
        ensure_not_cancelled(cancellation)?;
        revalidate_model(&input, cancellation)?;
        revalidate_runtime(&mut input, cancellation)?;
        ensure_not_cancelled(cancellation)?;

        let runtime_installation_generation = input
            .runtime_package
            .installation_key()
            .installation_generation();
        let model_installation_generation = input.launch_plan.model_installation_generation();
        if runtime_installation_generation == 0 || model_installation_generation == 0 {
            return Err(ManagedJudgePrecursorCompilationError::Relationship(
                ManagedJudgePrecursorRelationship::InstallationGeneration,
            ));
        }

        Ok(VerifiedManagedJudgePrecursor {
            runtime_package: input.runtime_package,
            launch_plan: input.launch_plan,
            characterized_package: input.characterized_package,
            runtime_manifest: input.runtime_manifest,
            runtime_build: input.runtime_build,
            admitted_runtime: input.admitted_runtime,
            generation_path: input.generation_path,
            frozen_components: input.frozen_components,
            prepared_isolation: input.prepared_isolation,
            static_model: input.static_model,
            expected_runtime_state: input.expected_runtime_state.clone(),
            judge_plan: input.judge_plan.clone(),
            judge_schedule: input.judge_schedule.clone(),
            request_aggregate: input.request_aggregate.clone(),
            judge_system: input.judge_system.clone(),
            runtime_installation_generation,
            model_installation_generation,
        })
    }
}

/// Noncloneable and nonserializable managed-judge precursor.
///
/// This compiler result exposes no launch operation. Consuming it produces an
/// app-side runner handoff that retains the model launch authority, so the supported
/// runner must separately pair that handoff with the eval-owned deterministic and
/// request-material authority before launching.
///
/// ```compile_fail
/// use rewrite_app::VerifiedManagedJudgePrecursor;
///
/// fn clone_capability(value: &VerifiedManagedJudgePrecursor<'_, '_, '_, '_>) {
///     let _forged: VerifiedManagedJudgePrecursor<'_, '_, '_, '_> = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_app::VerifiedManagedJudgePrecursor;
///
/// fn serialize_capability(value: &VerifiedManagedJudgePrecursor<'_, '_, '_, '_>) {
///     let _bytes = serde_json::to_vec(value).expect("capability must not serialize");
/// }
/// ```
pub struct VerifiedManagedJudgePrecursor<'records, 'model, 'runtime, 'characterized> {
    runtime_package: &'runtime mut RuntimePackageLease,
    launch_plan: VerifiedManagedOllamaLaunchPlan<'model>,
    characterized_package: &'characterized ReleasedGenerationEffectivePackageV2,
    runtime_manifest: &'records RuntimePackageManifest,
    runtime_build: &'records RuntimeBuildIdentity,
    admitted_runtime: &'records VerifiedAdmittedRuntime,
    generation_path: &'records VerifiedManagedGenerationPath,
    frozen_components: &'records VerifiedFrozenExternalNativeComponentSet,
    prepared_isolation: &'records PreparedIsolation,
    static_model: &'records StaticModelInterpretationV1,
    expected_runtime_state: EffectiveRuntimeState,
    judge_plan: CandidateJudgePlanV1,
    judge_schedule: CandidateJudgeScheduleV1,
    request_aggregate: CandidateJudgeRequestAggregateV1,
    judge_system: GenerationSystemRecordV1,
    runtime_installation_generation: u64,
    model_installation_generation: u64,
}

impl VerifiedManagedJudgePrecursor<'_, '_, '_, '_> {
    /// Returns the exact portable judge plan.
    #[must_use]
    pub const fn judge_plan(&self) -> &CandidateJudgePlanV1 {
        &self.judge_plan
    }

    /// Returns the exact complete judge schedule.
    #[must_use]
    pub const fn judge_schedule(&self) -> &CandidateJudgeScheduleV1 {
        &self.judge_schedule
    }

    /// Returns the exact schedule-order request identity aggregate.
    #[must_use]
    pub const fn request_aggregate(&self) -> &CandidateJudgeRequestAggregateV1 {
        &self.request_aggregate
    }

    /// Returns the exact separately loaded judge system.
    #[must_use]
    pub const fn judge_system(&self) -> &GenerationSystemRecordV1 {
        &self.judge_system
    }

    /// Returns the internally derived runtime installation generation.
    #[must_use]
    pub const fn runtime_installation_generation(&self) -> u64 {
        self.runtime_installation_generation
    }

    /// Returns the internally derived model installation generation.
    #[must_use]
    pub const fn model_installation_generation(&self) -> u64 {
        self.model_installation_generation
    }

    /// Revalidates both retained package authorities.
    ///
    /// # Errors
    ///
    /// Returns an error for cancellation or current runtime or model-package drift.
    pub fn revalidate(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<(), ManagedJudgePrecursorCompilationError> {
        ensure_not_cancelled(cancellation)?;
        self.launch_plan
            .revalidate_for_managed_judge_precursor(cancellation)
            .map_err(|error| {
                ManagedJudgePrecursorCompilationError::ModelRevalidation(Box::new(error))
            })?;
        self.runtime_package
            .revalidate(cancellation)
            .map_err(|error| {
                ManagedJudgePrecursorCompilationError::RuntimeRevalidation(Box::new(error))
            })?;
        ensure_not_cancelled(cancellation)
    }
}

impl fmt::Debug for VerifiedManagedJudgePrecursor<'_, '_, '_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedManagedJudgePrecursor")
            .field("judge_plan_id", self.judge_plan.candidate_judge_plan_id())
            .field(
                "judge_schedule_id",
                self.judge_schedule.candidate_judge_schedule_id(),
            )
            .field(
                "request_aggregate_id",
                self.request_aggregate.request_aggregate_id(),
            )
            .field(
                "judge_generation_system_id",
                self.judge_system.generation_system_id(),
            )
            .finish_non_exhaustive()
    }
}

/// Nonforgeable consuming seam into the later managed judge runner.
///
/// This is the app-side launch handoff for the exact retained runtime and model
/// authorities. It does not prove deterministic readiness or the correctness of
/// supplied request identities. The supported runner must compare its plan,
/// schedule, request aggregate, and judge system with the exact eval-owned ready
/// handoff before launching or issuing request traffic.
///
/// ```compile_fail
/// use rewrite_app::ManagedJudgePrecursorRunnerHandoff;
///
/// fn clone_handoff(value: &ManagedJudgePrecursorRunnerHandoff<'_, '_, '_, '_>) {
///     let _forged: ManagedJudgePrecursorRunnerHandoff<'_, '_, '_, '_> = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_app::ManagedJudgePrecursorRunnerHandoff;
///
/// fn serialize_handoff(value: &ManagedJudgePrecursorRunnerHandoff<'_, '_, '_, '_>) {
///     let _bytes = serde_json::to_vec(value).expect("handoff must not serialize");
/// }
/// ```
pub struct ManagedJudgePrecursorRunnerHandoff<'records, 'model, 'runtime, 'characterized> {
    runtime_package: &'runtime mut RuntimePackageLease,
    launch_plan: VerifiedManagedOllamaLaunchPlan<'model>,
    characterized_package: &'characterized ReleasedGenerationEffectivePackageV2,
    runtime_manifest: &'records RuntimePackageManifest,
    runtime_build: &'records RuntimeBuildIdentity,
    admitted_runtime: &'records VerifiedAdmittedRuntime,
    generation_path: &'records VerifiedManagedGenerationPath,
    frozen_components: &'records VerifiedFrozenExternalNativeComponentSet,
    prepared_isolation: &'records PreparedIsolation,
    static_model: &'records StaticModelInterpretationV1,
    expected_runtime_state: EffectiveRuntimeState,
    judge_plan: CandidateJudgePlanV1,
    judge_schedule: CandidateJudgeScheduleV1,
    request_aggregate: CandidateJudgeRequestAggregateV1,
    judge_system: GenerationSystemRecordV1,
    runtime_installation_generation: u64,
    model_installation_generation: u64,
}

impl<'records, 'model, 'runtime, 'characterized>
    VerifiedManagedJudgePrecursor<'records, 'model, 'runtime, 'characterized>
{
    /// Revalidates and consumes the precursor into the only runner handoff.
    ///
    /// # Errors
    ///
    /// Returns an error without producing a handoff when either retained package
    /// changed or cancellation was observed.
    pub fn into_runner_handoff(
        mut self,
        cancellation: &CancellationToken,
    ) -> Result<
        ManagedJudgePrecursorRunnerHandoff<'records, 'model, 'runtime, 'characterized>,
        ManagedJudgePrecursorCompilationError,
    > {
        self.revalidate(cancellation)?;
        Ok(ManagedJudgePrecursorRunnerHandoff {
            runtime_package: self.runtime_package,
            launch_plan: self.launch_plan,
            characterized_package: self.characterized_package,
            runtime_manifest: self.runtime_manifest,
            runtime_build: self.runtime_build,
            admitted_runtime: self.admitted_runtime,
            generation_path: self.generation_path,
            frozen_components: self.frozen_components,
            prepared_isolation: self.prepared_isolation,
            static_model: self.static_model,
            expected_runtime_state: self.expected_runtime_state,
            judge_plan: self.judge_plan,
            judge_schedule: self.judge_schedule,
            request_aggregate: self.request_aggregate,
            judge_system: self.judge_system,
            runtime_installation_generation: self.runtime_installation_generation,
            model_installation_generation: self.model_installation_generation,
        })
    }
}

impl ManagedJudgePrecursorRunnerHandoff<'_, '_, '_, '_> {
    /// Returns the exact specialized model package retained by the launch plan.
    #[must_use]
    pub const fn model_package(&self) -> &crate::VerifiedManagedOllamaModelPackageLease {
        self.launch_plan.package()
    }

    /// Returns the exact portable judge plan.
    #[must_use]
    pub const fn judge_plan(&self) -> &CandidateJudgePlanV1 {
        &self.judge_plan
    }

    /// Returns the exact portable schedule.
    #[must_use]
    pub const fn judge_schedule(&self) -> &CandidateJudgeScheduleV1 {
        &self.judge_schedule
    }

    /// Returns the exact portable request aggregate.
    #[must_use]
    pub const fn request_aggregate(&self) -> &CandidateJudgeRequestAggregateV1 {
        &self.request_aggregate
    }

    /// Returns the exact judge system.
    #[must_use]
    pub const fn judge_system(&self) -> &GenerationSystemRecordV1 {
        &self.judge_system
    }

    /// Revalidates the retained runtime and model package authorities.
    ///
    /// # Errors
    ///
    /// Returns an error for cancellation or current package drift.
    pub fn revalidate(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<(), ManagedJudgePrecursorCompilationError> {
        ensure_not_cancelled(cancellation)?;
        self.launch_plan
            .revalidate_for_managed_judge_precursor(cancellation)
            .map_err(|error| {
                ManagedJudgePrecursorCompilationError::ModelRevalidation(Box::new(error))
            })?;
        self.runtime_package
            .revalidate(cancellation)
            .map_err(|error| {
                ManagedJudgePrecursorCompilationError::RuntimeRevalidation(Box::new(error))
            })?;
        ensure_not_cancelled(cancellation)
    }
}

impl<'records, 'model, 'runtime, 'characterized>
    ManagedJudgePrecursorRunnerHandoff<'records, 'model, 'runtime, 'characterized>
{
    /// Consumes the handoff into the exact app-side inputs needed by the runner.
    ///
    /// This releases the retained launch authority without proving eval readiness.
    /// The supported runner must first match the separate eval-owned handoff.
    #[must_use]
    #[expect(
        clippy::type_complexity,
        reason = "the consuming seam preserves every independently verified authority and record"
    )]
    pub fn into_parts(
        self,
    ) -> (
        &'runtime mut RuntimePackageLease,
        VerifiedManagedOllamaLaunchPlan<'model>,
        &'model crate::VerifiedManagedOllamaModelPackageLease,
        &'characterized ReleasedGenerationEffectivePackageV2,
        &'records RuntimePackageManifest,
        &'records RuntimeBuildIdentity,
        &'records VerifiedAdmittedRuntime,
        &'records VerifiedManagedGenerationPath,
        &'records VerifiedFrozenExternalNativeComponentSet,
        &'records PreparedIsolation,
        &'records StaticModelInterpretationV1,
        EffectiveRuntimeState,
        CandidateJudgePlanV1,
        CandidateJudgeScheduleV1,
        CandidateJudgeRequestAggregateV1,
        GenerationSystemRecordV1,
        u64,
        u64,
    ) {
        let model_package = self.launch_plan.package();
        (
            self.runtime_package,
            self.launch_plan,
            model_package,
            self.characterized_package,
            self.runtime_manifest,
            self.runtime_build,
            self.admitted_runtime,
            self.generation_path,
            self.frozen_components,
            self.prepared_isolation,
            self.static_model,
            self.expected_runtime_state,
            self.judge_plan,
            self.judge_schedule,
            self.request_aggregate,
            self.judge_system,
            self.runtime_installation_generation,
            self.model_installation_generation,
        )
    }
}

impl fmt::Debug for ManagedJudgePrecursorRunnerHandoff<'_, '_, '_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagedJudgePrecursorRunnerHandoff")
            .field("judge_plan_id", self.judge_plan.candidate_judge_plan_id())
            .field(
                "judge_schedule_id",
                self.judge_schedule.candidate_judge_schedule_id(),
            )
            .field(
                "request_aggregate_id",
                self.request_aggregate.request_aggregate_id(),
            )
            .finish_non_exhaustive()
    }
}

fn ensure_not_cancelled(
    cancellation: &CancellationToken,
) -> Result<(), ManagedJudgePrecursorCompilationError> {
    if cancellation.is_cancelled() {
        Err(ManagedJudgePrecursorCompilationError::Cancelled)
    } else {
        Ok(())
    }
}

fn revalidate_runtime(
    input: &mut ManagedJudgePrecursorCompilationInput<'_, '_, '_, '_>,
    cancellation: &CancellationToken,
) -> Result<(), ManagedJudgePrecursorCompilationError> {
    input
        .runtime_package
        .revalidate(cancellation)
        .map_err(|error| {
            ManagedJudgePrecursorCompilationError::RuntimeRevalidation(Box::new(error))
        })
}

fn revalidate_model(
    input: &ManagedJudgePrecursorCompilationInput<'_, '_, '_, '_>,
    cancellation: &CancellationToken,
) -> Result<(), ManagedJudgePrecursorCompilationError> {
    input
        .launch_plan
        .revalidate_for_managed_judge_precursor(cancellation)
        .map_err(|error| ManagedJudgePrecursorCompilationError::ModelRevalidation(Box::new(error)))
}

#[cfg(test)]
#[path = "managed_judge_precursor/tests.rs"]
mod tests;

#[cfg(test)]
pub(crate) use tests::support::{JudgeFixture, portable_judge_plan_schedules};
