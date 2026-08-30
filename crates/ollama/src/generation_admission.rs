use rewrite_model::{
    ArtifactId, RuntimePackageLoadPolicy, RuntimePackageManifest, RuntimePackageManifestId,
    RuntimePackageMemberRole,
};
use serde::Serialize;

// Package and cloud-disable review do not authorize model generation. Production
// remains fail closed until the exact worker execution path has a separate review.
const REVIEWED_MANAGED_GENERATION_RUNTIMES: &[ReviewedManagedGenerationRuntime] = &[];

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReviewedManagedGenerationRuntime {
    version: super::OllamaVersion,
    runtime_package_manifest_id: RuntimePackageManifestId,
    worker_artifact_id: ArtifactId,
}

/// Result of applying the managed-generation admission policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OllamaManagedGenerationAdmissionStatus {
    /// The exact package and worker execution path have not been reviewed.
    Unreviewed,
    /// The exact package and worker execution path have a separate production review.
    Reviewed,
}

/// Production policy for Ollama managed-generation execution.
///
/// This policy is independent of package and cloud-disable review. A package may be
/// admitted for managed read-only observation while generation remains unreviewed.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OllamaManagedGenerationAdmissionPolicy;

impl OllamaManagedGenerationAdmissionPolicy {
    /// Returns the number of exact runtime and worker pairs reviewed for generation.
    #[must_use]
    pub const fn reviewed_runtime_count() -> usize {
        REVIEWED_MANAGED_GENERATION_RUNTIMES.len()
    }

    /// Assesses one exact runtime package against the production generation policy.
    #[must_use]
    pub fn assess(
        version: super::OllamaVersion,
        package: &RuntimePackageManifest,
    ) -> OllamaManagedGenerationAdmissionStatus {
        assess_runtime(
            version,
            &package.runtime_package_manifest_id(),
            exact_worker_artifact_id(package),
            REVIEWED_MANAGED_GENERATION_RUNTIMES,
        )
    }
}

fn exact_worker_artifact_id(package: &RuntimePackageManifest) -> Option<&ArtifactId> {
    let mut workers = package.members().iter().filter(|member| {
        member
            .roles()
            .contains(&RuntimePackageMemberRole::WorkerExecutable)
    });
    let worker = workers.next()?;
    if workers.next().is_some()
        || worker.roles() != [RuntimePackageMemberRole::WorkerExecutable]
        || worker.load_policy() != RuntimePackageLoadPolicy::BackendConditional
    {
        return None;
    }
    Some(worker.artifact_id())
}

fn assess_runtime(
    version: super::OllamaVersion,
    runtime_package_manifest_id: &RuntimePackageManifestId,
    worker_artifact_id: Option<&ArtifactId>,
    reviewed_runtimes: &[ReviewedManagedGenerationRuntime],
) -> OllamaManagedGenerationAdmissionStatus {
    let Some(worker_artifact_id) = worker_artifact_id else {
        return OllamaManagedGenerationAdmissionStatus::Unreviewed;
    };
    if reviewed_runtimes.iter().any(|reviewed| {
        reviewed.version == version
            && &reviewed.runtime_package_manifest_id == runtime_package_manifest_id
            && &reviewed.worker_artifact_id == worker_artifact_id
    }) {
        OllamaManagedGenerationAdmissionStatus::Reviewed
    } else {
        OllamaManagedGenerationAdmissionStatus::Unreviewed
    }
}

#[cfg(test)]
#[path = "generation_admission/tests.rs"]
mod tests;
