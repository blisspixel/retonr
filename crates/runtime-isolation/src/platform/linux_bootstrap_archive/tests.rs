use super::*;

fn limits() -> ArchiveLimits {
    ArchiveLimits {
        entries: 8,
        file_bytes: 16,
        total_bytes: 32,
        stream_bytes: 8 * 1024,
        links: 8,
        link_bytes: 1_024,
        link_depth: 8,
    }
}

fn archive(entries: &[(&str, tar::EntryType, &[u8])]) -> File {
    let file = tempfile::tempfile().expect("archive");
    let mut builder = tar::Builder::new(file);
    for (path, kind, bytes) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(*kind);
        header.set_mode(if kind.is_dir() { 0o755 } else { 0o644 });
        header.set_size(u64::try_from(bytes.len()).expect("size"));
        let path_bytes = path.as_bytes();
        assert!(path_bytes.len() < 100);
        header.as_mut_bytes()[..100].fill(0);
        header.as_mut_bytes()[..path_bytes.len()].copy_from_slice(path_bytes);
        header.set_cksum();
        builder.append(&header, *bytes).expect("append");
    }
    builder.into_inner().expect("finish")
}

fn linked_archive(entries: &[(&str, tar::EntryType, Option<&str>, &[u8])]) -> File {
    let file = tempfile::tempfile().expect("archive");
    let mut builder = tar::Builder::new(file);
    for (path, kind, target, bytes) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(*kind);
        header.set_mode(if kind.is_dir() { 0o755 } else { 0o644 });
        header.set_size(u64::try_from(bytes.len()).expect("size"));
        let path_bytes = path.as_bytes();
        header.as_mut_bytes()[..100].fill(0);
        header.as_mut_bytes()[..path_bytes.len()].copy_from_slice(path_bytes);
        if let Some(target) = target {
            header.set_link_name(target).expect("link target");
        }
        header.set_cksum();
        builder.append(&header, *bytes).expect("append");
    }
    builder.into_inner().expect("finish")
}

#[test]
fn canonical_archive_plan_is_bounded_and_root_stripped() {
    let mut file = archive(&[
        ("root/", tar::EntryType::Directory, b""),
        ("root/dir", tar::EntryType::Directory, b""),
        ("root/dir/file", tar::EntryType::Regular, b"value"),
    ]);
    let plan = plan::validate(&mut file, Some("root"), LinkPolicy::Reject, limits()).expect("plan");
    assert_eq!(plan.entries.len(), 3);
    assert_eq!(plan.entries[2].relative, Path::new("dir/file"));
    assert_eq!(plan.entries[2].bytes, 5);
}

#[test]
fn archive_rejects_links_absolute_parent_duplicate_and_missing_parent() {
    for (case, mut file) in [
        (
            "symbolic link",
            archive(&[("root/link", tar::EntryType::Symlink, b"")]),
        ),
        (
            "device",
            archive(&[("root/device", tar::EntryType::Char, b"")]),
        ),
        ("fifo", archive(&[("root/fifo", tar::EntryType::Fifo, b"")])),
        (
            "absolute",
            archive(&[("/absolute", tar::EntryType::Regular, b"x")]),
        ),
        (
            "parent",
            archive(&[("root/../escape", tar::EntryType::Regular, b"x")]),
        ),
        (
            "current directory",
            archive(&[("root/./file", tar::EntryType::Regular, b"x")]),
        ),
        (
            "empty component",
            archive(&[("root//file", tar::EntryType::Regular, b"x")]),
        ),
        (
            "backslash",
            archive(&[("root\\file", tar::EntryType::Regular, b"x")]),
        ),
        (
            "regular trailing slash",
            archive(&[("root/file/", tar::EntryType::Regular, b"x")]),
        ),
        (
            "duplicate",
            archive(&[
                ("root/file", tar::EntryType::Regular, b"x"),
                ("root/file", tar::EntryType::Regular, b"x"),
            ]),
        ),
        (
            "missing parent",
            archive(&[("root/missing/file", tar::EntryType::Regular, b"x")]),
        ),
    ] {
        assert!(
            plan::validate(&mut file, Some("root"), LinkPolicy::Reject, limits()).is_err(),
            "{case}"
        );
    }
}

#[test]
fn archive_rejects_quota_and_nonzero_trailer() {
    let mut oversized = archive(&[("root/file", tar::EntryType::Regular, b"too many bytes here")]);
    assert!(plan::validate(&mut oversized, Some("root"), LinkPolicy::Reject, limits()).is_err());
    let mut trailing = archive(&[("root/file", tar::EntryType::Regular, b"x")]);
    trailing.seek(SeekFrom::End(0)).expect("seek");
    trailing.write_all(b"trailing").expect("trailer");
    assert!(plan::validate(&mut trailing, Some("root"), LinkPolicy::Reject, limits()).is_err());
}

#[test]
fn archive_allows_directory_sticky_bits_but_rejects_privileged_file_modes() {
    let file = tempfile::tempfile().expect("archive");
    let mut builder = tar::Builder::new(file);
    for (path, kind, mode, bytes) in [
        ("./", tar::EntryType::Directory, 0o755, b"".as_slice()),
        ("./tmp/", tar::EntryType::Directory, 0o1777, b"".as_slice()),
        (
            "./privileged",
            tar::EntryType::Regular,
            0o4755,
            b"x".as_slice(),
        ),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(kind);
        header.set_mode(mode);
        header.set_size(u64::try_from(bytes.len()).expect("size"));
        header.set_path(path).expect("path");
        header.set_cksum();
        builder.append(&header, bytes).expect("append");
    }
    let mut file = builder.into_inner().expect("finish");
    assert!(plan::validate(&mut file, None, LinkPolicy::AlpineMinirootfs, limits(),).is_err());
}

#[test]
fn official_alpine_link_shape_is_normalized_and_deferred_mtab_is_exact() {
    let mut file = linked_archive(&[
        ("./", tar::EntryType::Directory, None, b""),
        ("./bin/", tar::EntryType::Directory, None, b""),
        ("./etc/", tar::EntryType::Directory, None, b""),
        ("./etc/ssl/", tar::EntryType::Directory, None, b""),
        ("./etc/ssl/certs/", tar::EntryType::Directory, None, b""),
        ("./etc/ssl1.1/", tar::EntryType::Directory, None, b""),
        ("./proc/", tar::EntryType::Directory, None, b""),
        ("./usr/", tar::EntryType::Directory, None, b""),
        ("./usr/bin/", tar::EntryType::Directory, None, b""),
        (
            "./usr/bin/yes",
            tar::EntryType::Symlink,
            Some("/bin/busybox"),
            b"",
        ),
        (
            "./etc/ssl/cert.pem",
            tar::EntryType::Symlink,
            Some("certs/ca-certificates.crt"),
            b"",
        ),
        (
            "./etc/ssl1.1/cert.pem",
            tar::EntryType::Symlink,
            Some("/etc/ssl/cert.pem"),
            b"",
        ),
        (
            "./etc/mtab",
            tar::EntryType::Symlink,
            Some("../proc/mounts"),
            b"",
        ),
        ("./bin/busybox", tar::EntryType::Regular, None, b"busybox"),
        (
            "./etc/ssl/certs/ca-certificates.crt",
            tar::EntryType::Regular,
            None,
            b"certificate",
        ),
    ]);
    let mut official_limits = limits();
    official_limits.entries = 32;
    let plan = plan::validate(
        &mut file,
        None,
        LinkPolicy::AlpineMinirootfs,
        official_limits,
    )
    .expect("official Alpine shape");
    assert!(plan.deferred_proc_mtab);
    assert_eq!(
        plan.entries[9]
            .link
            .as_ref()
            .expect("absolute link")
            .stored_target,
        Path::new("../../bin/busybox")
    );
}

#[test]
fn symlink_cycles_escape_dangling_and_hardlink_reordering_fail_closed() {
    for mut file in [
        linked_archive(&[
            ("./", tar::EntryType::Directory, None, b""),
            ("./a", tar::EntryType::Symlink, Some("b"), b""),
            ("./b", tar::EntryType::Symlink, Some("a"), b""),
        ]),
        linked_archive(&[
            ("./", tar::EntryType::Directory, None, b""),
            ("./a", tar::EntryType::Symlink, Some("../escape"), b""),
        ]),
        linked_archive(&[
            ("./", tar::EntryType::Directory, None, b""),
            ("./a", tar::EntryType::Symlink, Some("missing"), b""),
        ]),
        linked_archive(&[
            ("./", tar::EntryType::Directory, None, b""),
            ("./copy", tar::EntryType::Link, Some("./target"), b""),
            ("./target", tar::EntryType::Regular, None, b"value"),
        ]),
    ] {
        assert!(plan::validate(&mut file, None, LinkPolicy::AlpineMinirootfs, limits()).is_err());
    }
}

#[test]
fn link_count_target_bytes_and_resolution_depth_are_bounded() {
    let chained = || {
        linked_archive(&[
            ("./", tar::EntryType::Directory, None, b""),
            ("./a", tar::EntryType::Symlink, Some("b"), b""),
            ("./b", tar::EntryType::Symlink, Some("target"), b""),
            ("./target", tar::EntryType::Regular, None, b"value"),
        ])
    };
    let mut no_links = limits();
    no_links.links = 0;
    assert!(plan::validate(&mut chained(), None, LinkPolicy::AlpineMinirootfs, no_links,).is_err());
    let mut short_targets = limits();
    short_targets.link_bytes = 1;
    assert!(
        plan::validate(
            &mut chained(),
            None,
            LinkPolicy::AlpineMinirootfs,
            short_targets,
        )
        .is_err()
    );
    let mut shallow = limits();
    shallow.link_depth = 0;
    assert!(plan::validate(&mut chained(), None, LinkPolicy::AlpineMinirootfs, shallow,).is_err());
}

#[test]
fn validated_hardlinks_are_materialized_as_independent_regular_bytes() {
    use std::os::unix::fs::MetadataExt as _;

    let busybox = File::open("/bin/busybox").expect("test host BusyBox");
    let destination = tempfile::tempdir().expect("destination");
    let summary = extract(
        &busybox,
        &linked_archive(&[
            ("./", tar::EntryType::Directory, None, b""),
            ("./target", tar::EntryType::Regular, None, b"value"),
            ("./copy", tar::EntryType::Link, Some("./target"), b""),
        ]),
        Compression::None,
        destination.path(),
        None,
        LinkPolicy::AlpineMinirootfs,
        limits(),
    )
    .expect("hardlink materialization");
    assert_eq!(summary.regular_bytes, 10);
    assert_eq!(
        fs::read(destination.path().join("copy")).expect("copy"),
        b"value"
    );
    assert_ne!(
        fs::metadata(destination.path().join("target"))
            .expect("target")
            .ino(),
        fs::metadata(destination.path().join("copy"))
            .expect("copy")
            .ino()
    );
}

#[test]
fn exact_official_alpine_archive_fixture() {
    let Some(path) = std::env::var_os("RETONR_TEST_ALPINE_MINIROOTFS") else {
        return;
    };
    let busybox = File::open("/bin/busybox").expect("test host BusyBox");
    let archive = File::open(path).expect("official Alpine archive");
    let destination = tempfile::tempdir().expect("extraction destination");
    let exact_limits = ArchiveLimits {
        entries: 517,
        file_bytes: 16 * 1024 * 1024,
        total_bytes: 128 * 1024 * 1024,
        stream_bytes: 128 * 1024 * 1024,
        links: 335,
        link_bytes: 64 * 1024,
        link_depth: 8,
    };
    let mut decompressed = tempfile::tempfile().expect("decompressed archive");
    decompress(
        &busybox,
        &archive,
        Compression::Gzip,
        &mut decompressed,
        exact_limits,
    )
    .expect("official Alpine decompression");
    let plan = plan::validate(
        &mut decompressed,
        None,
        LinkPolicy::AlpineMinirootfs,
        exact_limits,
    )
    .expect("official Alpine plan");
    extract_plan(
        &mut decompressed,
        destination.path(),
        None,
        LinkPolicy::AlpineMinirootfs,
        exact_limits,
        &plan,
    )
    .expect("official Alpine plan extraction");
    let public_destination = tempfile::tempdir().expect("public extraction destination");
    let summary = extract(
        &busybox,
        &archive,
        Compression::Gzip,
        public_destination.path(),
        None,
        LinkPolicy::AlpineMinirootfs,
        exact_limits,
    )
    .expect("official Alpine extraction");
    assert_eq!(summary.entries, 517);
    assert!(summary.deferred_proc_mtab);
}

#[test]
fn exact_official_rust_gzip_archives_reject_links_and_extract_with_retained_busybox() {
    let busybox = File::open("/bin/busybox").expect("test host BusyBox");
    for (variable, root, required, expected_bytes) in [
        (
            "RETONR_TEST_RUST_CARGO_DIST",
            "cargo-1.97.1-x86_64-unknown-linux-musl",
            "cargo/bin/cargo",
            17_472_040,
        ),
        (
            "RETONR_TEST_RUSTC_DIST",
            "rustc-1.97.1-x86_64-unknown-linux-musl",
            "rustc/bin/rustc",
            172_143_184,
        ),
        (
            "RETONR_TEST_RUST_STD_DIST",
            "rust-std-1.97.1-x86_64-unknown-linux-musl",
            "rust-std-x86_64-unknown-linux-musl/lib/rustlib/x86_64-unknown-linux-musl",
            67_584_421,
        ),
    ] {
        let Some(path) = std::env::var_os(variable) else {
            continue;
        };
        let archive = File::open(&path).expect("official Rust distribution");
        assert_eq!(
            archive.metadata().expect("distribution metadata").len(),
            expected_bytes,
            "{variable}"
        );
        let destination = tempfile::tempdir().expect("Rust distribution destination");
        let exact_limits = ArchiveLimits {
            entries: 256,
            file_bytes: 2 * 1024 * 1024 * 1024,
            total_bytes: 2 * 1024 * 1024 * 1024,
            stream_bytes: 2 * 1024 * 1024 * 1024,
            links: 0,
            link_bytes: 0,
            link_depth: 0,
        };
        let summary = extract(
            &busybox,
            &archive,
            Compression::Gzip,
            destination.path(),
            Some(root),
            LinkPolicy::Reject,
            exact_limits,
        )
        .unwrap_or_else(|error| panic!("{variable}: {error:?}"));
        assert!(!summary.deferred_proc_mtab);
        assert!(destination.path().join(required).exists(), "{variable}");
    }
}

#[test]
fn retained_executable_drives_bounded_uncompressed_extraction() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let busybox = File::open("/bin/busybox").expect("test host BusyBox");
    let destination = temporary.path().join("output");
    fs::create_dir(&destination).expect("destination");
    let summary = extract(
        &busybox,
        &archive(&[
            ("root/", tar::EntryType::Directory, b""),
            ("root/file", tar::EntryType::Regular, b"value"),
        ]),
        Compression::None,
        &destination,
        Some("root"),
        LinkPolicy::Reject,
        limits(),
    )
    .expect("extract");
    assert_eq!(summary.entries, 2);
    assert_eq!(summary.regular_bytes, 5);
    assert_eq!(
        fs::read(destination.join("file")).expect("output"),
        b"value"
    );
}

#[test]
fn retained_executable_failure_and_stream_quota_fail_closed() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let failed_path = temporary.path().join("non-executable");
    fs::write(&failed_path, b"not an executable").expect("failing retained object");
    let failed = File::open(failed_path).expect("failing retained object");
    let destination = temporary.path().join("failed-output");
    fs::create_dir(&destination).expect("destination");
    assert!(
        extract(
            &failed,
            &archive(&[("root/file", tar::EntryType::Regular, b"value")]),
            Compression::None,
            &destination,
            Some("root"),
            LinkPolicy::Reject,
            limits(),
        )
        .is_err()
    );
    let overflow = File::open("/bin/busybox").expect("test host BusyBox");
    let bounded = temporary.path().join("bounded-output");
    fs::create_dir(&bounded).expect("bounded destination");
    let mut strict = limits();
    strict.stream_bytes = 32;
    assert!(
        extract(
            &overflow,
            &archive(&[("root/file", tar::EntryType::Regular, b"value")]),
            Compression::None,
            &bounded,
            Some("root"),
            LinkPolicy::Reject,
            strict,
        )
        .is_err()
    );
}
