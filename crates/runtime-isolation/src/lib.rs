//! Fail-closed managed runtime isolation.
//!
//! The public API separates capability preparation from launch and retains a
//! lease for the complete managed process tree. Unsupported platforms return a
//! deterministic error and never launch the requested runtime.

mod contract;
mod error;
mod platform;

pub use contract::{
    IsolationEvidence, IsolationPolicy, IsolationPreparationEvidence, LaunchSpec,
    LinuxSocketDiagnosticsCapability, MAXIMUM_STARTUP_STREAM_BYTES, ManagedLoopbackChannel,
    ManagedStartupOutput, NamespaceIdentity, PreparedIsolation, RetainedIsolationLease,
    TargetProcessEvidence,
};
pub use error::{IoErrorKind, IsolationError, IsolationResult};

/// Runs the private managed-launch helper protocol.
///
/// This entry point is public only so the package's dedicated helper binary can
/// remain a minimal wrapper. Applications must use [`PreparedIsolation`].
#[doc(hidden)]
#[must_use]
pub fn run_managed_helper() -> i32 {
    platform::run_helper()
}

#[cfg(all(test, not(target_os = "linux")))]
mod tests {
    #[test]
    fn helper_is_inert_on_unsupported_platforms() {
        assert_eq!(super::run_managed_helper(), 64);
    }
}
