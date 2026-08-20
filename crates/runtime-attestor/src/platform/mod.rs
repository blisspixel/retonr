#[cfg(any(target_os = "linux", windows))]
mod file;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
pub(crate) use linux::Lease;
#[cfg(target_os = "macos")]
pub(crate) use macos::Lease;
#[cfg(windows)]
pub(crate) use windows::Lease;

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
mod unsupported;
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
pub(crate) use unsupported::Lease;
