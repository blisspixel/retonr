use std::time::Duration;

use super::digest::RedactedDigestBuilder;
use crate::{IsolationError, IsolationResult};

/// Closed visibility policy for devices exposed to a managed runtime.
///
/// This policy provides bounded Linux device visibility evidence. It does not,
/// by itself, prove that a workload executed only on a CPU.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ManagedDeviceVisibilityPolicy {
    /// Exposes only one retained `/dev/null` object through a private `/dev`.
    LinuxCpuOnlyV1 = 1,
}

impl ManagedDeviceVisibilityPolicy {
    pub(crate) const fn code(self) -> u8 {
        self as u8
    }

    #[cfg(target_os = "linux")]
    pub(crate) const fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(Self::LinuxCpuOnlyV1),
            _ => None,
        }
    }
}

/// Immutable policy bounds applied before a managed runtime starts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IsolationPolicy {
    startup_timeout: Duration,
    shutdown_timeout: Duration,
    maximum_arguments: usize,
    maximum_environment_variables: usize,
    maximum_value_bytes: usize,
    maximum_open_files: u64,
    maximum_processes: u64,
    managed_device_visibility: ManagedDeviceVisibilityPolicy,
}

impl IsolationPolicy {
    /// Creates a policy after validating every explicit resource bound.
    ///
    /// # Errors
    ///
    /// Returns [`IsolationError::InvalidPolicy`] when any bound is zero,
    /// unreasonably large, or internally inconsistent.
    pub fn new(
        startup_timeout: Duration,
        shutdown_timeout: Duration,
        maximum_arguments: usize,
        maximum_environment_variables: usize,
        maximum_value_bytes: usize,
        maximum_open_files: u64,
        maximum_processes: u64,
    ) -> IsolationResult<Self> {
        let policy = Self {
            startup_timeout,
            shutdown_timeout,
            maximum_arguments,
            maximum_environment_variables,
            maximum_value_bytes,
            maximum_open_files,
            maximum_processes,
            managed_device_visibility: ManagedDeviceVisibilityPolicy::LinuxCpuOnlyV1,
        };
        policy.validate()?;
        Ok(policy)
    }

    pub(super) fn validate(self) -> IsolationResult<()> {
        if self.startup_timeout.is_zero() || self.startup_timeout > Duration::from_secs(30) {
            return Err(IsolationError::InvalidPolicy("startup timeout"));
        }
        if self.shutdown_timeout.is_zero() || self.shutdown_timeout > Duration::from_secs(30) {
            return Err(IsolationError::InvalidPolicy("shutdown timeout"));
        }
        if !(1..=4_096).contains(&self.maximum_arguments) {
            return Err(IsolationError::InvalidPolicy("argument count"));
        }
        if self.maximum_environment_variables > 1_024 {
            return Err(IsolationError::InvalidPolicy("environment count"));
        }
        if !(1..=1024 * 1024).contains(&self.maximum_value_bytes) {
            return Err(IsolationError::InvalidPolicy("value bytes"));
        }
        if !(64..=1_048_576).contains(&self.maximum_open_files) {
            return Err(IsolationError::InvalidPolicy("open-file limit"));
        }
        if !(8..=65_536).contains(&self.maximum_processes) {
            return Err(IsolationError::InvalidPolicy("process limit"));
        }
        Ok(())
    }

    /// Returns the maximum preparation and launch duration.
    #[must_use]
    pub const fn startup_timeout(self) -> Duration {
        self.startup_timeout
    }

    /// Returns the maximum graceful shutdown duration.
    #[must_use]
    pub const fn shutdown_timeout(self) -> Duration {
        self.shutdown_timeout
    }

    /// Returns a domain-separated digest of every exact policy bound.
    #[must_use]
    pub fn redacted_digest(self) -> rewrite_types::Digest {
        let mut digest = RedactedDigestBuilder::new(b"runtime-isolation/policy/v2");
        digest.push_u64(self.startup_timeout.as_secs());
        digest.push_u32(self.startup_timeout.subsec_nanos());
        digest.push_u64(self.shutdown_timeout.as_secs());
        digest.push_u32(self.shutdown_timeout.subsec_nanos());
        digest.push_usize(self.maximum_arguments);
        digest.push_usize(self.maximum_environment_variables);
        digest.push_usize(self.maximum_value_bytes);
        digest.push_u64(self.maximum_open_files);
        digest.push_u64(self.maximum_processes);
        digest.push_u8(self.managed_device_visibility.code());
        digest.finish()
    }

    /// Returns the exact closed device-visibility policy.
    #[must_use]
    pub const fn managed_device_visibility(self) -> ManagedDeviceVisibilityPolicy {
        self.managed_device_visibility
    }

    pub(crate) const fn maximum_arguments(self) -> usize {
        self.maximum_arguments
    }

    pub(crate) const fn maximum_environment_variables(self) -> usize {
        self.maximum_environment_variables
    }

    pub(crate) const fn maximum_value_bytes(self) -> usize {
        self.maximum_value_bytes
    }

    #[cfg(target_os = "linux")]
    pub(crate) const fn maximum_open_files(self) -> u64 {
        self.maximum_open_files
    }

    #[cfg(target_os = "linux")]
    pub(crate) const fn maximum_processes(self) -> u64 {
        self.maximum_processes
    }
}

impl Default for IsolationPolicy {
    fn default() -> Self {
        Self {
            startup_timeout: Duration::from_secs(10),
            shutdown_timeout: Duration::from_secs(5),
            maximum_arguments: 256,
            maximum_environment_variables: 256,
            maximum_value_bytes: 64 * 1024,
            maximum_open_files: 4_096,
            maximum_processes: 1_024,
            managed_device_visibility: ManagedDeviceVisibilityPolicy::LinuxCpuOnlyV1,
        }
    }
}
