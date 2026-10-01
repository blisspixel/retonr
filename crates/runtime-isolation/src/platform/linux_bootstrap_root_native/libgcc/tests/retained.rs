use std::{
    fs::{self, File},
    io::Write as _,
    os::unix::fs::PermissionsExt as _,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use rustix::mount::{MountFlags, UnmountFlags, mount, unmount};

use super::super::super::super::linux_bootstrap_archive::{self, Compression, LinkPolicy};
use super::super::super::{archive_limits, tree::hash_file};
use super::super::{HelperFailure, install};

#[test]
#[ignore = "requires privileged Linux and the exact retained Alpine, libgcc and BusyBox inputs"]
fn retained_signed_libgcc_installs_on_tmpfs_and_corrupt_signature_is_refused() {
    let inputs = PathBuf::from(
        std::env::var_os("RETONR_RETAINED_TOOLCHAIN_INPUTS").expect("retained input root"),
    );
    let temporary = tempfile::tempdir().expect("temporary root");
    let root = temporary.path().join("root");
    fs::create_dir(&root).expect("root directory");
    mount(
        "tmpfs",
        &root,
        "tmpfs",
        MountFlags::NODEV | MountFlags::NOSUID,
        Some(c"size=128m"),
    )
    .expect("actual temporary filesystem");
    let busybox_path = temporary.path().join("busybox");
    fs::copy(inputs.join("toolchains/busybox"), &busybox_path).expect("retained BusyBox");
    assert_eq!(
        hash_file(&busybox_path).expect("BusyBox digest").as_str(),
        "82bbbabec12a985ae58810cfe975c3399264dc888aa592d8e460732bdd30a8dd"
    );
    fs::set_permissions(&busybox_path, fs::Permissions::from_mode(0o555)).expect("BusyBox mode");
    let miniroot = inputs.join("lineage/host/alpine/alpine-minirootfs-3.23.3-x86_64.tar.gz");
    let package = inputs.join("lineage/host/alpine/libgcc-15.2.0-r2.apk");
    assert_eq!(
        hash_file(&miniroot).expect("miniroot digest").as_str(),
        "42d0e6d8de5521e7bf92e075e032b5690c1d948fa9775efa32a51a38b25460fb"
    );
    assert_eq!(
        hash_file(&package).expect("package digest").as_str(),
        "4279d14cbf43311312d3a7d43619f4c15ede2e46d5608ba421ed157ded57c700"
    );
    linux_bootstrap_archive::extract(
        &File::open(&busybox_path).expect("BusyBox file"),
        &File::open(miniroot).expect("miniroot file"),
        Compression::Gzip,
        &root,
        None,
        LinkPolicy::AlpineMinirootfs,
        archive_limits(),
    )
    .expect("production root extraction");
    let baseline = Command::new(root.join("lib/ld-musl-x86_64.so.1"))
        .arg(root.join("sbin/apk"))
        .args(["add", "--root"])
        .arg(&root)
        .args(["--no-cache", "--no-network"])
        .arg(&package)
        .env_clear()
        .env(
            "LD_LIBRARY_PATH",
            format!("{}/lib:{}/usr/lib", root.display(), root.display()),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("baseline APK");
    assert_eq!(
        baseline.code(),
        Some(99),
        "the original install refuses temporary-root persistence"
    );
    assert_eq!(install(&root, &package), Ok(()));
    assert!(root.join("usr/lib/libgcc_s.so.1").is_file());
    let corrupt = corrupt_signature(&busybox_path, &package, temporary.path());
    assert_eq!(
        install(&root, &corrupt),
        Err(HelperFailure::BootstrapRootVerification)
    );
    unmount(&root, UnmountFlags::empty()).expect("unmount temporary root");
}

fn transform(busybox: &Path, arguments: &[&str], input: &[u8]) -> Vec<u8> {
    let mut child = Command::new(busybox)
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("retained gzip");
    child
        .stdin
        .take()
        .expect("gzip stdin")
        .write_all(input)
        .expect("gzip input");
    let output = child.wait_with_output().expect("gzip completion");
    assert!(output.status.success());
    output.stdout
}

fn corrupt_signature(busybox: &Path, package: &Path, directory: &Path) -> PathBuf {
    let bytes = fs::read(package).expect("package bytes");
    let next_member = bytes[3..]
        .windows(3)
        .position(|bytes| bytes == [0x1f, 0x8b, 8])
        .expect("second gzip member")
        + 3;
    let mut signature = transform(busybox, &["gzip", "-dc"], &bytes[..next_member]);
    let mut archive = tar::Archive::new(signature.as_slice());
    let mut entries = archive.entries().expect("signature archive");
    let entry = entries
        .next()
        .expect("signature entry")
        .expect("signature tar entry");
    assert!(
        entry
            .path()
            .expect("signature path")
            .to_str()
            .expect("signature name")
            .starts_with(".SIGN.RSA")
    );
    let payload = usize::try_from(entry.raw_file_position()).expect("signature payload offset");
    assert!(entry.size() > 0);
    assert!(entries.next().is_none());
    signature[payload] ^= 1;
    let mut corrupt = transform(busybox, &["gzip", "-c"], &signature);
    corrupt.extend_from_slice(&bytes[next_member..]);
    let target = directory.join("corrupt-signature.apk");
    fs::write(&target, corrupt).expect("corrupted signature package");
    target
}
