//! Shared detection for hosts that cannot create symbolic links.
//!
//! Several hardening fixtures build a symbolic link or directory junction and
//! then require the service to reject it. Creating either one on Windows needs
//! `SeCreateSymbolicLinkPrivilege`, which an ordinary account holds only when
//! Developer Mode is enabled or the process is elevated.
//!
//! Windows reports the missing privilege as `ERROR_PRIVILEGE_NOT_HELD`, raw
//! operating-system error 1314. The standard library does not map that code to
//! [`std::io::ErrorKind::PermissionDenied`], so a fixture that inspects only the
//! error kind never recognizes it and fails instead of skipping. The raw code
//! must be compared explicitly.

/// `ERROR_PRIVILEGE_NOT_HELD`, raised when the account lacks
/// `SeCreateSymbolicLinkPrivilege`.
const ERROR_PRIVILEGE_NOT_HELD: i32 = 1314;

/// Returns whether the host refused to create a link, reporting the skip on
/// standard error so an unrunnable case stays visible instead of passing
/// silently.
///
/// A refusal is only ever inapplicability. Any other failure remains a genuine
/// fixture error and must still panic at the call site.
#[expect(
    clippy::print_stderr,
    reason = "a skipped fixture must be visible in the test run, and this \
              module is compiled only under cfg(test)"
)]
pub(crate) fn skip_unavailable_link(fixture: &str, error: &std::io::Error) -> bool {
    let unavailable = error.kind() == std::io::ErrorKind::PermissionDenied
        || cfg!(windows) && error.raw_os_error() == Some(ERROR_PRIVILEGE_NOT_HELD);
    if unavailable {
        eprintln!(
            "skipping {fixture}: this host cannot create the link the fixture needs \
             ({error}). Continuous integration covers this case."
        );
    }
    unavailable
}
