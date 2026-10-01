use std::{
    fs::{self, File, OpenOptions},
    io::{Read as _, Write as _},
    os::unix::fs::{FileTypeExt as _, MetadataExt as _},
    path::Path,
};

use rustix::{
    fs::{StatVfsMountFlags, fstatvfs},
    mount::{MountFlags, UnmountFlags, mount, mount_bind, mount_remount, unmount},
};

use super::super::write_cargo_configuration_at;
use super::establish;

#[test]
#[ignore = "requires privileged Linux mount operations in a private test container"]
fn root_is_sealed_after_child_mounts_preserving_read_only_inputs_and_writable_outputs() {
    let temporary = tempfile::tempdir().expect("temporary mount root");
    reproduce_hidden_child_mount(temporary.path());
    let root = temporary.path().join("corrected-root");
    let input = temporary.path().join("input");
    let output = temporary.path().join("output");
    fs::create_dir(&root).expect("root directory");
    fs::create_dir(&input).expect("input directory");
    fs::create_dir(&output).expect("output directory");
    let held_output = File::open(&output).expect("held output identity");
    fs::write(input.join("retained.txt"), b"retained").expect("input payload");
    for relative in [
        "inputs",
        "output",
        "toolchain",
        "source",
        "vendor",
        "raw-crates",
        "cargo-home",
        "target",
        "dev",
    ] {
        fs::create_dir(root.join(relative)).expect("root member directory");
    }
    for relative in ["toolchain", "source", "vendor", "raw-crates"] {
        fs::write(root.join(relative).join("retained.txt"), b"retained")
            .expect("root member payload");
    }
    establish(&root, &input, &output).expect("production mount construction");
    assert_read_only(&root);
    for relative in ["inputs", "toolchain", "source", "vendor", "raw-crates"] {
        let directory = root.join(relative);
        assert_read_only(&directory);
        assert_eq!(
            fs::read(directory.join("retained.txt")).expect("retained mounted payload"),
            b"retained"
        );
        assert_eq!(
            fs::write(directory.join("new.txt"), b"refused")
                .expect_err("read only child write")
                .raw_os_error(),
            Some(libc::EROFS)
        );
    }
    assert_eq!(
        fs::write(root.join("new.txt"), b"refused")
            .expect_err("read only root write")
            .raw_os_error(),
        Some(libc::EROFS)
    );
    let held = held_output.metadata().expect("held output metadata");
    let mounted = fs::metadata(root.join("output")).expect("mounted output metadata");
    assert_eq!((held.dev(), held.ino()), (mounted.dev(), mounted.ino()));
    fs::write(root.join("output/result.txt"), b"output").expect("exact writable output alias");
    assert_eq!(
        fs::read(output.join("result.txt")).expect("held output bytes"),
        b"output"
    );
    for relative in ["cargo-home", "target"] {
        let directory = root.join(relative);
        let descriptor = File::open(&directory).expect("writable tmpfs");
        let flags = fstatvfs(&descriptor).expect("tmpfs flags").f_flag;
        assert!(!flags.contains(StatVfsMountFlags::RDONLY));
        assert!(flags.contains(StatVfsMountFlags::NOSUID | StatVfsMountFlags::NODEV));
        assert_eq!(
            fs::metadata(&directory).expect("tmpfs metadata").mode() & 0o777,
            0o700
        );
        fs::write(directory.join("scratch"), b"private").expect("writable tmpfs visible");
    }
    write_cargo_configuration_at(&root).expect("production Cargo configuration write");
    assert_eq!(fs::read(root.join("cargo-home/config.toml")).expect("Cargo configuration"), b"[source.crates-io]\nreplace-with = \"vendored-sources\"\n[source.vendored-sources]\ndirectory = \"/vendor\"\n");
    let mut null = OpenOptions::new()
        .read(true)
        .write(true)
        .open(root.join("dev/null"))
        .expect("exact null alias");
    assert!(
        null.metadata()
            .expect("null metadata")
            .file_type()
            .is_char_device()
    );
    null.write_all(b"discard").expect("null write");
    assert_eq!(null.read(&mut [0; 1]).expect("null read"), 0);
    drop(null);
    for relative in [
        "dev/null",
        "target",
        "cargo-home",
        "raw-crates",
        "vendor",
        "source",
        "toolchain",
        "output",
        "inputs",
    ] {
        unmount(root.join(relative), UnmountFlags::empty()).expect("unmount child");
    }
    unmount(&root, UnmountFlags::empty()).expect("unmount root");
}

fn assert_read_only(path: &Path) {
    let file = File::open(path).expect("mounted directory");
    assert!(
        fstatvfs(&file)
            .expect("read only mount flags")
            .f_flag
            .contains(
                StatVfsMountFlags::RDONLY | StatVfsMountFlags::NOSUID | StatVfsMountFlags::NODEV
            )
    );
}

fn reproduce_hidden_child_mount(parent: &Path) {
    let root = parent.join("old-order");
    let child = root.join("cargo-home");
    fs::create_dir_all(&child).expect("old child directory");
    mount(
        "tmpfs",
        &child,
        "tmpfs",
        MountFlags::NODEV | MountFlags::NOSUID,
        Some(c"size=4m"),
    )
    .expect("old child tmpfs");
    fs::write(child.join("marker"), b"visible before root bind").expect("old child marker");
    mount_bind(&root, &root).expect("old late root bind");
    mount_remount(
        &root,
        MountFlags::BIND | MountFlags::RDONLY | MountFlags::NODEV | MountFlags::NOSUID,
        "",
    )
    .expect("old root read only");
    assert!(
        !child.join("marker").exists(),
        "late nonrecursive self-bind hides child mounts"
    );
    assert_eq!(
        fs::write(child.join("config.toml"), b"config")
            .expect_err("hidden writable child causes EROFS")
            .raw_os_error(),
        Some(libc::EROFS)
    );
    unmount(&root, UnmountFlags::empty()).expect("remove late root bind");
    assert!(
        child.join("marker").is_file(),
        "earlier child mount remains below the removed bind"
    );
    unmount(&child, UnmountFlags::empty()).expect("remove old child mount");
}
