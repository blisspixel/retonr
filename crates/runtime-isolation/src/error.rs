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
            IsolationError::HelperProtocol,
            IsolationError::InvalidChannelEndpoint,
            IsolationError::ChannelAlreadyRequested,
            IsolationError::StartupTimeout,
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
