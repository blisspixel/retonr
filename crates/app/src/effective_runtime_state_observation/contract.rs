use rewrite_model::{
    ArtifactId, ArtifactSetId, ComputeBackend, EffectiveRuntimeStateError, ExecutionPlacement,
    ModelPackageManifestId, RuntimeBuildId, RuntimeTarget,
};
use rewrite_types::Digest;
use serde::Serialize;
use thiserror::Error;

/// Schema shared by the effective-state leaf observation records.
pub const EFFECTIVE_RUNTIME_STATE_OBSERVATION_SCHEMA_VERSION: u32 = 1;

/// Accelerator-driver relationship represented by platform evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlatformDriverEvidenceClass {
    /// Accelerator-driver evidence is outside the reviewed native-CPU profile.
    ///
    /// This value does not claim that the host has no accelerator driver.
    NotApplicableForReviewedNativeCpuProfile,
}

/// Inert provider snapshot evidence from one retained Ollama bracket.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct OllamaProviderSnapshotEvidence {
    pub(super) schema_version: u32,
    pub(super) runtime_build_id: RuntimeBuildId,
    pub(super) runtime_package_manifest_digest: Digest,
    pub(super) model_artifact_set_id: ArtifactSetId,
    pub(super) model_package_manifest_id: ModelPackageManifestId,
    pub(super) model_artifact_id: ArtifactId,
    pub(super) model_installation_generation: u64,
    pub(super) request_binding_digest: Digest,
    pub(super) response_binding_digest: Digest,
    pub(super) snapshot_digest: Digest,
    pub(super) observation_binding_digest: Digest,
}

impl OllamaProviderSnapshotEvidence {
    /// Returns the record schema.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the exact package-declared runtime build.
    #[must_use]
    pub const fn runtime_build_id(&self) -> &RuntimeBuildId {
        &self.runtime_build_id
    }

    /// Returns the exact runtime package.
    #[must_use]
    pub const fn runtime_package_manifest_digest(&self) -> &Digest {
        &self.runtime_package_manifest_digest
    }

    /// Returns the exact model artifact set.
    #[must_use]
    pub const fn model_artifact_set_id(&self) -> &ArtifactSetId {
        &self.model_artifact_set_id
    }

    /// Returns the exact model package.
    #[must_use]
    pub const fn model_package_manifest_id(&self) -> &ModelPackageManifestId {
        &self.model_package_manifest_id
    }

    /// Returns the exact model weight.
    #[must_use]
    pub const fn model_artifact_id(&self) -> &ArtifactId {
        &self.model_artifact_id
    }

    /// Returns the retained model installation generation.
    #[must_use]
    pub const fn model_installation_generation(&self) -> u64 {
        self.model_installation_generation
    }

    /// Returns the exact structured-request binding.
    #[must_use]
    pub const fn request_binding_digest(&self) -> &Digest {
        &self.request_binding_digest
    }

    /// Returns the exact structured-response binding.
    #[must_use]
    pub const fn response_binding_digest(&self) -> &Digest {
        &self.response_binding_digest
    }

    /// Returns the canonical provider snapshot digest.
    #[must_use]
    pub const fn snapshot_digest(&self) -> &Digest {
        &self.snapshot_digest
    }

    /// Returns the per-attempt binding over request, response, and live observations.
    #[must_use]
    pub const fn observation_binding_digest(&self) -> &Digest {
        &self.observation_binding_digest
    }

    /// Always false because a leaf observation cannot grant qualification.
    #[must_use]
    pub const fn qualified(&self) -> bool {
        false
    }
}

/// Inert wire output-configuration evidence for one Ollama request.
///
/// This record deliberately excludes the closed launch and private model-root
/// relationships required by a complete effective configuration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct OllamaWireOutputConfigurationEvidence {
    pub(super) schema_version: u32,
    pub(super) runtime_build_id: RuntimeBuildId,
    pub(super) model_artifact_id: ArtifactId,
    pub(super) request_binding_digest: Digest,
    pub(super) response_binding_digest: Digest,
    pub(super) effective_context_tokens: u32,
    pub(super) configuration_digest: Digest,
}

impl OllamaWireOutputConfigurationEvidence {
    /// Returns the record schema.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the exact package-declared runtime build.
    #[must_use]
    pub const fn runtime_build_id(&self) -> &RuntimeBuildId {
        &self.runtime_build_id
    }

    /// Returns the selected model weight.
    #[must_use]
    pub const fn model_artifact_id(&self) -> &ArtifactId {
        &self.model_artifact_id
    }

    /// Returns the exact structured-request binding.
    #[must_use]
    pub const fn request_binding_digest(&self) -> &Digest {
        &self.request_binding_digest
    }

    /// Returns the exact structured-response binding.
    #[must_use]
    pub const fn response_binding_digest(&self) -> &Digest {
        &self.response_binding_digest
    }

    /// Returns the directly observed effective context capacity.
    #[must_use]
    pub const fn effective_context_tokens(&self) -> u32 {
        self.effective_context_tokens
    }

    /// Returns the canonical wire output-configuration digest.
    #[must_use]
    pub const fn configuration_digest(&self) -> &Digest {
        &self.configuration_digest
    }

    /// Always false because a leaf observation cannot grant live-use authority.
    #[must_use]
    pub const fn live_use_authorized(&self) -> bool {
        false
    }

    /// Always false because launch and private model-root relationships are absent.
    #[must_use]
    pub const fn complete_effective_configuration(&self) -> bool {
        false
    }
}

/// Inert Linux kernel, runtime target, and native-framework evidence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct LinuxPlatformFrameworkEvidence {
    pub(super) schema_version: u32,
    pub(super) runtime_build_id: RuntimeBuildId,
    pub(super) target: RuntimeTarget,
    pub(super) kernel_observation_digest: Digest,
    pub(super) worker_portable_closure_digest: Digest,
    pub(super) worker_native_load_digest: Digest,
    pub(super) driver_evidence: PlatformDriverEvidenceClass,
    pub(super) platform_digest: Digest,
}

impl LinuxPlatformFrameworkEvidence {
    /// Returns the record schema.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the exact package-declared runtime build.
    #[must_use]
    pub const fn runtime_build_id(&self) -> &RuntimeBuildId {
        &self.runtime_build_id
    }

    /// Returns the exact runtime target.
    #[must_use]
    pub const fn target(&self) -> RuntimeTarget {
        self.target
    }

    /// Returns the digest of the bounded canonical kernel observation.
    #[must_use]
    pub const fn kernel_observation_digest(&self) -> &Digest {
        &self.kernel_observation_digest
    }

    /// Returns the portable identity of the actual canonical native closure.
    #[must_use]
    pub const fn worker_portable_closure_digest(&self) -> &Digest {
        &self.worker_portable_closure_digest
    }

    /// Returns the separately observed worker native-code closure digest.
    #[must_use]
    pub const fn worker_native_load_digest(&self) -> &Digest {
        &self.worker_native_load_digest
    }

    /// Returns the explicit driver relationship class.
    #[must_use]
    pub const fn driver_evidence(&self) -> PlatformDriverEvidenceClass {
        self.driver_evidence
    }

    /// Returns the canonical platform and framework digest.
    #[must_use]
    pub const fn platform_digest(&self) -> &Digest {
        &self.platform_digest
    }

    /// Always false because this record does not prove accelerator-driver absence.
    #[must_use]
    pub const fn accelerator_driver_absence_proven(&self) -> bool {
        false
    }
}

/// Inert bounded native-CPU execution evidence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct OllamaCpuExecutionEvidence {
    pub(super) schema_version: u32,
    pub(super) model_artifact_id: ArtifactId,
    pub(super) isolation_evidence_digest: Digest,
    pub(super) worker_evidence_digest: Digest,
    pub(super) worker_portable_configuration_digest: Digest,
    pub(super) worker_portable_closure_digest: Digest,
    pub(super) worker_native_load_digest: Digest,
    pub(super) model_mapping_digest: Digest,
    pub(super) residency_observation_digest: Digest,
    pub(super) effective_context_tokens: u32,
    pub(super) compute_backend: ComputeBackend,
    pub(super) placement: ExecutionPlacement,
    pub(super) execution_class_digest: Digest,
    pub(super) observation_binding_digest: Digest,
}

impl OllamaCpuExecutionEvidence {
    /// Returns the record schema.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the exact mapped model weight.
    #[must_use]
    pub const fn model_artifact_id(&self) -> &ArtifactId {
        &self.model_artifact_id
    }

    /// Returns the portable identity of the actual canonical native closure.
    #[must_use]
    pub const fn worker_portable_closure_digest(&self) -> &Digest {
        &self.worker_portable_closure_digest
    }

    /// Returns the normalized worker output and execution configuration.
    #[must_use]
    pub const fn worker_portable_configuration_digest(&self) -> &Digest {
        &self.worker_portable_configuration_digest
    }

    /// Returns the effective context capacity observed around generation.
    #[must_use]
    pub const fn effective_context_tokens(&self) -> u32 {
        self.effective_context_tokens
    }

    /// Returns the bounded observed compute backend.
    #[must_use]
    pub const fn compute_backend(&self) -> ComputeBackend {
        self.compute_backend
    }

    /// Returns the bounded observed placement class.
    #[must_use]
    pub const fn placement(&self) -> ExecutionPlacement {
        self.placement
    }

    /// Returns the canonical bounded execution-class digest.
    #[must_use]
    pub const fn execution_class_digest(&self) -> &Digest {
        &self.execution_class_digest
    }

    /// Returns the per-attempt isolation, worker, mapping, and residency binding.
    #[must_use]
    pub const fn observation_binding_digest(&self) -> &Digest {
        &self.observation_binding_digest
    }

    /// Always false because the observations are not formal placement proof.
    #[must_use]
    pub const fn formal_placement_proven(&self) -> bool {
        false
    }

    /// Always false because bounded mapping and residency do not prove model use.
    #[must_use]
    pub const fn model_use_proven(&self) -> bool {
        false
    }

    /// Always false because process and transport evidence do not identify a handler.
    #[must_use]
    pub const fn application_handler_proven(&self) -> bool {
        false
    }

    /// Always false because a leaf observation cannot grant qualification.
    #[must_use]
    pub const fn qualified(&self) -> bool {
        false
    }
}

/// Failure while deriving one inert effective-state leaf observation.
#[derive(Debug, Error)]
pub enum EffectiveRuntimeStateObservationError {
    /// One or more source records name another runtime, model, request, or bracket.
    #[error("effective runtime-state observation relationship is invalid")]
    RelationshipMismatch,
    /// The runtime or target is outside the closed Ollama v0.32.15 Linux CPU profile.
    #[error("effective runtime-state observation profile is unsupported")]
    UnsupportedProfile,
    /// Cancellation was observed before a stable leaf could be constructed.
    #[error("effective runtime-state observation was cancelled")]
    Cancelled,
    /// One fixed kernel metadata record could not be opened or read.
    #[error("bounded Linux kernel observation failed")]
    PlatformIo(#[source] std::io::Error),
    /// Kernel metadata was empty, malformed, or over its fixed byte ceiling.
    #[error("bounded Linux kernel observation is invalid")]
    InvalidPlatformObservation,
    /// Kernel metadata changed between the two confirming observations.
    #[error("bounded Linux kernel observation changed")]
    PlatformObservationChanged,
    /// The retained launch, private input tree, or concrete model lease changed.
    #[error("live effective runtime-state reobservation failed")]
    LiveReobservation(#[source] crate::ManagedOllamaLaunchError),
    /// The fully observed relationships could not form a structural state record.
    #[error("observed effective runtime state is structurally invalid")]
    InvalidEffectiveState(#[source] EffectiveRuntimeStateError),
}
