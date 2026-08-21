#[cfg(any(target_os = "linux", windows))]
mod file;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
mod linux_connection;
#[cfg(target_os = "linux")]
mod linux_managed;
#[cfg(target_os = "linux")]
mod linux_native_load;
#[cfg(target_os = "linux")]
mod linux_proc_holders;
#[cfg(target_os = "linux")]
mod linux_sock_diag;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
mod native_load_common;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
mod windows_connection;

#[cfg(target_os = "linux")]
pub(crate) use linux::Lease;
#[cfg(target_os = "linux")]
pub(crate) use linux_managed::Lease as ManagedLease;
#[cfg(target_os = "macos")]
pub(crate) use macos::Lease;
#[cfg(windows)]
pub(crate) use windows::Lease;

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
mod unsupported;
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
pub(crate) use unsupported::Lease;

#[cfg(not(target_os = "linux"))]
mod managed_unsupported;
#[cfg(not(target_os = "linux"))]
pub(crate) use managed_unsupported::Lease as ManagedLease;
