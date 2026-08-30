use std::fmt;

#[cfg(target_os = "linux")]
use std::io;

use thiserror::Error;

/// Result type for managed runtime isolation operations.
pub type IsolationResult<T> = Result<T, IsolationError>;

/// Bounded, redacted failure modes for managed runtime isolation.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum IsolationError {
    /// The platform has no qualifying implementation in this release.
    #[error("managed runtime isolation is unsupported on this platform")]
    UnsupportedPlatform,
    /// Cooperative cancellation was requested.
    #[error("managed runtime isolation was cancelled")]
    Cancelled,
    /// A public policy field was outside its accepted range.
    #[error("invalid isolation policy: {0}")]
    InvalidPolicy(&'static str),
    /// The dedicated helper was absent or not a regular executable.
    #[error("the isolation helper is not an available regular executable")]
    InvalidHelper,
    /// The managed runtime launch description was invalid.
    #[error("invalid managed launch: {0}")]
    InvalidLaunch(&'static str),
    /// The retained-program bootstrap description was incomplete or inconsistent.
    #[error("invalid retained-program bootstrap: {0}")]
    InvalidBootstrap(&'static str),
    /// Host policy denied creation of the required user or network namespace.
    #[error("host policy denied the required Linux namespaces")]
    HostPolicyDenied,
    /// A namespace could not be established or mapped completely.
    #[error("the managed namespace could not be established")]
    NamespaceSetup,
    /// Loopback could not be enabled as the only network interface.
    #[error("the loopback-only network could not be established")]
    LoopbackSetup,
    /// A local-allow or non-loopback-deny canary failed.
    #[error("the loopback-only network canaries did not pass")]
    NetworkCanary,
    /// An ambient descriptor could not be sealed before the next helper exec.
    #[error("the helper retained an inheritable file descriptor")]
    DescriptorLeak,
    /// Privileges could not be irreversibly reduced before target launch.
    #[error("the helper could not drop privileges completely")]
    PrivilegeDrop,
    /// The target socket-family policy could not be compiled.
    #[error("the target socket-family policy could not be compiled")]
    SocketPolicyCompile,
    /// The target socket-family policy could not be installed.
    #[error("the target socket-family policy could not be installed")]
    SocketPolicyInstall,
    /// The installed target socket-family policy was not active.
    #[error("the target socket-family policy was not active")]
    SocketPolicyInactive,
    /// The installed target socket-family policy failed its behavioral checks.
    #[error("the target socket-family policy failed its behavioral checks")]
    SocketPolicyBehavior,
    /// The host cannot establish the required private managed device mounts.
    #[error("the required managed device boundary is unavailable")]
    ManagedDeviceBoundaryUnavailable,
    /// The private managed device boundary could not be constructed.
    #[error("the managed device boundary could not be established")]
    ManagedDeviceBoundarySetup,
    /// The private managed device boundary failed an exact visibility check.
    #[error("the managed device boundary failed its visibility checks")]
    ManagedDeviceBoundaryBehavior,
    /// The managed namespace and mount escape filter could not be compiled.
    #[error("the managed containment policy could not be compiled")]
    ManagedContainmentPolicyCompile,
    /// The managed namespace and mount escape filter could not be installed.
    #[error("the managed containment policy could not be installed")]
    ManagedContainmentPolicyInstall,
    /// The managed namespace and mount escape filter was not active.
    #[error("the managed containment policy was not active")]
    ManagedContainmentPolicyInactive,
    /// The managed namespace and mount escape filter failed its behavioral checks.
    #[error("the managed containment policy failed its behavioral checks")]
    ManagedContainmentPolicyBehavior,
    /// A managed runtime-input source violated the closed capability contract.
    #[error("the retained runtime input capability was invalid")]
    RuntimeInputObjectMismatch,
    /// The host cannot establish the required private runtime-input mounts.
    #[error("the required managed runtime input boundary is unavailable")]
    RuntimeInputBoundaryUnavailable,
    /// The private managed runtime-input tree could not be constructed.
    #[error("the managed runtime input boundary could not be established")]
    RuntimeInputBoundarySetup,
    /// The private managed runtime-input tree failed exact reobservation.
    #[error("the managed runtime input boundary failed its exact checks")]
    RuntimeInputBoundaryBehavior,
    /// The host kernel lacks the required Landlock filesystem ABI.
    #[error("the required Linux filesystem-isolation ABI is unavailable")]
    FilesystemIsolationUnavailable,
    /// The controlled build filesystem policy could not be installed.
    #[error("the controlled build filesystem policy could not be installed")]
    FilesystemIsolationSetup,
    /// A fixed controlled-build mount alias could not be established.
    #[error("controlled build filesystem alias setup failed: {0}")]
    FilesystemAliasSetup(&'static str),
    /// The controlled build filesystem policy failed its behavioral checks.
    #[error("the controlled build filesystem policy failed its behavioral checks")]
    FilesystemIsolationBehavior,
    /// A retained controlled-build object did not match its declared identity.
    #[error("a retained controlled build object did not match its declaration")]
    ControlledBuildObjectMismatch,
    /// The controlled build output directory was not initially empty.
    #[error("the controlled build output boundary was not initially empty")]
    ControlledBuildOutputNotEmpty,
    /// The helper sent malformed, inconsistent, or oversized evidence.
    #[error("the isolation helper protocol was invalid")]
    HelperProtocol,
    /// The requested endpoint was not an exact, nonzero loopback socket address.
    #[error("the managed loopback endpoint was invalid")]
    InvalidChannelEndpoint,
    /// The lease's single managed loopback request was already consumed.
    #[error("the managed loopback channel was already requested")]
    ChannelAlreadyRequested,
    /// The helper did not produce launch evidence within the startup bound.
    #[error("managed runtime isolation startup timed out")]
    StartupTimeout,
    /// The caller's already-captured operation deadline was reached.
    #[error("managed runtime isolation operation deadline was exceeded")]
    OperationDeadlineExceeded,
    /// The controlled build process tree exceeded its wall-time bound.
    #[error("the controlled build process tree timed out")]
    ControlledBuildTimeout,
    /// The retained input tree could not be snapshotted within its fixed bound.
    #[error("the controlled build input snapshot timed out")]
    ControlledBuildSnapshotTimeout,
    /// The bootstrap root could not be prepared before privilege reduction.
    #[error("the retained-program bootstrap root could not be prepared")]
    BootstrapRootPreparation,
    /// A required bootstrap root-transition postcondition was not observed.
    #[error("the retained-program bootstrap root postconditions did not verify")]
    BootstrapRootVerification,
    /// The retained guardian or runtime exited.
    #[error("the managed runtime process tree exited")]
    ProcessExited,
    /// Retained native isolation evidence changed during reobservation.
    #[error("managed runtime isolation evidence changed")]
    EvidenceChanged,
    /// The process tree did not terminate within the shutdown bound.
    #[error("managed runtime isolation shutdown timed out")]
    ShutdownTimeout,
    /// A redacted native operation failed.
    #[error("native operation {operation} failed with {kind}")]
    NativeOperation {
        /// Stable operation label without a host path or payload.
        operation: &'static str,
        /// Portable I/O error category.
        kind: IoErrorKind,
    },
}

/// Stable I/O error categories safe to retain in evidence and logs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IoErrorKind {
    NotFound,
    PermissionDenied,
    InvalidInput,
    ResourceLimit,
    Interrupted,
    Other,
}

impl fmt::Display for IoErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::NotFound => "not-found",
            Self::PermissionDenied => "permission-denied",
            Self::InvalidInput => "invalid-input",
            Self::ResourceLimit => "resource-limit",
            Self::Interrupted => "interrupted",
            Self::Other => "other",
        };
        formatter.write_str(value)
    }
}

#[cfg(test)]
mod display_tests {
    use super::{IoErrorKind, IsolationError};

    #[test]
    fn every_error_has_a_stable_redacted_display() {
        let errors = [
            IsolationError::UnsupportedPlatform,
            IsolationError::Cancelled,
            IsolationError::InvalidPolicy("field"),
            IsolationError::InvalidHelper,
            IsolationError::InvalidLaunch("field"),
            IsolationError::InvalidBootstrap("field"),
            IsolationError::HostPolicyDenied,
            IsolationError::NamespaceSetup,
            IsolationError::LoopbackSetup,
            IsolationError::NetworkCanary,
            IsolationError::DescriptorLeak,
            IsolationError::PrivilegeDrop,
            IsolationError::SocketPolicyCompile,
            IsolationError::SocketPolicyInstall,
            IsolationError::SocketPolicyInactive,
            IsolationError::SocketPolicyBehavior,
            IsolationError::ManagedDeviceBoundaryUnavailable,
            IsolationError::ManagedDeviceBoundarySetup,
            IsolationError::ManagedDeviceBoundaryBehavior,
            IsolationError::ManagedContainmentPolicyCompile,
            IsolationError::ManagedContainmentPolicyInstall,
            IsolationError::ManagedContainmentPolicyInactive,
            IsolationError::ManagedContainmentPolicyBehavior,
            IsolationError::RuntimeInputObjectMismatch,
            IsolationError::RuntimeInputBoundaryUnavailable,
            IsolationError::RuntimeInputBoundarySetup,
            IsolationError::RuntimeInputBoundaryBehavior,
            IsolationError::FilesystemIsolationUnavailable,
            IsolationError::FilesystemIsolationSetup,
            IsolationError::FilesystemAliasSetup("stage"),
            IsolationError::FilesystemIsolationBehavior,
            IsolationError::ControlledBuildObjectMismatch,
            IsolationError::ControlledBuildOutputNotEmpty,
            IsolationError::HelperProtocol,
            IsolationError::InvalidChannelEndpoint,
            IsolationError::ChannelAlreadyRequested,
            IsolationError::StartupTimeout,
            IsolationError::OperationDeadlineExceeded,
            IsolationError::ControlledBuildTimeout,
            IsolationError::ControlledBuildSnapshotTimeout,
            IsolationError::BootstrapRootPreparation,
            IsolationError::BootstrapRootVerification,
            IsolationError::ProcessExited,
            IsolationError::EvidenceChanged,
            IsolationError::ShutdownTimeout,
            IsolationError::NativeOperation {
                operation: "operation",
                kind: IoErrorKind::Other,
            },
        ];
        for error in errors {
            assert!(!error.to_string().is_empty());
        }
        let kinds = [
            IoErrorKind::NotFound,
            IoErrorKind::PermissionDenied,
            IoErrorKind::InvalidInput,
            IoErrorKind::ResourceLimit,
            IoErrorKind::Interrupted,
            IoErrorKind::Other,
        ];
        for kind in kinds {
            assert!(!kind.to_string().is_empty());
        }
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn native(operation: &'static str, error: &io::Error) -> IsolationError {
    let kind = match error.kind() {
        io::ErrorKind::NotFound => IoErrorKind::NotFound,
        io::ErrorKind::PermissionDenied => IoErrorKind::PermissionDenied,
        io::ErrorKind::InvalidInput | io::ErrorKind::InvalidData => IoErrorKind::InvalidInput,
        io::ErrorKind::OutOfMemory | io::ErrorKind::StorageFull => IoErrorKind::ResourceLimit,
        io::ErrorKind::Interrupted => IoErrorKind::Interrupted,
        _ => IoErrorKind::Other,
    };
    IsolationError::NativeOperation { operation, kind }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use std::io;

    use super::{IoErrorKind, IsolationError, native};

    #[test]
    fn native_errors_are_redacted_and_stable() {
        let error = native(
            "open-helper",
            &io::Error::new(io::ErrorKind::NotFound, "secret"),
        );
        assert_eq!(
            error,
            IsolationError::NativeOperation {
                operation: "open-helper",
                kind: IoErrorKind::NotFound,
            }
        );
        assert!(!error.to_string().contains("secret"));
    }
}
