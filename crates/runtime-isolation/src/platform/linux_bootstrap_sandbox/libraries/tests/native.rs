use std::{fs, process::Command};

use landlock::{
    ABI, Access as _, CompatLevel, Compatible as _, PathBeneath, Ruleset, RulesetAttr as _,
    RulesetCreatedAttr as _, RulesetStatus,
};
use rustix::{
    mount::{MountFlags, mount_bind, mount_remount},
    thread::UnshareFlags,
};

use super::*;

const TEST: &str = "platform::linux_bootstrap_sandbox::libraries::tests::native::read_only_library_grants_refuse_aliases_and_leave_other_files_denied";

fn profile_directory() -> Option<File> {
    let profile = std::env::var_os("LLVM_PROFILE_FILE")?;
    assert_eq!(std::env::var("CARGO_LLVM_COV").as_deref(), Ok("1"));
    let path = std::path::Path::new(&profile);
    assert!(path.is_absolute());
    assert!(
        path.file_name()
            .expect("profile name")
            .to_string_lossy()
            .ends_with(".profraw")
    );
    let parent = path.parent().expect("private profile directory");
    assert!(matches!(
        parent.file_name().and_then(|name| name.to_str()),
        Some("native" | "retonr-profiles")
    ));
    assert_eq!(
        fs::canonicalize(parent).expect("canonical profile directory"),
        parent
    );
    let file = File::open(parent).expect("held private profile directory");
    assert!(file.metadata().expect("profile metadata").is_dir());
    Some(file)
}

#[test]
#[ignore = "requires an isolated privileged networkless Linux container"]
fn read_only_library_grants_refuse_aliases_and_leave_other_files_denied() {
    if std::env::var_os("RETONR_LIBRARY_GRANT_NATIVE_CHILD").is_none() {
        let completed = Command::new(std::env::current_exe().expect("test executable"))
            .args(["--ignored", "--exact", TEST, "--test-threads=1"])
            .env("RETONR_LIBRARY_GRANT_NATIVE_CHILD", "1")
            .output()
            .expect("isolated native child");
        assert!(completed.status.success(), "native child: {completed:?}");
        return;
    }
    #[expect(
        deprecated,
        reason = "the isolated native child serializes this mount-only fixture"
    )]
    rustix::thread::unshare(UnshareFlags::NEWNS).expect("private mount namespace");
    let directory = tempfile::tempdir().expect("payload root");
    fs::create_dir_all(directory.path().join("lib")).expect("lib");
    fs::create_dir_all(directory.path().join("usr/lib")).expect("usr lib");
    let loader = directory.path().join(LOADER);
    let libgcc = directory.path().join(LIBGCC);
    fs::write(&loader, b"\x7fELFretained loader").expect("loader fixture");
    fs::write(&libgcc, b"\x7fELFretained libgcc").expect("libgcc fixture");
    fs::write(directory.path().join("unlisted"), b"private").expect("unlisted");
    fs::hard_link(&libgcc, directory.path().join("hardlink")).expect("hard alias");
    symlink("usr/lib/libgcc_s.so.1", directory.path().join("symbolic")).expect("symbolic alias");
    let root = File::open(directory.path()).expect("held root");
    assert!(open_at(&root).is_err(), "writable root must fail");
    fs::remove_file(directory.path().join("hardlink")).expect("remove alias");
    mount_bind(directory.path(), directory.path()).expect("private root bind");
    mount_remount(
        directory.path(),
        MountFlags::BIND | MountFlags::RDONLY | MountFlags::NOSUID | MountFlags::NODEV,
        "",
    )
    .expect("read only bind");
    assert!(open_at(&root).is_err(), "pre-bind handle remains writable");
    let root = File::open(directory.path()).expect("held read-only root");
    assert!(payload(&root, "symbolic").is_err());
    let libraries = open_at(&root).expect("verified held libraries");
    assert_eq!(
        libraries[0].1,
        make_bitflags!(AccessFs::{Execute | ReadFile})
    );
    assert_eq!(libraries[1].1, AccessFs::ReadFile);
    let mut rules = Ruleset::default()
        .handle_access(AccessFs::from_all(ABI::V3))
        .expect("handled ABI3")
        .set_compatibility(CompatLevel::HardRequirement)
        .create()
        .expect("ruleset");
    for (file, access) in libraries {
        rules = rules
            .add_rule(PathBeneath::new(file, access))
            .expect("exact file rule");
    }
    if let Some(directory) = profile_directory() {
        // This test-only grant lets the instrumented child save its own profile.
        // It does not grant execution, directory enumeration, removal or access
        // to any other host path, and is absent from the production ruleset.
        rules = rules
            .add_rule(PathBeneath::new(
                directory,
                make_bitflags!(AccessFs::{ReadFile | WriteFile | MakeReg | Truncate}),
            ))
            .expect("exact native profile rule");
    }
    let status = rules
        .set_compatibility(CompatLevel::HardRequirement)
        .restrict_self()
        .expect("restrict");
    assert_eq!(status.ruleset, RulesetStatus::FullyEnforced);
    assert!(status.no_new_privs);
    assert_eq!(
        fs::read(&loader).expect("allowed loader"),
        b"\x7fELFretained loader"
    );
    assert_eq!(
        fs::read(&libgcc).expect("allowed libgcc"),
        b"\x7fELFretained libgcc"
    );
    assert!(fs::read(directory.path().join("unlisted")).is_err());
    assert!(fs::read("/etc/passwd").is_err());
    assert!(File::options().write(true).open(&loader).is_err());
    assert!(File::options().write(true).open(&libgcc).is_err());
    // The child owns the private mount namespace and exits immediately; no
    // mount or Landlock policy is inherited by the parent fixture process.
}
