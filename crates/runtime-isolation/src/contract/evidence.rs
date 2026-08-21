use super::digest::RedactedDigestBuilder;
use rewrite_types::Digest;

/// Stable device and inode identity for a retained Linux namespace.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NamespaceIdentity {
    pub(super) device: u64,
    pub(super) inode: u64,
}

impl NamespaceIdentity {
    #[cfg(target_os = "linux")]
    pub(crate) const fn new(device: u64, inode: u64) -> Self {
        Self { device, inode }
    }

    /// Returns the namespace filesystem device.
    #[must_use]
    pub const fn device(self) -> u64 {
        self.device
    }

    /// Returns the namespace inode.
    #[must_use]
    pub const fn inode(self) -> u64 {
        self.inode
    }
}

/// Active capability-probe evidence captured during preparation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IsolationPreparationEvidence {
    pub(super) loopback_interface_index: u32,
    pub(super) canary_protocol_version: u8,
    pub(super) helper_digest: Digest,
    pub(super) helper_bytes: u64,
}

impl IsolationPreparationEvidence {
    #[cfg(target_os = "linux")]
    pub(crate) const fn verified(
        loopback_interface_index: u32,
        helper_digest: Digest,
        helper_bytes: u64,
    ) -> Self {
        Self {
            loopback_interface_index,
            canary_protocol_version: 1,
            helper_digest,
            helper_bytes,
        }
    }

    /// Returns whether all required local-allow and non-loopback-deny canaries passed.
    #[must_use]
    pub const fn all_canaries_passed(&self) -> bool {
        self.loopback_interface_index > 0 && self.canary_protocol_version == 1
    }

    /// Returns the digest of the retained helper executable object.
    #[must_use]
    pub const fn helper_digest(&self) -> &Digest {
        &self.helper_digest
    }

    /// Returns the byte length of the retained helper executable object.
    #[must_use]
    pub const fn helper_bytes(&self) -> u64 {
        self.helper_bytes
    }
}

/// Exact launched target incarnation and executable object evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TargetProcessEvidence {
    pub(super) outer_pid: u32,
    pub(super) namespace_pid: u32,
    pub(super) process_start_token: u64,
    pub(super) namespace_user_id: u32,
    pub(super) executable_device: u64,
    pub(super) executable_inode: u64,
    pub(super) executable_bytes: u64,
}

impl TargetProcessEvidence {
    #[cfg(target_os = "linux")]
    pub(crate) const fn new(
        outer_pid: u32,
        namespace_pid: u32,
        process_start_token: u64,
        namespace_user_id: u32,
        executable_device: u64,
        executable_inode: u64,
        executable_bytes: u64,
    ) -> Self {
        Self {
            outer_pid,
            namespace_pid,
            process_start_token,
            namespace_user_id,
            executable_device,
            executable_inode,
            executable_bytes,
        }
    }

    /// Returns the target PID visible to the launching parent.
    #[must_use]
    pub const fn outer_pid(self) -> u32 {
        self.outer_pid
    }

    /// Returns the target PID inside the retained PID namespace.
    #[must_use]
    pub const fn namespace_pid(self) -> u32 {
        self.namespace_pid
    }

    /// Returns the kernel process start token for this exact incarnation.
    #[must_use]
    pub const fn process_start_token(self) -> u64 {
        self.process_start_token
    }

    /// Returns the target user ID inside its retained user namespace.
    ///
    /// This is the expected UID for namespace-local socket diagnostics. It is
    /// derived from the target's retained mapping and credentials at launch.
    #[must_use]
    pub const fn namespace_user_id(self) -> u32 {
        self.namespace_user_id
    }

    /// Returns the executable filesystem device.
    #[must_use]
    pub const fn executable_device(self) -> u64 {
        self.executable_device
    }

    /// Returns the executable inode.
    #[must_use]
    pub const fn executable_inode(self) -> u64 {
        self.executable_inode
    }

    /// Returns the executable byte length.
    #[must_use]
    pub const fn executable_bytes(self) -> u64 {
        self.executable_bytes
    }
}

/// Initial and reobserved evidence for one retained managed process tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IsolationEvidence {
    pub(super) guardian_pid: u32,
    pub(super) network_namespace: NamespaceIdentity,
    pub(super) user_namespace: NamespaceIdentity,
    pub(super) process_namespace: NamespaceIdentity,
    pub(super) preparation: IsolationPreparationEvidence,
    pub(super) target: TargetProcessEvidence,
}

impl IsolationEvidence {
    #[cfg(target_os = "linux")]
    pub(crate) const fn new(
        guardian_pid: u32,
        network_namespace: NamespaceIdentity,
        user_namespace: NamespaceIdentity,
        process_namespace: NamespaceIdentity,
        preparation: IsolationPreparationEvidence,
        target: TargetProcessEvidence,
    ) -> Self {
        Self {
            guardian_pid,
            network_namespace,
            user_namespace,
            process_namespace,
            preparation,
            target,
        }
    }

    /// Returns the retained outer process-tree guardian identifier.
    #[must_use]
    pub const fn guardian_pid(&self) -> u32 {
        self.guardian_pid
    }

    /// Returns the retained network namespace identity.
    #[must_use]
    pub const fn network_namespace(&self) -> NamespaceIdentity {
        self.network_namespace
    }

    /// Returns the retained user namespace identity.
    #[must_use]
    pub const fn user_namespace(&self) -> NamespaceIdentity {
        self.user_namespace
    }

    /// Returns the retained process namespace identity.
    #[must_use]
    pub const fn process_namespace(&self) -> NamespaceIdentity {
        self.process_namespace
    }

    /// Returns the launch canary evidence.
    #[must_use]
    pub const fn preparation(&self) -> &IsolationPreparationEvidence {
        &self.preparation
    }

    /// Returns exact target incarnation and executable object evidence.
    #[must_use]
    pub const fn target(&self) -> TargetProcessEvidence {
        self.target
    }

    /// Returns a domain-separated digest of the complete verified isolation evidence.
    ///
    /// The digest commits to preparation, helper, namespace, process, executable,
    /// canary, and namespace-local user identities without serializing raw values.
    #[must_use]
    pub fn redacted_digest(&self) -> Digest {
        let mut digest = RedactedDigestBuilder::new(b"runtime-isolation/evidence/v1");
        digest.push_u32(self.guardian_pid);
        push_namespace(&mut digest, self.network_namespace);
        push_namespace(&mut digest, self.user_namespace);
        push_namespace(&mut digest, self.process_namespace);
        digest.push_u32(self.preparation.loopback_interface_index);
        digest.push_u8(self.preparation.canary_protocol_version);
        digest.push_bytes(self.preparation.helper_digest.as_str().as_bytes());
        digest.push_u64(self.preparation.helper_bytes);
        digest.push_u32(self.target.outer_pid);
        digest.push_u32(self.target.namespace_pid);
        digest.push_u64(self.target.process_start_token);
        digest.push_u32(self.target.namespace_user_id);
        digest.push_u64(self.target.executable_device);
        digest.push_u64(self.target.executable_inode);
        digest.push_u64(self.target.executable_bytes);
        digest.finish()
    }
}

fn push_namespace(digest: &mut RedactedDigestBuilder, namespace: NamespaceIdentity) {
    digest.push_u64(namespace.device);
    digest.push_u64(namespace.inode);
}
