use std::{fs::File, io::Write as _, path::Path};

use landlock::{
    ABI, Access, AccessFs, CompatLevel, Compatible, PathBeneath, Ruleset, RulesetAttr,
    RulesetCreatedAttr, RulesetStatus, make_bitflags,
};
use rustix::fs::{AtFlags, Mode, OFlags, openat, unlinkat};

use super::{linux_build_mount::require_read_only_mount, linux_helper_setup::HelperFailure};

const SELECTED_LANDLOCK_ABI: ABI = ABI::V3;
const SELECTED_LANDLOCK_ABI_NUMBER: u32 = 3;
const READ_ONLY_ROOTS: [&str; 5] = ["/inputs", "/toolchain", "/source", "/vendor", "/raw-crates"];
const WRITABLE_ROOTS: [&str; 3] = ["/cargo-home", "/target", "/output"];

pub(super) fn install_and_probe() -> Result<u32, HelperFailure> {
    let read_access = make_bitflags!(AccessFs::{Execute | ReadFile | ReadDir});
    let write_access = AccessFs::from_all(SELECTED_LANDLOCK_ABI);
    let read_roots = READ_ONLY_ROOTS
        .map(open_directory)
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    for root in &read_roots {
        require_read_only_mount(root)?;
    }
    let write_roots = WRITABLE_ROOTS
        .map(open_directory)
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let dev_null = File::options()
        .read(true)
        .write(true)
        .open("/dev/null")
        .map_err(|_| HelperFailure::FilesystemIsolationSetup)?;
    let mut ruleset = Ruleset::default()
        .handle_access(write_access)
        .map_err(|_| HelperFailure::FilesystemIsolationSetup)?
        .set_compatibility(CompatLevel::HardRequirement)
        .create()
        .map_err(|_| HelperFailure::FilesystemIsolationUnavailable)?;
    for root in read_roots {
        ruleset = ruleset
            .add_rule(PathBeneath::new(root, read_access))
            .map_err(|_| HelperFailure::FilesystemIsolationSetup)?;
    }
    for root in write_roots {
        ruleset = ruleset
            .add_rule(PathBeneath::new(root, write_access))
            .map_err(|_| HelperFailure::FilesystemIsolationSetup)?;
    }
    let status = ruleset
        .add_rule(PathBeneath::new(
            dev_null,
            make_bitflags!(AccessFs::{ReadFile | WriteFile}),
        ))
        .map_err(|_| HelperFailure::FilesystemIsolationSetup)?
        .set_compatibility(CompatLevel::HardRequirement)
        .restrict_self()
        .map_err(|_| HelperFailure::FilesystemIsolationSetup)?;
    if status.ruleset != RulesetStatus::FullyEnforced || !status.no_new_privs {
        return Err(HelperFailure::FilesystemIsolationUnavailable);
    }
    probe_denied_read_only_write()?;
    probe_allowed_output()?;
    probe_old_root_absent()?;
    Ok(SELECTED_LANDLOCK_ABI_NUMBER)
}

fn open_directory(path: &str) -> Result<File, HelperFailure> {
    let file = File::open(path).map_err(|_| HelperFailure::FilesystemIsolationSetup)?;
    if file
        .metadata()
        .map_err(|_| HelperFailure::FilesystemIsolationSetup)?
        .is_dir()
    {
        Ok(file)
    } else {
        Err(HelperFailure::FilesystemIsolationSetup)
    }
}

fn probe_denied_read_only_write() -> Result<(), HelperFailure> {
    if File::options()
        .write(true)
        .create_new(true)
        .open("/source/.retonr-bootstrap-write-canary")
        .is_err()
    {
        Ok(())
    } else {
        Err(HelperFailure::FilesystemIsolationBehavior)
    }
}

fn probe_allowed_output() -> Result<(), HelperFailure> {
    const CANARY: &str = ".retonr-bootstrap-output-canary";
    let output = File::open("/output").map_err(|_| HelperFailure::FilesystemIsolationBehavior)?;
    let descriptor = openat(
        &output,
        CANARY,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(|_| HelperFailure::FilesystemIsolationBehavior)?;
    let mut file = File::from(descriptor);
    file.write_all(b"ok")
        .map_err(|_| HelperFailure::FilesystemIsolationBehavior)?;
    drop(file);
    unlinkat(&output, CANARY, AtFlags::empty())
        .map_err(|_| HelperFailure::FilesystemIsolationBehavior)
}

fn probe_old_root_absent() -> Result<(), HelperFailure> {
    if Path::new("/.old-root/proc/self").exists() {
        Err(HelperFailure::FilesystemIsolationBehavior)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_sets_are_disjoint_and_exact() {
        assert_eq!(SELECTED_LANDLOCK_ABI_NUMBER, 3);
        assert!(
            READ_ONLY_ROOTS
                .iter()
                .all(|path| !WRITABLE_ROOTS.contains(path))
        );
        assert_eq!(READ_ONLY_ROOTS.len(), 5);
        assert_eq!(WRITABLE_ROOTS.len(), 3);
    }
}
