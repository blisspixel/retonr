use std::{net::SocketAddr, time::Duration};

use rewrite_types::{CancellationToken, Digest};
use serde::Serialize;
use thiserror::Error;

use crate::{RetainedTcpConnection, RetainedTcpConnectionEvidence};

/// Current attached-process witness contract version.
pub const ATTACHED_PROCESS_WITNESS_SCHEMA_VERSION: u32 = 1;

/// Hard maximum bytes admitted from one operating-system socket table.
pub const MAXIMUM_SOCKET_TABLE_BYTES: usize = 16 * 1024 * 1024;
/// Hard maximum socket rows inspected in one observation.
pub const MAXIMUM_SOCKET_TABLE_ENTRIES: usize = 65_536;
/// Hard maximum processes inspected in one observation.
pub const MAXIMUM_OBSERVED_PROCESSES: usize = 65_536;
/// Hard maximum descriptors inspected for one process.
pub const MAXIMUM_DESCRIPTORS_PER_PROCESS: usize = 65_536;
/// Hard maximum executable bytes hashed in one observation.
pub const MAXIMUM_ENTRYPOINT_BYTES: u64 = 2 * 1024 * 1024 * 1024;
/// Hard maximum elapsed time for one observation bracket.
pub const MAXIMUM_OBSERVATION_MILLIS: u64 = 120_000;

/// Validated exact TCP loopback listener endpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ListenerEndpoint(SocketAddr);

impl ListenerEndpoint {
    /// Creates an endpoint from an exact nonzero loopback socket address.
    ///
    /// # Errors
    ///
    /// Returns [`AttachedProcessWitnessError::InvalidEndpoint`] for wildcard,
    /// non-loopback, or zero-port input.
    pub fn new(socket: SocketAddr) -> Result<Self, AttachedProcessWitnessError> {
        if !socket.ip().is_loopback() || socket.port() == 0 {
            return Err(AttachedProcessWitnessError::InvalidEndpoint);
        }
        Ok(Self(socket))
    }

    /// Returns the normalized socket address.
    #[must_use]
    pub const fn socket(self) -> SocketAddr {
        self.0
    }
}

/// Caller-owned ceilings for native process observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttachedProcessWitnessLimits {
    /// Maximum socket-table bytes admitted.
    pub maximum_socket_table_bytes: usize,
    /// Maximum socket rows inspected.
    pub maximum_socket_table_entries: usize,
    /// Maximum processes inspected.
    pub maximum_processes: usize,
    /// Maximum descriptors inspected for one process.
    pub maximum_descriptors_per_process: usize,
    /// Maximum executable bytes hashed.
    pub maximum_entrypoint_bytes: u64,
    /// Maximum elapsed time for the complete observation bracket.
    pub maximum_elapsed: Duration,
}

impl Default for AttachedProcessWitnessLimits {
    fn default() -> Self {
        Self {
            maximum_socket_table_bytes: MAXIMUM_SOCKET_TABLE_BYTES,
            maximum_socket_table_entries: MAXIMUM_SOCKET_TABLE_ENTRIES,
            maximum_processes: MAXIMUM_OBSERVED_PROCESSES,
            maximum_descriptors_per_process: MAXIMUM_DESCRIPTORS_PER_PROCESS,
            maximum_entrypoint_bytes: 1024 * 1024 * 1024,
            maximum_elapsed: Duration::from_secs(30),
        }
    }
}

impl AttachedProcessWitnessLimits {
    pub(crate) fn validate(self) -> Result<Self, AttachedProcessWitnessError> {
        if self.maximum_socket_table_bytes == 0
            || self.maximum_socket_table_bytes > MAXIMUM_SOCKET_TABLE_BYTES
            || self.maximum_socket_table_entries == 0
            || self.maximum_socket_table_entries > MAXIMUM_SOCKET_TABLE_ENTRIES
            || self.maximum_processes == 0
            || self.maximum_processes > MAXIMUM_OBSERVED_PROCESSES
            || self.maximum_descriptors_per_process == 0
            || self.maximum_descriptors_per_process > MAXIMUM_DESCRIPTORS_PER_PROCESS
            || self.maximum_entrypoint_bytes == 0
            || self.maximum_entrypoint_bytes > MAXIMUM_ENTRYPOINT_BYTES
            || self.maximum_elapsed.is_zero()
            || self.maximum_elapsed > Duration::from_millis(MAXIMUM_OBSERVATION_MILLIS)
        {
            return Err(AttachedProcessWitnessError::InvalidLimits);
        }
        Ok(self)
    }
}

/// Native evidence mechanism used for one successful observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachedProcessEvidenceClass {
    /// Windows owner-PID table plus a retained process handle.
    WindowsOwnerPidProcessHandle,
    /// Linux proc socket ownership plus a retained pidfd.
    LinuxProcPidfd,
}

/// Launch classification supported by the first witness contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachedProcessLaunchMode {
    /// The observer did not claim a service-manager or desktop launch mode.
    AttachedUnknown,
}

/// Provider-neutral input produced only by a reviewed process observer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedProcessEvidenceInput {
    /// Native observation mechanism.
    pub evidence_class: AttachedProcessEvidenceClass,
    /// Kernel-reported listener owner PID.
    pub owner_pid: u32,
    /// Digest of PID plus the native process-incarnation token.
    pub process_instance_digest: Digest,
    /// Digest of the exact endpoint and native ownership snapshot.
    pub ownership_snapshot_digest: Digest,
    /// Digest of the opened executable object identity.
    pub entrypoint_object_digest: Digest,
    /// SHA-256 digest of the opened executable bytes.
    pub entrypoint_digest: Digest,
    /// Number of opened executable bytes.
    pub entrypoint_bytes: u64,
    /// Digest of platform and namespace evidence relevant to this observation.
    pub platform_evidence_digest: Digest,
}

/// Redacted point-in-time evidence for one native listener owner.
///
/// This record is inert. It does not prove which process served an HTTP response,
/// create a runtime-build identity, qualify a runtime, or authorize a role.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AttachedProcessEvidence {
    schema_version: u32,
    evidence_class: AttachedProcessEvidenceClass,
    launch_mode: AttachedProcessLaunchMode,
    owner_pid: u32,
    process_instance_digest: Digest,
    ownership_snapshot_digest: Digest,
    entrypoint_object_digest: Digest,
    entrypoint_digest: Digest,
    entrypoint_bytes: u64,
    platform_evidence_digest: Digest,
    evidence_digest: Digest,
}

impl AttachedProcessEvidence {
    /// Builds one redacted inert record from reviewed observer facts.
    ///
    /// # Errors
    ///
    /// Returns [`AttachedProcessWitnessError::InvalidEvidence`] when required
    /// numeric facts are empty.
    pub fn new(input: AttachedProcessEvidenceInput) -> Result<Self, AttachedProcessWitnessError> {
        if input.owner_pid == 0 || input.entrypoint_bytes == 0 {
            return Err(AttachedProcessWitnessError::InvalidEvidence);
        }
        let evidence_digest = evidence_digest(&input);
        Ok(Self {
            schema_version: ATTACHED_PROCESS_WITNESS_SCHEMA_VERSION,
            evidence_class: input.evidence_class,
            launch_mode: AttachedProcessLaunchMode::AttachedUnknown,
            owner_pid: input.owner_pid,
            process_instance_digest: input.process_instance_digest,
            ownership_snapshot_digest: input.ownership_snapshot_digest,
            entrypoint_object_digest: input.entrypoint_object_digest,
            entrypoint_digest: input.entrypoint_digest,
            entrypoint_bytes: input.entrypoint_bytes,
            platform_evidence_digest: input.platform_evidence_digest,
            evidence_digest,
        })
    }

    /// Returns the witness contract version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the operating-system evidence class.
    #[must_use]
    pub const fn evidence_class(&self) -> AttachedProcessEvidenceClass {
        self.evidence_class
    }

    /// Returns the kernel-reported owner PID.
    #[must_use]
    pub const fn owner_pid(&self) -> u32 {
        self.owner_pid
    }

    /// Returns the observed executable byte digest.
    #[must_use]
    pub const fn entrypoint_digest(&self) -> &Digest {
        &self.entrypoint_digest
    }

    /// Returns the observed executable length.
    #[must_use]
    pub const fn entrypoint_bytes(&self) -> u64 {
        self.entrypoint_bytes
    }

    /// Returns the digest of the complete redacted witness.
    #[must_use]
    pub const fn evidence_digest(&self) -> &Digest {
        &self.evidence_digest
    }

    pub(crate) fn process_instance_digest(&self) -> &Digest {
        &self.process_instance_digest
    }

    pub(crate) fn ownership_snapshot_digest(&self) -> &Digest {
        &self.ownership_snapshot_digest
    }

    pub(crate) fn entrypoint_object_digest(&self) -> &Digest {
        &self.entrypoint_object_digest
    }

    pub(crate) fn platform_evidence_digest(&self) -> &Digest {
        &self.platform_evidence_digest
    }
}

/// Retained native process capability used across application work.
pub trait AttachedProcessLease {
    /// Returns the initial redacted listener-owner evidence.
    fn initial_evidence(&self) -> &AttachedProcessEvidence;

    /// Re-observes the retained process and listener after application work.
    ///
    /// # Errors
    ///
    /// Returns [`AttachedProcessWitnessError`] for any process, listener,
    /// executable, permission, resource, deadline, or cancellation failure.
    fn reobserve(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<AttachedProcessEvidence, AttachedProcessWitnessError>;

    /// Attributes one exact caller-retained established TCP connection.
    ///
    /// The caller must keep the same connected stream open until
    /// [`Self::reobserve_connection`] succeeds after application work.
    ///
    /// # Errors
    ///
    /// Returns [`AttachedProcessWitnessError`] unless one bounded exact native
    /// connection observation matches this retained process lease.
    fn observe_connection(
        &mut self,
        _connection: RetainedTcpConnection,
        _cancellation: &CancellationToken,
    ) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError> {
        Err(AttachedProcessWitnessError::Unsupported)
    }

    /// Re-observes and compares the exact retained connection after application work.
    ///
    /// # Errors
    ///
    /// Returns [`AttachedProcessWitnessError`] for any closed, changed,
    /// incomplete, mismatched, cancelled, expired, or resource-limited view.
    fn reobserve_connection(
        &mut self,
        _connection: RetainedTcpConnection,
        _initial: &RetainedTcpConnectionEvidence,
        _cancellation: &CancellationToken,
    ) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError> {
        Err(AttachedProcessWitnessError::Unsupported)
    }
}

/// Safe observer port used by read-only preflight orchestration.
pub trait AttachedProcessObserver {
    /// Retained lease implementation.
    type Lease: AttachedProcessLease;

    /// Attaches to one exact native listener owner.
    ///
    /// # Errors
    ///
    /// Returns [`AttachedProcessWitnessError`] unless a complete bounded native
    /// observation can be retained.
    fn attach(
        &self,
        endpoint: ListenerEndpoint,
        limits: AttachedProcessWitnessLimits,
        cancellation: &CancellationToken,
    ) -> Result<Self::Lease, AttachedProcessWitnessError>;
}

/// Stable redacted failure from native listener-owner observation.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum AttachedProcessWitnessError {
    /// Endpoint is not an exact nonzero loopback address.
    #[error("attached process witness endpoint is invalid")]
    InvalidEndpoint,
    /// Connection endpoints are not one exact valid loopback TCP pair.
    #[error("attached process witness connection endpoints are invalid")]
    InvalidConnectionEndpoints,
    /// One or more observation ceilings are zero or exceed hard maxima.
    #[error("attached process witness limits are invalid")]
    InvalidLimits,
    /// Observer-produced evidence omitted a required numeric fact.
    #[error("attached process witness evidence is invalid")]
    InvalidEvidence,
    /// Observation was cancelled.
    #[error("attached process witness was cancelled")]
    Cancelled,
    /// Complete observation exceeded its elapsed-time ceiling.
    #[error("attached process witness deadline was exceeded")]
    DeadlineExceeded,
    /// The current operating system has no admitted observation mechanism.
    #[error("attached process witness is unsupported on this platform")]
    Unsupported,
    /// No exact established connection row was found.
    #[error("attached process witness connection was not found")]
    ConnectionNotFound,
    /// More than one exact connection row matched.
    #[error("attached process witness connection attribution is ambiguous")]
    ConnectionAmbiguous,
    /// The observer could not establish the required bounded connection view.
    #[error("attached process witness connection snapshot is incomplete")]
    ConnectionSnapshotIncomplete,
    /// The exact connection row was not established.
    #[error("attached process witness connection is not established")]
    ConnectionNotEstablished,
    /// The exact connection was not attributed to the retained process.
    #[error("attached process witness connection process does not match")]
    ConnectionProcessMismatch,
    /// The initially attributed connection is now closed or absent.
    #[error("attached process witness connection closed")]
    ConnectionClosed,
    /// The connection object or attribution changed across observations.
    #[error("attached process witness connection changed")]
    ConnectionChanged,
    /// No exact listener was found.
    #[error("attached process witness listener was not found")]
    ListenerNotFound,
    /// More than one listener or owner matched.
    #[error("attached process witness listener ownership is ambiguous")]
    ListenerOwnershipAmbiguous,
    /// The observer could not establish a complete ownership view.
    #[error("attached process witness listener snapshot is incomplete")]
    ListenerSnapshotIncomplete,
    /// Native process access was denied.
    #[error("attached process witness process access was denied")]
    ProcessAccessDenied,
    /// A stable process-incarnation capability could not be acquired.
    #[error("attached process witness process instance is unavailable")]
    ProcessInstanceUnavailable,
    /// The retained process exited.
    #[error("attached process witness process exited")]
    ProcessExited,
    /// The listener owner changed during the observation bracket.
    #[error("attached process witness listener owner changed")]
    ListenerRebound,
    /// The process-incarnation token changed.
    #[error("attached process witness process instance changed")]
    ProcessInstanceChanged,
    /// The executable object could not be opened or inspected.
    #[error("attached process witness entrypoint is unavailable")]
    EntrypointUnavailable,
    /// The executable object is not one regular file.
    #[error("attached process witness entrypoint is not regular")]
    EntrypointNotRegular,
    /// The executable object has aliases outside this observation.
    #[error("attached process witness entrypoint has multiple links")]
    EntrypointAliased,
    /// The executable exceeds the configured byte ceiling.
    #[error("attached process witness entrypoint exceeds its byte limit")]
    EntrypointTooLarge,
    /// The executable object or bytes changed.
    #[error("attached process witness entrypoint changed")]
    EntrypointChanged,
    /// The observed executable digest differs from the frozen expectation.
    #[error("attached process witness entrypoint digest does not match")]
    EntrypointDigestMismatch,
    /// A bounded native enumeration ceiling was exhausted.
    #[error("attached process witness observation exceeded a resource limit")]
    ResourceLimit,
    /// A redacted native observation operation failed.
    #[error("attached process witness native observation failed")]
    PlatformObservationFailed,
}

fn evidence_digest(input: &AttachedProcessEvidenceInput) -> Digest {
    let mut bytes = Vec::with_capacity(512);
    bytes.extend_from_slice(b"retonr:attached-process-witness:v1\0");
    bytes.push(match input.evidence_class {
        AttachedProcessEvidenceClass::WindowsOwnerPidProcessHandle => 0,
        AttachedProcessEvidenceClass::LinuxProcPidfd => 1,
    });
    bytes.extend_from_slice(&input.owner_pid.to_be_bytes());
    for digest in [
        &input.process_instance_digest,
        &input.ownership_snapshot_digest,
        &input.entrypoint_object_digest,
        &input.entrypoint_digest,
        &input.platform_evidence_digest,
    ] {
        bytes.extend_from_slice(digest.as_str().as_bytes());
    }
    bytes.extend_from_slice(&input.entrypoint_bytes.to_be_bytes());
    Digest::sha256(&bytes)
}
