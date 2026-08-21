use std::{fs::File, net::TcpStream};

/// Maximum startup bytes retained independently for target standard output and error.
pub const MAXIMUM_STARTUP_STREAM_BYTES: usize = 32 * 1024;

/// Bounded in-memory target startup output.
///
/// This value deliberately has no serialization implementation. Debug output reports
/// byte counts and truncation only, never captured process output.
#[derive(Clone, Eq, PartialEq)]
pub struct ManagedStartupOutput {
    output: Vec<u8>,
    error: Vec<u8>,
    output_truncated: bool,
    error_truncated: bool,
}

impl ManagedStartupOutput {
    #[cfg(target_os = "linux")]
    pub(crate) const fn new(
        standard_output: Vec<u8>,
        standard_error: Vec<u8>,
        standard_output_truncated: bool,
        standard_error_truncated: bool,
    ) -> Self {
        Self {
            output: standard_output,
            error: standard_error,
            output_truncated: standard_output_truncated,
            error_truncated: standard_error_truncated,
        }
    }

    /// Returns the bounded standard-output prefix.
    #[must_use]
    pub fn standard_output(&self) -> &[u8] {
        &self.output
    }

    /// Returns the bounded standard-error prefix.
    #[must_use]
    pub fn standard_error(&self) -> &[u8] {
        &self.error
    }

    /// Returns whether more standard-output bytes were drained but not retained.
    #[must_use]
    pub const fn standard_output_truncated(&self) -> bool {
        self.output_truncated
    }

    /// Returns whether more standard-error bytes were drained but not retained.
    #[must_use]
    pub const fn standard_error_truncated(&self) -> bool {
        self.error_truncated
    }
}

impl std::fmt::Debug for ManagedStartupOutput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ManagedStartupOutput")
            .field("standard_output_bytes", &self.output.len())
            .field("standard_error_bytes", &self.error.len())
            .field("standard_output_truncated", &self.output_truncated)
            .field("standard_error_truncated", &self.error_truncated)
            .finish()
    }
}

/// Retained Linux socket-diagnostics capability created inside the target network namespace.
///
/// This is an inert native capability. It does not identify a listener, process, or
/// connection. A later attestor must validate its socket type, binding, namespace,
/// protocol replies, and relationship to retained process evidence.
pub struct LinuxSocketDiagnosticsCapability {
    file: File,
}

impl LinuxSocketDiagnosticsCapability {
    #[cfg(target_os = "linux")]
    pub(crate) const fn new(file: File) -> Self {
        Self { file }
    }

    /// Consumes the capability and returns its owned native descriptor.
    #[must_use]
    pub fn into_file(self) -> File {
        self.file
    }
}

impl std::fmt::Debug for LinuxSocketDiagnosticsCapability {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LinuxSocketDiagnosticsCapability")
            .finish_non_exhaustive()
    }
}

/// One connected loopback stream plus a namespace-bound diagnostics capability.
///
/// The isolation layer validates only the exact loopback endpoints and descriptor
/// properties. It makes no listener, process-owner, socket-sharing, or handler claim.
pub struct ManagedLoopbackChannel {
    stream: TcpStream,
    socket_diagnostics: LinuxSocketDiagnosticsCapability,
    startup_output: ManagedStartupOutput,
}

impl ManagedLoopbackChannel {
    #[cfg(target_os = "linux")]
    pub(crate) const fn new(
        stream: TcpStream,
        socket_diagnostics: LinuxSocketDiagnosticsCapability,
        startup_output: ManagedStartupOutput,
    ) -> Self {
        Self {
            stream,
            socket_diagnostics,
            startup_output,
        }
    }

    /// Borrows the exact connected stream retained by this capability.
    #[must_use]
    pub const fn stream(&self) -> &TcpStream {
        &self.stream
    }

    /// Returns the bounded startup output captured before channel handoff.
    #[must_use]
    pub const fn startup_output(&self) -> &ManagedStartupOutput {
        &self.startup_output
    }

    /// Consumes the channel into its separately retained capabilities.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        TcpStream,
        LinuxSocketDiagnosticsCapability,
        ManagedStartupOutput,
    ) {
        (self.stream, self.socket_diagnostics, self.startup_output)
    }
}

impl std::fmt::Debug for ManagedLoopbackChannel {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ManagedLoopbackChannel")
            .field("startup_output", &self.startup_output)
            .finish_non_exhaustive()
    }
}
