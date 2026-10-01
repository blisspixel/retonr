//! Inert two-operation runtime-admission observation foundation.

use std::{ffi::OsStr, net::TcpStream};

use rewrite_inference::{InferenceError, OperationContext};
use rewrite_model::{
    ArtifactSetId, NativeLoadObservation, RuntimeOperatingSystem, RuntimePackageManifest,
    RuntimePackageManifestId, RuntimePackageMemberRole,
};
use rewrite_ollama::{
    OllamaCloudDisableEvidenceError, OllamaCloudDisableFeaturePolicy,
    OllamaCloudDisableStartupMarker, OllamaCloudDisableVersionStatus, OllamaEndpoint, OllamaLimits,
    OllamaManagedCloudDisableEnvironment, OllamaObservedRuntimeProbeError,
    OllamaResponseObservationPhase, OllamaRuntimeProbeEvidence, OllamaSingleConnectionRuntimeProbe,
    OllamaVersion,
};
use rewrite_ollama_package::ADMITTED_RUNTIME_FAMILY;
use rewrite_runtime_attestor::{
    AttachedProcessEvidence, AttachedProcessEvidenceClass, AttachedProcessLaunchMode,
    AttachedProcessLease, AttachedProcessWitnessError, AttachedProcessWitnessLimits,
    FrozenExternalNativeComponentSetId, ListenerEndpoint, MAXIMUM_DESCRIPTORS_PER_PROCESS,
    MAXIMUM_ENTRYPOINT_BYTES, MAXIMUM_NATIVE_LOAD_HASH_BYTES,
    MAXIMUM_NATIVE_LOAD_OBSERVATION_MILLIS, MAXIMUM_NATIVE_LOADED_COMPONENTS,
    MAXIMUM_NATIVE_MAPPING_METADATA_BYTES, MAXIMUM_NATIVE_MAPPING_REGIONS,
    MAXIMUM_OBSERVATION_MILLIS, MAXIMUM_OBSERVED_PROCESSES, MAXIMUM_SOCKET_TABLE_BYTES,
    MAXIMUM_SOCKET_TABLE_ENTRIES, ManagedLinuxProcessExpectation, NativeLoadDiscovery,
    NativeLoadDiscoveryRequest, NativeLoadObservationLimits, NativeLoadObservationRequest,
    NativeLoadObserverError, NativeManagedLinuxProcessObserver, RetainedNativePackageMember,
    RetainedTcpConnection, RetainedTcpConnectionEvidence, VerifiedFrozenExternalNativeComponentSet,
};
use rewrite_runtime_isolation::{
    IsolationError, IsolationEvidence, LaunchSpec, ManagedStartupOutput, RetainedIsolationLease,
};
use rewrite_types::{CancellationToken, Digest};
use thiserror::Error;

use crate::{
    PACKAGE_ATTESTATION_SCHEMA_VERSION, PackageAttestationError, PackageAttestationScope,
    RuntimePackageAttestationEvidence, RuntimePackageLease,
};

mod discovery;
mod observed_records;
mod operation;
mod probe;
mod records;
mod validation;
mod verify;

pub use observed_records::RuntimeAdmissionObservedFinalOperation;
pub use operation::{
    RuntimeAdmissionFinalOperation, RuntimeAdmissionFinalOperationRequest,
    RuntimeAdmissionNativeClosureDiscovery, RuntimeAdmissionNativeClosureDiscoveryRequest,
};
pub use records::{
    MAX_RUNTIME_ADMISSION_MANAGED_FINAL_RECORD_BYTES,
    MAX_RUNTIME_ADMISSION_NATIVE_LOAD_RECORD_BYTES, RuntimeAdmissionManagedFinalRecord,
    RuntimeAdmissionManagedFinalRecordId, RuntimeAdmissionManagedFinalRecordVerifier,
    RuntimeAdmissionNativeLoadRecord, RuntimeAdmissionNativeLoadRecordId,
    RuntimeAdmissionNativeLoadRecordVerifier, VerifiedPassedRuntimeAdmissionCloudDisableControl,
    VerifiedPassedRuntimeAdmissionManagedStartupControl,
    VerifiedPassedRuntimeAdmissionNativeClosureControl,
};
#[cfg(test)]
pub(crate) use records::{
    test_execution_control_fixtures, test_execution_control_fixtures_with_launch,
};

#[cfg(test)]
use discovery::discover_with_retained_members;
#[cfg(test)]
use probe::run_exact_probe;
#[cfg(test)]
use validation::{
    compile_managed_startup_evidence, validate_actual_launch_binding,
    validate_final_identity_bindings, validate_native_binding,
};

/// Caller-owned ceilings for both admission observation operations.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimeAdmissionRunnerLimits {
    /// Bounds the managed listener, process, and connection witness.
    pub managed_process: AttachedProcessWitnessLimits,
    /// Bounds native loaded-component discovery and verification.
    pub native_load: NativeLoadObservationLimits,
    /// Bounds the exact one-request Ollama runtime probe.
    pub ollama: OllamaLimits,
}

/// Complete caller selection for one fresh managed final verification.
///
/// The managed runtime capability exclusively owns the process lifecycle. The
/// remaining fields are inert expected values and explicit resource bounds.
pub struct RuntimeAdmissionFinalVerificationRequest<'a> {
    /// Exact typed package expected to back the managed runtime.
    pub package: &'a RuntimePackageManifest,
    /// Retained exact managed package member capabilities.
    pub package_lease: &'a mut RuntimePackageLease,
    /// Exclusive retained managed runtime lifecycle capability.
    pub managed_runtime: &'a mut RuntimeAdmissionManagedRuntimeLease,
    /// Independently reviewed frozen external native-component set.
    pub frozen: &'a VerifiedFrozenExternalNativeComponentSet,
    /// Exact loopback endpoint requested through managed isolation.
    pub endpoint: OllamaEndpoint,
    /// Reviewed launch declaration whose digest must match the actual launch.
    pub launch: &'a LaunchSpec,
    /// Explicit native observation and request ceilings.
    pub limits: RuntimeAdmissionRunnerLimits,
    /// Cooperative cancellation observed at every expensive boundary.
    pub cancellation: &'a CancellationToken,
}

/// Exclusive application-owned lifecycle capability for one managed runtime.
///
/// Construction consumes one retained isolation lease. Final verification can
/// obtain its stream, diagnostics, startup output, and isolation observations
/// only through this capability, so those facts cannot be cross-wired from
/// different launches. The caller retains explicit teardown authority after
/// both successful and failed verification attempts.
pub struct RuntimeAdmissionManagedRuntimeLease {
    isolation: RetainedIsolationLease,
}

impl RuntimeAdmissionManagedRuntimeLease {
    /// Takes exclusive ownership of one retained managed runtime.
    #[must_use]
    pub const fn new(isolation: RetainedIsolationLease) -> Self {
        Self { isolation }
    }

    /// Returns the retained digest of the exact launch description.
    #[must_use]
    pub const fn launch_spec_digest(&self) -> &Digest {
        self.isolation.launch_spec_digest()
    }

    /// Returns the retained digest of the exact isolation policy.
    #[must_use]
    pub const fn isolation_policy_digest(&self) -> &Digest {
        self.isolation.isolation_policy_digest()
    }

    /// Terminates and reaps the complete managed process tree.
    ///
    /// # Errors
    ///
    /// Returns [`IsolationError`] if cancellation was already active or the
    /// complete process tree cannot be reaped within its retained policy bound.
    pub fn close(self, cancellation: &CancellationToken) -> Result<(), IsolationError> {
        self.isolation.close(cancellation)
    }
}

impl std::fmt::Debug for RuntimeAdmissionManagedRuntimeLease {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeAdmissionManagedRuntimeLease")
            .field("launch_spec_digest", &self.launch_spec_digest())
            .field("isolation_policy_digest", &self.isolation_policy_digest())
            .finish_non_exhaustive()
    }
}

/// Typed evidence from the exact managed launch description and bounded startup streams.
///
/// This value is inert and contains no raw environment value, path, argument, or startup text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAdmissionManagedStartupEvidence {
    runtime_package_manifest_id: RuntimePackageManifestId,
    process_evidence_digest: Digest,
    launch_spec_digest: Digest,
    isolation_policy_digest: Digest,
    standard_output_digest: Digest,
    standard_error_digest: Digest,
    standard_output_bytes: u64,
    standard_error_bytes: u64,
}

impl RuntimeAdmissionManagedStartupEvidence {
    /// Returns the exact runtime package observed at startup.
    #[must_use]
    pub const fn runtime_package_manifest_id(&self) -> &RuntimePackageManifestId {
        &self.runtime_package_manifest_id
    }

    /// Returns the retained managed-process evidence binding.
    #[must_use]
    pub const fn process_evidence_digest(&self) -> &Digest {
        &self.process_evidence_digest
    }

    /// Returns the redacted digest of the exact managed launch description.
    #[must_use]
    pub const fn launch_spec_digest(&self) -> &Digest {
        &self.launch_spec_digest
    }

    /// Returns the retained digest of the exact managed isolation policy.
    #[must_use]
    pub const fn isolation_policy_digest(&self) -> &Digest {
        &self.isolation_policy_digest
    }

    /// Returns the digest of the complete retained standard-output prefix.
    #[must_use]
    pub const fn standard_output_digest(&self) -> &Digest {
        &self.standard_output_digest
    }

    /// Returns the digest of the complete retained standard-error prefix.
    #[must_use]
    pub const fn standard_error_digest(&self) -> &Digest {
        &self.standard_error_digest
    }

    /// Returns the retained standard-output byte count.
    #[must_use]
    pub const fn standard_output_bytes(&self) -> u64 {
        self.standard_output_bytes
    }

    /// Returns the retained standard-error byte count.
    #[must_use]
    pub const fn standard_error_bytes(&self) -> u64 {
        self.standard_error_bytes
    }
}

/// Exact live cloud-disable observations joined to one package and runtime version.
///
/// This value records validated observations only. It does not mutate the reviewed-runtime
/// allowlist and does not grant admission or generation authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAdmissionCloudDisableLiveEvidence {
    runtime_package_manifest_id: RuntimePackageManifestId,
    runtime_version: OllamaVersion,
    version_status: OllamaCloudDisableVersionStatus,
    managed_environment: OllamaManagedCloudDisableEnvironment,
    startup_marker: OllamaCloudDisableStartupMarker,
}

impl RuntimeAdmissionCloudDisableLiveEvidence {
    /// Returns the exact runtime package joined to these live observations.
    #[must_use]
    pub const fn runtime_package_manifest_id(&self) -> &RuntimePackageManifestId {
        &self.runtime_package_manifest_id
    }

    /// Returns the exact version observed by the one-request live probe.
    #[must_use]
    pub const fn runtime_version(&self) -> OllamaVersion {
        self.runtime_version
    }

    /// Returns the production review status without granting that status authority here.
    #[must_use]
    pub const fn version_status(&self) -> OllamaCloudDisableVersionStatus {
        self.version_status
    }

    /// Returns the typed exact managed-environment observation.
    #[must_use]
    pub const fn managed_environment(&self) -> OllamaManagedCloudDisableEnvironment {
        self.managed_environment
    }

    /// Returns the typed exact startup-marker observation.
    #[must_use]
    pub const fn startup_marker(&self) -> OllamaCloudDisableStartupMarker {
        self.startup_marker
    }
}

/// Complete inert result of the fresh frozen-set-gated final verification operation.
///
/// The result is intentionally not serializable and has no all-pass, admitted, qualified,
/// allowed, or generation-capable state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAdmissionFinalVerification {
    runtime_package_manifest_id: RuntimePackageManifestId,
    frozen_external_component_set_id: FrozenExternalNativeComponentSetId,
    isolation_policy_digest: Digest,
    initial_isolation_evidence: IsolationEvidence,
    final_isolation_evidence: IsolationEvidence,
    initial_process_evidence: AttachedProcessEvidence,
    final_process_evidence: AttachedProcessEvidence,
    initial_connection_evidence: RetainedTcpConnectionEvidence,
    final_connection_evidence: RetainedTcpConnectionEvidence,
    native_load: NativeLoadObservation,
    runtime_probe: OllamaRuntimeProbeEvidence,
    managed_startup: RuntimeAdmissionManagedStartupEvidence,
    cloud_disable: RuntimeAdmissionCloudDisableLiveEvidence,
}

impl RuntimeAdmissionFinalVerification {
    /// Returns the exact verified runtime-package identity.
    #[must_use]
    pub const fn runtime_package_manifest_id(&self) -> &RuntimePackageManifestId {
        &self.runtime_package_manifest_id
    }

    /// Returns the exact independently verified frozen component-set identity.
    #[must_use]
    pub const fn frozen_external_component_set_id(&self) -> &FrozenExternalNativeComponentSetId {
        &self.frozen_external_component_set_id
    }

    /// Returns the retained digest of the exact managed isolation policy.
    #[must_use]
    pub const fn isolation_policy_digest(&self) -> &Digest {
        &self.isolation_policy_digest
    }

    /// Returns the initial evidence from the exact retained isolation lease.
    #[must_use]
    pub const fn initial_isolation_evidence(&self) -> &IsolationEvidence {
        &self.initial_isolation_evidence
    }

    /// Returns the final reobservation of the same retained isolation lease.
    #[must_use]
    pub const fn final_isolation_evidence(&self) -> &IsolationEvidence {
        &self.final_isolation_evidence
    }

    /// Returns the initial retained managed-process evidence.
    #[must_use]
    pub const fn initial_process_evidence(&self) -> &AttachedProcessEvidence {
        &self.initial_process_evidence
    }

    /// Returns the final fresh managed-process reobservation.
    #[must_use]
    pub const fn final_process_evidence(&self) -> &AttachedProcessEvidence {
        &self.final_process_evidence
    }

    /// Returns the pre-request exact retained-connection evidence.
    #[must_use]
    pub const fn initial_connection_evidence(&self) -> &RetainedTcpConnectionEvidence {
        &self.initial_connection_evidence
    }

    /// Returns the post-response exact retained-connection evidence.
    #[must_use]
    pub const fn final_connection_evidence(&self) -> &RetainedTcpConnectionEvidence {
        &self.final_connection_evidence
    }

    /// Returns fresh native-closure evidence created only from the frozen expected set.
    #[must_use]
    pub const fn native_load(&self) -> &NativeLoadObservation {
        &self.native_load
    }

    /// Returns the exact one-request `/api/version` probe evidence.
    #[must_use]
    pub const fn runtime_probe(&self) -> OllamaRuntimeProbeEvidence {
        self.runtime_probe
    }

    /// Returns typed managed-startup evidence.
    #[must_use]
    pub const fn managed_startup(&self) -> &RuntimeAdmissionManagedStartupEvidence {
        &self.managed_startup
    }

    /// Returns exact live cloud-disable evidence.
    #[must_use]
    pub const fn cloud_disable(&self) -> &RuntimeAdmissionCloudDisableLiveEvidence {
        &self.cloud_disable
    }
}

/// Failure from the inert runtime-admission observation foundation.
#[derive(Debug, Error)]
pub enum RuntimeAdmissionRunnerError {
    /// The package is not an exact Linux Ollama runtime with a stable version.
    #[error("runtime admission input is invalid")]
    InvalidInput,
    /// The supplied retained package lease is for different or inconsistent static bytes.
    #[error("runtime admission package binding is invalid")]
    InvalidPackageBinding,
    /// The verified frozen component set belongs to another runtime package.
    #[error("runtime admission frozen native-component set binding is invalid")]
    InvalidFrozenSetBinding,
    /// The supplied process is not the exact managed package entrypoint.
    #[error("runtime admission managed process binding is invalid")]
    InvalidProcessBinding,
    /// The reviewed launch description differs from the exact retained launch.
    #[error("runtime admission managed launch binding is invalid")]
    InvalidLaunchBinding,
    /// Observer output was stale or did not preserve its exact package/process binding.
    #[error("runtime admission live evidence binding is invalid")]
    InvalidEvidenceBinding,
    /// Startup output was truncated and cannot support an exact marker observation.
    #[error("runtime admission managed startup output was truncated")]
    TruncatedStartupOutput,
    /// The response observation callback sequence differed from one exact response.
    #[error("runtime admission response observation sequence is invalid")]
    InvalidResponseObservationSequence,
    /// Canonical execution publication bytes were malformed or noncanonical.
    #[error("runtime admission execution record encoding is invalid")]
    InvalidRecordEncoding,
    /// Canonical execution publication bytes exceeded their fixed ceiling.
    #[error("runtime admission execution record exceeds its limit")]
    RecordLimitExceeded,
    /// Cancellation was observed before more admission observation work began.
    #[error("runtime admission observation was cancelled")]
    Cancelled,
    /// The exact managed cloud-disable declaration or marker was invalid.
    #[error("runtime admission cloud-disable evidence is invalid: {0}")]
    CloudDisable(#[source] OllamaCloudDisableEvidenceError),
    /// The exact runtime version predates the supported cloud-disable feature.
    #[error("runtime admission cloud-disable feature is unavailable")]
    CloudDisableFeatureUnavailable,
    /// The exact runtime is absent from the production cloud-disable review policy.
    #[error("runtime admission cloud-disable runtime is unreviewed")]
    CloudDisableRuntimeUnreviewed,
    /// Static retained package revalidation failed.
    #[error("runtime admission package revalidation failed: {0}")]
    Package(#[source] PackageAttestationError),
    /// Managed process or exact-connection observation failed.
    #[error("runtime admission native witness failed: {0}")]
    Witness(#[source] AttachedProcessWitnessError),
    /// Native component discovery or final frozen-set verification failed.
    #[error("runtime admission native-load observation failed: {0}")]
    NativeLoad(#[source] NativeLoadObserverError),
    /// Managed isolation channel acquisition or final reobservation failed.
    #[error("runtime admission managed isolation failed: {0}")]
    Isolation(#[source] IsolationError),
    /// A launched managed runtime could not be completely terminated and reaped.
    #[error("runtime admission managed runtime cleanup failed: {0}")]
    Cleanup(#[source] IsolationError),
    /// Admission failed and the launched runtime also could not be completely reaped.
    #[error("runtime admission operation and managed runtime cleanup both failed")]
    CleanupAfterFailure {
        /// The primary admission failure.
        operation: Box<RuntimeAdmissionRunnerError>,
        /// The independent cleanup failure.
        cleanup: IsolationError,
    },
    /// Operation or cleanup failed and mandatory package finalization also failed.
    #[error("runtime admission operation and package finalization both failed")]
    FinalizationAfterFailure {
        /// The independent operation or cleanup failure.
        operation: Box<RuntimeAdmissionRunnerError>,
        /// The independent package finalization failure.
        finalization: Box<RuntimeAdmissionRunnerError>,
    },
    /// The exact one-request Ollama runtime probe failed.
    #[error("runtime admission Ollama runtime probe failed: {0}")]
    Probe(#[source] InferenceError),
}

/// Stateless runner for the two deliberately separate admission observation operations.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimeAdmissionRunner;

#[cfg(test)]
#[path = "runtime_admission_runner/tests.rs"]
mod tests;
