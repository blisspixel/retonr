//! Grounded rewrite command with the shared document output policy.

use std::{path::PathBuf, process::ExitCode};

use rewrite_app::{GroundedRewriteRequest, GroundedRewriteSelection, MAX_CANDIDATE_CHECK_BYTES};
use rewrite_types::{CancellationToken, ReasonCode, RewriteMode};

use crate::{
    check::{emit_document, report, report_target, resolve_output_sink_for},
    contract::{ArtifactIdArgument, CommandName, ReportFormat, read_input_bounded},
    failure::RunFailure,
};

/// Owned arguments for one grounded rewrite invocation.
pub(crate) struct RewriteRequest {
    pub(crate) source: PathBuf,
    pub(crate) output: Option<PathBuf>,
    pub(crate) data_directory: Option<PathBuf>,
    pub(crate) artifact_id: Option<ArtifactIdArgument>,
    pub(crate) protected_terms: Vec<String>,
    pub(crate) format: ReportFormat,
}

/// Reads one source document and rewrites it only after a recovered fake binding attaches.
///
/// The command uses the same output-destination policy as `check` so an existing
/// file is never replaced. It attaches in-process fake-backend conformance when
/// a recovered generation binding names that backend. It does not start a
/// runtime, open a network path, or invent a production backend.
pub(crate) fn run(request: &RewriteRequest) -> Result<ExitCode, RunFailure> {
    if request.artifact_id.is_some() && request.data_directory.is_none() {
        return Err(RunFailure::usage_for(CommandName::Rewrite));
    }
    let sink = resolve_output_sink_for(request.output.as_deref(), CommandName::Rewrite)?;
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
                source,
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
    emit_document(&sink, &result.output, false, false, CommandName::Rewrite)?;
    report::write(
        CommandName::Rewrite,
        &result.record,
        request.format,
        report_target(&sink),
    )
    .map_err(|_| RunFailure::operational(CommandName::Rewrite))?;
    Ok(crate::check::exit_status(
        result.record.status,
        result.record.reason,
        false,
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
