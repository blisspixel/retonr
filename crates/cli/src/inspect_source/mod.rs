//! Pre-model inventory of one source document or directory without mutation.

use std::{fs, path::Path, process::ExitCode};

use crate::contract::{
    CommandName, EXIT_USAGE, ErrorBody, ErrorCategory, ErrorCode, STANDARD_STREAM_PATH,
};
use crate::failure::RunFailure;
use crate::model::ModelOutput;

pub(crate) mod directory;
mod report;

/// Applies the shared carrier and sidecar policy to already retained input bytes.
pub(crate) fn requires_derivative_decision(
    source: &Path,
    bytes: &[u8],
    command: CommandName,
) -> Result<bool, RunFailure> {
    report::inventory_report(
        source,
        command,
        bytes,
        &report::InspectDetail::counts_only(),
    )
    .map(|(report, _)| report.derivative() == "explicit_decision_required")
}

/// Flags for one inspect invocation.
pub(crate) struct InspectOptions {
    pub(crate) recursive: bool,
    pub(crate) locate: bool,
    pub(crate) decode_tags: bool,
}

/// Inventories one source file, standard input, or directory.
pub(crate) fn run(
    source: &Path,
    options: &InspectOptions,
) -> Result<(CommandName, ModelOutput, ExitCode), RunFailure> {
    if (options.locate || options.decode_tags) && is_real_directory(source) {
        return Err(detail_requires_one_document());
    }
    let recursive = options.recursive;
    if recursive {
        if source.as_os_str() == STANDARD_STREAM_PATH {
            return Err(recursive_requires_directory());
        }
        let metadata = fs::symlink_metadata(source)
            .map_err(|error| RunFailure::input_read(CommandName::Inspect, &error))?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(recursive_requires_directory());
        }
        return directory::inspect(source, true);
    }
    if source.as_os_str() != STANDARD_STREAM_PATH
        && let Ok(metadata) = fs::symlink_metadata(source)
        && metadata.is_dir()
        && !metadata.file_type().is_symlink()
    {
        return directory::inspect(source, false);
    }
    let detail = report::InspectDetail {
        locate: options.locate,
        decode_tags: options.decode_tags,
    };
    let report = report::inspect_file(source, CommandName::Inspect, &detail)?;
    Ok((
        CommandName::Inspect,
        ModelOutput {
            value: serde_json::to_value(&report).expect("inspect report serializes"),
            text: report.text(),
            findings: false,
        },
        ExitCode::SUCCESS,
    ))
}

fn is_real_directory(source: &Path) -> bool {
    source.as_os_str() != STANDARD_STREAM_PATH
        && fs::symlink_metadata(source)
            .ok()
            .is_some_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
}

fn detail_requires_one_document() -> RunFailure {
    RunFailure {
        command: CommandName::Inspect,
        body: ErrorBody::new(ErrorCategory::Usage, ErrorCode::InvalidInvocation, false),
        exit_code: ExitCode::from(EXIT_USAGE),
        message: "locate and decode-tags inspect one document",
    }
}

fn recursive_requires_directory() -> RunFailure {
    RunFailure {
        command: CommandName::Inspect,
        body: ErrorBody::new(ErrorCategory::Usage, ErrorCode::InvalidInvocation, false),
        exit_code: ExitCode::from(EXIT_USAGE),
        message: "recursive inspect requires a directory",
    }
}
