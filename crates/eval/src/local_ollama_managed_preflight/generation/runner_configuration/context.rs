use std::fmt;

use rewrite_app::{
    ManagedJudgePrecursorRunnerHandoff, ReleasedGenerationEffectivePackageV2, RuntimePackageLease,
    StaticModelInterpretationV1, VerifiedAdmittedRuntime, VerifiedManagedGenerationPath,
    VerifiedManagedOllamaLaunchPlan, VerifiedManagedOllamaModelPackageLease,
};
use rewrite_model::{
    CandidateJudgePlanV1, CandidateJudgeRequestAggregateV1, CandidateJudgeScheduleV1,
    EffectiveRuntimeState, GenerationSystemRecordV1, RuntimeBuildIdentity, RuntimePackageManifest,
};
use rewrite_ollama::OllamaModelBinding;
use rewrite_runtime_attestor::{
    ManagedGenerationWorkerLimits, VerifiedFrozenExternalNativeComponentSet,
};
use rewrite_runtime_isolation::PreparedIsolation;
use rewrite_types::CancellationToken;

use crate::candidate_judge_preparation::CandidateJudgeRunnerHandoff;
use crate::{
    LocalOllamaBoundPreflightPlan, LocalOllamaManagedPreflightLimits,
    LocalOllamaModelBindingEvidence,
};

use super::{ManagedJudgeRunnerConfigurationError, validation::validate_exact_context};

/// Owned inert caller configuration inspected before live authority release.
pub(in crate::local_ollama_managed_preflight::generation) struct ManagedJudgeRunnerConfigurationInput
{
    pub(in crate::local_ollama_managed_preflight::generation) preflight_plan:
        LocalOllamaBoundPreflightPlan,
    pub(in crate::local_ollama_managed_preflight::generation) preflight_limits:
        LocalOllamaManagedPreflightLimits,
    pub(in crate::local_ollama_managed_preflight::generation) worker_limits:
        ManagedGenerationWorkerLimits,
    pub(in crate::local_ollama_managed_preflight::generation) model_evidence:
        LocalOllamaModelBindingEvidence,
    pub(in crate::local_ollama_managed_preflight::generation) model: OllamaModelBinding,
}

/// Verified no-launch configuration capability for the later managed runner.
///
/// The capability owns every inert caller value and retains both authority
/// families through its private context. It deliberately exposes no launch,
/// connection, request, cleanup, receipt, or package-release operation.
pub(in crate::local_ollama_managed_preflight::generation) struct VerifiedManagedJudgeRunnerConfiguration<
    'store,
    'records,
    'model,
    'runtime,
    'characterized,
> {
    pub(super) context:
        ManagedJudgeRunnerContext<'store, 'records, 'model, 'runtime, 'characterized>,
}

impl VerifiedManagedJudgeRunnerConfiguration<'_, '_, '_, '_, '_> {
    pub(super) fn judge_plan_id(&self) -> &rewrite_model::CandidateJudgePlanId {
        self.context.eval.judge_plan().candidate_judge_plan_id()
    }

    pub(super) fn judge_schedule_id(&self) -> &rewrite_model::CandidateJudgeScheduleId {
        self.context
            .eval
            .judge_schedule()
            .candidate_judge_schedule_id()
    }

    pub(super) fn request_aggregate_id(&self) -> &rewrite_model::CandidateJudgeRequestAggregateId {
        self.context.eval.request_aggregate().request_aggregate_id()
    }

    pub(super) fn judge_generation_system_id(&self) -> &rewrite_model::GenerationSystemId {
        self.context.eval.judge_system().generation_system_id()
    }

    pub(super) fn attempt_count(&self) -> u32 {
        self.context.eval.judge_schedule().entry_count()
    }

    pub(super) fn preflight_response_count(&self) -> usize {
        self.context
            .preflight_plan
            .preflight
            .models
            .len()
            .saturating_add(6)
    }
}

impl fmt::Debug for VerifiedManagedJudgeRunnerConfiguration<'_, '_, '_, '_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedManagedJudgeRunnerConfiguration")
            .field("judge_plan_id", self.judge_plan_id())
            .field("judge_schedule_id", self.judge_schedule_id())
            .field("request_aggregate_id", self.request_aggregate_id())
            .field(
                "judge_generation_system_id",
                self.judge_generation_system_id(),
            )
            .field("attempt_count", &self.attempt_count())
            .field("preflight_response_count", &self.preflight_response_count())
            .finish_non_exhaustive()
    }
}

impl<'store, 'records, 'model, 'runtime, 'characterized>
    VerifiedManagedJudgeRunnerConfiguration<'store, 'records, 'model, 'runtime, 'characterized>
{
    /// Revalidates and consumes the configuration into the actual runner's context.
    ///
    /// The caller must invoke this immediately before launch. It repeats the exact
    /// deterministic joins and retained eval and runtime-package revalidation.
    /// App-owned materialization independently performs the mandatory exact model
    /// and license revalidation. This seam itself grants no launch authority.
    pub(in crate::local_ollama_managed_preflight::generation) fn into_revalidated_runner_context(
        mut self,
        cancellation: &CancellationToken,
    ) -> Result<
        ManagedJudgeRunnerContext<'store, 'records, 'model, 'runtime, 'characterized>,
        ManagedJudgeRunnerConfigurationError,
    > {
        validate_exact_context(&mut self.context, cancellation)?;
        Ok(self.context)
    }
}

/// Named noncloneable and nonserializable retained runner context.
pub(in crate::local_ollama_managed_preflight::generation) struct ManagedJudgeRunnerContext<
    'store,
    'records,
    'model,
    'runtime,
    'characterized,
> {
    pub(in crate::local_ollama_managed_preflight::generation) eval:
        CandidateJudgeRunnerHandoff<'store>,
    pub(in crate::local_ollama_managed_preflight::generation) runtime_package:
        &'runtime mut RuntimePackageLease,
    pub(in crate::local_ollama_managed_preflight::generation) launch_plan:
        VerifiedManagedOllamaLaunchPlan<'model>,
    pub(in crate::local_ollama_managed_preflight::generation) model_package:
        &'model VerifiedManagedOllamaModelPackageLease,
    pub(in crate::local_ollama_managed_preflight::generation) characterized_package:
        &'characterized ReleasedGenerationEffectivePackageV2,
    pub(in crate::local_ollama_managed_preflight::generation) runtime_manifest:
        &'records RuntimePackageManifest,
    pub(in crate::local_ollama_managed_preflight::generation) runtime_build:
        &'records RuntimeBuildIdentity,
    pub(in crate::local_ollama_managed_preflight::generation) admitted_runtime:
        &'records VerifiedAdmittedRuntime,
    pub(in crate::local_ollama_managed_preflight::generation) generation_path:
        &'records VerifiedManagedGenerationPath,
    pub(in crate::local_ollama_managed_preflight::generation) frozen_components:
        &'records VerifiedFrozenExternalNativeComponentSet,
    pub(in crate::local_ollama_managed_preflight::generation) prepared_isolation:
        &'records PreparedIsolation,
    pub(in crate::local_ollama_managed_preflight::generation) app_static_model:
        &'records StaticModelInterpretationV1,
    pub(in crate::local_ollama_managed_preflight::generation) expected_runtime_state:
        EffectiveRuntimeState,
    pub(in crate::local_ollama_managed_preflight::generation) app_judge_plan: CandidateJudgePlanV1,
    pub(in crate::local_ollama_managed_preflight::generation) app_judge_schedule:
        CandidateJudgeScheduleV1,
    pub(in crate::local_ollama_managed_preflight::generation) app_request_aggregate:
        CandidateJudgeRequestAggregateV1,
    pub(in crate::local_ollama_managed_preflight::generation) app_judge_system:
        GenerationSystemRecordV1,
    pub(in crate::local_ollama_managed_preflight::generation) runtime_installation_generation: u64,
    pub(in crate::local_ollama_managed_preflight::generation) model_installation_generation: u64,
    pub(in crate::local_ollama_managed_preflight::generation) preflight_plan:
        LocalOllamaBoundPreflightPlan,
    pub(in crate::local_ollama_managed_preflight::generation) preflight_limits:
        LocalOllamaManagedPreflightLimits,
    pub(in crate::local_ollama_managed_preflight::generation) worker_limits:
        ManagedGenerationWorkerLimits,
    pub(in crate::local_ollama_managed_preflight::generation) model_evidence:
        LocalOllamaModelBindingEvidence,
    pub(in crate::local_ollama_managed_preflight::generation) model: OllamaModelBinding,
}

pub(super) fn named_context<'store, 'records, 'model, 'runtime, 'characterized>(
    eval: CandidateJudgeRunnerHandoff<'store>,
    app: ManagedJudgePrecursorRunnerHandoff<'records, 'model, 'runtime, 'characterized>,
    input: ManagedJudgeRunnerConfigurationInput,
) -> ManagedJudgeRunnerContext<'store, 'records, 'model, 'runtime, 'characterized> {
    let (
        runtime_package,
        launch_plan,
        model_package,
        characterized_package,
        runtime_manifest,
        runtime_build,
        admitted_runtime,
        generation_path,
        frozen_components,
        prepared_isolation,
        app_static_model,
        expected_runtime_state,
        app_judge_plan,
        app_judge_schedule,
        app_request_aggregate,
        app_judge_system,
        runtime_installation_generation,
        model_installation_generation,
    ) = app.into_parts();
    ManagedJudgeRunnerContext {
        eval,
        runtime_package,
        launch_plan,
        model_package,
        characterized_package,
        runtime_manifest,
        runtime_build,
        admitted_runtime,
        generation_path,
        frozen_components,
        prepared_isolation,
        app_static_model,
        expected_runtime_state,
        app_judge_plan,
        app_judge_schedule,
        app_request_aggregate,
        app_judge_system,
        runtime_installation_generation,
        model_installation_generation,
        preflight_plan: input.preflight_plan,
        preflight_limits: input.preflight_limits,
        worker_limits: input.worker_limits,
        model_evidence: input.model_evidence,
        model: input.model,
    }
}
