use std::{ffi::CString, fs::File, path::Path};

use rustix::mount::{MountFlags, mount, mount_bind, mount_remount};

use super::{
    HelperFailure, MAXIMUM_CONTROLLED_BUILD_WORKSPACE_BYTES,
    MAXIMUM_CONTROLLED_BUILD_WORKSPACE_INODES,
};

pub(super) fn establish(root: &Path, input: &Path, output: &Path) -> Result<(), HelperFailure> {
    // A nonrecursive self-bind made after child mounts would hide those mounts.
    // Establish the root mount first, then retain every child while sealing it.
    mount_bind(root, root).map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    bind_at(input, &root.join("inputs"), true)?;
    bind_at(output, &root.join("output"), false)?;
    for relative in ["toolchain", "source", "vendor", "raw-crates"] {
        let path = root.join(relative);
        bind_at(&path, &path, true)?;
    }
    for relative in ["cargo-home", "target"] {
        mount_writable_tmpfs(&root.join(relative))?;
    }
    let dev_null = root.join("dev/null");
    if !dev_null.exists() {
        File::create(&dev_null).map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    }
    mount_bind("/dev/null", &dev_null).map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    remount_read_only(root)
}

fn bind_at(source: &Path, destination: &Path, read_only: bool) -> Result<(), HelperFailure> {
    mount_bind(source, destination).map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    if read_only {
        remount_read_only(destination)?;
    }
    Ok(())
}

fn remount_read_only(path: &Path) -> Result<(), HelperFailure> {
    mount_remount(
        path,
        MountFlags::BIND | MountFlags::RDONLY | MountFlags::NODEV | MountFlags::NOSUID,
        "",
    )
    .map_err(|_| HelperFailure::BootstrapRootPreparation)
}

fn mount_writable_tmpfs(path: &Path) -> Result<(), HelperFailure> {
    let options = CString::new(format!("size={MAXIMUM_CONTROLLED_BUILD_WORKSPACE_BYTES},nr_inodes={MAXIMUM_CONTROLLED_BUILD_WORKSPACE_INODES},mode=0700"))
        .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    mount(
        "tmpfs",
        path,
        "tmpfs",
        MountFlags::NODEV | MountFlags::NOSUID,
        Some(options.as_c_str()),
    )
    .map_err(|_| HelperFailure::BootstrapRootPreparation)
}

#[cfg(test)]
mod tests;
