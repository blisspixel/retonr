use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    fs::File,
    net::SocketAddr,
    path::{Path, PathBuf},
};

use rewrite_types::CancellationToken;

use crate::{IsolationError, IsolationResult, platform};

mod channel;
mod digest;
mod evidence;
mod policy;
mod validation;

pub use channel::{
    LinuxSocketDiagnosticsCapability, MAXIMUM_STARTUP_STREAM_BYTES, ManagedLoopbackChannel,
    ManagedStartupOutput,
};
use digest::RedactedDigestBuilder;
pub use evidence::{
    IsolationEvidence, IsolationPreparationEvidence, NamespaceIdentity, TargetProcessEvidence,
};
pub use policy::IsolationPolicy;
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
    pub(crate) fn executable(&self) -> &Path {
        &self.executable
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn arguments(&self) -> &[OsString] {
        &self.arguments
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn environment(&self) -> &BTreeMap<OsString, OsString> {
        &self.environment
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn current_directory(&self) -> Option<&Path> {
        self.current_directory.as_deref()
    }
}

/// Prepared, actively probed platform isolation capability.
#[derive(Debug)]
pub struct PreparedIsolation {
    policy: IsolationPolicy,
    preparation: IsolationPreparationEvidence,
    platform: platform::Prepared,
}

impl PreparedIsolation {
    /// Validates a retained helper and actively probes the complete isolation path.
    ///
    /// # Errors
    ///
    /// Returns a bounded isolation error when cancellation is active, the
    /// helper is invalid, host namespace policy denies the operation, or any
    /// preparation invariant cannot be verified.
    pub fn prepare(
        helper_executable: impl AsRef<Path>,
        policy: IsolationPolicy,
        cancellation: &CancellationToken,
    ) -> IsolationResult<Self> {
        policy.validate()?;
        let (platform, preparation) =
            platform::prepare(helper_executable.as_ref(), policy, cancellation)?;
        Ok(Self {
            policy,
            preparation,
            platform,
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
        let platform = self
            .platform
            .launch(specification, self.policy, cancellation)?;
        Ok(RetainedIsolationLease { platform })
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
        let platform =
            self.platform
                .launch_retained(specification, executable, self.policy, cancellation)?;
        Ok(RetainedIsolationLease { platform })
    }
}

/// Retained native lifecycle and isolation evidence for one managed process tree.
#[derive(Debug)]
pub struct RetainedIsolationLease {
    platform: platform::Lease,
}

impl RetainedIsolationLease {
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
        self.platform.reobserve(cancellation)
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
        self.platform.connect_loopback(endpoint, cancellation)
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
mod tests {
    use std::{ffi::OsString, path::PathBuf, time::Duration};

    use rewrite_types::{CancellationToken, Digest};

    #[cfg(not(target_os = "linux"))]
    use super::RetainedIsolationLease;
    use super::{
        IsolationEvidence, IsolationPolicy, IsolationPreparationEvidence, LaunchSpec,
        NamespaceIdentity, PreparedIsolation, TargetProcessEvidence,
    };
    use crate::IsolationError;

    #[test]
    fn policy_rejects_zero_and_unbounded_values() {
        assert_eq!(
            IsolationPolicy::new(Duration::ZERO, Duration::from_secs(1), 1, 1, 1, 64, 8,),
            Err(IsolationError::InvalidPolicy("startup timeout"))
        );
        assert_eq!(
            IsolationPolicy::new(
                Duration::from_secs(1),
                Duration::from_secs(1),
                4_097,
                1,
                1,
                64,
                8,
            ),
            Err(IsolationError::InvalidPolicy("argument count"))
        );
    }

    #[test]
    fn launch_validation_rejects_relative_paths_and_internal_environment() {
        let token = CancellationToken::new();
        let result =
            PreparedIsolation::prepare("relative-helper", IsolationPolicy::default(), &token);
        assert!(matches!(
            result,
            Err(IsolationError::UnsupportedPlatform | IsolationError::InvalidHelper)
        ));

        let mut spec = LaunchSpec::new(PathBuf::from("relative-target"));
        spec.insert_environment("REWRITE_ISOLATION_INTERNAL_BAD", "value");
        assert_eq!(
            spec.validate(IsolationPolicy::default()),
            Err(IsolationError::InvalidLaunch("executable path"))
        );
    }

    #[test]
    fn every_policy_bound_is_validated_and_exposed() {
        let valid = IsolationPolicy::new(
            Duration::from_secs(2),
            Duration::from_secs(3),
            2,
            2,
            16,
            64,
            8,
        )
        .expect("valid policy");
        assert_eq!(valid.startup_timeout(), Duration::from_secs(2));
        assert_eq!(valid.shutdown_timeout(), Duration::from_secs(3));
        assert_ne!(
            valid.redacted_digest(),
            IsolationPolicy::default().redacted_digest()
        );

        let cases = [
            (
                IsolationPolicy::new(
                    Duration::from_secs(31),
                    Duration::from_secs(1),
                    1,
                    1,
                    1,
                    64,
                    8,
                ),
                "startup timeout",
            ),
            (
                IsolationPolicy::new(Duration::from_secs(1), Duration::ZERO, 1, 1, 1, 64, 8),
                "shutdown timeout",
            ),
            (
                IsolationPolicy::new(
                    Duration::from_secs(1),
                    Duration::from_secs(1),
                    1,
                    1_025,
                    1,
                    64,
                    8,
                ),
                "environment count",
            ),
            (
                IsolationPolicy::new(
                    Duration::from_secs(1),
                    Duration::from_secs(1),
                    1,
                    1,
                    0,
                    64,
                    8,
                ),
                "value bytes",
            ),
            (
                IsolationPolicy::new(
                    Duration::from_secs(1),
                    Duration::from_secs(1),
                    1,
                    1,
                    1,
                    63,
                    8,
                ),
                "open-file limit",
            ),
            (
                IsolationPolicy::new(
                    Duration::from_secs(1),
                    Duration::from_secs(1),
                    1,
                    1,
                    1,
                    64,
                    7,
                ),
                "process limit",
            ),
        ];
        for (result, field) in cases {
            assert_eq!(result, Err(IsolationError::InvalidPolicy(field)));
        }
    }

    #[test]
    fn launch_builder_enforces_counts_values_keys_and_directories() {
        let executable = std::env::current_exe().expect("current executable");
        let policy = IsolationPolicy::new(
            Duration::from_secs(1),
            Duration::from_secs(1),
            1,
            1,
            4,
            64,
            8,
        )
        .expect("valid policy");

        let mut valid = LaunchSpec::new(&executable);
        valid.push_argument("a");
        valid.insert_environment("K", "V");
        valid.set_current_directory(std::env::current_dir().expect("current directory"));
        assert_eq!(
            valid.environment_value(std::ffi::OsStr::new("K")),
            Some(std::ffi::OsStr::new("V"))
        );
        assert_eq!(
            valid.environment_value(std::ffi::OsStr::new("MISSING")),
            None
        );
        assert!(valid.validate(policy).is_ok());
        let original_digest = valid.redacted_digest();
        let mut changed = valid.clone();
        changed.insert_environment("K", "W");
        assert_ne!(changed.redacted_digest(), original_digest);

        let mut too_many_arguments = valid.clone();
        too_many_arguments.push_argument("b");
        assert_eq!(
            too_many_arguments.validate(policy),
            Err(IsolationError::InvalidLaunch("argument count"))
        );

        let mut too_many_environment = valid.clone();
        too_many_environment.insert_environment("X", "Y");
        assert_eq!(
            too_many_environment.validate(policy),
            Err(IsolationError::InvalidLaunch("environment count"))
        );

        let mut oversized = LaunchSpec::new(&executable);
        oversized.push_argument("12345");
        assert_eq!(
            oversized.validate(policy),
            Err(IsolationError::InvalidLaunch("value bytes"))
        );

        let mut invalid_key = LaunchSpec::new(&executable);
        invalid_key.insert_environment("A=B", "V");
        assert_eq!(
            invalid_key.validate(policy),
            Err(IsolationError::InvalidLaunch("environment key"))
        );

        let mut nul = LaunchSpec::new(&executable);
        nul.push_argument(OsString::from("a\0b"));
        assert_eq!(
            nul.validate(policy),
            Err(IsolationError::InvalidLaunch("value bytes"))
        );

        let mut relative_directory = LaunchSpec::new(executable);
        relative_directory.set_current_directory("relative");
        assert_eq!(
            relative_directory.validate(policy),
            Err(IsolationError::InvalidLaunch("current directory"))
        );
    }

    #[test]
    fn evidence_getters_preserve_exact_native_identity() {
        let preparation = IsolationPreparationEvidence {
            loopback_interface_index: 1,
            canary_protocol_version: 1,
            helper_digest: Digest::sha256(b"helper"),
            helper_bytes: 6,
        };
        assert!(preparation.all_canaries_passed());
        assert_eq!(preparation.helper_digest(), &Digest::sha256(b"helper"));
        assert_eq!(preparation.helper_bytes(), 6);
        assert!(
            !IsolationPreparationEvidence {
                loopback_interface_index: 0,
                canary_protocol_version: 1,
                helper_digest: Digest::sha256(b"helper"),
                helper_bytes: 6,
            }
            .all_canaries_passed()
        );
        let network = NamespaceIdentity {
            device: 1,
            inode: 2,
        };
        let user = NamespaceIdentity {
            device: 3,
            inode: 4,
        };
        let process = NamespaceIdentity {
            device: 5,
            inode: 6,
        };
        assert_eq!(network.device(), 1);
        assert_eq!(network.inode(), 2);
        let target = TargetProcessEvidence {
            outer_pid: 8,
            namespace_pid: 2,
            process_start_token: 9,
            namespace_user_id: 0,
            executable_device: 10,
            executable_inode: 11,
            executable_bytes: 12,
        };
        let evidence = IsolationEvidence {
            guardian_pid: 7,
            network_namespace: network,
            user_namespace: user,
            process_namespace: process,
            preparation: preparation.clone(),
            target,
        };
        assert_eq!(evidence.guardian_pid(), 7);
        assert_eq!(evidence.network_namespace(), network);
        assert_eq!(evidence.user_namespace(), user);
        assert_eq!(evidence.process_namespace(), process);
        assert_eq!(evidence.preparation(), &preparation);
        assert_eq!(evidence.target().outer_pid(), 8);
        assert_eq!(evidence.target().namespace_pid(), 2);
        assert_eq!(evidence.target().process_start_token(), 9);
        assert_eq!(evidence.target().namespace_user_id(), 0);
        assert_eq!(evidence.target().executable_device(), 10);
        assert_eq!(evidence.target().executable_inode(), 11);
        assert_eq!(evidence.target().executable_bytes(), 12);
        assert_eq!(evidence.redacted_digest().as_str().len(), 64);
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn unsupported_prepared_and_lease_paths_remain_inert() {
        let token = CancellationToken::new();
        let prepared = PreparedIsolation {
            policy: IsolationPolicy::default(),
            preparation: IsolationPreparationEvidence {
                loopback_interface_index: 1,
                canary_protocol_version: 1,
                helper_digest: Digest::sha256(b"helper"),
                helper_bytes: 6,
            },
            platform: crate::platform::Prepared,
        };
        assert!(prepared.preparation_evidence().all_canaries_passed());
        assert_eq!(prepared.policy_digest(), prepared.policy.redacted_digest());
        let specification =
            LaunchSpec::new(std::env::current_exe().expect("absolute current executable"));
        assert!(matches!(
            prepared.launch(&specification, &token),
            Err(IsolationError::UnsupportedPlatform)
        ));
        let executable =
            std::fs::File::open(std::env::current_exe().expect("absolute current executable"))
                .expect("open retained executable");
        assert!(matches!(
            prepared.launch_retained(&specification, executable, &token),
            Err(IsolationError::UnsupportedPlatform)
        ));

        let mut lease = RetainedIsolationLease {
            platform: crate::platform::Lease,
        };
        assert_eq!(
            lease.reobserve(&token),
            Err(IsolationError::UnsupportedPlatform)
        );
        assert_eq!(
            lease.close(&token),
            Err(IsolationError::UnsupportedPlatform)
        );
    }
}
