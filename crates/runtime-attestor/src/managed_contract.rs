#[cfg(target_os = "linux")]
use rewrite_types::Digest;

use crate::AttachedProcessWitnessError;
#[cfg(target_os = "linux")]
use crate::{
    AttachedProcessEvidenceClass, AttachedProcessEvidenceInput, AttachedProcessLaunchMode,
};

/// Exact caller-owned facts for one managed Linux target process.
///
/// These facts are values only. Constructing this record does not attest them.
/// The managed observer must match every value against retained native objects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManagedLinuxProcessExpectation {
    outer_pid: u32,
    process_start_token: u64,
    executable_device: u64,
    executable_inode: u64,
    executable_bytes: u64,
    network_namespace_device: u64,
    network_namespace_inode: u64,
    diagnostics_uid: u32,
}

impl ManagedLinuxProcessExpectation {
    /// Creates one complete expected managed-process identity.
    ///
    /// # Errors
    ///
    /// Returns [`AttachedProcessWitnessError::InvalidEvidence`] when a required
    /// process, executable, or namespace value is zero.
    #[expect(
        clippy::too_many_arguments,
        reason = "the trust-boundary record keeps every expected native fact explicit"
    )]
    pub fn new(
        outer_pid: u32,
        process_start_token: u64,
        executable_device: u64,
        executable_inode: u64,
        executable_bytes: u64,
        network_namespace_device: u64,
        network_namespace_inode: u64,
        diagnostics_uid: u32,
    ) -> Result<Self, AttachedProcessWitnessError> {
        if outer_pid == 0
            || process_start_token == 0
            || executable_device == 0
            || executable_inode == 0
            || executable_bytes == 0
            || network_namespace_device == 0
            || network_namespace_inode == 0
        {
            return Err(AttachedProcessWitnessError::InvalidEvidence);
        }
        Ok(Self {
            outer_pid,
            process_start_token,
            executable_device,
            executable_inode,
            executable_bytes,
            network_namespace_device,
            network_namespace_inode,
            diagnostics_uid,
        })
    }

    /// Returns the PID visible in the observer's process namespace.
    #[must_use]
    pub const fn outer_pid(self) -> u32 {
        self.outer_pid
    }

    /// Returns the expected Linux process start-time token.
    #[must_use]
    pub const fn process_start_token(self) -> u64 {
        self.process_start_token
    }

    /// Returns the expected executable device number.
    #[must_use]
    pub const fn executable_device(self) -> u64 {
        self.executable_device
    }

    /// Returns the expected executable inode number.
    #[must_use]
    pub const fn executable_inode(self) -> u64 {
        self.executable_inode
    }

    /// Returns the expected executable byte length.
    #[must_use]
    pub const fn executable_bytes(self) -> u64 {
        self.executable_bytes
    }

    /// Returns the expected network-namespace device number.
    #[must_use]
    pub const fn network_namespace_device(self) -> u64 {
        self.network_namespace_device
    }

    /// Returns the expected network-namespace inode number.
    #[must_use]
    pub const fn network_namespace_inode(self) -> u64 {
        self.network_namespace_inode
    }

    /// Returns the UID expected in namespace-local socket-diagnostics rows.
    #[must_use]
    pub const fn diagnostics_uid(self) -> u32 {
        self.diagnostics_uid
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn managed_linux_evidence_digest(
    input: &AttachedProcessEvidenceInput,
    launch_mode: AttachedProcessLaunchMode,
) -> Digest {
    let mut bytes = Vec::with_capacity(512);
    bytes.extend_from_slice(b"retonr:managed-linux-process-witness:v2\0");
    bytes.push(match input.evidence_class {
        AttachedProcessEvidenceClass::LinuxManagedNamespaceSockDiag => 2,
        AttachedProcessEvidenceClass::WindowsOwnerPidProcessHandle => 0,
        AttachedProcessEvidenceClass::LinuxProcPidfd => 1,
    });
    bytes.push(match launch_mode {
        AttachedProcessLaunchMode::AttachedUnknown => 0,
        AttachedProcessLaunchMode::ManagedLinuxIsolation => 1,
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
