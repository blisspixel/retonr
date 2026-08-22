//! Recoverable in-place replacement with an explicit same-directory backup.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::ExitCode,
};

use super::OutputSink;
use crate::{
    check::is_standard_stream,
    contract::{CommandName, EXIT_USAGE, ErrorBody, ErrorCategory, ErrorCode},
    failure::RunFailure,
};

const BACKUP_SUFFIX: &str = ".retonr-backup";
const STAGING_SUFFIX: &str = ".retonr-staging";

/// `--in-place` request, with an optional redundant `--backup` flag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct InPlaceFlags {
    /// Replace the source file when paired with `backup`.
    pub(crate) requested: bool,
    /// Retain a sibling backup before replacement.
    pub(crate) backup: bool,
}

/// Where accepted document bytes go after validation.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Destination {
    /// The shared non-replacing output policy.
    Sink(OutputSink),
    /// Replace the source after retaining a sibling backup.
    InPlace { source: PathBuf },
}

/// Result of a completed in-place commit.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum InPlaceOutcome {
    /// Accepted bytes already matched the source, so nothing was written.
    Unchanged,
    /// Source was replaced and a sibling backup of the original was retained.
    Replaced { backup_name: String },
}

/// Resolves `--output` versus `--in-place` before document work.
pub(crate) fn resolve_destination(
    source: &Path,
    output: Option<&Path>,
    in_place: InPlaceFlags,
    command: CommandName,
) -> Result<Destination, RunFailure> {
    if in_place.backup && !in_place.requested {
        return Err(usage(command, "--backup requires --in-place"));
    }
    if !in_place.requested {
        return Ok(Destination::Sink(super::resolve_output_sink_for(
            output, command,
        )?));
    }
    if output.is_some() {
        return Err(usage(command, "in-place is incompatible with --output"));
    }
    if is_standard_stream(source) {
        return Err(usage(
            command,
            "in-place is incompatible with standard input",
        ));
    }
    require_regular_file(source, command)?;
    let backup_path = sibling(source, BACKUP_SUFFIX, command)?;
    let staging_path = sibling(source, STAGING_SUFFIX, command)?;
    if backup_path.exists() || staging_path.exists() {
        return Err(RunFailure::output_exists_for(command));
    }
    Ok(Destination::InPlace {
        source: source.to_path_buf(),
    })
}

/// Writes accepted bytes according to the in-place commit protocol.
///
/// Identical bytes leave the source untouched and create no backup. Changed
/// bytes first retain an exclusive sibling backup of the original, then a
/// same-directory staging file, then replace the source. An existing backup
/// or staging path is never overwritten.
pub(crate) fn commit(
    source: &Path,
    original: &[u8],
    accepted: &[u8],
    command: CommandName,
) -> Result<InPlaceOutcome, RunFailure> {
    require_regular_file(source, command)?;
    if original == accepted {
        return Ok(InPlaceOutcome::Unchanged);
    }
    let backup_path = sibling(source, BACKUP_SUFFIX, command)?;
    let staging_path = sibling(source, STAGING_SUFFIX, command)?;
    if backup_path.exists() || staging_path.exists() {
        return Err(RunFailure::output_exists_for(command));
    }
    let current = crate::contract::read_input_bounded(source, original.len().saturating_add(1))
        .map_err(|error| RunFailure::input_read(command, &error))?;
    if current != original {
        return Err(RunFailure::operational(command));
    }
    write_exclusive(&backup_path, original, command)?;
    write_exclusive(&staging_path, accepted, command)?;
    let staged =
        crate::contract::read_input_bounded(&staging_path, accepted.len().saturating_add(1))
            .map_err(|_| RunFailure::operational(command))?;
    if staged != accepted {
        return Err(RunFailure::operational(command));
    }
    install(source, &staging_path, accepted, command)?;
    let replaced = crate::contract::read_input_bounded(source, accepted.len().saturating_add(1))
        .map_err(|_| RunFailure::operational(command))?;
    if replaced != accepted {
        return Err(RunFailure::operational(command));
    }
    let backup_name = backup_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| usage(command, "in-place requires a regular file"))?
        .to_owned();
    Ok(InPlaceOutcome::Replaced { backup_name })
}

pub(crate) fn backup_name(outcome: &InPlaceOutcome) -> Option<&str> {
    match outcome {
        InPlaceOutcome::Unchanged => None,
        InPlaceOutcome::Replaced { backup_name } => Some(backup_name.as_str()),
    }
}

/// Creates a destination exclusively so an existing file is never replaced.
pub(crate) fn write_exclusive(
    path: &Path,
    bytes: &[u8],
    command: CommandName,
) -> Result<(), RunFailure> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                RunFailure::output_exists_for(command)
            } else {
                RunFailure::operational(command)
            }
        })?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| RunFailure::operational(command))
}

fn install(
    source: &Path,
    staging: &Path,
    accepted: &[u8],
    command: CommandName,
) -> Result<(), RunFailure> {
    #[cfg(unix)]
    {
        let _ = accepted;
        fs::rename(staging, source).map_err(|_| RunFailure::operational(command))
    }
    #[cfg(windows)]
    {
        overwrite_regular_file(source, accepted, command)?;
        match fs::remove_file(staging) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(RunFailure::operational(command)),
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (staging, accepted);
        Err(RunFailure::operational(command))
    }
}

#[cfg(windows)]
fn overwrite_regular_file(
    path: &Path,
    bytes: &[u8],
    command: CommandName,
) -> Result<(), RunFailure> {
    require_regular_file(path, command)?;
    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path)
        .map_err(|_| RunFailure::operational(command))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| RunFailure::operational(command))
}

fn require_regular_file(path: &Path, command: CommandName) -> Result<(), RunFailure> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            RunFailure::input_read(command, &error)
        } else {
            RunFailure::operational(command)
        }
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(usage(command, "in-place requires a regular file"));
    }
    require_single_link(path, &metadata, command)?;
    Ok(())
}

fn hard_link_usage(command: CommandName) -> RunFailure {
    usage(
        command,
        "in-place requires a regular file without hard-link aliases",
    )
}

#[cfg(unix)]
fn require_single_link(
    _path: &Path,
    metadata: &fs::Metadata,
    command: CommandName,
) -> Result<(), RunFailure> {
    use std::os::unix::fs::MetadataExt as _;

    if metadata.nlink() != 1 {
        return Err(hard_link_usage(command));
    }
    Ok(())
}

#[cfg(windows)]
fn require_single_link(
    path: &Path,
    _metadata: &fs::Metadata,
    command: CommandName,
) -> Result<(), RunFailure> {
    let file = fs::File::open(path).map_err(|_| RunFailure::operational(command))?;
    let information = winx::winapi_util::file::information(&file)
        .map_err(|_| RunFailure::operational(command))?;
    if information.number_of_links() != 1 {
        return Err(hard_link_usage(command));
    }
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn require_single_link(
    _path: &Path,
    _metadata: &fs::Metadata,
    command: CommandName,
) -> Result<(), RunFailure> {
    Err(hard_link_usage(command))
}

fn sibling(source: &Path, suffix: &str, command: CommandName) -> Result<PathBuf, RunFailure> {
    let name = source
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| usage(command, "in-place requires a regular file"))?;
    if name.is_empty() || name == "." || name == ".." {
        return Err(usage(command, "in-place requires a regular file"));
    }
    let parent = source.parent().unwrap_or_else(|| Path::new(""));
    Ok(parent.join(format!("{name}{suffix}")))
}

fn usage(command: CommandName, message: &'static str) -> RunFailure {
    RunFailure {
        command,
        body: ErrorBody::new(ErrorCategory::Usage, ErrorCode::InvalidInvocation, false),
        exit_code: ExitCode::from(EXIT_USAGE),
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    const fn flags(requested: bool, backup: bool) -> InPlaceFlags {
        InPlaceFlags { requested, backup }
    }

    #[test]
    fn in_place_implies_backup() {
        let directory = tempdir().expect("temporary directory");
        let source = directory.path().join("draft.txt");
        fs::write(&source, b"original\n").expect("write source");
        let destination =
            resolve_destination(&source, None, flags(true, false), CommandName::Check)
                .expect("in-place implies backup");
        assert!(matches!(destination, Destination::InPlace { .. }));
        let with_flag = resolve_destination(&source, None, flags(true, true), CommandName::Check)
            .expect("redundant --backup remains valid");
        assert_eq!(destination, with_flag);
    }

    #[test]
    fn backup_without_in_place_is_usage() {
        let failure = resolve_destination(
            Path::new("draft.txt"),
            None,
            flags(false, true),
            CommandName::Rewrite,
        )
        .expect_err("backup requires in-place");
        assert!(failure.message.contains("requires --in-place"));
    }

    #[test]
    fn in_place_with_output_is_usage() {
        let failure = resolve_destination(
            Path::new("draft.txt"),
            Some(Path::new("out.txt")),
            flags(true, true),
            CommandName::Check,
        )
        .expect_err("in-place rejects --output");
        assert!(failure.message.contains("incompatible with --output"));
    }

    #[test]
    fn in_place_on_standard_input_is_usage() {
        let failure =
            resolve_destination(Path::new("-"), None, flags(true, true), CommandName::Check)
                .expect_err("stdin is refused");
        assert!(failure.message.contains("standard input"));
    }

    #[test]
    fn commit_leaves_identical_bytes_untouched() {
        let directory = tempdir().expect("temporary directory");
        let source = directory.path().join("draft.txt");
        fs::write(&source, b"same\n").expect("write source");
        let outcome = commit(&source, b"same\n", b"same\n", CommandName::Check).expect("commit");
        assert!(matches!(outcome, InPlaceOutcome::Unchanged));
        assert_eq!(fs::read(&source).expect("read source"), b"same\n");
        assert!(!directory.path().join("draft.txt.retonr-backup").exists());
        assert!(!directory.path().join("draft.txt.retonr-staging").exists());
    }

    #[test]
    fn commit_retains_a_backup_and_replaces_the_source() {
        let directory = tempdir().expect("temporary directory");
        let source = directory.path().join("draft.txt");
        fs::write(&source, b"original\n").expect("write source");
        let outcome =
            commit(&source, b"original\n", b"accepted\n", CommandName::Check).expect("commit");
        match outcome {
            InPlaceOutcome::Replaced { backup_name } => {
                assert_eq!(backup_name, "draft.txt.retonr-backup");
            }
            InPlaceOutcome::Unchanged => panic!("changed bytes must replace"),
        }
        assert_eq!(fs::read(&source).expect("read source"), b"accepted\n");
        assert_eq!(
            fs::read(directory.path().join("draft.txt.retonr-backup")).expect("read backup"),
            b"original\n"
        );
        assert!(!directory.path().join("draft.txt.retonr-staging").exists());
    }

    #[test]
    fn existing_backup_is_refused_without_mutation() {
        let directory = tempdir().expect("temporary directory");
        let source = directory.path().join("draft.txt");
        let backup = directory.path().join("draft.txt.retonr-backup");
        fs::write(&source, b"original\n").expect("write source");
        fs::write(&backup, b"keep\n").expect("write backup");
        let failure = commit(&source, b"original\n", b"accepted\n", CommandName::Check)
            .expect_err("existing backup");
        assert_eq!(
            failure.exit_code,
            ExitCode::from(crate::contract::EXIT_POLICY)
        );
        assert_eq!(fs::read(&source).expect("read source"), b"original\n");
        assert_eq!(fs::read(&backup).expect("read backup"), b"keep\n");
    }

    #[test]
    fn existing_staging_is_refused_without_mutation() {
        let directory = tempdir().expect("temporary directory");
        let source = directory.path().join("draft.txt");
        let staging = directory.path().join("draft.txt.retonr-staging");
        fs::write(&source, b"original\n").expect("write source");
        fs::write(&staging, b"keep\n").expect("write staging");
        let failure = commit(&source, b"original\n", b"accepted\n", CommandName::Rewrite)
            .expect_err("existing staging");
        assert_eq!(
            failure.exit_code,
            ExitCode::from(crate::contract::EXIT_POLICY)
        );
        assert_eq!(fs::read(&source).expect("read source"), b"original\n");
        assert_eq!(fs::read(&staging).expect("read staging"), b"keep\n");
        assert!(!directory.path().join("draft.txt.retonr-backup").exists());
    }

    #[test]
    fn hard_link_alias_is_refused_without_mutation() {
        let directory = tempdir().expect("temporary directory");
        let source = directory.path().join("draft.txt");
        let alias = directory.path().join("alias.txt");
        fs::write(&source, b"original\n").expect("write source");
        fs::hard_link(&source, &alias).expect("create hard-link alias");

        let failure = commit(&source, b"original\n", b"accepted\n", CommandName::Check)
            .expect_err("hard-linked source is ambiguous");

        assert_eq!(failure.exit_code, ExitCode::from(EXIT_USAGE));
        assert!(failure.message.contains("hard-link aliases"));
        assert_eq!(fs::read(&source).expect("read source"), b"original\n");
        assert_eq!(fs::read(&alias).expect("read alias"), b"original\n");
        assert!(!directory.path().join("draft.txt.retonr-backup").exists());
        assert!(!directory.path().join("draft.txt.retonr-staging").exists());
    }

    #[test]
    fn sibling_names_stay_in_the_source_directory() {
        let path = Path::new("nested").join("draft.txt");
        let backup = sibling(&path, BACKUP_SUFFIX, CommandName::Check).expect("backup");
        assert_eq!(backup, Path::new("nested").join("draft.txt.retonr-backup"));
        assert_eq!(
            backup.file_name().and_then(|name| name.to_str()),
            Some("draft.txt.retonr-backup")
        );
    }
}
