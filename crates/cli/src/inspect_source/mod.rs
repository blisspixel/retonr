//! Pre-model inventory of one source document or directory without mutation.

use std::{fs, path::Path, process::ExitCode};

use crate::contract::{
    CommandName, EXIT_COMPATIBILITY, ErrorBody, ErrorCategory, ErrorCode, STANDARD_STREAM_PATH,
};
use crate::failure::RunFailure;
use crate::model::ModelOutput;

mod directory;
mod report;

/// Inventories one source file, standard input, or non-recursive directory.
pub(crate) fn run(
    source: &Path,
    recursive: bool,
) -> Result<(CommandName, ModelOutput, ExitCode), RunFailure> {
    if recursive {
        return Err(unsupported_recursive());
    }
    if source.as_os_str() != STANDARD_STREAM_PATH
        && let Ok(metadata) = fs::symlink_metadata(source)
        && metadata.is_dir()
        && !metadata.file_type().is_symlink()
    {
        return directory::inspect(source);
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

fn unsupported_recursive() -> RunFailure {
    RunFailure {
        command: CommandName::Inspect,
        body: ErrorBody::new(ErrorCategory::Compatibility, ErrorCode::Unsupported, false),
        exit_code: ExitCode::from(EXIT_COMPATIBILITY),
        message: "recursive directory inspect is not implemented",
    }
}
