//! App-owned derivation and cleanup-gated release of effective-package v2 evidence.
//!
//! Every digest here is derived from an existing typed capability, manifest, or
//! closed policy. Evidence remains hidden until the exact managed isolation lease
//! has closed and both retained packages have been independently revalidated.

mod derivation;
mod plan;

pub(crate) use derivation::{ManagedJudgeBatchInputs, derive_managed_judge_batch};

pub use plan::{
    GenerationEffectivePackageCleanupTimingError, GenerationEffectivePackageDerivationError,
    GenerationEffectivePackagePlanError, GenerationEffectivePackageReleaseError,
    ReleasedGenerationEffectivePackageV2, VerifiedGenerationEffectivePackagePlan,
};

/// Stable app-owned derivation contract for managed Ollama effective packages.
pub const MANAGED_OLLAMA_EFFECTIVE_PACKAGE_V2_CONTRACT_ID: &str =
    "managed-ollama-effective-package";
/// Version of the app-owned effective-package derivation contract.
pub const MANAGED_OLLAMA_EFFECTIVE_PACKAGE_V2_CONTRACT_VERSION: u32 = 1;

#[cfg(test)]
use derivation::{
    derive_member_evidence, isolation_exclusion_digest_from_portable_facts, purpose_for_role,
    relationship_checks_pass, runtime_closure_digest_from_portable_facts, transformation,
};
#[cfg(test)]
use plan::{run_release_and_validation_steps, run_release_steps};

#[cfg(test)]
#[path = "generation_effective_package/tests.rs"]
mod tests;
