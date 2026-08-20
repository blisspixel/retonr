//! Grounded rewrite command with the shared document output policy.

use std::{path::PathBuf, process::ExitCode};

mod directory;

use rewrite_app::{GroundedRewriteRequest, GroundedRewriteSelection, MAX_CANDIDATE_CHECK_BYTES};
use rewrite_types::{CancellationToken, ReasonCode, RewriteMode};

use crate::{
    check::{
        CheckInspection, destination_report_target, emit_destination, inspect, replace, report,
    },
    contract::{ArtifactIdArgument, CommandName, ReportFormat, read_input_bounded},
    failure::RunFailure,
};

/// Owned arguments for one grounded rewrite invocation.
pub(crate) struct RewriteRequest {
    pub(crate) source: PathBuf,
    pub(crate) output: Option<PathBuf>,
    pub(crate) in_place: crate::check::replace::InPlaceFlags,
    pub(crate) data_directory: Option<PathBuf>,
    pub(crate) artifact_id: Option<ArtifactIdArgument>,
    pub(crate) protected_terms: Vec<String>,
    pub(crate) fail_on_abstain: bool,
    pub(crate) raw_terminal: bool,
    pub(crate) confirmed: bool,
    pub(crate) inspection: CheckInspection,
    pub(crate) directory: DirectoryFlags,
    pub(crate) format: ReportFormat,
}

/// Directory discovery and destination-root options.
pub(crate) struct DirectoryFlags {
    pub(crate) recursive: bool,
    pub(crate) output_dir: Option<PathBuf>,
}

/// Reads one source document and rewrites it only after a recovered fake binding attaches.
///
/// The command uses the same output-destination and inspection policy as `check`.
/// `--diff`, `--dry-run`, and `--trace` do not change acceptance. A directory
/// source is a dry-run destination manifest and does not mutate. An existing
/// file is never replaced unless `--in-place` retains a sibling copy of the
/// original first. It attaches in-process fake-backend conformance when a
/// recovered generation binding names that backend. It does not start a runtime,
/// open a network path, or invent a production backend.
pub(crate) fn run(request: &RewriteRequest) -> Result<ExitCode, RunFailure> {
    if directory::is_real_directory(&request.source) {
        return directory::run(request);
    }
    if request.directory.output_dir.is_some() || request.directory.recursive {
        return Err(RunFailure::usage_for(CommandName::Rewrite));
    }
    if request.artifact_id.is_some() && request.data_directory.is_none() {
        return Err(RunFailure::usage_for(CommandName::Rewrite));
    }
    let destination = replace::resolve_destination(
        &request.source,
        request.output.as_deref(),
        request.in_place,
        CommandName::Rewrite,
    )?;
    let source = read_input_bounded(&request.source, MAX_CANDIDATE_CHECK_BYTES)
        .map_err(|error| RunFailure::input_read(CommandName::Rewrite, &error))?;
    GroundedRewriteSelection::validate_source(&source)
        .map_err(|error| RunFailure::app(CommandName::Rewrite, &error))?;
    let requested = request
        .artifact_id
        .as_ref()
        .map(ArtifactIdArgument::to_artifact_id);
    let attached = GroundedRewriteSelection::require_ready(
        request.data_directory.as_deref(),
        requested.as_ref(),
    )
    .map_err(|error| RunFailure::app(CommandName::Rewrite, &error))?;
    let cancellation = CancellationToken::new();
    let signal_cancellation = cancellation.clone();
    ctrlc::try_set_handler(move || signal_cancellation.cancel())
        .map_err(|_| RunFailure::operational(CommandName::Rewrite))?;
    let result = attached
        .rewrite(
            GroundedRewriteRequest {
                source: source.clone(),
                protected_terms: request.protected_terms.clone(),
                mode: RewriteMode::Literal,
                style_context: String::new(),
                claim_shadow: None,
            },
            &cancellation,
            None,
        )
        .map_err(|error| RunFailure::app(CommandName::Rewrite, &error))?;
    if result.record.reason == Some(ReasonCode::Cancelled) {
        return Err(RunFailure::cancelled(CommandName::Rewrite));
    }
    let backup = if request.inspection.dry_run {
        None
    } else {
        emit_destination(
            &destination,
            &request.source,
            &source,
            &result.output,
            request.raw_terminal,
            request.confirmed,
            CommandName::Rewrite,
        )?
    };
    if request.inspection.diff {
        inspect::write_diff(
            &inspect::SafeDiff::compare(&source, &result.output),
            crate::check::ReportTarget::Diagnostic,
            CommandName::Rewrite,
        )?;
    }
    if let Some(trace) = request.inspection.trace.as_ref() {
        inspect::write_trace(trace, &result.record, CommandName::Rewrite)?;
    }
    report::write(
        CommandName::Rewrite,
        &result.record,
        request.format,
        destination_report_target(&destination),
        backup.as_deref(),
    )
    .map_err(|_| RunFailure::operational(CommandName::Rewrite))?;
    Ok(crate::check::exit_status(
        result.record.status,
        result.record.reason,
        request.fail_on_abstain,
    ))
}

#[cfg(test)]
mod tests {
    use std::process::ExitCode;

    use rewrite_app::AppError;

    use crate::contract::{CommandName, EXIT_COMPATIBILITY};
    use crate::failure::RunFailure;

    #[test]
    fn missing_selection_is_a_compatibility_refusal() {
        let failure = RunFailure::app(CommandName::Rewrite, &AppError::GroundedUnavailable);
        assert_eq!(failure.command, CommandName::Rewrite);
        assert_eq!(failure.exit_code, ExitCode::from(EXIT_COMPATIBILITY));
        assert!(failure.message.contains("qualified local artifact"));
    }

    #[test]
    fn selected_artifact_without_runtime_is_a_compatibility_refusal() {
        let failure = RunFailure::app(CommandName::Rewrite, &AppError::GroundedRuntimeUnavailable);
        assert_eq!(failure.exit_code, ExitCode::from(EXIT_COMPATIBILITY));
        assert!(failure.message.contains("attached local runtime"));
    }

    #[test]
    fn requested_mismatch_is_a_compatibility_refusal() {
        let failure = RunFailure::app(CommandName::Rewrite, &AppError::GroundedSelectionMismatch);
        assert_eq!(failure.exit_code, ExitCode::from(EXIT_COMPATIBILITY));
        assert!(
            failure
                .message
                .contains("active qualified generation binding")
        );
    }
}
