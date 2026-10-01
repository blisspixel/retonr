use super::*;
use rustix::{
    mount::{MountPropagationFlags, mount_bind, mount_change},
    process::{chdir, chroot},
    thread::UnshareFlags,
};
use std::{os::fd::AsRawFd as _, path::Path, process::Command};

const TEST: &str = "platform::linux_build_output::tests::native::held_output_inspection_and_publication_work_without_proc";
const CHILD_ROOT: &str = "RETONR_OUTPUT_NO_PROC_CHILD_ROOT";

#[test]
#[ignore = "requires an isolated privileged networkless Linux container"]
fn held_output_inspection_and_publication_work_without_proc() {
    let Some(root) = std::env::var_os(CHILD_ROOT) else {
        let directory = tempfile::tempdir().expect("private chroot fixture");
        for name in ["output/nested", "destination", "proc"] {
            fs::create_dir_all(directory.path().join(name)).expect("fixture directory");
        }
        fs::write(
            directory.path().join("output/nested/payload"),
            b"retained bytes",
        )
        .expect("payload");
        fs::set_permissions(
            directory.path().join("output/nested/payload"),
            fs::Permissions::from_mode(0o755),
        )
        .expect("executable mode");
        let completed = Command::new(std::env::current_exe().expect("test binary"))
            .args(["--ignored", "--exact", TEST, "--test-threads=1"])
            .env(CHILD_ROOT, directory.path())
            .output()
            .expect("isolated child");
        assert!(completed.status.success(), "no-proc child: {completed:?}");
        return;
    };
    #[expect(
        deprecated,
        reason = "isolated single-thread child owns the mount namespace"
    )]
    rustix::thread::unshare(UnshareFlags::NEWNS).expect("private mount namespace");
    mount_change(
        "/",
        MountPropagationFlags::PRIVATE | MountPropagationFlags::REC,
    )
    .expect("private propagation");
    let root = Path::new(&root);
    retain_test_profile_directory(root);
    let output = File::open(root.join("output")).expect("held source");
    let destination = File::open(root.join("destination")).expect("held destination");
    let expected = inspect_retained_output(&output).expect("baseline commitment");
    chroot(root).expect("real private chroot");
    chdir("/").expect("private root cwd");
    assert!(
        fs::read_dir("/proc")
            .expect("empty proc directory")
            .next()
            .is_none()
    );
    let proc_path = format!("/proc/self/fd/{}", output.as_raw_fd());
    assert_eq!(
        fs::read_dir(proc_path)
            .expect_err("old proc-based enumeration fails")
            .raw_os_error(),
        Some(libc::ENOENT)
    );
    assert_eq!(
        inspect_retained_output(&output).expect("no-proc inspection"),
        expected
    );
    assert_eq!(
        inspect_retained_output(&output).expect("independent cursor"),
        expected
    );
    require_empty(&destination).expect("empty destination");
    require_empty(&destination).expect("repeated empty inspection");
    publish_retained_output(&destination, &output, &expected).expect("no-proc publication");
    assert_eq!(
        inspect_retained_output(&destination).expect("exact readback"),
        expected
    );
    assert_eq!(
        fs::read("/destination/nested/payload").expect("published bytes"),
        b"retained bytes"
    );
    assert_eq!(
        fs::metadata("/destination/nested/payload")
            .expect("published mode")
            .mode()
            & 0o777,
        0o755
    );
    assert_eq!(
        require_empty(&destination),
        Err(HelperFailure::ControlledBuildOutputNotEmpty)
    );
    assert_eq!(
        publish_retained_output(&destination, &output, &expected),
        Err(HelperFailure::ControlledBuildOutputNotEmpty)
    );
    symlink("nested/payload", "/output/alias").expect("symbolic alias");
    assert!(inspect_retained_output(&output).is_err());
    fs::remove_file("/output/alias").expect("remove alias");
    fs::hard_link("/output/nested/payload", "/output/hardlink").expect("hard alias");
    assert!(inspect_retained_output(&output).is_err());
}

fn retain_test_profile_directory(root: &Path) {
    let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") else {
        return;
    };
    assert_eq!(std::env::var("CARGO_LLVM_COV").as_deref(), Ok("1"));
    let profile = Path::new(&profile);
    assert!(profile.is_absolute());
    assert!(
        profile
            .file_name()
            .expect("profile name")
            .to_string_lossy()
            .ends_with(".profraw")
    );
    let parent = profile.parent().expect("profile directory");
    assert!(matches!(
        parent.file_name().and_then(|name| name.to_str()),
        Some("native" | "retonr-profiles")
    ));
    assert_eq!(
        fs::canonicalize(parent).expect("canonical profile parent"),
        parent
    );
    // Test-only exact private profile mount preserves child coverage after chroot;
    // the production guardian receives no host or proc filesystem access.
    let target = root.join(parent.strip_prefix("/").expect("absolute profile parent"));
    fs::create_dir_all(&target).expect("private profile mount point");
    mount_bind(parent, &target).expect("exact private profile mount");
}
