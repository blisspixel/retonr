//! Pre-model inventory of one source document or directory without mutation.

use std::{fs, path::Path, process::ExitCode};

use crate::contract::{
    CommandName, EXIT_USAGE, ErrorBody, ErrorCategory, ErrorCode, STANDARD_STREAM_PATH,
};
use crate::failure::RunFailure;
use crate::model::ModelOutput;

mod directory;
mod report;

/// Inventories one source file, standard input, or directory.
pub(crate) fn run(
    source: &Path,
    recursive: bool,
) -> Result<(CommandName, ModelOutput, ExitCode), RunFailure> {
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
    let report = report::inspect_file(source)?;
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

fn recursive_requires_directory() -> RunFailure {
    RunFailure {
        command: CommandName::Inspect,
        body: ErrorBody::new(ErrorCategory::Usage, ErrorCode::InvalidInvocation, false),
        exit_code: ExitCode::from(EXIT_USAGE),
        message: "recursive inspect requires a directory",
    }
}
