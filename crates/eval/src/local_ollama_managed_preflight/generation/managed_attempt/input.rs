use rewrite_model::RuntimePackageManifest;
use rewrite_ollama::OllamaModelBinding;
use rewrite_runtime_attestor::{
    ManagedGenerationWorkerLimits, VerifiedFrozenExternalNativeComponentSet,
};
use rewrite_runtime_isolation::PreparedIsolation;

use crate::{
    LocalOllamaBoundPreflightPlan, LocalOllamaManagedPreflightLimits,
    LocalOllamaModelBindingEvidence,
};

/// Exact public non-ID inputs for one verified managed candidate attempt.
///
/// The precursor handoff separately owns the mutable runtime lease, current model
/// launch authority, provider-neutral request, and exact structured request. This
/// input therefore cannot substitute any of those values with caller-selected IDs.
pub struct ManagedCandidateAttemptRunInput<'a> {
    /// Current semantic runtime-package manifest.
    pub runtime_manifest: &'a RuntimePackageManifest,
    /// Opaque all-pass admitted-runtime authority used by the launch.
    pub admitted_runtime: &'a rewrite_app::VerifiedAdmittedRuntime,
    /// Opaque reviewed managed-generation path used by the worker.
    pub generation_path: &'a rewrite_app::VerifiedManagedGenerationPath,
    /// Independently verified native-component closure.
    pub frozen_components: &'a VerifiedFrozenExternalNativeComponentSet,
    /// Prepared managed isolation environment.
    pub isolation: &'a PreparedIsolation,
    /// Exact retained preflight plan for this runtime and model.
    pub preflight_plan: &'a LocalOllamaBoundPreflightPlan,
    /// Process and native-load observation ceilings.
    pub limits: LocalOllamaManagedPreflightLimits,
    /// Separately retained generation-worker ceilings.
    pub worker_limits: ManagedGenerationWorkerLimits,
    /// Existing static package-to-Ollama inventory binding.
    pub static_model: &'a LocalOllamaModelBindingEvidence,
    /// Exact Ollama model reference and artifact binding.
    pub model: &'a OllamaModelBinding,
}
