use std::{
    ffi::CString,
    fs::{self, File, OpenOptions},
    io::Write as _,
    os::unix::fs::{MetadataExt as _, OpenOptionsExt as _},
    path::Path,
    process::{Command, Stdio},
};

use crate::{
    MAXIMUM_CONTROLLED_BUILD_WORKSPACE_BYTES, MAXIMUM_CONTROLLED_BUILD_WORKSPACE_INODES,
    NamespaceIdentity,
    contract::{
        RetainedProgramBootstrapRootObservation, RetainedProgramBootstrapRootPostconditions,
    },
};
use rustix::{
    fs::{StatVfsMountFlags, fstatvfs},
    mount::{MountFlags, UnmountFlags, mount, mount_bind, mount_remount, unmount},
    process::{chdir, pivot_root},
};

use super::{
    linux_bootstrap_archive::{self, ArchiveLimits, Compression, LinkPolicy},
    linux_bootstrap_root::{RootPreparation, RootPreparationStep},
    linux_build_mount::{BUILD_INPUT_ROOT, BUILD_OUTPUT_ROOT, open_private_regular},
    linux_helper_setup::HelperFailure,
};

const ROOTFS: &str = "/tmp/retonr-bootstrap-root";
const TOOLCHAIN_STAGING: &str = "/tmp/retonr-bootstrap-toolchain";
const MAXIMUM_ARCHIVE_FILE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

const MINIROOTFS: &str = "lineage/host/alpine/alpine-minirootfs-3.23.3-x86_64.tar.gz";
const LIBGCC_APK: &str = "lineage/host/alpine/libgcc-15.2.0-r2.apk";
const BUSYBOX: &str = "toolchains/busybox";
const CARGO: &str = "lineage/rust/cargo-1.97.1-x86_64-unknown-linux-musl.tar.gz";
const RUSTC: &str = "lineage/rust/rustc-1.97.1-x86_64-unknown-linux-musl.tar.gz";
const RUST_STD: &str = "lineage/rust/rust-std-1.97.1-x86_64-unknown-linux-musl.tar.gz";
const SOURCE: &str = "lineage/source/retonr-source.tar";
const VENDOR: &str = "lineage/source/cargo-vendor.tar";
const RAW_CRATES: &str = "lineage/source/cargo-crates.tar";

#[derive(Clone, Copy)]
struct HostIdentity {
    device: u64,
    inode: u64,
}

mod linker_name;
mod tree;

use linker_name::{install_linker_names, validate_libc_linker_name, validate_libgcc_linker_name};
use tree::{copy_file, hash_file, hash_tree, merge_tree};

pub(super) fn execute(
    preparation: &mut RootPreparation,
    helper: &File,
) -> Result<RetainedProgramBootstrapRootObservation, HelperFailure> {
    let host_identities = capture_host_identities()?;
    create_workspace()?;
    let busybox = open_private_regular(BUSYBOX)?;
    let alpine_archive = extract_rootfs(&busybox)?;
    preparation.advance(RootPreparationStep::ExtractAlpineMinirootfs)?;
    install_libgcc()?;
    install_authenticated_busybox(&busybox)?;
    install_retained_helper(helper)?;
    preparation.advance(RootPreparationStep::InstallOfflinePackages)?;
    assemble_toolchain(&busybox)?;
    extract_build_inputs(&busybox)?;
    preparation.advance(RootPreparationStep::AssembleRustToolchain)?;
    establish_mounts()?;
    write_cargo_configuration()?;
    preparation.advance(RootPreparationStep::EstablishReadOnlyMounts)?;
    pivot_and_detach()?;
    preparation.advance(RootPreparationStep::PivotRoot)?;
    preparation.advance(RootPreparationStep::DetachOldRoot)?;
    let observation = observe_root(host_identities, alpine_archive.link_plan_digest)?;
    preparation.advance(RootPreparationStep::PassCanaries)?;
    Ok(observation)
}

pub(super) fn remount_proc_for_namespace_init() -> Result<(), HelperFailure> {
    mount(
        "proc",
        "/proc",
        "proc",
        MountFlags::NOSUID | MountFlags::NODEV | MountFlags::NOEXEC,
        None,
    )
    .map_err(|_| HelperFailure::BootstrapRootVerification)
}

pub(super) fn validate_proc_mtab_after_mount() -> Result<(), HelperFailure> {
    validate_proc_mtab_at(Path::new("/"))
}

fn validate_proc_mtab_at(root: &Path) -> Result<(), HelperFailure> {
    let mtab = root.join("etc/mtab");
    let proc_mounts = root.join("proc/mounts");
    let link = fs::symlink_metadata(&mtab).map_err(|_| HelperFailure::BootstrapRootVerification)?;
    if !link.file_type().is_symlink()
        || fs::read_link(&mtab).map_err(|_| HelperFailure::BootstrapRootVerification)?
            != Path::new("../proc/mounts")
    {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    let resolved = fs::metadata(&mtab).map_err(|_| HelperFailure::BootstrapRootVerification)?;
    let proc_mounts =
        fs::metadata(proc_mounts).map_err(|_| HelperFailure::BootstrapRootVerification)?;
    if resolved.is_file()
        && proc_mounts.is_file()
        && resolved.dev() == proc_mounts.dev()
        && resolved.ino() == proc_mounts.ino()
    {
        Ok(())
    } else {
        Err(HelperFailure::BootstrapRootVerification)
    }
}

#[cfg(test)]
#[path = "linux_bootstrap_root_native/tests.rs"]
mod tests;

fn archive_limits() -> ArchiveLimits {
    ArchiveLimits {
        entries: usize::try_from(MAXIMUM_CONTROLLED_BUILD_WORKSPACE_INODES).unwrap_or(usize::MAX),
        file_bytes: MAXIMUM_ARCHIVE_FILE_BYTES,
        total_bytes: MAXIMUM_CONTROLLED_BUILD_WORKSPACE_BYTES,
        stream_bytes: MAXIMUM_CONTROLLED_BUILD_WORKSPACE_BYTES,
        links: 4_096,
        link_bytes: 4 * 1024 * 1024,
        link_depth: 32,
    }
}

fn create_workspace() -> Result<(), HelperFailure> {
    fs::create_dir(ROOTFS).map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    fs::create_dir(TOOLCHAIN_STAGING).map_err(|_| HelperFailure::BootstrapRootPreparation)
}

fn extract_rootfs(
    busybox: &File,
) -> Result<linux_bootstrap_archive::ArchiveSummary, HelperFailure> {
    let archive = open_private_regular(MINIROOTFS)?;
    let summary = linux_bootstrap_archive::extract(
        busybox,
        &archive,
        Compression::Gzip,
        Path::new(ROOTFS),
        None,
        LinkPolicy::AlpineMinirootfs,
        archive_limits(),
    )?;
    if !summary.deferred_proc_mtab {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    for relative in [
        "inputs",
        "toolchain",
        "source",
        "vendor",
        "raw-crates",
        "cargo-home",
        "target",
        "output",
        ".old-root",
    ] {
        create_required_directory(&Path::new(ROOTFS).join(relative))?;
    }
    Ok(summary)
}

fn install_libgcc() -> Result<(), HelperFailure> {
    let loader = Path::new(ROOTFS).join("lib/ld-musl-x86_64.so.1");
    let apk = Path::new(ROOTFS).join("sbin/apk");
    let package = Path::new(BUILD_INPUT_ROOT).join(LIBGCC_APK);
    for path in [&loader, &apk, &package] {
        if !path
            .metadata()
            .map_err(|_| HelperFailure::BootstrapRootVerification)?
            .is_file()
        {
            return Err(HelperFailure::BootstrapRootVerification);
        }
    }
    let status = Command::new(loader)
        .arg(apk)
        .args(["add", "--root", ROOTFS, "--no-cache", "--no-network"])
        .arg(package)
        .env_clear()
        .env("LD_LIBRARY_PATH", format!("{ROOTFS}/lib:{ROOTFS}/usr/lib"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    if !status.success() {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    install_linker_names(Path::new(ROOTFS))
}

fn install_authenticated_busybox(busybox: &File) -> Result<(), HelperFailure> {
    let destination = Path::new(ROOTFS).join("bin/retonr-busybox");
    copy_file(busybox, &destination, 0o555)
}

fn install_retained_helper(helper: &File) -> Result<(), HelperFailure> {
    let destination = Path::new(ROOTFS).join("bin/retonr-isolation-helper");
    copy_file(helper, &destination, 0o555)
}

fn assemble_toolchain(busybox: &File) -> Result<(), HelperFailure> {
    for (archive_path, staging, root, component) in [
        (
            CARGO,
            "cargo",
            "cargo-1.97.1-x86_64-unknown-linux-musl",
            "cargo",
        ),
        (
            RUSTC,
            "rustc",
            "rustc-1.97.1-x86_64-unknown-linux-musl",
            "rustc",
        ),
        (
            RUST_STD,
            "rust-std",
            "rust-std-1.97.1-x86_64-unknown-linux-musl",
            "rust-std-x86_64-unknown-linux-musl",
        ),
    ] {
        let destination = Path::new(TOOLCHAIN_STAGING).join(staging);
        fs::create_dir(&destination).map_err(|_| HelperFailure::BootstrapRootPreparation)?;
        linux_bootstrap_archive::extract(
            busybox,
            &open_private_regular(archive_path)?,
            Compression::Gzip,
            &destination,
            Some(root),
            LinkPolicy::Reject,
            archive_limits(),
        )?;
        merge_tree(
            &destination.join(component),
            &Path::new(ROOTFS).join("toolchain"),
        )?;
    }
    for required in [
        "toolchain/bin/cargo",
        "toolchain/bin/rustc",
        "toolchain/lib/rustlib/x86_64-unknown-linux-musl",
    ] {
        if !Path::new(ROOTFS)
            .join(required)
            .try_exists()
            .map_err(|_| HelperFailure::BootstrapRootVerification)?
        {
            return Err(HelperFailure::BootstrapRootVerification);
        }
    }
    Ok(())
}

fn extract_build_inputs(busybox: &File) -> Result<(), HelperFailure> {
    for (archive_path, destination, root) in [
        (SOURCE, "source", "retonr-source"),
        (VENDOR, "vendor", "cargo-vendor"),
        (RAW_CRATES, "raw-crates", "cargo-crates"),
    ] {
        linux_bootstrap_archive::extract(
            busybox,
            &open_private_regular(archive_path)?,
            Compression::None,
            &Path::new(ROOTFS).join(destination),
            Some(root),
            LinkPolicy::Reject,
            archive_limits(),
        )?;
    }
    Ok(())
}

fn write_cargo_configuration() -> Result<(), HelperFailure> {
    let cargo_home = Path::new(ROOTFS).join("cargo-home");
    let configuration = cargo_home.join("config.toml");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(configuration)
        .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    file.write_all(
        b"[source.crates-io]\nreplace-with = \"vendored-sources\"\n[source.vendored-sources]\ndirectory = \"/vendor\"\n",
    )
    .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    file.sync_data()
        .map_err(|_| HelperFailure::BootstrapRootPreparation)
}

fn establish_mounts() -> Result<(), HelperFailure> {
    bind_at(BUILD_INPUT_ROOT, &Path::new(ROOTFS).join("inputs"), true)?;
    bind_at(BUILD_OUTPUT_ROOT, &Path::new(ROOTFS).join("output"), false)?;
    for relative in ["toolchain", "source", "vendor", "raw-crates"] {
        bind_self_read_only(&Path::new(ROOTFS).join(relative))?;
    }
    for relative in ["cargo-home", "target"] {
        mount_writable_tmpfs(&Path::new(ROOTFS).join(relative))?;
    }
    let dev_null = Path::new(ROOTFS).join("dev/null");
    if !dev_null.exists() {
        File::create(&dev_null).map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    }
    mount_bind("/dev/null", &dev_null).map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    mount_bind(ROOTFS, ROOTFS).map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    mount_remount(
        ROOTFS,
        MountFlags::BIND | MountFlags::RDONLY | MountFlags::NODEV | MountFlags::NOSUID,
        "",
    )
    .map_err(|_| HelperFailure::BootstrapRootPreparation)
}

fn bind_at(source: &str, destination: &Path, read_only: bool) -> Result<(), HelperFailure> {
    mount_bind(source, destination).map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    if read_only {
        mount_remount(
            destination,
            MountFlags::BIND | MountFlags::RDONLY | MountFlags::NODEV | MountFlags::NOSUID,
            "",
        )
        .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    }
    Ok(())
}

fn bind_self_read_only(path: &Path) -> Result<(), HelperFailure> {
    mount_bind(path, path).map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    mount_remount(
        path,
        MountFlags::BIND | MountFlags::RDONLY | MountFlags::NODEV | MountFlags::NOSUID,
        "",
    )
    .map_err(|_| HelperFailure::BootstrapRootPreparation)
}

fn mount_writable_tmpfs(path: &Path) -> Result<(), HelperFailure> {
    let options = CString::new(format!(
        "size={MAXIMUM_CONTROLLED_BUILD_WORKSPACE_BYTES},nr_inodes={MAXIMUM_CONTROLLED_BUILD_WORKSPACE_INODES},mode=0700"
    ))
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

fn pivot_and_detach() -> Result<(), HelperFailure> {
    chdir(ROOTFS).map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    pivot_root(".", ".old-root").map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    chdir("/").map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    unmount("/.old-root", UnmountFlags::DETACH).map_err(|_| HelperFailure::BootstrapRootPreparation)
}

fn observe_root(
    host_identities: [Option<HostIdentity>; 3],
    normalized_alpine_link_plan_digest: rewrite_types::Digest,
) -> Result<RetainedProgramBootstrapRootObservation, HelperFailure> {
    let root = File::open("/").map_err(|_| HelperFailure::BootstrapRootVerification)?;
    let toolchain =
        File::open("/toolchain").map_err(|_| HelperFailure::BootstrapRootVerification)?;
    if !read_only(&root)? || !read_only(&toolchain)? || !host_paths_replaced(host_identities)? {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    if Path::new("/.old-root/proc/self").exists() {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    if fs::read_dir("/proc")
        .map_err(|_| HelperFailure::BootstrapRootVerification)?
        .next()
        .is_some()
    {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    let metadata = root
        .metadata()
        .map_err(|_| HelperFailure::BootstrapRootVerification)?;
    Ok(RetainedProgramBootstrapRootObservation {
        root_mount: NamespaceIdentity::new(metadata.dev(), metadata.ino()),
        alpine_installed_database_digest: hash_file("/lib/apk/db/installed")?,
        alpine_loader_digest: hash_file("/lib/ld-musl-x86_64.so.1")?,
        alpine_libc_linker_name_digest: validate_libc_linker_name(Path::new("/"))?,
        alpine_libgcc_digest: hash_file("/usr/lib/libgcc_s.so.1")?,
        alpine_libgcc_linker_name_digest: validate_libgcc_linker_name(Path::new("/"))?,
        alpine_busybox_digest: hash_file("/bin/retonr-busybox")?,
        rust_toolchain_layout_digest: hash_tree(Path::new("/toolchain"))?,
        normalized_alpine_link_plan_digest,
        postconditions: RetainedProgramBootstrapRootPostconditions::from_observations([
            true, true, true, true, false,
        ]),
    })
}

fn capture_host_identities() -> Result<[Option<HostIdentity>; 3], HelperFailure> {
    let mut identities = [None; 3];
    for (slot, path) in identities.iter_mut().zip(["/lib", "/lib64", "/usr"]) {
        *slot = match fs::metadata(path) {
            Ok(metadata) => Some(HostIdentity {
                device: metadata.dev(),
                inode: metadata.ino(),
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(_) => return Err(HelperFailure::BootstrapRootVerification),
        };
    }
    Ok(identities)
}

fn host_paths_replaced(identities: [Option<HostIdentity>; 3]) -> Result<bool, HelperFailure> {
    for (expected, path) in identities.into_iter().zip(["/lib", "/lib64", "/usr"]) {
        if let Some(expected) = expected {
            let current = match fs::metadata(path) {
                Ok(current) => current,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(_) => return Err(HelperFailure::BootstrapRootVerification),
            };
            if current.dev() == expected.device && current.ino() == expected.inode {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn read_only(file: &File) -> Result<bool, HelperFailure> {
    Ok(fstatvfs(file)
        .map_err(|_| HelperFailure::BootstrapRootVerification)?
        .f_flag
        .contains(StatVfsMountFlags::RDONLY))
}

fn create_required_directory(path: &Path) -> Result<(), HelperFailure> {
    if path.exists() {
        if path.is_dir() {
            Ok(())
        } else {
            Err(HelperFailure::BootstrapRootVerification)
        }
    } else {
        fs::create_dir(path).map_err(|_| HelperFailure::BootstrapRootPreparation)
    }
}
