//! Inert point-in-time runtime-build binding from a successful managed preflight.

use rewrite_model::{
    NativeLoadEvidenceClass, NativeLoadVisibilityScope, RuntimeBuildIdentity, RuntimeBuildMode,
    RuntimeOperatingSystem, RuntimePackageLoadPolicy, RuntimePackageManifest,
    RuntimePackageMemberRole,
};
use rewrite_ollama::OllamaCloudDisableVersionStatus;
use rewrite_types::Digest;
use serde::Serialize;

use crate::{
    LOCAL_OLLAMA_BOUND_PREFLIGHT_PLAN_SCHEMA_VERSION, LOCAL_OLLAMA_PREFLIGHT_PLAN_SCHEMA_VERSION,
    LOCAL_OLLAMA_PREFLIGHT_REPORT_SCHEMA_VERSION, LocalOllamaBoundPreflightPlan,
    LocalOllamaManagedPreflightError, LocalOllamaManagedPreflightReport,
    LocalOllamaManagedProcessEvidenceLevel, LocalOllamaPreflightMode,
    local_ollama_bound_preflight::validate_bound_plan,
};

/// Current managed runtime-build binding contract version.
pub const LOCAL_OLLAMA_MANAGED_BUILD_BINDING_SCHEMA_VERSION: u32 = 1;

/// Evidence class for one successful managed package, process, and native-load join.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalOllamaManagedBuildEvidenceClass {
    /// Managed Linux isolation, namespace socket attribution, and native-load evidence.
    ManagedLinuxPreflightPackageProcessLoad,
}

/// Closed set of relationships still required to construct an effective runtime state.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalOllamaEffectiveStateMissingRelationship {
    /// A provider snapshot observed during the exact generative execution bracket.
    GenerationBoundProviderSnapshot,
    /// Direct evidence of effective output-affecting runtime configuration.
    EffectiveOutputConfiguration,
    /// Direct operating-system, framework, and driver evidence.
    PlatformFrameworkAndDriver,
    /// Direct compute-backend and device-placement evidence.
    ComputeBackendAndPlacement,
    /// Direct evidence of the effective context capacity used for generation.
    EffectiveContextCapacity,
    /// A retained live runtime spanning state construction and the qualifying work.
    RetainedLiveRuntime,
}

const MISSING_EFFECTIVE_STATE_RELATIONSHIPS: [LocalOllamaEffectiveStateMissingRelationship; 6] = [
    LocalOllamaEffectiveStateMissingRelationship::GenerationBoundProviderSnapshot,
    LocalOllamaEffectiveStateMissingRelationship::EffectiveOutputConfiguration,
    LocalOllamaEffectiveStateMissingRelationship::PlatformFrameworkAndDriver,
    LocalOllamaEffectiveStateMissingRelationship::ComputeBackendAndPlacement,
    LocalOllamaEffectiveStateMissingRelationship::EffectiveContextCapacity,
    LocalOllamaEffectiveStateMissingRelationship::RetainedLiveRuntime,
];

/// Redacted, inert binding of package-declared runtime identity to managed evidence.
///
/// The typed build identity is derived from the exact package declaration. The
/// package entrypoint is joined to managed process and native-load evidence emitted
/// by the same successful operation. Target, revision, and other package semantics
/// are not independently observed from the live process. Cleanup completes before
/// this binding is returned. It cannot construct an effective runtime state, prove
/// model use or handler execution, qualify a runtime, or authorize activation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each positive and negative security claim remains explicit in the binding"
)]
pub struct LocalOllamaManagedBuildBinding {
    schema_version: u32,
    binding_digest: Digest,
    evidence_class: LocalOllamaManagedBuildEvidenceClass,
    managed_preflight_binding_digest: Digest,
    native_load_observation_digest: Digest,
    runtime_build: RuntimeBuildIdentity,
    missing_effective_state_relationships: Vec<LocalOllamaEffectiveStateMissingRelationship>,
    package_declared_runtime_build_identity_constructed: bool,
    process_retained_after_return: bool,
    effective_runtime_state_proven: bool,
    model_loaded_or_used_proven: bool,
    application_handler_proven: bool,
    qualified: bool,
}

impl LocalOllamaManagedBuildBinding {
    /// Returns the binding contract version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the digest binding every positive and negative claim.
    #[must_use]
    pub const fn binding_digest(&self) -> &Digest {
        &self.binding_digest
    }

    /// Returns the exact managed evidence class.
    #[must_use]
    pub const fn evidence_class(&self) -> LocalOllamaManagedBuildEvidenceClass {
        self.evidence_class
    }

    /// Returns the exact managed preflight report binding consumed by this join.
    #[must_use]
    pub const fn managed_preflight_binding_digest(&self) -> &Digest {
        &self.managed_preflight_binding_digest
    }

    /// Returns the exact native-load observation consumed by this join.
    #[must_use]
    pub const fn native_load_observation_digest(&self) -> &Digest {
        &self.native_load_observation_digest
    }

    /// Returns the exact typed identity constructed from package declarations.
    #[must_use]
    pub const fn runtime_build(&self) -> &RuntimeBuildIdentity {
        &self.runtime_build
    }

    /// Returns every relationship still absent from an effective runtime state.
    #[must_use]
    pub fn missing_effective_state_relationships(
        &self,
    ) -> &[LocalOllamaEffectiveStateMissingRelationship] {
        &self.missing_effective_state_relationships
    }

    /// Returns whether the typed package-declared runtime identity was constructed.
    ///
    /// This does not mean every semantic identity field was independently observed
    /// from the live process.
    #[must_use]
    pub const fn package_declared_runtime_build_identity_constructed(&self) -> bool {
        self.package_declared_runtime_build_identity_constructed
    }

    /// Returns whether the process remains retained after this result is returned.
    #[must_use]
    pub const fn process_retained_after_return(&self) -> bool {
        self.process_retained_after_return
    }

    /// Returns whether an effective runtime state was directly proven.
    #[must_use]
    pub const fn effective_runtime_state_proven(&self) -> bool {
        self.effective_runtime_state_proven
    }

    /// Returns whether model loading or use was proven.
    #[must_use]
    pub const fn model_loaded_or_used_proven(&self) -> bool {
        self.model_loaded_or_used_proven
    }

    /// Returns whether application-handler execution was proven.
    #[must_use]
    pub const fn application_handler_proven(&self) -> bool {
        self.application_handler_proven
    }

    /// Returns whether this binding has qualification authority.
    #[must_use]
    pub const fn qualified(&self) -> bool {
        self.qualified
    }
}

/// Successful managed preflight plus its inert point-in-time runtime-build binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalOllamaManagedPreflightOutcome {
    report: LocalOllamaManagedPreflightReport,
    build_binding: LocalOllamaManagedBuildBinding,
}

impl LocalOllamaManagedPreflightOutcome {
    pub(super) const fn new(
        report: LocalOllamaManagedPreflightReport,
        build_binding: LocalOllamaManagedBuildBinding,
    ) -> Self {
        Self {
            report,
            build_binding,
        }
    }

    /// Returns the unchanged managed preflight report.
    #[must_use]
    pub const fn report(&self) -> &LocalOllamaManagedPreflightReport {
        &self.report
    }

    /// Returns the inert point-in-time runtime-build binding.
    #[must_use]
    pub const fn build_binding(&self) -> &LocalOllamaManagedBuildBinding {
        &self.build_binding
    }

    /// Splits the outcome into its unchanged report and build binding.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        LocalOllamaManagedPreflightReport,
        LocalOllamaManagedBuildBinding,
    ) {
        (self.report, self.build_binding)
    }

    pub(super) fn into_report(self) -> LocalOllamaManagedPreflightReport {
        self.report
    }
}

pub(super) fn bind_successful_managed_preflight(
    package: &RuntimePackageManifest,
    plan: &LocalOllamaBoundPreflightPlan,
    report: &LocalOllamaManagedPreflightReport,
) -> Result<LocalOllamaManagedBuildBinding, LocalOllamaManagedPreflightError> {
    validate_relationships(package, plan, report)?;
    let runtime_build =
        RuntimeBuildIdentity::new_from_package_manifest(RuntimeBuildMode::ManagedProcess, package)
            .map_err(|_error| LocalOllamaManagedPreflightError::InvalidEvidenceBinding)?;
    let native_load_observation_digest = report
        .native_load
        .native_load_observation_id()
        .digest()
        .clone();
    let mut binding = LocalOllamaManagedBuildBinding {
        schema_version: LOCAL_OLLAMA_MANAGED_BUILD_BINDING_SCHEMA_VERSION,
        binding_digest: Digest::sha256(b"pending"),
        evidence_class:
            LocalOllamaManagedBuildEvidenceClass::ManagedLinuxPreflightPackageProcessLoad,
        managed_preflight_binding_digest: report.binding_digest.clone(),
        native_load_observation_digest,
        runtime_build,
        missing_effective_state_relationships: MISSING_EFFECTIVE_STATE_RELATIONSHIPS.to_vec(),
        package_declared_runtime_build_identity_constructed: true,
        process_retained_after_return: false,
        effective_runtime_state_proven: false,
        model_loaded_or_used_proven: false,
        application_handler_proven: false,
        qualified: false,
    };
    binding.binding_digest = binding_digest(&binding)?;
    Ok(binding)
}

fn validate_relationships(
    package: &RuntimePackageManifest,
    plan: &LocalOllamaBoundPreflightPlan,
    report: &LocalOllamaManagedPreflightReport,
) -> Result<(), LocalOllamaManagedPreflightError> {
    validate_bound_plan(plan).map_err(LocalOllamaManagedPreflightError::BoundObservation)?;
    let plan_digest = digest_json(plan)?;
    let preflight_plan_digest = digest_json(&plan.preflight)?;
    let package_id = package.runtime_package_manifest_id();
    let process = &report.initial_process_witness;
    let expected_observations = plan.preflight.models.len().saturating_add(7);

    let cloud_status_valid = match report.cloud_disable_version_status {
        OllamaCloudDisableVersionStatus::Reviewed => report.cloud_disable_runtime_reviewed,
        OllamaCloudDisableVersionStatus::Unreviewed => !report.cloud_disable_runtime_reviewed,
        OllamaCloudDisableVersionStatus::FeatureUnavailable => false,
    };
    let helper_valid = package
        .members()
        .iter()
        .filter(|member| {
            member
                .roles()
                .contains(&RuntimePackageMemberRole::HelperExecutable)
                && member.load_policy() == RuntimePackageLoadPolicy::MustNotBeCodeLoaded
                && member.artifact_id() == &report.helper_member_artifact_id
                && member.relative_path() == &report.helper_member_relative_path
                && member.byte_size() == report.helper_member_bytes
        })
        .count()
        == 1;
    let process_valid = [
        &report.post_preflight_process_witness,
        &report.final_process_witness,
    ]
    .into_iter()
    .all(|witness| witness == process)
        && process.entrypoint_digest() == package.entrypoint().artifact_id().digest()
        && process.entrypoint_bytes() == package.entrypoint().byte_size();
    let observations_valid = report.connection_observations.len() == expected_observations
        && report.connection_observations.last() == Some(&report.connection_witness);
    let preflight_valid = report.preflight.schema_version
        == LOCAL_OLLAMA_PREFLIGHT_REPORT_SCHEMA_VERSION
        && report.preflight.plan_digest == preflight_plan_digest
        && report.preflight.plan_id == plan.preflight.plan_id
        && report.preflight.mode == LocalOllamaPreflightMode::Verify
        && report.preflight.observed.runtime.backend == "ollama_native"
        && report.preflight.observed.runtime.version == package.reported_version()
        && report.preflight.observed.running.is_empty()
        && !report.preflight.qualified;
    let static_valid = report.schema_version
        == crate::LOCAL_OLLAMA_MANAGED_PREFLIGHT_REPORT_SCHEMA_VERSION
        && plan.schema_version == LOCAL_OLLAMA_BOUND_PREFLIGHT_PLAN_SCHEMA_VERSION
        && plan.preflight.schema_version == LOCAL_OLLAMA_PREFLIGHT_PLAN_SCHEMA_VERSION
        && package.runtime_family() == "ollama"
        && package.target().operating_system() == RuntimeOperatingSystem::Linux
        && package.reported_version() == plan.preflight.expected_runtime_version
        && plan.preflight.require_idle
        && plan.expected_entrypoint_digest.as_ref()
            == Some(package.entrypoint().artifact_id().digest())
        && report.plan_digest == plan_digest
        && report.runtime_package_manifest_id == package_id
        && report.artifact_set_id == *package.artifact_set_id()
        && report.process_evidence_level
            == LocalOllamaManagedProcessEvidenceLevel::ManagedLinuxIsolationSockDiag
        && report.native_load.runtime_package_manifest_id() == &package_id
        && report.native_load.evidence_class() == NativeLoadEvidenceClass::LinuxProcMapFiles
        && report.native_load.visibility_scope()
            == NativeLoadVisibilityScope::FileBackedExecutableMappings
        && report.native_load.process_evidence_digest() == process.evidence_digest();
    let claims_valid = report.cloud_disable_environment_observed
        && report.cloud_disable_startup_marker_observed
        && cloud_status_valid
        && report.operating_system_network_isolation_enforced
        && report.all_responses_used_retained_transport
        && report.kernel_attribution_checked_around_every_response
        && !report.application_handler_proven
        && !report.exclusive_socket_owner_proven
        && !report.model_loaded_or_used_proven
        && !report.effective_runtime_identity_proven
        && !report.qualified;
    if !helper_valid
        || !process_valid
        || !observations_valid
        || !preflight_valid
        || !static_valid
        || !claims_valid
    {
        return Err(LocalOllamaManagedPreflightError::InvalidEvidenceBinding);
    }
    Ok(())
}

fn binding_digest(
    binding: &LocalOllamaManagedBuildBinding,
) -> Result<Digest, LocalOllamaManagedPreflightError> {
    let mut canonical = binding.clone();
    canonical.binding_digest = Digest::sha256(b"binding-field-excluded");
    let encoded = serde_json::to_vec(&canonical)
        .map_err(|_error| LocalOllamaManagedPreflightError::ReportEncoding)?;
    let mut bytes = b"retonr:local-ollama-managed-build-binding:v1\0".to_vec();
    bytes.extend_from_slice(&encoded);
    Ok(Digest::sha256(&bytes))
}

fn digest_json(value: &impl Serialize) -> Result<Digest, LocalOllamaManagedPreflightError> {
    serde_json::to_vec(value)
        .map(|bytes| Digest::sha256(&bytes))
        .map_err(|_error| LocalOllamaManagedPreflightError::ReportEncoding)
}

#[cfg(test)]
#[path = "build_binding/tests.rs"]
mod tests;
