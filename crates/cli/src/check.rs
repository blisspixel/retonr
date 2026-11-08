//! Deterministic candidate validation with explicit input and output policy.

use std::{
    io::{self, IsTerminal, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};

use rewrite_app::{
    CandidateCheckRequest, CandidateCheckService, MAX_CANDIDATE_CHECK_BYTES, StyleSampleProfile,
};
use rewrite_types::{CancellationToken, ReasonCode, RewriteStatus};

use crate::{
    contract::{
        CommandName, EXIT_CANCELLED, EXIT_POLICY, ReportFormat, STANDARD_STREAM_PATH,
        read_input_bounded,
    },
    failure::{InputRole, RunFailure},
};

mod directory;
pub(crate) mod inspect;
pub(crate) mod receipt;
pub(crate) mod replace;
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

/// Human-report switches for one command.
pub(crate) struct HumanReport {
    /// Show short digests in the human report.
    pub(crate) verbose: bool,
    /// Color choice for the human report.
    pub(crate) color: crate::presentation::ColorChoice,
}

/// Owned arguments for one candidate check.
pub(crate) struct CheckRequest {
    pub(crate) source: PathBuf,
    pub(crate) candidate: PathBuf,
    pub(crate) traversal: CheckTraversal,
    pub(crate) protected_terms: Vec<String>,
    pub(crate) fail_on_abstain: bool,
    pub(crate) output: Option<PathBuf>,
    pub(crate) in_place: replace::InPlaceFlags,
    pub(crate) raw_terminal: bool,
    pub(crate) confirmed: bool,
    pub(crate) inspection: CheckInspection,
    pub(crate) layout: Option<rewrite_types::LayoutConstraints>,
    pub(crate) edit_level: Option<rewrite_types::EditLevel>,
    /// Inclusive change-share limit. Values above 100 are a usage error.
    pub(crate) max_change_percent: Option<u8>,
    /// Paths of authorized samples. Empty means no style-distance report.
    pub(crate) style_sample_paths: Vec<PathBuf>,
    /// New file that receives the opt-in local receipt.
    pub(crate) receipt: Option<PathBuf>,
    /// Human-report switches.
    pub(crate) report: HumanReport,
}

/// Whether directory discovery visits children.
#[derive(Clone, Copy)]
pub(crate) enum CheckTraversal {
    Flat,
    Recursive,
}

impl CheckTraversal {
    pub(crate) const fn recursive(self) -> bool {
        matches!(self, Self::Recursive)
    }
}

/// Optional inspection views that do not change acceptance.
pub(crate) struct CheckInspection {
    pub(crate) diff: bool,
    pub(crate) dry_run: bool,
    pub(crate) trace: Option<PathBuf>,
}

const MAX_STYLE_SAMPLES: usize = 16;

/// Validates one candidate and applies the explicit input and output policy.
pub(crate) fn run(request: &CheckRequest, format: ReportFormat) -> Result<ExitCode, RunFailure> {
    if request
        .max_change_percent
        .is_some_and(|percent| percent > 100)
    {
        return Err(RunFailure::usage_for(CommandName::Check));
    }
    let style_samples = load_style_samples(&request.style_sample_paths)?;
    if directory::is_real_directory(&request.source)
        && directory::is_real_directory(&request.candidate)
    {
        return directory::run(request, format, style_samples.as_ref());
    }
    if request.traversal.recursive() {
        return Err(RunFailure::usage_for(CommandName::Check));
    }
    let destination = replace::resolve_destination(
        &request.source,
        request.output.as_deref(),
        request.in_place,
        CommandName::Check,
    )?;
    replace::validate_destination_preflight(
        &destination,
        request.inspection.dry_run,
        CommandName::Check,
    )?;
    ensure_distinct_inputs(&request.source, &request.candidate)?;
    validate_trace_destination(
        &destination,
        request.inspection.trace.as_deref(),
        request.inspection.dry_run,
        CommandName::Check,
    )?;
    receipt::validate(
        &destination,
        request.receipt.as_deref(),
        request.inspection.trace.as_deref(),
        CommandName::Check,
    )?;

    let cancellation = CancellationToken::new();
    let signal_cancellation = cancellation.clone();
    ctrlc::try_set_handler(move || signal_cancellation.cancel())
        .map_err(|_| RunFailure::operational(CommandName::Check))?;

    let source_bytes = read_input_bounded(&request.source, MAX_CANDIDATE_CHECK_BYTES)
        .map_err(|error| RunFailure::check_read_at(InputRole::Source, &request.source, &error))?;
    let candidate_bytes = read_input_bounded(&request.candidate, MAX_CANDIDATE_CHECK_BYTES)
        .map_err(|error| {
            RunFailure::check_read_at(InputRole::Candidate, &request.candidate, &error)
        })?;
    let candidate =
        String::from_utf8(candidate_bytes).map_err(|_| RunFailure::check_invalid_utf8())?;
    let source_for_diff = request.inspection.diff.then(|| source_bytes.clone());

    let result = CandidateCheckService::check_with_cancellation(
        CandidateCheckRequest {
            source: source_bytes.clone(),
            candidate,
            protected_terms: request.protected_terms.clone(),
            layout: request.layout,
            edit_level: request.edit_level,
            max_change_percent: request.max_change_percent,
            style_samples,
        },
        &cancellation,
    )
    .map_err(|error| RunFailure::check_app(&error))?;
    if result.record.reason == Some(ReasonCode::Cancelled) {
        return Err(RunFailure::cancelled(CommandName::Check));
    }
    require_not_cancelled(&cancellation, CommandName::Check)?;
    publish_check(
        format,
        &destination,
        request,
        &source_bytes,
        source_for_diff.as_deref(),
        &result,
    )
}

fn publish_check(
    format: ReportFormat,
    destination: &replace::Destination,
    request: &CheckRequest,
    source_bytes: &[u8],
    source_for_diff: Option<&[u8]>,
    result: &rewrite_app::CandidateCheckResult,
) -> Result<ExitCode, RunFailure> {
    let backup = if request.inspection.dry_run {
        None
    } else {
        emit_destination(
            destination,
            &request.source,
            source_bytes,
            &result.output,
            request.raw_terminal,
            request.confirmed,
            CommandName::Check,
        )?
    };
    if let Some(trace) = request.inspection.trace.as_ref() {
        inspect::write_trace(trace, &result.record, CommandName::Check)?;
    }
    if let Some(path) = request.receipt.as_ref() {
        receipt::write(
            path,
            &result.record,
            &receipt::ReceiptFacts {
                fidelity: report::fidelity_for(request.edit_level),
                changed_token_pct: result.change.changed_token_pct,
                changed_char_pct: result.change.changed_char_pct,
                created_at: std::time::SystemTime::now(),
            },
        )?;
    }
    let target = destination_report_target(destination);
    let stream_is_terminal = match target {
        ReportTarget::Data => crate::presentation::stdout_is_terminal(),
        ReportTarget::Diagnostic => crate::presentation::stderr_is_terminal(),
    };
    report::write(
        CommandName::Check,
        &result.record,
        &report::ReportContext {
            format,
            target,
            backup: backup.as_deref(),
            fidelity: Some(report::fidelity_for(request.edit_level)),
            measurement: Some(&report::CheckMeasurement {
                changed_token_pct: result.change.changed_token_pct,
                changed_char_pct: result.change.changed_char_pct,
                changed_word_count: result.change.changed_word_count,
                style_distance: result.style_distance.as_ref(),
            }),
            verbose: request.report.verbose,
            color: crate::presentation::color_enabled(request.report.color, stream_is_terminal),
            width: crate::presentation::terminal_width(stream_is_terminal),
            emission: report::emission(
                request.inspection.dry_run,
                document_was_requested(destination),
            ),
        },
    )
    .map_err(|_| RunFailure::operational(CommandName::Check))?;
    if let Some(diff_source) = source_for_diff {
        inspect::write_diff(
            &inspect::SafeDiff::compare(diff_source, &result.output),
            ReportTarget::Diagnostic,
            CommandName::Check,
        )?;
    }
    Ok(exit_status(
        result.record.status,
        result.record.reason,
        request.fail_on_abstain,
    ))
}

fn document_was_requested(destination: &replace::Destination) -> bool {
    !matches!(destination, replace::Destination::Sink(OutputSink::None))
}

fn load_style_samples(paths: &[PathBuf]) -> Result<Option<StyleSampleProfile>, RunFailure> {
    if paths.is_empty() {
        return Ok(None);
    }
    if paths.len() > MAX_STYLE_SAMPLES {
        return Err(RunFailure::usage_for(CommandName::Check));
    }
    let mut remaining = MAX_CANDIDATE_CHECK_BYTES;
    let mut texts = Vec::with_capacity(paths.len());
    for path in paths {
        let bytes = read_input_bounded(path, remaining)
            .map_err(|error| RunFailure::check_read_at(InputRole::StyleSample, path, &error))?;
        remaining = remaining.saturating_sub(bytes.len());
        let text =
            String::from_utf8(bytes).map_err(|_| RunFailure::check_invalid_style_sample())?;
        texts.push(text);
    }
    Ok(Some(StyleSampleProfile::from_samples(
        texts.iter().map(String::as_str),
    )))
}

pub(crate) fn validate_trace_destination(
    destination: &replace::Destination,
    trace: Option<&Path>,
    dry_run: bool,
    command: CommandName,
) -> Result<(), RunFailure> {
    let Some(trace) = trace else {
        return Ok(());
    };
    inspect::require_new_trace_path(trace, command)?;
    if !dry_run && destination.reserves_path(trace, command)? {
        return Err(RunFailure::usage_for(command));
    }
    Ok(())
}

pub(crate) fn require_not_cancelled(
    cancellation: &CancellationToken,
    command: CommandName,
) -> Result<(), RunFailure> {
    if cancellation.is_cancelled() {
        return Err(RunFailure::cancelled(command));
    }
    Ok(())
}

pub(crate) fn emit_destination(
    destination: &replace::Destination,
    source: &Path,
    original: &[u8],
    accepted: &[u8],
    raw_terminal: bool,
    confirmed: bool,
    command: CommandName,
) -> Result<Option<String>, RunFailure> {
    match destination {
        replace::Destination::Sink(sink) => {
            emit_document(sink, accepted, raw_terminal, confirmed, command)?;
            Ok(None)
        }
        replace::Destination::InPlace { source: planned } => {
            if planned != source {
                return Err(RunFailure::operational(command));
            }
            let outcome = replace::commit(planned, original, accepted, command)?;
            Ok(replace::backup_name(&outcome).map(str::to_owned))
        }
    }
}

pub(crate) fn destination_report_target(destination: &replace::Destination) -> ReportTarget {
    match destination {
        replace::Destination::Sink(sink) => report_target(sink),
        replace::Destination::InPlace { .. } => ReportTarget::Data,
    }
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
    replace::require_unoccupied_path(output, command)?;
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
/// The default path never modifies the source and never replaces an existing
/// destination. `--in-place` uses a separately tested backup-then-replace protocol.
/// A terminal receives escaped rendering unless `--raw-terminal --yes` both
/// appear. Either flag alone stays escaped. Raw terminal emission warns on
/// standard error before the exact bytes.
pub(crate) fn emit_document(
    sink: &OutputSink,
    bytes: &[u8],
    raw_terminal: bool,
    confirmed: bool,
    command: CommandName,
) -> Result<(), RunFailure> {
    match sink {
        OutputSink::None => Ok(()),
        OutputSink::File(path) => replace::write_exclusive(path, bytes, command),
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
                DocumentRender::Escaped => crate::render::render_document_for_terminal(bytes)
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
        | RewriteStatus::UnchangedIdentical
        | RewriteStatus::Abstained => ExitCode::SUCCESS,
    }
}
