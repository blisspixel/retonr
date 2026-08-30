use super::digest::RedactedDigestBuilder;
use super::{ManagedDeviceVisibilityPolicy, ManagedRuntimeInputEvidence};
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
    pub(super) device_canary_protocol_version: u8,
    pub(super) runtime_input_canary_protocol_version: u8,
    pub(super) managed_device_visibility: ManagedDeviceVisibilityPolicy,
    pub(super) helper_digest: Digest,
    pub(super) helper_bytes: u64,
}

impl IsolationPreparationEvidence {
    #[cfg(target_os = "linux")]
    pub(crate) const fn verified(
        loopback_interface_index: u32,
        managed_device_visibility: ManagedDeviceVisibilityPolicy,
        helper_digest: Digest,
        helper_bytes: u64,
    ) -> Self {
        Self {
            loopback_interface_index,
            canary_protocol_version: 1,
            device_canary_protocol_version: 1,
            runtime_input_canary_protocol_version: 1,
            managed_device_visibility,
            helper_digest,
            helper_bytes,
        }
    }

    /// Returns whether all required local-allow and non-loopback-deny canaries passed.
    #[must_use]
    pub const fn all_canaries_passed(&self) -> bool {
        self.loopback_interface_index > 0
            && self.canary_protocol_version == 1
            && self.device_canary_protocol_version == 1
            && self.runtime_input_canary_protocol_version == 1
            && matches!(
                self.managed_device_visibility,
                ManagedDeviceVisibilityPolicy::LinuxCpuOnlyV1
            )
    }

    /// Returns the loopback interface observed by the preparation canary.
    #[must_use]
    pub const fn loopback_interface_index(&self) -> u32 {
        self.loopback_interface_index
    }

    /// Returns the exact preparation canary protocol version.
    #[must_use]
    pub const fn canary_protocol_version(&self) -> u8 {
        self.canary_protocol_version
    }

    /// Returns the exact private-device canary protocol version.
    #[must_use]
    pub const fn device_canary_protocol_version(&self) -> u8 {
        self.device_canary_protocol_version
    }

    /// Returns the exact private runtime-input canary protocol version.
    #[must_use]
    pub const fn runtime_input_canary_protocol_version(&self) -> u8 {
        self.runtime_input_canary_protocol_version
    }

    /// Returns the closed device-visibility policy exercised by preparation.
    #[must_use]
    pub const fn managed_device_visibility(&self) -> ManagedDeviceVisibilityPolicy {
        self.managed_device_visibility
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

/// Bounded visibility evidence for one managed Linux device boundary.
///
/// The evidence records a private mount view, fresh procfs, one retained null
/// device, descriptor closure, and installed containment filter. It is not a
/// formal proof of CPU placement or a universal device access-control claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManagedDeviceBoundaryEvidence {
    pub(super) policy: ManagedDeviceVisibilityPolicy,
    pub(super) device_mount: NamespaceIdentity,
    pub(super) proc_mount: NamespaceIdentity,
    pub(super) null_device: NamespaceIdentity,
    pub(super) null_major: u32,
    pub(super) null_minor: u32,
    pub(super) visible_device_entries: u32,
    pub(super) postconditions: u8,
}

impl ManagedDeviceBoundaryEvidence {
    pub(crate) const REQUIRED_POSTCONDITIONS: u8 = 0b00_111_111;

    #[cfg(target_os = "linux")]
    pub(crate) const fn verified(
        device_mount: NamespaceIdentity,
        proc_mount: NamespaceIdentity,
        null_device: NamespaceIdentity,
        null_major: u32,
        null_minor: u32,
        visible_device_entries: u32,
    ) -> Self {
        Self {
            policy: ManagedDeviceVisibilityPolicy::LinuxCpuOnlyV1,
            device_mount,
            proc_mount,
            null_device,
            null_major,
            null_minor,
            visible_device_entries,
            postconditions: Self::REQUIRED_POSTCONDITIONS,
        }
    }

    /// Returns the closed device-visibility policy represented by this evidence.
    #[must_use]
    pub const fn policy(&self) -> ManagedDeviceVisibilityPolicy {
        self.policy
    }

    /// Returns the private `/dev` mount identity.
    #[must_use]
    pub const fn device_mount(&self) -> NamespaceIdentity {
        self.device_mount
    }

    /// Returns the fresh PID-namespace procfs mount identity.
    #[must_use]
    pub const fn proc_mount(&self) -> NamespaceIdentity {
        self.proc_mount
    }

    /// Returns the retained `/dev/null` object identity.
    #[must_use]
    pub const fn null_device(&self) -> NamespaceIdentity {
        self.null_device
    }

    /// Returns the exact retained null-device major and minor numbers.
    #[must_use]
    pub const fn null_device_numbers(&self) -> (u32, u32) {
        (self.null_major, self.null_minor)
    }

    /// Returns the bounded direct-entry count observed under private `/dev`.
    #[must_use]
    pub const fn visible_device_entries(&self) -> u32 {
        self.visible_device_entries
    }

    /// Reports whether every bounded visibility postcondition was observed.
    #[must_use]
    pub const fn all_visibility_canaries_passed(&self) -> bool {
        matches!(self.policy, ManagedDeviceVisibilityPolicy::LinuxCpuOnlyV1)
            && self.postconditions == Self::REQUIRED_POSTCONDITIONS
            && self.device_mount.device != 0
            && self.device_mount.inode != 0
            && self.proc_mount.device != 0
            && self.proc_mount.inode != 0
            && self.null_device.device != 0
            && self.null_device.inode != 0
            && self.null_major == 1
            && self.null_minor == 3
            && self.visible_device_entries == 1
    }

    #[cfg(target_os = "linux")]
    pub(crate) const fn postconditions(&self) -> u8 {
        self.postconditions
    }

    pub(crate) fn redacted_digest(&self) -> Digest {
        let mut digest = RedactedDigestBuilder::new(b"runtime-isolation/device-boundary/v1");
        digest.push_u8(self.policy.code());
        push_namespace(&mut digest, self.device_mount);
        push_namespace(&mut digest, self.proc_mount);
        push_namespace(&mut digest, self.null_device);
        digest.push_u32(self.null_major);
        digest.push_u32(self.null_minor);
        digest.push_u32(self.visible_device_entries);
        digest.push_u8(self.postconditions);
        digest.finish()
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
    pub(super) mount_namespace: NamespaceIdentity,
    pub(super) device_boundary: ManagedDeviceBoundaryEvidence,
    pub(super) runtime_inputs: ManagedRuntimeInputEvidence,
    pub(super) preparation: IsolationPreparationEvidence,
    pub(super) target: TargetProcessEvidence,
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy)]
pub(crate) struct ManagedNamespaceEvidence {
    pub(crate) network: NamespaceIdentity,
    pub(crate) user: NamespaceIdentity,
    pub(crate) process: NamespaceIdentity,
    pub(crate) mount: NamespaceIdentity,
}

impl IsolationEvidence {
    #[cfg(target_os = "linux")]
    pub(crate) const fn new(
        guardian_pid: u32,
        namespaces: ManagedNamespaceEvidence,
        device_boundary: ManagedDeviceBoundaryEvidence,
        runtime_inputs: ManagedRuntimeInputEvidence,
        preparation: IsolationPreparationEvidence,
        target: TargetProcessEvidence,
    ) -> Self {
        Self {
            guardian_pid,
            network_namespace: namespaces.network,
            user_namespace: namespaces.user,
            process_namespace: namespaces.process,
            mount_namespace: namespaces.mount,
            device_boundary,
            runtime_inputs,
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

    /// Returns the retained managed mount namespace identity.
    #[must_use]
    pub const fn mount_namespace(&self) -> NamespaceIdentity {
        self.mount_namespace
    }

    /// Returns bounded private device and procfs visibility evidence.
    #[must_use]
    pub const fn device_boundary(&self) -> &ManagedDeviceBoundaryEvidence {
        &self.device_boundary
    }

    /// Returns the exact private runtime-input tree and mount evidence.
    #[must_use]
    pub const fn runtime_inputs(&self) -> &ManagedRuntimeInputEvidence {
        &self.runtime_inputs
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
        let mut digest = RedactedDigestBuilder::new(b"runtime-isolation/evidence/v3");
        digest.push_u32(self.guardian_pid);
        push_namespace(&mut digest, self.network_namespace);
        push_namespace(&mut digest, self.user_namespace);
        push_namespace(&mut digest, self.process_namespace);
        push_namespace(&mut digest, self.mount_namespace);
        digest.push_u32(self.preparation.loopback_interface_index);
        digest.push_u8(self.preparation.canary_protocol_version);
        digest.push_u8(self.preparation.device_canary_protocol_version);
        digest.push_u8(self.preparation.runtime_input_canary_protocol_version);
        digest.push_u8(self.preparation.managed_device_visibility.code());
        digest.push_bytes(self.preparation.helper_digest.as_str().as_bytes());
        digest.push_u64(self.preparation.helper_bytes);
        digest.push_u32(self.target.outer_pid);
        digest.push_u32(self.target.namespace_pid);
        digest.push_u64(self.target.process_start_token);
        digest.push_u32(self.target.namespace_user_id);
        digest.push_u64(self.target.executable_device);
        digest.push_u64(self.target.executable_inode);
        digest.push_u64(self.target.executable_bytes);
        digest.push_bytes(self.device_boundary.redacted_digest().as_str().as_bytes());
        digest.push_bytes(self.runtime_inputs.redacted_digest().as_str().as_bytes());
        digest.finish()
    }
}

fn push_namespace(digest: &mut RedactedDigestBuilder, namespace: NamespaceIdentity) {
    digest.push_u64(namespace.device);
    digest.push_u64(namespace.inode);
}
