use rewrite_types::Digest;

#[cfg(target_os = "linux")]
use super::super::ControlledBuildProcessStatus;
use super::RetainedProgramBootstrapAttempt;
#[cfg(target_os = "linux")]
use super::RetainedProgramBootstrapLaunchSpec;
use crate::{ControlledBuildExecution, NamespaceIdentity, contract::RedactedDigestBuilder};
#[cfg(target_os = "linux")]
use crate::{IsolationError, IsolationResult};

/// Observed, content-free postconditions from the bootstrap root transition.
#[derive(Debug, Eq, PartialEq)]
pub struct RetainedProgramBootstrapRootEvidence {
    pub(super) root_mount: NamespaceIdentity,
    pub(super) alpine_installed_database_digest: Digest,
    pub(super) alpine_loader_digest: Digest,
    pub(super) alpine_libc_linker_name_digest: Digest,
    pub(super) alpine_libgcc_digest: Digest,
    pub(super) alpine_libgcc_linker_name_digest: Digest,
    pub(super) alpine_busybox_digest: Digest,
    pub(super) rust_toolchain_layout_digest: Digest,
    pub(super) normalized_alpine_link_plan_digest: Digest,
    pub(super) postconditions: RetainedProgramBootstrapRootPostconditions,
}

impl RetainedProgramBootstrapRootEvidence {
    /// Returns the prepared root mount identity.
    #[must_use]
    pub const fn root_mount(&self) -> NamespaceIdentity {
        self.root_mount
    }

    /// Returns the exact installed Alpine package database digest.
    #[must_use]
    pub const fn alpine_installed_database_digest(&self) -> &Digest {
        &self.alpine_installed_database_digest
    }

    /// Returns the exact installed musl loader digest.
    #[must_use]
    pub const fn alpine_loader_digest(&self) -> &Digest {
        &self.alpine_loader_digest
    }

    /// Returns the exact derived linker-name binding for the installed musl payload.
    #[must_use]
    pub const fn alpine_libc_linker_name_digest(&self) -> &Digest {
        &self.alpine_libc_linker_name_digest
    }

    /// Returns the exact installed `libgcc` payload digest.
    #[must_use]
    pub const fn alpine_libgcc_digest(&self) -> &Digest {
        &self.alpine_libgcc_digest
    }

    /// Returns the exact derived linker-name binding for the installed `libgcc` payload.
    #[must_use]
    pub const fn alpine_libgcc_linker_name_digest(&self) -> &Digest {
        &self.alpine_libgcc_linker_name_digest
    }

    /// Returns the exact installed static `BusyBox` executable digest.
    #[must_use]
    pub const fn alpine_busybox_digest(&self) -> &Digest {
        &self.alpine_busybox_digest
    }

    /// Returns the deterministic assembled Rust toolchain layout digest.
    #[must_use]
    pub const fn rust_toolchain_layout_digest(&self) -> &Digest {
        &self.rust_toolchain_layout_digest
    }

    /// Returns the deterministic normalized Alpine symlink and hardlink plan digest.
    #[must_use]
    pub const fn normalized_alpine_link_plan_digest(&self) -> &Digest {
        &self.normalized_alpine_link_plan_digest
    }

    /// Reports whether every required root-transition canary passed.
    #[must_use]
    pub const fn all_canaries_passed(&self) -> bool {
        self.postconditions.all_observed()
            && self.root_mount.device() != 0
            && self.root_mount.inode() != 0
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn from_observation(observation: RetainedProgramBootstrapRootObservation) -> Self {
        Self {
            root_mount: observation.root_mount,
            alpine_installed_database_digest: observation.alpine_installed_database_digest,
            alpine_loader_digest: observation.alpine_loader_digest,
            alpine_libc_linker_name_digest: observation.alpine_libc_linker_name_digest,
            alpine_libgcc_digest: observation.alpine_libgcc_digest,
            alpine_libgcc_linker_name_digest: observation.alpine_libgcc_linker_name_digest,
            alpine_busybox_digest: observation.alpine_busybox_digest,
            rust_toolchain_layout_digest: observation.rust_toolchain_layout_digest,
            normalized_alpine_link_plan_digest: observation.normalized_alpine_link_plan_digest,
            postconditions: observation.postconditions,
        }
    }

    pub(super) fn redacted_digest(&self) -> Digest {
        let mut digest = RedactedDigestBuilder::new(b"runtime-isolation/bootstrap-root/v3");
        digest.push_u64(self.root_mount.device());
        digest.push_u64(self.root_mount.inode());
        for value in [
            &self.alpine_installed_database_digest,
            &self.alpine_loader_digest,
            &self.alpine_libc_linker_name_digest,
            &self.alpine_libgcc_digest,
            &self.alpine_libgcc_linker_name_digest,
            &self.alpine_busybox_digest,
            &self.rust_toolchain_layout_digest,
            &self.normalized_alpine_link_plan_digest,
        ] {
            digest.push_bytes(value.as_str().as_bytes());
        }
        for observed in self.postconditions.observations() {
            digest.push_bool(observed);
        }
        digest.finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RetainedProgramBootstrapRootPostconditions {
    observed: u8,
}

impl RetainedProgramBootstrapRootPostconditions {
    const OLD_ROOT_DETACHED: u8 = 1;
    const ROOT_READ_ONLY: u8 = 2;
    const TOOLCHAIN_READ_ONLY: u8 = 4;
    const HOST_PATHS_ABSENT: u8 = 8;
    const PROC_MTAB_VALIDATED: u8 = 16;
    const ALL_OBSERVED: Self = Self::from_observations([true, true, true, true, true]);

    pub(crate) const fn from_observations(observations: [bool; 5]) -> Self {
        let mut observed = 0;
        if observations[0] {
            observed |= Self::OLD_ROOT_DETACHED;
        }
        if observations[1] {
            observed |= Self::ROOT_READ_ONLY;
        }
        if observations[2] {
            observed |= Self::TOOLCHAIN_READ_ONLY;
        }
        if observations[3] {
            observed |= Self::HOST_PATHS_ABSENT;
        }
        if observations[4] {
            observed |= Self::PROC_MTAB_VALIDATED;
        }
        Self { observed }
    }

    #[cfg(target_os = "linux")]
    pub(crate) const fn with_proc_mtab_validated(mut self) -> Self {
        self.observed |= Self::PROC_MTAB_VALIDATED;
        self
    }

    const fn all_observed(self) -> bool {
        self.observed == Self::ALL_OBSERVED.observed
    }

    const fn observations(self) -> [bool; 5] {
        [
            self.observed & Self::OLD_ROOT_DETACHED != 0,
            self.observed & Self::ROOT_READ_ONLY != 0,
            self.observed & Self::TOOLCHAIN_READ_ONLY != 0,
            self.observed & Self::HOST_PATHS_ABSENT != 0,
            self.observed & Self::PROC_MTAB_VALIDATED != 0,
        ]
    }
}

#[cfg(target_os = "linux")]
pub(crate) struct RetainedProgramBootstrapRootObservation {
    pub(crate) root_mount: NamespaceIdentity,
    pub(crate) alpine_installed_database_digest: Digest,
    pub(crate) alpine_loader_digest: Digest,
    pub(crate) alpine_libc_linker_name_digest: Digest,
    pub(crate) alpine_libgcc_digest: Digest,
    pub(crate) alpine_libgcc_linker_name_digest: Digest,
    pub(crate) alpine_busybox_digest: Digest,
    pub(crate) rust_toolchain_layout_digest: Digest,
    pub(crate) normalized_alpine_link_plan_digest: Digest,
    pub(crate) postconditions: RetainedProgramBootstrapRootPostconditions,
}

#[cfg(target_os = "linux")]
impl RetainedProgramBootstrapRootObservation {
    pub(crate) fn record_proc_mtab_validation(&mut self) {
        self.postconditions = self.postconditions.with_proc_mtab_validated();
    }
}

/// Opaque live result from one retained-program bootstrap attempt.
///
/// This type is intentionally neither cloneable nor serializable. Durable JSON
/// derived from it remains audit data and cannot recreate this execution result.
#[derive(Debug)]
pub struct RetainedProgramBootstrapExecution {
    attempt: RetainedProgramBootstrapAttempt,
    launch_digest: Digest,
    root: RetainedProgramBootstrapRootEvidence,
    controlled_build: ControlledBuildExecution,
}

impl RetainedProgramBootstrapExecution {
    /// Returns the exact attempt identity.
    #[must_use]
    pub const fn attempt(&self) -> RetainedProgramBootstrapAttempt {
        self.attempt
    }

    /// Returns the complete bootstrap launch digest.
    #[must_use]
    pub const fn launch_digest(&self) -> &Digest {
        &self.launch_digest
    }

    /// Returns root-transition evidence observed before privilege reduction.
    #[must_use]
    pub const fn root_evidence(&self) -> &RetainedProgramBootstrapRootEvidence {
        &self.root
    }

    /// Returns the existing controlled-build lifecycle result.
    #[must_use]
    pub const fn controlled_build(&self) -> &ControlledBuildExecution {
        &self.controlled_build
    }

    /// Returns a domain-separated digest of the live result's content-free evidence.
    #[must_use]
    pub fn redacted_digest(&self) -> Digest {
        let mut digest = RedactedDigestBuilder::new(b"runtime-isolation/bootstrap-execution/v1");
        digest.push_u8(self.attempt.code());
        digest.push_bytes(self.launch_digest.as_str().as_bytes());
        digest.push_bytes(self.root.redacted_digest().as_str().as_bytes());
        digest.push_bytes(
            self.controlled_build
                .isolation()
                .redacted_digest()
                .as_str()
                .as_bytes(),
        );
        digest.finish()
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn new(
        specification: &RetainedProgramBootstrapLaunchSpec,
        root: RetainedProgramBootstrapRootEvidence,
        controlled_build: ControlledBuildExecution,
    ) -> IsolationResult<Self> {
        if !root.all_canaries_passed()
            || controlled_build.output().status() != ControlledBuildProcessStatus::Success
            || controlled_build.guardian_helper_status() != ControlledBuildProcessStatus::Success
        {
            return Err(IsolationError::BootstrapRootVerification);
        }
        Ok(Self {
            attempt: specification.attempt,
            launch_digest: specification.redacted_digest(),
            root,
            controlled_build,
        })
    }
}
