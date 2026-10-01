use std::{fs, os::unix::fs::PermissionsExt as _, path::Path};

use super::{HelperFailure, install};

fn fixture(root: &Path, body: &str) -> std::path::PathBuf {
    fs::create_dir_all(root.join("lib")).expect("lib directory");
    fs::create_dir_all(root.join("sbin")).expect("sbin directory");
    let loader = root.join("lib/ld-musl-x86_64.so.1");
    fs::write(&loader, format!("#!/bin/sh\n{body}\n")).expect("loader fixture");
    fs::set_permissions(loader, fs::Permissions::from_mode(0o700)).expect("loader mode");
    fs::write(root.join("sbin/apk"), b"retained apk").expect("apk fixture");
    let package = root.join("libgcc.apk");
    fs::write(&package, b"retained package").expect("package fixture");
    package
}

#[test]
fn temporary_root_install_uses_only_the_narrow_persistence_exception() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let package = fixture(
        temporary.path(),
        r#"
test "$#" -eq 8 || exit 99
test "$2" = add && test "$3" = --root || exit 99
test "$5" = --no-cache && test "$6" = --no-network || exit 99
test "$7" = --force-non-repository || exit 99
test "$LD_LIBRARY_PATH" = "$4/lib:$4/usr/lib" || exit 99
test -f "$1" && test -f "$8" || exit 99
test -z "$HOME" || exit 99
exit 0
"#,
    );
    assert_eq!(install(temporary.path(), &package), Ok(()));
}

#[test]
fn rejected_package_and_unexecutable_installer_cannot_report_success() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let package = fixture(temporary.path(), "exit 99");
    assert_eq!(
        install(temporary.path(), &package),
        Err(HelperFailure::BootstrapRootVerification)
    );
    fs::set_permissions(
        temporary.path().join("lib/ld-musl-x86_64.so.1"),
        fs::Permissions::from_mode(0o600),
    )
    .expect("non executable loader");
    assert_eq!(
        install(temporary.path(), &package),
        Err(HelperFailure::BootstrapRootPreparation)
    );
}

#[test]
fn missing_or_non_file_install_inputs_are_refused_before_execution() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let package = fixture(temporary.path(), "exit 0");
    fs::remove_file(&package).expect("remove package");
    assert_eq!(
        install(temporary.path(), &package),
        Err(HelperFailure::BootstrapRootVerification)
    );
    fs::create_dir(&package).expect("directory package");
    assert_eq!(
        install(temporary.path(), &package),
        Err(HelperFailure::BootstrapRootVerification)
    );
}

mod retained;
