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

mod escape;
mod inspect;
pub(crate) mod report;
#[cfg(test)]
mod tests;

/// Where the exact accepted document bytes are written, if anywhere.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum OutputSink {
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
    pub(crate) inspection: CheckInspection,
}

/// Optional inspection views that do not change acceptance.
pub(crate) struct CheckInspection {
    pub(crate) diff: bool,
    pub(crate) dry_run: bool,
    pub(crate) trace: Option<PathBuf>,
}

/// Validates one candidate and applies the explicit input and output policy.
pub(crate) fn run(request: CheckRequest, format: ReportFormat) -> Result<ExitCode, RunFailure> {
    let sink = resolve_output_sink(request.output.as_deref())?;
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
    let source_for_diff = request.inspection.diff.then(|| source.clone());

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

    if !request.inspection.dry_run {
        emit_document(
            &sink,
            &result.output,
            request.raw_terminal,
            request.confirmed,
            CommandName::Check,
        )?;
    }
    if let Some(source_bytes) = source_for_diff.as_deref() {
        inspect::write_diff(
            &inspect::SafeDiff::compare(source_bytes, &result.output),
            ReportTarget::Diagnostic,
        )?;
    }
    if let Some(trace) = request.inspection.trace.as_ref() {
        inspect::write_trace(trace, &result.record)?;
    }
    report::write(
        CommandName::Check,
        &result.record,
        format,
        report_target(&sink),
    )
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

pub(crate) fn is_standard_stream(path: &Path) -> bool {
    path.as_os_str() == STANDARD_STREAM_PATH
}

/// Resolves and validates the output policy before any document work happens.
pub(crate) fn resolve_output_sink(output: Option<&Path>) -> Result<OutputSink, RunFailure> {
    resolve_output_sink_for(output, CommandName::Check)
}

pub(crate) fn resolve_output_sink_for(
    output: Option<&Path>,
    command: CommandName,
) -> Result<OutputSink, RunFailure> {
    let Some(output) = output else {
        return Ok(OutputSink::None);
    };
    if is_standard_stream(output) {
        return Ok(OutputSink::Standard);
    }
    if output.exists() {
        return Err(RunFailure::output_exists_for(command));
    }
    Ok(OutputSink::File(output.to_path_buf()))
}

/// Reports are separated from document bytes so one stream is never both.
pub(crate) const fn report_target(sink: &OutputSink) -> ReportTarget {
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

/// How accepted document bytes are written to a destination stream.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DocumentRender {
    /// Exact accepted bytes. Used for files, pipes, and the raw-terminal opt-in.
    Exact,
    /// Escaped interactive rendering that cannot drive a terminal.
    Escaped,
}

/// Chooses exact bytes versus escaped rendering without consulting the filesystem.
const fn resolve_document_render(
    is_terminal: bool,
    raw_terminal: bool,
    confirmed: bool,
) -> DocumentRender {
    if is_terminal && !(raw_terminal && confirmed) {
        DocumentRender::Escaped
    } else {
        DocumentRender::Exact
    }
}

/// Writes accepted document bytes according to the resolved output policy.
///
/// The source document is never modified and an existing destination is never
/// replaced. A terminal receives escaped rendering unless `--raw-terminal --yes`
/// both appear. Either flag alone stays escaped. Raw terminal emission warns
/// on standard error before the exact bytes.
pub(crate) fn emit_document(
    sink: &OutputSink,
    bytes: &[u8],
    raw_terminal: bool,
    confirmed: bool,
    command: CommandName,
) -> Result<(), RunFailure> {
    match sink {
        OutputSink::None => Ok(()),
        OutputSink::File(path) => write_new_file(path, bytes, command),
        OutputSink::Standard => {
            let terminal = io::stdout().is_terminal();
            let render = resolve_document_render(terminal, raw_terminal, confirmed);
            if render == DocumentRender::Exact && terminal {
                let mut stderr = io::stderr().lock();
                writeln!(
                    stderr,
                    "warning: writing exact unescaped document bytes to a terminal"
                )
                .map_err(|_| RunFailure::operational(command))?;
            }
            let payload = match render {
                DocumentRender::Exact => bytes.to_vec(),
                DocumentRender::Escaped => escape::render_document_for_terminal(bytes)
                    .map_err(|_| RunFailure::operational(command))?,
            };
            let mut stdout = io::stdout().lock();
            stdout
                .write_all(&payload)
                .and_then(|()| stdout.flush())
                .map_err(|_| RunFailure::operational(command))
        }
    }
}

/// Creates the destination exclusively so an existing file is never replaced.
fn write_new_file(path: &Path, bytes: &[u8], command: CommandName) -> Result<(), RunFailure> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| {
            if error.kind() == io::ErrorKind::AlreadyExists {
                RunFailure::output_exists_for(command)
            } else {
                RunFailure::operational(command)
            }
        })?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| RunFailure::operational(command))
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
