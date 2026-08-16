//! Deterministic candidate validation with explicit input and output policy.

use std::{
    fs::OpenOptions,
    io::{self, IsTerminal, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};

use rewrite_app::{CandidateCheckRequest, CandidateCheckService, MAX_CANDIDATE_CHECK_BYTES};
use rewrite_types::{CancellationToken, ReasonCode, RewriteStatus};

use crate::{
    contract::{
        CommandName, EXIT_CANCELLED, EXIT_POLICY, ReportFormat, STANDARD_STREAM_PATH,
        read_input_bounded,
    },
    failure::RunFailure,
};

mod report;
#[cfg(test)]
mod tests;

/// Where the exact accepted document bytes are written, if anywhere.
#[derive(Clone, Debug, Eq, PartialEq)]
enum OutputSink {
    /// Only the report is emitted.
    None,
    /// Exact bytes are written to a new file that must not already exist.
    File(PathBuf),
    /// Exact bytes are written to standard output.
    Standard,
}

/// Owned arguments for one candidate check.
pub(crate) struct CheckRequest {
    pub(crate) source: PathBuf,
    pub(crate) candidate: PathBuf,
    pub(crate) protected_terms: Vec<String>,
    pub(crate) fail_on_abstain: bool,
    pub(crate) output: Option<PathBuf>,
    pub(crate) raw_terminal: bool,
    pub(crate) confirmed: bool,
}

/// Validates one candidate and applies the explicit input and output policy.
pub(crate) fn run(request: CheckRequest, format: ReportFormat) -> Result<ExitCode, RunFailure> {
    let sink = resolve_output_sink(&request)?;
    ensure_distinct_inputs(&request.source, &request.candidate)?;

    let cancellation = CancellationToken::new();
    let signal_cancellation = cancellation.clone();
    ctrlc::try_set_handler(move || signal_cancellation.cancel())
        .map_err(|_| RunFailure::operational(CommandName::Check))?;

    let source = read_input_bounded(&request.source, MAX_CANDIDATE_CHECK_BYTES)
        .map_err(|error| RunFailure::check_read(&error))?;
    let candidate_bytes = read_input_bounded(&request.candidate, MAX_CANDIDATE_CHECK_BYTES)
        .map_err(|error| RunFailure::check_read(&error))?;
    let candidate =
        String::from_utf8(candidate_bytes).map_err(|_| RunFailure::check_invalid_utf8())?;

    let result = CandidateCheckService::check_with_cancellation(
        CandidateCheckRequest {
            source,
            candidate,
            protected_terms: request.protected_terms,
        },
        &cancellation,
    )
    .map_err(|error| RunFailure::check_app(&error))?;
    if result.record.reason == Some(ReasonCode::Cancelled) {
        return Err(RunFailure::cancelled(CommandName::Check));
    }

    emit_document(&sink, &result.output, request.raw_terminal)?;
    report::write(&result.record, format, report_target(&sink))
        .map_err(|_| RunFailure::operational(CommandName::Check))?;
    Ok(exit_status(
        result.record.status,
        result.record.reason,
        request.fail_on_abstain,
    ))
}

/// Rejects reading both documents from one standard-input stream.
fn ensure_distinct_inputs(source: &Path, candidate: &Path) -> Result<(), RunFailure> {
    if is_standard_stream(source) && is_standard_stream(candidate) {
        return Err(RunFailure::usage_for(CommandName::Check));
    }
    Ok(())
}

fn is_standard_stream(path: &Path) -> bool {
    path.as_os_str() == STANDARD_STREAM_PATH
}

/// Resolves and validates the output policy before any document work happens.
fn resolve_output_sink(request: &CheckRequest) -> Result<OutputSink, RunFailure> {
    let Some(output) = request.output.as_ref() else {
        return Ok(OutputSink::None);
    };
    if is_standard_stream(output) {
        if io::stdout().is_terminal() && !(request.raw_terminal && request.confirmed) {
            return Err(RunFailure::raw_terminal_refused());
        }
        return Ok(OutputSink::Standard);
    }
    if output.exists() {
        return Err(RunFailure::output_exists());
    }
    Ok(OutputSink::File(output.clone()))
}

/// Reports are separated from document bytes so one stream is never both.
const fn report_target(sink: &OutputSink) -> ReportTarget {
    match sink {
        OutputSink::Standard => ReportTarget::Diagnostic,
        OutputSink::None | OutputSink::File(_) => ReportTarget::Data,
    }
}

/// Stream that carries the versioned report for this invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReportTarget {
    /// Standard output, used whenever document bytes do not occupy it.
    Data,
    /// Standard error, used when document bytes occupy standard output.
    Diagnostic,
}

/// Writes the exact accepted bytes according to the resolved output policy.
///
/// The source document is never modified and an existing destination is never
/// replaced. Raw terminal emission warns on standard error before the bytes.
fn emit_document(sink: &OutputSink, bytes: &[u8], raw_terminal: bool) -> Result<(), RunFailure> {
    match sink {
        OutputSink::None => Ok(()),
        OutputSink::File(path) => write_new_file(path, bytes),
        OutputSink::Standard => {
            if raw_terminal && io::stdout().is_terminal() {
                let mut stderr = io::stderr().lock();
                writeln!(
                    stderr,
                    "warning: writing exact unescaped document bytes to a terminal"
                )
                .map_err(|_| RunFailure::operational(CommandName::Check))?;
            }
            let mut stdout = io::stdout().lock();
            stdout
                .write_all(bytes)
                .and_then(|()| stdout.flush())
                .map_err(|_| RunFailure::operational(CommandName::Check))
        }
    }
}

/// Creates the destination exclusively so an existing file is never replaced.
fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), RunFailure> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| {
            if error.kind() == io::ErrorKind::AlreadyExists {
                RunFailure::output_exists()
            } else {
                RunFailure::operational(CommandName::Check)
            }
        })?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| RunFailure::operational(CommandName::Check))
}

pub(crate) fn exit_status(
    status: RewriteStatus,
    reason: Option<ReasonCode>,
    fail_on_abstain: bool,
) -> ExitCode {
    if reason == Some(ReasonCode::Cancelled) {
        return ExitCode::from(EXIT_CANCELLED);
    }
    match status {
        RewriteStatus::Failed => ExitCode::FAILURE,
        RewriteStatus::Abstained if fail_on_abstain => ExitCode::from(EXIT_POLICY),
        RewriteStatus::Rewritten
        | RewriteStatus::UnchangedNoEligibleContent
        | RewriteStatus::Abstained => ExitCode::SUCCESS,
    }
}
