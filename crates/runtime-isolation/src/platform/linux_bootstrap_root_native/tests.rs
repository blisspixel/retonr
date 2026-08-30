use std::{fs, os::unix::fs::symlink};

use super::{
    HelperFailure, install_linker_names, validate_libc_linker_name, validate_libgcc_linker_name,
    validate_proc_mtab_at,
};

#[test]
fn linker_names_are_exact_relative_and_payload_bound() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let musl = temporary.path().join("lib");
    let library = temporary.path().join("usr/lib");
    fs::create_dir_all(&musl).expect("musl directory");
    fs::create_dir_all(&library).expect("library directory");
    fs::write(musl.join("ld-musl-x86_64.so.1"), b"authenticated musl").expect("musl payload");
    fs::write(library.join("libgcc_s.so.1"), b"authenticated libgcc").expect("libgcc payload");
    install_linker_names(temporary.path()).expect("linker names");
    let libc_digest = validate_libc_linker_name(temporary.path()).expect("libc linker name");
    let libgcc_digest = validate_libgcc_linker_name(temporary.path()).expect("libgcc linker name");
    assert_eq!(
        fs::read_link(musl.join("libc.so")).expect("link target"),
        std::path::Path::new("ld-musl-x86_64.so.1")
    );
    assert_eq!(
        fs::read_link(library.join("libgcc_s.so")).expect("link target"),
        std::path::Path::new("libgcc_s.so.1")
    );
    assert_eq!(validate_libc_linker_name(temporary.path()), Ok(libc_digest));
    assert_eq!(
        validate_libgcc_linker_name(temporary.path()),
        Ok(libgcc_digest)
    );
    assert_eq!(
        install_linker_names(temporary.path()),
        Err(HelperFailure::BootstrapRootPreparation)
    );

    fs::remove_file(library.join("libgcc_s.so")).expect("remove linker name");
    symlink("./libgcc_s.so.1", library.join("libgcc_s.so")).expect("aliased linker name");
    assert_eq!(
        validate_libgcc_linker_name(temporary.path()),
        Err(HelperFailure::BootstrapRootVerification)
    );

    fs::remove_file(library.join("libgcc_s.so")).expect("remove alias");
    symlink("missing.so.1", library.join("libgcc_s.so")).expect("dangling linker name");
    assert_eq!(
        validate_libgcc_linker_name(temporary.path()),
        Err(HelperFailure::BootstrapRootVerification)
    );

    fs::remove_file(musl.join("libc.so")).expect("remove libc linker name");
    symlink("./ld-musl-x86_64.so.1", musl.join("libc.so")).expect("aliased libc linker name");
    assert_eq!(
        validate_libc_linker_name(temporary.path()),
        Err(HelperFailure::BootstrapRootVerification)
    );

    fs::remove_file(musl.join("libc.so")).expect("remove libc alias");
    fs::write(musl.join("libc.so"), b"forged regular linker name")
        .expect("regular libc linker name");
    assert_eq!(
        validate_libc_linker_name(temporary.path()),
        Err(HelperFailure::BootstrapRootVerification)
    );

    fs::remove_file(musl.join("libc.so")).expect("remove forged linker name");
    fs::remove_file(musl.join("ld-musl-x86_64.so.1")).expect("remove loader payload");
    fs::write(musl.join("terminal-loader"), b"indirect musl").expect("terminal loader");
    symlink("terminal-loader", musl.join("ld-musl-x86_64.so.1")).expect("indirect loader payload");
    symlink("ld-musl-x86_64.so.1", musl.join("libc.so")).expect("libc linker name");
    assert_eq!(
        validate_libc_linker_name(temporary.path()),
        Err(HelperFailure::BootstrapRootVerification)
    );
}

#[test]
fn proc_mtab_validation_requires_the_exact_post_mount_resolution() {
    let temporary = tempfile::tempdir().expect("temporary root");
    fs::create_dir(temporary.path().join("etc")).expect("etc");
    fs::create_dir(temporary.path().join("proc")).expect("proc");
    fs::write(temporary.path().join("proc/mounts"), b"private proc").expect("mounts");
    symlink("../proc/mounts", temporary.path().join("etc/mtab")).expect("mtab");
    assert_eq!(validate_proc_mtab_at(temporary.path()), Ok(()));

    fs::remove_file(temporary.path().join("etc/mtab")).expect("remove mtab");
    symlink("../proc/missing", temporary.path().join("etc/mtab")).expect("dangling mtab");
    assert_eq!(
        validate_proc_mtab_at(temporary.path()),
        Err(HelperFailure::BootstrapRootVerification)
    );
}

#[test]
fn proc_mtab_validation_rejects_regular_and_aliased_records() {
    let temporary = tempfile::tempdir().expect("temporary root");
    fs::create_dir(temporary.path().join("etc")).expect("etc");
    fs::create_dir(temporary.path().join("proc")).expect("proc");
    fs::write(temporary.path().join("proc/mounts"), b"private proc").expect("mounts");
    fs::write(temporary.path().join("etc/mtab"), b"forged").expect("regular mtab");
    assert_eq!(
        validate_proc_mtab_at(temporary.path()),
        Err(HelperFailure::BootstrapRootVerification)
    );

    fs::remove_file(temporary.path().join("etc/mtab")).expect("remove mtab");
    symlink("../proc/../proc/mounts", temporary.path().join("etc/mtab")).expect("aliased mtab");
    assert_eq!(
        validate_proc_mtab_at(temporary.path()),
        Err(HelperFailure::BootstrapRootVerification)
    );
}
