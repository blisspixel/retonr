//! Fail-closed managed runtime isolation.
//!
//! The public API separates capability preparation from launch and retains a
//! lease for the complete managed process tree. Unsupported platforms return a
//! deterministic error and never launch the requested runtime.

mod contract;
#[cfg(any(target_os = "linux", test))]
mod deadline;
mod error;
mod platform;

#[cfg(target_os = "linux")]
pub(crate) use contract::CONTROLLED_BUILD_INPUT_SNAPSHOT_TIMEOUT;

pub use contract::{
    AuthenticatedBusyboxExecutable, ControlledBuildExecution, ControlledBuildInputFile,
    ControlledBuildIsolationEvidence, ControlledBuildLaunchSpec, ControlledBuildOutput,
    ControlledBuildOutputTree, ControlledBuildOutputTreeEntry, ControlledBuildProcessStatus,
    IsolationEvidence, IsolationPolicy, IsolationPreparationEvidence, LaunchSpec,
    LinuxSocketDiagnosticsCapability, MANAGED_RUNTIME_INPUT_ROOT_V1,
    MAXIMUM_CONTROLLED_BUILD_INPUT_BYTES, MAXIMUM_CONTROLLED_BUILD_INPUT_FILES,
    MAXIMUM_CONTROLLED_BUILD_OUTPUT_BYTES, MAXIMUM_CONTROLLED_BUILD_OUTPUT_TREE_ENTRIES,
    MAXIMUM_CONTROLLED_BUILD_WORKSPACE_BYTES, MAXIMUM_CONTROLLED_BUILD_WORKSPACE_INODES,
    MAXIMUM_MANAGED_RUNTIME_INPUT_BYTES, MAXIMUM_MANAGED_RUNTIME_INPUT_FILES,
    MAXIMUM_STARTUP_STREAM_BYTES, ManagedDeviceBoundaryEvidence, ManagedDeviceVisibilityPolicy,
    ManagedLoopbackChannel, ManagedRuntimeInputEvidence, ManagedStartupOutput, NamespaceIdentity,
    PreparedIsolation, PreparedIsolationSubjectToken, RetainedIsolationLease,
    RetainedProgramBootstrapAttempt, RetainedProgramBootstrapCapabilities,
    RetainedProgramBootstrapExecution, RetainedProgramBootstrapInputKind,
    RetainedProgramBootstrapInputMeasurement, RetainedProgramBootstrapLaunchSpec,
    RetainedProgramBootstrapRootEvidence, RetainedProgramBootstrapSignedInputs,
    RetainedRuntimeInputSink, RetainedRuntimeInputSource, RetainedRuntimeInputTree,
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
