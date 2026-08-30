use rewrite_types::Digest;

use super::{
    MAXIMUM_MANAGED_RUNTIME_INPUT_BYTES, MAXIMUM_MANAGED_RUNTIME_INPUT_FILES,
    managed_input_launch_digest,
};
use crate::contract::{NamespaceIdentity, RedactedDigestBuilder};

/// Redacted private byte-materialization and read-only mount evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedRuntimeInputEvidence {
    pub(in crate::contract) scratch_mount: NamespaceIdentity,
    pub(in crate::contract) input_mount: NamespaceIdentity,
    pub(in crate::contract) member_count: u32,
    pub(in crate::contract) total_bytes: u64,
    pub(in crate::contract) layout_digest: Digest,
    pub(in crate::contract) postconditions: u8,
}

#[cfg_attr(
    not(target_os = "linux"),
    expect(dead_code, reason = "postconditions are consumed by the Linux helper")
)]
impl ManagedRuntimeInputEvidence {
    pub(crate) const REQUIRED_POSTCONDITIONS: u8 = 0b00_111_111;

    #[cfg(target_os = "linux")]
    pub(crate) const fn verified(
        scratch_mount: NamespaceIdentity,
        input_mount: NamespaceIdentity,
        member_count: u32,
        total_bytes: u64,
        layout_digest: Digest,
    ) -> Self {
        Self {
            scratch_mount,
            input_mount,
            member_count,
            total_bytes,
            layout_digest,
            postconditions: Self::REQUIRED_POSTCONDITIONS,
        }
    }

    /// Returns whether this is the explicit empty-input form used by admission probes.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.member_count == 0 && self.total_bytes == 0
    }

    /// Returns the exact target-visible member count.
    #[must_use]
    pub const fn member_count(&self) -> u32 {
        self.member_count
    }

    /// Returns aggregate target-visible logical bytes.
    #[must_use]
    pub const fn total_bytes(&self) -> u64 {
        self.total_bytes
    }

    /// Returns the redacted ordered input-layout digest.
    #[must_use]
    pub const fn layout_digest(&self) -> &Digest {
        &self.layout_digest
    }

    /// Returns the private bounded scratch mount identity.
    #[must_use]
    pub const fn scratch_mount(&self) -> NamespaceIdentity {
        self.scratch_mount
    }

    /// Returns the private read-only input-root mount identity.
    #[must_use]
    pub const fn input_mount(&self) -> NamespaceIdentity {
        self.input_mount
    }

    /// Returns the input-bound launch digest derived from one plain launch digest
    /// and this exact target-visible input layout.
    ///
    /// This is the composite stored by a retained isolation lease launched with
    /// inputs. It is distinct from the plain launch digest used by admission.
    #[must_use]
    pub fn input_bound_launch_digest(&self, plain_launch_spec_digest: &Digest) -> Digest {
        managed_input_launch_digest(plain_launch_spec_digest, &self.layout_digest)
    }

    pub(crate) const fn postconditions(&self) -> u8 {
        self.postconditions
    }

    pub(crate) fn all_postconditions_passed(&self) -> bool {
        self.postconditions == Self::REQUIRED_POSTCONDITIONS
            && self.scratch_mount.device() != 0
            && self.scratch_mount.inode() != 0
            && self.input_mount.device() != 0
            && self.input_mount.inode() != 0
            && usize::try_from(self.member_count)
                .is_ok_and(|count| count <= MAXIMUM_MANAGED_RUNTIME_INPUT_FILES)
            && self.total_bytes <= MAXIMUM_MANAGED_RUNTIME_INPUT_BYTES
            && (self.member_count != 0 || self.total_bytes == 0)
    }

    pub(crate) fn redacted_digest(&self) -> Digest {
        let mut digest = RedactedDigestBuilder::new(b"runtime-isolation/managed-input-evidence/v1");
        digest.push_u64(self.scratch_mount.device());
        digest.push_u64(self.scratch_mount.inode());
        digest.push_u64(self.input_mount.device());
        digest.push_u64(self.input_mount.inode());
        digest.push_u32(self.member_count);
        digest.push_u64(self.total_bytes);
        digest.push_bytes(self.layout_digest.as_str().as_bytes());
        digest.push_u8(self.postconditions);
        digest.finish()
    }
}
