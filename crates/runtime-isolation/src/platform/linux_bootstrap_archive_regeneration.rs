use std::{
    fs::{self, File},
    io::{Read as _, Seek as _, SeekFrom},
    os::{
        fd::AsRawFd as _,
        unix::{fs::MetadataExt as _, process::CommandExt as _},
    },
    path::Path,
    process::{Command, Stdio},
};

use rustix::fs::{Mode, OFlags, ResolveFlags, openat2};

use super::linux_helper_setup::HelperFailure;

const ARCHIVE_TOOL_OUTPUT: &str = "rewrite-runtime-source-archive";
const COMPARISON_ROOT: &str = "/target/.retonr-source-archive-comparison";
const COPY_BUFFER_BYTES: usize = 1024 * 1024;

const PROGRAMS: [(&str, &str); 4] = [
    ("rewrite-runtime-isolation-helper", "helper/isolation"),
    (
        ARCHIVE_TOOL_OUTPUT,
        "lineage/tools/rewrite-runtime-source-archive",
    ),
    ("rewrite-runtime-source-builder", "scripts/build"),
    (
        "rewrite-runtime-source-manifest",
        "lineage/tools/rewrite-runtime-source-manifest",
    ),
];

const ARCHIVES: [(&str, &str, &str, &str); 3] = [
    (
        "retonr-source",
        "/source",
        "retonr-source.tar",
        "lineage/source/retonr-source.tar",
    ),
    (
        "cargo-vendor",
        "/vendor",
        "cargo-vendor.tar",
        "lineage/source/cargo-vendor.tar",
    ),
    (
        "cargo-crates",
        "/raw-crates",
        "cargo-crates.tar",
        "lineage/source/cargo-crates.tar",
    ),
];

pub(super) fn execute(input_root: &File, output_root: &File) -> Result<(), HelperFailure> {
    execute_at(
        input_root,
        output_root,
        Path::new(COMPARISON_ROOT),
        run_archive_tool,
    )
}

fn execute_at<R>(
    input_root: &File,
    output_root: &File,
    comparison_root: &Path,
    mut regenerate: R,
) -> Result<(), HelperFailure>
where
    R: FnMut(&File, &str, &str, &Path) -> Result<(), HelperFailure>,
{
    require_expected_programs(input_root, output_root)?;
    let archive_tool = open_regular(output_root, ARCHIVE_TOOL_OUTPUT)?;
    fs::create_dir(comparison_root).map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    let comparison_directory =
        File::open(comparison_root).map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    if !comparison_directory
        .metadata()
        .map_err(|_| HelperFailure::BootstrapRootPreparation)?
        .is_dir()
    {
        return Err(HelperFailure::BootstrapRootPreparation);
    }
    for (kind, source, file_name, expected_path) in ARCHIVES {
        let destination = comparison_root.join(file_name);
        regenerate(&archive_tool, kind, source, &destination)?;
        let regenerated = open_regular(&comparison_directory, file_name)?;
        let expected = open_regular(input_root, expected_path)?;
        require_exact_bytes(&regenerated, &expected)?;
        fs::remove_file(destination).map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    }
    fs::remove_dir(comparison_root).map_err(|_| HelperFailure::BootstrapRootPreparation)
}

fn require_expected_programs(input_root: &File, output_root: &File) -> Result<(), HelperFailure> {
    for (output_path, expected_path) in PROGRAMS {
        let output = open_regular(output_root, output_path)?;
        let expected = open_regular(input_root, expected_path)?;
        require_exact_bytes(&output, &expected)?;
    }
    Ok(())
}

fn open_regular(root: &File, relative_path: &str) -> Result<File, HelperFailure> {
    let file = openat2(
        root,
        relative_path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
        ResolveFlags::BENEATH
            | ResolveFlags::NO_MAGICLINKS
            | ResolveFlags::NO_SYMLINKS
            | ResolveFlags::NO_XDEV,
    )
    .map(File::from)
    .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?;
    let metadata = file
        .metadata()
        .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?;
    if metadata.is_file() && metadata.nlink() == 1 {
        Ok(file)
    } else {
        Err(HelperFailure::ControlledBuildObjectMismatch)
    }
}

fn run_archive_tool(
    tool: &File,
    kind: &str,
    source: &str,
    destination: &Path,
) -> Result<(), HelperFailure> {
    let mut command = Command::new(format!("/proc/self/fd/{}", tool.as_raw_fd()));
    command
        .arg0(ARCHIVE_TOOL_OUTPUT)
        .args([kind, source])
        .arg(destination)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let status = command
        .status()
        .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    if status.success() {
        Ok(())
    } else {
        Err(HelperFailure::BootstrapRootVerification)
    }
}

fn require_exact_bytes(left: &File, right: &File) -> Result<(), HelperFailure> {
    let left_metadata = left
        .metadata()
        .map_err(|_| HelperFailure::BootstrapRootVerification)?;
    let right_metadata = right
        .metadata()
        .map_err(|_| HelperFailure::BootstrapRootVerification)?;
    if !left_metadata.is_file()
        || !right_metadata.is_file()
        || left_metadata.nlink() != 1
        || right_metadata.nlink() != 1
        || left_metadata.len() != right_metadata.len()
    {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    let mut left = left
        .try_clone()
        .map_err(|_| HelperFailure::BootstrapRootVerification)?;
    let mut right = right
        .try_clone()
        .map_err(|_| HelperFailure::BootstrapRootVerification)?;
    left.seek(SeekFrom::Start(0))
        .map_err(|_| HelperFailure::BootstrapRootVerification)?;
    right
        .seek(SeekFrom::Start(0))
        .map_err(|_| HelperFailure::BootstrapRootVerification)?;
    let mut left_buffer = vec![0_u8; COPY_BUFFER_BYTES];
    let mut right_buffer = vec![0_u8; COPY_BUFFER_BYTES];
    loop {
        let left_read = left
            .read(&mut left_buffer)
            .map_err(|_| HelperFailure::BootstrapRootVerification)?;
        let right_read = right
            .read(&mut right_buffer)
            .map_err(|_| HelperFailure::BootstrapRootVerification)?;
        if left_read != right_read || left_buffer[..left_read] != right_buffer[..right_read] {
            return Err(HelperFailure::BootstrapRootVerification);
        }
        if left_read == 0 {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{os::unix::fs::PermissionsExt as _, path::PathBuf};

    use rustix::io::{FdFlags, fcntl_setfd};

    use super::*;

    #[test]
    fn byte_comparison_rejects_length_and_content_drift() {
        let temporary = tempfile::tempdir().expect("temporary");
        let first_path = temporary.path().join("first");
        let second_path = temporary.path().join("second");
        fs::write(&first_path, b"same").expect("first bytes");
        fs::write(&second_path, b"same").expect("second bytes");
        let first = File::open(first_path).expect("first");
        let second = File::open(&second_path).expect("second");
        assert_eq!(require_exact_bytes(&first, &second), Ok(()));
        fs::write(&second_path, b"drift").expect("drift");
        assert_eq!(
            require_exact_bytes(&first, &second),
            Err(HelperFailure::BootstrapRootVerification)
        );
        fs::write(second_path, b"else").expect("same length");
        assert_eq!(
            require_exact_bytes(&first, &second),
            Err(HelperFailure::BootstrapRootVerification)
        );
    }

    #[test]
    fn regeneration_plan_is_closed_and_root_adjusted() {
        assert_eq!(PROGRAMS.len(), 4);
        assert_eq!(ARCHIVES.len(), 3);
        assert_eq!(ARCHIVES[0].0, "retonr-source");
        assert_eq!(ARCHIVES[0].1, "/source");
        assert_eq!(ARCHIVES[1].0, "cargo-vendor");
        assert_eq!(ARCHIVES[1].1, "/vendor");
        assert_eq!(ARCHIVES[2].0, "cargo-crates");
        assert_eq!(ARCHIVES[2].1, "/raw-crates");
    }

    #[test]
    fn coordinator_joins_all_programs_and_archives_before_cleanup() {
        let input = tempfile::tempdir().expect("input");
        let output = tempfile::tempdir().expect("output");
        for (output_path, expected_path) in PROGRAMS {
            write_tree_file(input.path(), expected_path, output_path.as_bytes());
            write_tree_file(output.path(), output_path, output_path.as_bytes());
        }
        for (kind, _, _, expected_path) in ARCHIVES {
            write_tree_file(input.path(), expected_path, kind.as_bytes());
        }
        let input_root = File::open(input.path()).expect("input root");
        let output_root = File::open(output.path()).expect("output root");
        let comparison = output.path().join("comparison");
        let mut observed = Vec::new();
        execute_at(
            &input_root,
            &output_root,
            &comparison,
            |_, kind, source, destination| {
                observed.push((kind.to_owned(), source.to_owned()));
                fs::write(destination, kind.as_bytes())
                    .map_err(|_| HelperFailure::BootstrapRootPreparation)
            },
        )
        .expect("complete regeneration");
        assert_eq!(
            observed,
            ARCHIVES
                .map(|(kind, source, _, _)| (kind.to_owned(), source.to_owned()))
                .to_vec()
        );
        assert!(!comparison.exists());
    }

    #[test]
    fn retained_fd_executor_requires_exact_success_status() {
        let temporary = tempfile::tempdir().expect("temporary");
        let destination = temporary.path().join("archive.tar");
        let successful = executable_script(temporary.path(), "successful", b"#!/bin/sh\nexit 0\n");
        assert_eq!(
            run_archive_tool(&successful, "kind", "/source", &destination),
            Ok(())
        );
        let failing = executable_script(temporary.path(), "failing", b"#!/bin/sh\nexit 7\n");
        assert_eq!(
            run_archive_tool(&failing, "kind", "/source", &destination),
            Err(HelperFailure::BootstrapRootVerification)
        );
    }

    fn executable_script(root: &Path, name: &str, bytes: &[u8]) -> File {
        let path = write_tree_file(root, name, bytes);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).expect("executable mode");
        let file = File::open(path).expect("executable");
        fcntl_setfd(&file, FdFlags::empty()).expect("retained script descriptor");
        file
    }

    fn write_tree_file(root: &Path, relative: &str, bytes: &[u8]) -> PathBuf {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().expect("parent")).expect("parent directories");
        fs::write(&path, bytes).expect("tree file");
        path
    }
}
