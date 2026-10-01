use std::{fs, os::unix::fs::PermissionsExt as _, path::PathBuf};

use rustix::{
    fs::{FileType, Mode, mknodat},
    mount::{MountFlags, UnmountFlags, mount_bind, mount_remount, unmount},
    process::{Resource, getrlimit},
};

use super::super::*;

struct ReadOnlyMount {
    target: PathBuf,
}

impl ReadOnlyMount {
    fn bind(source: &Path, target: &Path) -> Self {
        mount_bind(source, target).expect("private fixture bind mount");
        mount_remount(target, MountFlags::BIND | MountFlags::RDONLY, "")
            .expect("private fixture read-only remount");
        Self {
            target: target.to_path_buf(),
        }
    }
}

impl Drop for ReadOnlyMount {
    fn drop(&mut self) {
        let _ = unmount(&self.target, UnmountFlags::DETACH);
    }
}

#[test]
#[ignore = "requires the retained canonical source/vendor archives and exactly4096 descriptors"]
fn retained_archives_are_byte_identical_under_the_actual_4096_descriptor_limit() {
    assert_eq!(getrlimit(Resource::Nofile).current, Some(4096));
    let fixture = PathBuf::from(
        std::env::var_os("RETONR_NATIVE_ARCHIVE_ROOT").expect("retained read-only fixture root"),
    );
    let output = tempfile::tempdir().expect("writable output");
    for (kind, source, expected) in [
        ("retonr-source", "source", "retonr-source.tar"),
        ("cargo-vendor", "vendor", "cargo-vendor.tar"),
        ("cargo-crates", "raw-crates", "cargo-crates.tar"),
    ] {
        let root = File::open(fixture.join(source)).expect("retained source descriptor");
        let destination = output.path().join(expected);
        serialize_root(&root, kind, &destination).expect("bounded read-only serialization");
        let mut regenerated = File::open(destination).expect("regenerated archive");
        let mut original = File::open(fixture.join("prepared-archives").join(expected))
            .expect("public-normalizer reference archive");
        assert_eq!(
            snapshot::hash_file(&mut regenerated).expect("regenerated digest"),
            snapshot::hash_file(&mut original).expect("reference digest")
        );
    }
}

#[test]
#[ignore = "requires a privileged isolated Linux mount namespace"]
fn kernel_read_only_streaming_refuses_aliases_special_files_nested_mounts_and_drift() {
    assert_eq!(getrlimit(Resource::Nofile).current, Some(4096));
    let temporary = tempfile::tempdir().expect("fixture root");
    for name in ["source", "sealed", "external"] {
        fs::create_dir(temporary.path().join(name)).expect("fixture directory");
    }
    let source = temporary.path().join("source");
    let sealed = temporary.path().join("sealed");
    fs::write(source.join("file"), b"initial").expect("payload");
    fs::create_dir(source.join("nested")).expect("nested mount point");
    fs::create_dir(source.join("many")).expect("large tree directory");
    for index in 0..8192 {
        fs::write(source.join("many").join(format!("{index:05}")), b"retained")
            .expect("large bounded tree payload");
    }
    let protected = ReadOnlyMount::bind(&source, &sealed);
    let root = File::open(&sealed).expect("sealed root");
    let archive_path = temporary.path().join("bounded.tar");
    serialize_root(&root, "retonr-source", &archive_path)
        .expect("8193 files under4096 descriptors");
    let mut archive = tar::Archive::new(File::open(archive_path).expect("serialized archive"));
    let mut files = 0_usize;
    for entry in archive.entries().expect("archive entries") {
        let mut entry = entry.expect("archive entry");
        if entry.header().entry_type().is_file() {
            let mut contents = Vec::new();
            std::io::Read::read_to_end(&mut entry, &mut contents).expect("retained payload");
            assert!(contents == b"initial" || contents == b"retained");
            assert_eq!(entry.header().mode().expect("normalized mode"), 0o644);
            files += 1;
        }
    }
    assert_eq!(files, 8193);
    let original = Snapshot::acquire(&root).expect("sealed snapshot");
    fs::write(source.join("file"), b"changed").expect("test-only writable alias mutation");
    let mut output = File::create(temporary.path().join("drift.tar")).expect("output");
    assert!(serialize_into(&root, "retonr-source", &original, &mut output).is_err());
    let nested_target = sealed.join("nested");
    mount_bind(temporary.path().join("external"), &nested_target).expect("same-device nested bind");
    assert!(Snapshot::acquire(&root).is_err());
    unmount(&nested_target, UnmountFlags::DETACH).expect("nested fixture unmount");
    drop(root);
    drop(protected);
    std::os::unix::fs::symlink("file", source.join("alias")).expect("symlink");
    let protected = ReadOnlyMount::bind(&source, &sealed);
    assert!(Snapshot::acquire(&File::open(&sealed).expect("root")).is_err());
    drop(protected);
    fs::remove_file(source.join("alias")).expect("remove symlink");
    mknodat(
        File::open(&source).expect("source root"),
        "fifo",
        FileType::Fifo,
        Mode::RUSR | Mode::WUSR,
        0,
    )
    .expect("FIFO fixture");
    let protected = ReadOnlyMount::bind(&source, &sealed);
    assert!(Snapshot::acquire(&File::open(&sealed).expect("root")).is_err());
    drop(protected);
    fs::remove_file(source.join("fifo")).expect("remove FIFO");
    fs::hard_link(source.join("file"), source.join("hard-alias")).expect("hard link");
    let protected = ReadOnlyMount::bind(&source, &sealed);
    assert!(Snapshot::acquire(&File::open(&sealed).expect("root")).is_err());
    drop(protected);
}

#[test]
#[ignore = "requires privileged mount isolation and the retained public archive normalizer"]
fn long_paths_and_permissions_match_the_public_mutable_normalizer_exactly() {
    let temporary = tempfile::tempdir().expect("fixture root");
    let source = temporary.path().join("source");
    let sealed = temporary.path().join("sealed");
    fs::create_dir(&source).expect("source");
    fs::create_dir(&sealed).expect("sealed");
    let path = source.join("long-directory-name".repeat(7)).join("payload");
    fs::create_dir(path.parent().expect("parent")).expect("long directory");
    fs::write(&path, b"retained payload\n").expect("payload");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("source executable mode");
    let normalizer = std::env::var_os("RETONR_PUBLIC_ARCHIVE_TOOL").expect("retained normalizer");
    let expected = temporary.path().join("public.tar");
    assert!(
        std::process::Command::new(normalizer)
            .args(["retonr-source"])
            .arg(&source)
            .arg(&expected)
            .status()
            .expect("public normalizer")
            .success()
    );
    let protected = ReadOnlyMount::bind(&source, &sealed);
    let destination = temporary.path().join("private.tar");
    serialize_root(
        &File::open(&sealed).expect("sealed root"),
        "retonr-source",
        &destination,
    )
    .expect("private normalization");
    assert_eq!(
        fs::read(expected).expect("reference bytes"),
        fs::read(destination).expect("private bytes")
    );
    drop(protected);
}
