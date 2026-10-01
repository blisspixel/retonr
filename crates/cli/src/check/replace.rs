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

impl Destination {
    /// Returns whether this invocation reserves `path` for document output.
    pub(crate) fn reserves_path(
        &self,
        path: &Path,
        command: CommandName,
    ) -> Result<bool, RunFailure> {
        match self {
            Self::Sink(OutputSink::File(output)) => Ok(same_planned_path(output, path)),
            Self::Sink(OutputSink::None | OutputSink::Standard) => Ok(false),
            Self::InPlace { source } => Ok(same_planned_path(source, path)
                || same_planned_path(&sibling(source, BACKUP_SUFFIX, command)?, path)
                || same_planned_path(&sibling(source, STAGING_SUFFIX, command)?, path)),
        }
    }
}

fn same_planned_path(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    let Some(left_name) = left.file_name() else {
        return false;
    };
    let Some(right_name) = right.file_name() else {
        return false;
    };
    if !same_file_name(left_name, right_name) {
        return false;
    }
    let left_parent = nonempty_parent(left);
    let right_parent = nonempty_parent(right);
    match (
        fs::canonicalize(left_parent),
        fs::canonicalize(right_parent),
    ) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn nonempty_parent(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

#[cfg(windows)]
fn same_file_name(left: &std::ffi::OsStr, right: &std::ffi::OsStr) -> bool {
    left.to_string_lossy()
        .eq_ignore_ascii_case(&right.to_string_lossy())
}

#[cfg(not(windows))]
fn same_file_name(left: &std::ffi::OsStr, right: &std::ffi::OsStr) -> bool {
    left == right
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
    require_unoccupied_path(&backup_path, command)?;
    require_unoccupied_path(&staging_path, command)?;
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
    require_original_bytes(source, original, command)?;
    if original == accepted {
        return Ok(InPlaceOutcome::Unchanged);
    }
    let backup_path = sibling(source, BACKUP_SUFFIX, command)?;
    let staging_path = sibling(source, STAGING_SUFFIX, command)?;
    require_unoccupied_path(&backup_path, command)?;
    require_unoccupied_path(&staging_path, command)?;
    write_exclusive(&backup_path, original, command)?;
    write_exclusive(&staging_path, accepted, command)?;
    let staged = crate::file_input::read_direct_unaliased_bounded(
        &staging_path,
        accepted.len().saturating_add(1),
    )
    .map_err(|_| RunFailure::operational(command))?;
    if staged != accepted {
        return Err(RunFailure::operational(command));
    }
    install_verified(source, &staging_path, original, accepted, command)?;
    let replaced =
        crate::file_input::read_direct_unaliased_bounded(source, accepted.len().saturating_add(1))
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

fn require_original_bytes(
    source: &Path,
    original: &[u8],
    command: CommandName,
) -> Result<(), RunFailure> {
    let current =
        crate::file_input::read_direct_unaliased_bounded(source, original.len().saturating_add(1))
            .map_err(|_| RunFailure::concurrent_modification(command))?;
    if current != original {
        return Err(RunFailure::concurrent_modification(command));
    }
    Ok(())
}

fn install_verified(
    source: &Path,
    staging: &Path,
    original: &[u8],
    accepted: &[u8],
    command: CommandName,
) -> Result<(), RunFailure> {
    require_regular_file(source, command)?;
    require_original_bytes(source, original, command)?;
    let staged =
        crate::file_input::read_direct_unaliased_bounded(staging, accepted.len().saturating_add(1))
            .map_err(|_| RunFailure::concurrent_modification(command))?;
    if staged != accepted {
        return Err(RunFailure::concurrent_modification(command));
    }
    install(source, staging, command)
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

/// Rejects any filesystem entry, including a dangling link, at a planned new path.
pub(crate) fn require_unoccupied_path(path: &Path, command: CommandName) -> Result<(), RunFailure> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(RunFailure::output_exists_for(command)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(RunFailure::operational(command)),
    }
}

/// Requires the parent of a planned new file to exist as a directory.
pub(crate) fn require_existing_parent(path: &Path, command: CommandName) -> Result<(), RunFailure> {
    let parent = nonempty_parent(path);
    let metadata = fs::metadata(parent).map_err(|_| RunFailure::operational(command))?;
    if metadata.is_dir() {
        Ok(())
    } else {
        Err(RunFailure::operational(command))
    }
}

/// Performs the complete preflight for one exclusively created output file.
pub(crate) fn require_new_file_path(path: &Path, command: CommandName) -> Result<(), RunFailure> {
    require_existing_parent(path, command)?;
    require_unoccupied_path(path, command)
}

/// Validates work that a non-dry-run destination will require before document work.
pub(crate) fn validate_destination_preflight(
    destination: &Destination,
    dry_run: bool,
    command: CommandName,
) -> Result<(), RunFailure> {
    if dry_run {
        return Ok(());
    }
    if let Destination::Sink(OutputSink::File(path)) = destination {
        require_existing_parent(path, command)?;
    }
    Ok(())
}

fn install(source: &Path, staging: &Path, command: CommandName) -> Result<(), RunFailure> {
    #[cfg(any(unix, windows))]
    {
        fs::rename(staging, source).map_err(|_| RunFailure::operational(command))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (source, staging);
        Err(RunFailure::operational(command))
    }
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
pub(super) fn require_single_link(
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
pub(super) fn require_single_link(
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
pub(super) fn require_single_link(
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
mod tests;
