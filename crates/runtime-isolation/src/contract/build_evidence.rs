use rewrite_types::Digest;

use super::{IsolationPreparationEvidence, NamespaceIdentity, RedactedDigestBuilder};

/// Content-free evidence for one filesystem-confined controlled build.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlledBuildIsolationEvidence {
    guardian_pid: u32,
    namespace_init_pid: u32,
    network_namespace: NamespaceIdentity,
    user_namespace: NamespaceIdentity,
    process_namespace: NamespaceIdentity,
    mount_namespace: NamespaceIdentity,
    preparation: IsolationPreparationEvidence,
    isolation_policy_digest: Digest,
    landlock_abi: u32,
    program_digest: Digest,
    program_bytes: u64,
    program_device: u64,
    program_inode: u64,
    input_device: u64,
    input_inode: u64,
    output_device: u64,
    output_inode: u64,
    launch_digest: Digest,
}

impl ControlledBuildIsolationEvidence {
    #[cfg(target_os = "linux")]
    pub(crate) fn from_observation(
        observation: ControlledBuildIsolationObservation,
        preparation: IsolationPreparationEvidence,
        isolation_policy_digest: Digest,
        program_digest: Digest,
        launch_digest: Digest,
    ) -> Self {
        Self {
            guardian_pid: observation.guardian_pid,
            namespace_init_pid: observation.namespace_init_pid,
            network_namespace: observation.network_namespace,
            user_namespace: observation.user_namespace,
            process_namespace: observation.process_namespace,
            mount_namespace: observation.mount_namespace,
            preparation,
            isolation_policy_digest,
            landlock_abi: observation.landlock_abi,
            program_digest,
            program_bytes: observation.program_bytes,
            program_device: observation.program_device,
            program_inode: observation.program_inode,
            input_device: observation.input_device,
            input_inode: observation.input_inode,
            output_device: observation.output_device,
            output_inode: observation.output_inode,
            launch_digest,
        }
    }

    /// Returns the retained outer guardian PID.
    #[must_use]
    pub const fn guardian_pid(&self) -> u32 {
        self.guardian_pid
    }

    /// Returns the PID namespace's retained init PID as seen by the parent.
    #[must_use]
    pub const fn namespace_init_pid(&self) -> u32 {
        self.namespace_init_pid
    }

    /// Returns the observed network namespace identity.
    #[must_use]
    pub const fn network_namespace(&self) -> NamespaceIdentity {
        self.network_namespace
    }

    /// Returns the observed user namespace identity.
    #[must_use]
    pub const fn user_namespace(&self) -> NamespaceIdentity {
        self.user_namespace
    }

    /// Returns the observed PID namespace identity.
    #[must_use]
    pub const fn process_namespace(&self) -> NamespaceIdentity {
        self.process_namespace
    }

    /// Returns the observed private mount namespace identity.
    #[must_use]
    pub const fn mount_namespace(&self) -> NamespaceIdentity {
        self.mount_namespace
    }

    /// Returns the retained helper and network-canary preparation evidence.
    #[must_use]
    pub const fn preparation(&self) -> &IsolationPreparationEvidence {
        &self.preparation
    }

    /// Returns the active Landlock ABI. ABI 3 or newer is required.
    #[must_use]
    pub const fn landlock_abi(&self) -> u32 {
        self.landlock_abi
    }

    /// Returns the digest of every exact isolation resource and time bound.
    #[must_use]
    pub const fn isolation_policy_digest(&self) -> &Digest {
        &self.isolation_policy_digest
    }

    /// Returns the digest of the retained build-program executable.
    #[must_use]
    pub const fn program_digest(&self) -> &Digest {
        &self.program_digest
    }

    /// Returns the byte length of the retained build-program executable.
    #[must_use]
    pub const fn program_bytes(&self) -> u64 {
        self.program_bytes
    }

    /// Returns the retained build-program filesystem identity.
    #[must_use]
    pub const fn program_object_identity(&self) -> (u64, u64) {
        (self.program_device, self.program_inode)
    }

    /// Returns the retained input-root filesystem identity.
    #[must_use]
    pub const fn input_root_identity(&self) -> (u64, u64) {
        (self.input_device, self.input_inode)
    }

    /// Returns the retained output-root filesystem identity.
    #[must_use]
    pub const fn output_root_identity(&self) -> (u64, u64) {
        (self.output_device, self.output_inode)
    }

    /// Returns the digest of the complete controlled build launch description.
    #[must_use]
    pub const fn launch_digest(&self) -> &Digest {
        &self.launch_digest
    }

    /// Returns a domain-separated digest of the complete content-free evidence.
    #[must_use]
    pub fn redacted_digest(&self) -> Digest {
        let mut digest =
            RedactedDigestBuilder::new(b"runtime-isolation/controlled-build-evidence/v3");
        digest.push_u32(self.guardian_pid);
        digest.push_u32(self.namespace_init_pid);
        for namespace in [
            self.network_namespace,
            self.user_namespace,
            self.process_namespace,
            self.mount_namespace,
        ] {
            digest.push_u64(namespace.device());
            digest.push_u64(namespace.inode());
        }
        digest.push_bytes(self.preparation.helper_digest().as_str().as_bytes());
        digest.push_u64(self.preparation.helper_bytes());
        digest.push_u32(self.preparation.loopback_interface_index());
        digest.push_u8(self.preparation.canary_protocol_version());
        digest.push_bytes(self.isolation_policy_digest.as_str().as_bytes());
        digest.push_u32(self.landlock_abi);
        digest.push_bytes(self.program_digest.as_str().as_bytes());
        digest.push_u64(self.program_bytes);
        for value in [
            self.program_device,
            self.program_inode,
            self.input_device,
            self.input_inode,
            self.output_device,
            self.output_inode,
        ] {
            digest.push_u64(value);
        }
        digest.push_bytes(self.launch_digest.as_str().as_bytes());
        digest.finish()
    }
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy)]
pub(crate) struct ControlledBuildIsolationObservation {
    pub(crate) guardian_pid: u32,
    pub(crate) namespace_init_pid: u32,
    pub(crate) network_namespace: NamespaceIdentity,
    pub(crate) user_namespace: NamespaceIdentity,
    pub(crate) process_namespace: NamespaceIdentity,
    pub(crate) mount_namespace: NamespaceIdentity,
    pub(crate) landlock_abi: u32,
    pub(crate) program_bytes: u64,
    pub(crate) program_device: u64,
    pub(crate) program_inode: u64,
    pub(crate) input_device: u64,
    pub(crate) input_inode: u64,
    pub(crate) output_device: u64,
    pub(crate) output_inode: u64,
}
