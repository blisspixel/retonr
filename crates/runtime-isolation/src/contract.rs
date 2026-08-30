use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    fs::File,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

use rewrite_types::{CancellationToken, Digest};

use crate::{IsolationError, IsolationResult, platform};

mod bootstrap;
mod build;
mod build_evidence;
mod build_output;
mod channel;
mod digest;
mod evidence;
mod policy;
mod runtime_input;
mod validation;

pub use bootstrap::{
    AuthenticatedBusyboxExecutable, RetainedProgramBootstrapAttempt,
    RetainedProgramBootstrapCapabilities, RetainedProgramBootstrapExecution,
    RetainedProgramBootstrapInputKind, RetainedProgramBootstrapInputMeasurement,
    RetainedProgramBootstrapLaunchSpec, RetainedProgramBootstrapRootEvidence,
    RetainedProgramBootstrapSignedInputs,
};
#[cfg(target_os = "linux")]
pub(crate) use bootstrap::{
    RetainedProgramBootstrapRootObservation, RetainedProgramBootstrapRootPostconditions,
};
#[cfg(target_os = "linux")]
pub(crate) use build::CONTROLLED_BUILD_INPUT_SNAPSHOT_TIMEOUT;
pub use build::{
    ControlledBuildExecution, ControlledBuildInputFile, ControlledBuildLaunchSpec,
    ControlledBuildOutput, ControlledBuildProcessStatus, MAXIMUM_CONTROLLED_BUILD_INPUT_BYTES,
    MAXIMUM_CONTROLLED_BUILD_INPUT_FILES,
};
pub use build_evidence::ControlledBuildIsolationEvidence;
#[cfg(target_os = "linux")]
pub(crate) use build_evidence::ControlledBuildIsolationObservation;
pub use build_output::{
    ControlledBuildOutputTree, ControlledBuildOutputTreeEntry,
    MAXIMUM_CONTROLLED_BUILD_OUTPUT_BYTES, MAXIMUM_CONTROLLED_BUILD_OUTPUT_TREE_ENTRIES,
    MAXIMUM_CONTROLLED_BUILD_WORKSPACE_BYTES, MAXIMUM_CONTROLLED_BUILD_WORKSPACE_INODES,
};
pub use channel::{
    LinuxSocketDiagnosticsCapability, MAXIMUM_STARTUP_STREAM_BYTES, ManagedLoopbackChannel,
    ManagedStartupOutput,
};
use digest::RedactedDigestBuilder;
#[cfg(target_os = "linux")]
pub(crate) use evidence::ManagedNamespaceEvidence;
pub use evidence::{
    IsolationEvidence, IsolationPreparationEvidence, ManagedDeviceBoundaryEvidence,
    NamespaceIdentity, TargetProcessEvidence,
};
pub use policy::{IsolationPolicy, ManagedDeviceVisibilityPolicy};
pub use runtime_input::{
    MANAGED_RUNTIME_INPUT_ROOT_V1, MAXIMUM_MANAGED_RUNTIME_INPUT_BYTES,
    MAXIMUM_MANAGED_RUNTIME_INPUT_FILES, ManagedRuntimeInputEvidence, RetainedRuntimeInputSink,
    RetainedRuntimeInputSource, RetainedRuntimeInputTree,
};
#[cfg(target_os = "linux")]
pub(crate) use runtime_input::{
    RetainedRuntimeInputDeclaration, RetainedRuntimeInputMember, RuntimeInputObjectIdentity,
    runtime_input_file_digest, runtime_input_layout_digest,
};
use validation::{validate_absolute_path, validate_environment_key, validate_value};

/// A bounded, explicit target process description.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchSpec {
    executable: PathBuf,
    arguments: Vec<OsString>,
    environment: BTreeMap<OsString, OsString>,
    current_directory: Option<PathBuf>,
}

impl LaunchSpec {
    /// Creates an empty managed launch for an explicit executable path.
    #[must_use]
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            arguments: Vec::new(),
            environment: BTreeMap::new(),
            current_directory: None,
        }
    }

    /// Appends one target argument.
    pub fn push_argument(&mut self, argument: impl Into<OsString>) {
        self.arguments.push(argument.into());
    }

    /// Inserts one target environment variable into an otherwise cleared environment.
    pub fn insert_environment(&mut self, key: impl Into<OsString>, value: impl Into<OsString>) {
        self.environment.insert(key.into(), value.into());
    }

    /// Sets the target working directory.
    pub fn set_current_directory(&mut self, directory: impl Into<PathBuf>) {
        self.current_directory = Some(directory.into());
    }

    /// Returns the exact value from the cleared launch environment for `key`.
    #[must_use]
    pub fn environment_value(&self, key: &OsStr) -> Option<&OsStr> {
        self.environment.get(key).map(OsString::as_os_str)
    }

    /// Returns the exact diagnostic executable path committed by this specification.
    #[must_use]
    pub fn executable(&self) -> &Path {
        &self.executable
    }

    /// Returns the exact ordered target arguments.
    #[must_use]
    pub fn arguments(&self) -> &[OsString] {
        &self.arguments
    }

    /// Returns the number of entries in the otherwise cleared environment.
    #[must_use]
    pub fn environment_count(&self) -> usize {
        self.environment.len()
    }

    /// Returns the exact target working directory when one was explicitly set.
    #[must_use]
    pub fn current_directory(&self) -> Option<&Path> {
        self.current_directory.as_deref()
    }

    /// Returns a domain-separated digest of the exact launch description.
    ///
    /// The digest commits to the executable path, ordered arguments, cleared
    /// environment block, and optional current directory without exposing them.
    #[must_use]
    pub fn redacted_digest(&self) -> rewrite_types::Digest {
        let mut digest = RedactedDigestBuilder::new(b"runtime-isolation/launch-spec/v1");
        digest.push_bytes(self.executable.as_os_str().as_encoded_bytes());
        digest.push_usize(self.arguments.len());
        for argument in &self.arguments {
            digest.push_bytes(argument.as_encoded_bytes());
        }
        digest.push_usize(self.environment.len());
        for (key, value) in &self.environment {
            digest.push_bytes(key.as_encoded_bytes());
            digest.push_bytes(value.as_encoded_bytes());
        }
        digest.push_bool(self.current_directory.is_some());
        if let Some(directory) = &self.current_directory {
            digest.push_bytes(directory.as_os_str().as_encoded_bytes());
        }
        digest.finish()
    }

    pub(crate) fn validate(&self, policy: IsolationPolicy) -> IsolationResult<()> {
        validate_absolute_path(&self.executable, "executable path")?;
        if self.arguments.len() > policy.maximum_arguments() {
            return Err(IsolationError::InvalidLaunch("argument count"));
        }
        if self.environment.len() > policy.maximum_environment_variables() {
            return Err(IsolationError::InvalidLaunch("environment count"));
        }
        for value in &self.arguments {
            validate_value(value, policy.maximum_value_bytes())?;
        }
        for (key, value) in &self.environment {
            validate_environment_key(key, policy.maximum_value_bytes())?;
            validate_value(value, policy.maximum_value_bytes())?;
        }
        if let Some(directory) = &self.current_directory {
            validate_absolute_path(directory, "current directory")?;
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn environment(&self) -> &BTreeMap<OsString, OsString> {
        &self.environment
    }
}

/// Prepared, actively probed platform isolation capability.
#[derive(Debug)]
pub struct PreparedIsolation {
    policy: IsolationPolicy,
    preparation: IsolationPreparationEvidence,
    platform: platform::Prepared,
    subject: PreparedIsolationSubjectToken,
}

/// Opaque identity for one exact prepared isolation authority.
#[derive(Clone, Debug)]
#[doc(hidden)]
pub struct PreparedIsolationSubjectToken(Arc<()>);

impl PreparedIsolationSubjectToken {
    /// Tests exact in-process authority identity without exposing identity material.
    #[doc(hidden)]
    #[must_use]
    pub fn binds_exact(&self, prepared: &PreparedIsolation) -> bool {
        Arc::ptr_eq(&self.0, &prepared.subject.0)
    }
}

impl PreparedIsolation {
    /// Constructs an inert prepared-isolation fixture with one exact policy.
    ///
    /// This helper is absent unless the `test-support` feature is enabled. The
    /// returned value exposes policy evidence but rejects every launch operation.
    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    #[must_use]
    pub fn test_support_from_policy(policy: IsolationPolicy) -> Self {
        let preparation = IsolationPreparationEvidence {
            loopback_interface_index: 1,
            canary_protocol_version: 1,
            device_canary_protocol_version: 1,
            runtime_input_canary_protocol_version: 1,
            managed_device_visibility: policy.managed_device_visibility(),
            helper_digest: rewrite_types::Digest::sha256(b"test-support-isolation-helper"),
            helper_bytes: 1,
        };
        Self {
            policy,
            platform: platform::test_support_prepared(preparation.clone()),
            preparation,
            subject: PreparedIsolationSubjectToken(Arc::new(())),
        }
    }

    /// Snapshots an expected helper and actively probes the complete isolation path.
    ///
    /// # Errors
    ///
    /// Returns a bounded isolation error when cancellation is active, the
    /// helper is invalid, host namespace policy denies the operation, or any
    /// preparation invariant cannot be verified.
    pub fn prepare(
        helper_executable: impl AsRef<Path>,
        expected_digest: &Digest,
        expected_bytes: u64,
        policy: IsolationPolicy,
        cancellation: &CancellationToken,
    ) -> IsolationResult<Self> {
        policy.validate()?;
        let (platform, preparation) = platform::prepare(
            helper_executable.as_ref(),
            expected_digest,
            expected_bytes,
            policy,
            cancellation,
        )?;
        Ok(Self {
            policy,
            preparation,
            platform,
            subject: PreparedIsolationSubjectToken(Arc::new(())),
        })
    }

    /// Snapshots and probes an already-open helper without reopening its path.
    ///
    /// # Errors
    ///
    /// Returns a bounded isolation error when cancellation is active, the retained
    /// object is not a regular executable, host policy denies isolation, or the
    /// complete preparation probe cannot be verified.
    pub fn prepare_retained(
        helper_executable: File,
        expected_digest: &Digest,
        expected_bytes: u64,
        policy: IsolationPolicy,
        cancellation: &CancellationToken,
    ) -> IsolationResult<Self> {
        policy.validate()?;
        let (platform, preparation) = platform::prepare_retained(
            helper_executable,
            expected_digest,
            expected_bytes,
            policy,
            cancellation,
        )?;
        Ok(Self {
            policy,
            preparation,
            platform,
            subject: PreparedIsolationSubjectToken(Arc::new(())),
        })
    }

    /// Returns evidence from the active preparation probe.
    #[must_use]
    pub fn preparation_evidence(&self) -> IsolationPreparationEvidence {
        self.preparation.clone()
    }

    /// Returns a domain-separated digest of the exact retained isolation policy.
    #[must_use]
    pub fn policy_digest(&self) -> rewrite_types::Digest {
        self.policy.redacted_digest()
    }

    /// Returns an opaque identity token for cross-crate owning joins.
    #[doc(hidden)]
    #[must_use]
    pub fn subject_token(&self) -> PreparedIsolationSubjectToken {
        self.subject.clone()
    }

    /// Launches a target only after isolation is established and verified.
    ///
    /// # Errors
    ///
    /// Returns a bounded isolation error when the launch description is
    /// invalid, cancellation is active, the helper fails, or retained native
    /// evidence cannot be established.
    pub fn launch(
        &self,
        specification: &LaunchSpec,
        cancellation: &CancellationToken,
    ) -> IsolationResult<RetainedIsolationLease> {
        specification.validate(self.policy)?;
        let launch_spec_digest = specification.redacted_digest();
        let isolation_policy_digest = self.policy.redacted_digest();
        let platform = self
            .platform
            .launch(specification, self.policy, cancellation, None)?;
        Ok(RetainedIsolationLease {
            platform,
            launch_spec_digest,
            isolation_policy_digest,
        })
    }

    /// Launches an already-open executable object without reopening its pathname.
    ///
    /// The launch description still supplies arguments, environment, and the path
    /// used for validation and diagnostics. On Linux, execution and target identity
    /// bind directly to `executable`. Unsupported platforms return without inspecting
    /// or opening the described executable path.
    ///
    /// # Errors
    ///
    /// Returns a bounded isolation error when the retained object is not a regular
    /// executable, validation fails, cancellation is active, or the platform cannot
    /// establish the managed launch.
    pub fn launch_retained(
        &self,
        specification: &LaunchSpec,
        executable: File,
        cancellation: &CancellationToken,
    ) -> IsolationResult<RetainedIsolationLease> {
        specification.validate(self.policy)?;
        let launch_spec_digest = specification.redacted_digest();
        let isolation_policy_digest = self.policy.redacted_digest();
        let platform = self.platform.launch_retained(
            specification,
            executable,
            self.policy,
            cancellation,
            None,
        )?;
        Ok(RetainedIsolationLease {
            platform,
            launch_spec_digest,
            isolation_policy_digest,
        })
    }

    /// Launches an already-open executable with a nonempty retained runtime input
    /// tree while preserving an already-captured absolute operation deadline.
    ///
    /// The platform uses the earlier of `operation_deadline` and its fixed local
    /// startup ceiling. The supplied deadline is never extended or recaptured.
    /// Cleanup after a failed launch retains its independent shutdown bound.
    ///
    /// # Errors
    ///
    /// Returns [`IsolationError::OperationDeadlineExceeded`] at or after the
    /// supplied deadline, or the same validation and launch errors as
    /// [`Self::launch_retained_with_inputs`].
    pub fn launch_retained_with_inputs_until(
        &self,
        specification: &LaunchSpec,
        executable: File,
        inputs: RetainedRuntimeInputTree,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> IsolationResult<RetainedIsolationLease> {
        specification.validate(self.policy)?;
        inputs.require_nonempty()?;
        let launch_spec_digest = inputs.launch_digest(&specification.redacted_digest());
        let isolation_policy_digest = self.policy.redacted_digest();
        let platform = self.platform.launch_retained_with_inputs(
            specification,
            executable,
            inputs,
            self.policy,
            cancellation,
            Some(operation_deadline),
        )?;
        Ok(RetainedIsolationLease {
            platform,
            launch_spec_digest,
            isolation_policy_digest,
        })
    }

    /// Launches an already-open executable with a nonempty retained runtime input tree.
    ///
    /// The Linux helper materializes each content-bound retained descriptor at
    /// the fixed [`MANAGED_RUNTIME_INPUT_ROOT_V1`] alias. It transfers no caller
    /// filesystem path and writes no host path. Evidence establishes retained
    /// source identity plus a byte-identical private read-only view, not shared
    /// inode identity, model use, or model semantics.
    ///
    /// Nonempty input bytes are materialized in a private tmpfs. Its 129 GiB
    /// bound is a ceiling rather than eager allocation, but a 17-18 GiB model
    /// still requires corresponding memory or swap-backed tmpfs capacity during
    /// the lease. Callers must treat that resource cost as an admission concern.
    ///
    /// # Errors
    ///
    /// Returns a bounded isolation error when the input capability is empty,
    /// reordered, changed, outside its fixed bounds, or cannot be mounted and
    /// reobserved exactly.
    pub fn launch_retained_with_inputs(
        &self,
        specification: &LaunchSpec,
        executable: File,
        inputs: RetainedRuntimeInputTree,
        cancellation: &CancellationToken,
    ) -> IsolationResult<RetainedIsolationLease> {
        specification.validate(self.policy)?;
        inputs.require_nonempty()?;
        let launch_spec_digest = inputs.launch_digest(&specification.redacted_digest());
        let isolation_policy_digest = self.policy.redacted_digest();
        let platform = self.platform.launch_retained_with_inputs(
            specification,
            executable,
            inputs,
            self.policy,
            cancellation,
            None,
        )?;
        Ok(RetainedIsolationLease {
            platform,
            launch_spec_digest,
            isolation_policy_digest,
        })
    }

    /// Runs one retained build program with a read-only input capability and an
    /// initially empty writable output capability.
    ///
    /// The Linux implementation adds a private mount namespace and Landlock policy
    /// to the managed user, PID, and network namespaces. It validates the complete
    /// retained input mapping against `input_root`, then constructs the target-facing
    /// input tree only from those file descriptors. The mutable root is not passed to
    /// either helper stage or the target.
    ///
    /// # Errors
    ///
    /// Returns a bounded isolation error when validation, retained-object
    /// identity, filesystem confinement, cancellation, execution, or cleanup fails.
    pub fn run_controlled_build_retained(
        &self,
        specification: &ControlledBuildLaunchSpec,
        program: File,
        input_root: File,
        input_files: Vec<ControlledBuildInputFile>,
        output_root: File,
        cancellation: &CancellationToken,
    ) -> IsolationResult<ControlledBuildExecution> {
        specification.validate(self.policy)?;
        self.platform.run_controlled_build_retained(
            specification,
            program,
            input_root,
            input_files,
            output_root,
            self.policy,
            cancellation,
        )
    }

    /// Runs one retained-program bootstrap attempt through its distinct root
    /// preparation and controlled-build protocol.
    ///
    /// This path cannot fall back to the ordinary controlled-build mode. The
    /// result is created only after root transition, old-root detachment,
    /// read-only mount, host-path, privilege, Landlock, seccomp, and output
    /// postconditions are observed.
    ///
    /// # Errors
    ///
    /// Returns a bounded isolation error for any invalid input join, unavailable
    /// root-preparation operation, failed postcondition, cancellation, or build failure.
    pub fn run_retained_program_bootstrap(
        &self,
        specification: &RetainedProgramBootstrapLaunchSpec,
        capabilities: RetainedProgramBootstrapCapabilities,
        cancellation: &CancellationToken,
    ) -> IsolationResult<RetainedProgramBootstrapExecution> {
        specification.controlled_build().validate(self.policy)?;
        specification.validate_input_files(capabilities.input_files())?;
        self.platform.run_retained_program_bootstrap(
            specification,
            capabilities,
            self.policy,
            cancellation,
        )
    }
}

/// Retained native lifecycle and isolation evidence for one managed process tree.
#[derive(Debug)]
pub struct RetainedIsolationLease {
    platform: platform::Lease,
    launch_spec_digest: Digest,
    isolation_policy_digest: Digest,
}

impl RetainedIsolationLease {
    /// Returns the redacted digest of the exact launch specification used to
    /// create this retained process lease.
    #[must_use]
    pub const fn launch_spec_digest(&self) -> &Digest {
        &self.launch_spec_digest
    }

    /// Returns the redacted digest of the exact isolation policy used to create
    /// this retained process lease.
    #[must_use]
    pub const fn isolation_policy_digest(&self) -> &Digest {
        &self.isolation_policy_digest
    }

    /// Returns launch-time evidence.
    #[must_use]
    pub fn initial_evidence(&self) -> IsolationEvidence {
        self.platform.initial_evidence()
    }

    /// Rechecks the target incarnation, process tree, privilege state, and namespaces.
    ///
    /// # Errors
    ///
    /// Returns an error when cancellation is active, the process tree exited,
    /// or any retained privilege or namespace invariant changed.
    pub fn reobserve(
        &mut self,
        cancellation: &CancellationToken,
    ) -> IsolationResult<IsolationEvidence> {
        self.platform.reobserve(cancellation, None)
    }

    /// Rechecks the retained isolation under an already-captured absolute deadline.
    ///
    /// # Errors
    ///
    /// Returns [`IsolationError::OperationDeadlineExceeded`] at or after the
    /// supplied deadline, or the same live-evidence errors as [`Self::reobserve`].
    pub fn reobserve_until(
        &mut self,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> IsolationResult<IsolationEvidence> {
        self.platform
            .reobserve(cancellation, Some(operation_deadline))
    }

    /// Opens the lease's single exact loopback channel inside the retained namespace.
    ///
    /// The returned stream and socket-diagnostics descriptor are capabilities only.
    /// This method does not identify the listener or attribute the connection to a
    /// process. A failed or completed request cannot be retried on this lease.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-loopback endpoint, cancellation, deadline,
    /// duplicate request, target exit, malformed helper response, or native failure.
    pub fn connect_loopback(
        &mut self,
        endpoint: SocketAddr,
        cancellation: &CancellationToken,
    ) -> IsolationResult<ManagedLoopbackChannel> {
        self.platform.connect_loopback(endpoint, cancellation, None)
    }

    /// Opens the lease's single loopback channel under an already-captured
    /// absolute operation deadline.
    ///
    /// # Errors
    ///
    /// Returns [`IsolationError::OperationDeadlineExceeded`] at or after the
    /// supplied deadline, or the same channel errors as [`Self::connect_loopback`].
    pub fn connect_loopback_until(
        &mut self,
        endpoint: SocketAddr,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> IsolationResult<ManagedLoopbackChannel> {
        self.platform
            .connect_loopback(endpoint, cancellation, Some(operation_deadline))
    }

    /// Terminates and reaps the complete managed process tree within the policy bound.
    ///
    /// # Errors
    ///
    /// Returns an error when cancellation was already active or the complete
    /// process tree cannot be reaped within the shutdown bound.
    pub fn close(mut self, cancellation: &CancellationToken) -> IsolationResult<()> {
        self.platform.close(cancellation)
    }
}

#[cfg(test)]
mod tests;
