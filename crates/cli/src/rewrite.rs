//! Grounded rewrite command with the shared document output policy.

use std::{path::PathBuf, process::ExitCode};

use rewrite_app::{GroundedRewriteSelection, MAX_CANDIDATE_CHECK_BYTES};

use crate::{
    check::resolve_output_sink_for,
    contract::{ArtifactIdArgument, CommandName, read_input_bounded},
    failure::RunFailure,
};

/// Owned arguments for one grounded rewrite invocation.
pub(crate) struct RewriteRequest {
    pub(crate) source: PathBuf,
    pub(crate) output: Option<PathBuf>,
    pub(crate) data_directory: Option<PathBuf>,
    pub(crate) artifact_id: Option<ArtifactIdArgument>,
}

/// Reads one source document and fails closed without a ready local generation path.
///
/// The command uses the same output-destination policy as `check` so an existing
/// file is never replaced. It does not start a runtime, open a network path, or
/// invent a production backend.
pub(crate) fn run(request: &RewriteRequest) -> Result<ExitCode, RunFailure> {
    if request.artifact_id.is_some() && request.data_directory.is_none() {
        return Err(RunFailure::usage_for(CommandName::Rewrite));
    }
    let _sink = resolve_output_sink_for(request.output.as_deref(), CommandName::Rewrite)?;
    let source = read_input_bounded(&request.source, MAX_CANDIDATE_CHECK_BYTES)
        .map_err(|error| RunFailure::input_read(CommandName::Rewrite, &error))?;
    GroundedRewriteSelection::validate_source(&source)
        .map_err(|error| RunFailure::app(CommandName::Rewrite, &error))?;
    let requested = request
        .artifact_id
        .as_ref()
        .map(ArtifactIdArgument::to_artifact_id);
    GroundedRewriteSelection::require_ready(request.data_directory.as_deref(), requested.as_ref())
        .map_err(|error| RunFailure::app(CommandName::Rewrite, &error))?;
    Ok(ExitCode::SUCCESS)
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
