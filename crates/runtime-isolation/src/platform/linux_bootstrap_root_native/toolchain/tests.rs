use super::*;

fn component(root: &Path, name: &str, files: &[(&str, &[u8])]) -> PathBuf {
    let source = root.join(name);
    fs::create_dir(&source).expect("component directory");
    let mut manifest = String::new();
    for (relative, bytes) in files {
        let target = source.join(relative);
        fs::create_dir_all(target.parent().expect("parent")).expect("parents");
        fs::write(target, bytes).expect("payload");
        manifest.push_str("file:");
        manifest.push_str(relative);
        manifest.push('\n');
    }
    fs::write(source.join("manifest.in"), manifest).expect("installer manifest");
    source
}

#[test]
fn three_distribution_components_install_declared_payload_without_manifest_collision() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let target = temporary.path().join("toolchain");
    fs::create_dir(&target).expect("toolchain");
    for (name, path, bytes) in [
        ("cargo", "bin/cargo", b"cargo".as_slice()),
        ("rustc", "bin/rustc", b"rustc".as_slice()),
        (
            "rust-std",
            "lib/rustlib/host/lib/libstd.rlib",
            b"standard library".as_slice(),
        ),
    ] {
        let source = component(temporary.path(), name, &[(path, bytes)]);
        install_component(&source, &target).expect("component installed");
        assert_eq!(fs::read(target.join(path)).expect("installed bytes"), bytes);
        assert!(source.join("manifest.in").is_file());
    }
    assert!(!target.join("manifest.in").exists());
}

#[test]
fn rejects_malformed_declarations_and_incomplete_payload_before_copying() {
    for manifest in [
        "",
        "file:bin/cargo",
        "file:../escape\n",
        "file:/escape\n",
        "file:bin//cargo\n",
        "file:bin/./cargo\n",
        "file:bin\\cargo\n",
        "dir:bin\n",
        "file:manifest.in\n",
        "file:bin/cargo\nfile:bin/cargo\n",
        "file:missing\n",
    ] {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let source = component(temporary.path(), "cargo", &[("bin/cargo", b"cargo")]);
        fs::write(source.join("manifest.in"), manifest).expect("mutated manifest");
        let target = temporary.path().join("toolchain");
        fs::create_dir(&target).expect("toolchain");
        assert_eq!(
            install_component(&source, &target),
            Err(HelperFailure::BootstrapRootVerification)
        );
        assert_eq!(fs::read_dir(target).expect("target entries").count(), 0);
    }
}

#[test]
fn rejects_undeclared_files_links_and_shared_destination_collisions() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let source = component(temporary.path(), "cargo", &[("bin/cargo", b"cargo")]);
    let target = temporary.path().join("toolchain");
    fs::create_dir(&target).expect("toolchain");
    fs::write(source.join("extra"), b"undeclared").expect("extra");
    assert_eq!(
        install_component(&source, &target),
        Err(HelperFailure::BootstrapRootVerification)
    );
    fs::remove_file(source.join("extra")).expect("remove extra");
    std::os::unix::fs::symlink("bin/cargo", source.join("link")).expect("link");
    assert_eq!(
        install_component(&source, &target),
        Err(HelperFailure::BootstrapRootVerification)
    );
    fs::remove_file(source.join("link")).expect("remove link");
    fs::hard_link(source.join("bin/cargo"), source.join("hard-link")).expect("hard link");
    assert_eq!(
        install_component(&source, &target),
        Err(HelperFailure::BootstrapRootVerification)
    );
    fs::remove_file(source.join("hard-link")).expect("remove hard link");
    install_component(&source, &target).expect("initial installation");
    assert_eq!(
        install_component(&source, &target),
        Err(HelperFailure::BootstrapRootVerification)
    );
    assert_eq!(
        fs::read(target.join("bin/cargo")).expect("preserved payload"),
        b"cargo"
    );
}

#[test]
fn refuses_symlinked_manifest_and_conflicting_directory_modes() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let source = component(temporary.path(), "cargo", &[("bin/cargo", b"cargo")]);
    let target = temporary.path().join("toolchain");
    fs::create_dir(&target).expect("toolchain");
    fs::rename(source.join("manifest.in"), source.join("manifest-original")).expect("rename");
    std::os::unix::fs::symlink("manifest-original", source.join("manifest.in")).expect("link");
    assert_eq!(
        install_component(&source, &target),
        Err(HelperFailure::BootstrapRootVerification)
    );
    fs::remove_file(source.join("manifest.in")).expect("remove link");
    fs::rename(source.join("manifest-original"), source.join("manifest.in")).expect("restore");
    fs::create_dir(target.join("bin")).expect("destination bin");
    let source_mode = fs::metadata(source.join("bin"))
        .expect("source metadata")
        .mode()
        & 0o777;
    fs::set_permissions(
        target.join("bin"),
        fs::Permissions::from_mode(source_mode ^ 0o100),
    )
    .expect("changed directory mode");
    assert_eq!(
        install_component(&source, &target),
        Err(HelperFailure::BootstrapRootVerification)
    );
}

#[test]
#[ignore = "requires the pinned retained Rust archives and BusyBox input"]
fn retained_official_archives_install_the_exact_declared_toolchain_payload() {
    use super::super::super::linux_bootstrap_archive::{self, Compression, LinkPolicy};
    use super::super::{archive_limits, tree::hash_file};

    let inputs = PathBuf::from(
        std::env::var_os("RETONR_RETAINED_TOOLCHAIN_INPUTS").expect("retained component root"),
    );
    let temporary = tempfile::tempdir().expect("temporary directory");
    let busybox_path = temporary.path().join("busybox");
    fs::copy(inputs.join("toolchains/busybox"), &busybox_path).expect("copy BusyBox");
    assert_eq!(
        hash_file(&busybox_path).expect("BusyBox hash").as_str(),
        "82bbbabec12a985ae58810cfe975c3399264dc888aa592d8e460732bdd30a8dd"
    );
    fs::set_permissions(&busybox_path, fs::Permissions::from_mode(0o555)).expect("BusyBox mode");
    let busybox = File::open(busybox_path).expect("BusyBox descriptor");
    let toolchain = temporary.path().join("toolchain");
    fs::create_dir(&toolchain).expect("toolchain directory");
    for (name, component, digest) in [
        (
            "cargo",
            "cargo",
            "d0aeadfea55964a8866014efbe08bcfd6225d68529d522a6eef300d4f8d5c9d2",
        ),
        (
            "rustc",
            "rustc",
            "33a15df85ab0faf63b4c75b1113e47b41fd745a73bdc898c41034e9a9257b154",
        ),
        (
            "rust-std",
            "rust-std-x86_64-unknown-linux-musl",
            "d160dfc81d21fdc72534859fae249fecf6ab70640375f64fc4e77005a48c18d0",
        ),
    ] {
        let root = format!("{name}-1.97.1-x86_64-unknown-linux-musl");
        let archive_path = inputs.join("lineage/rust").join(format!("{root}.tar.gz"));
        assert_eq!(
            hash_file(&archive_path)
                .expect("official archive hash")
                .as_str(),
            digest
        );
        let archive = File::open(archive_path).expect("official archive descriptor");
        let staging = temporary.path().join(name);
        fs::create_dir(&staging).expect("component staging");
        linux_bootstrap_archive::extract(
            &busybox,
            &archive,
            Compression::Gzip,
            &staging,
            Some(&root),
            LinkPolicy::Reject,
            archive_limits(),
        )
        .expect("production bounded extraction");
        install_component(&staging.join(component), &toolchain)
            .expect("production component installation");
    }
    assert!(toolchain.join("bin/cargo").is_file());
    assert!(toolchain.join("bin/rustc").is_file());
    assert!(
        toolchain
            .join("lib/rustlib/x86_64-unknown-linux-musl/lib")
            .is_dir()
    );
    assert!(!toolchain.join("manifest.in").exists());
}
