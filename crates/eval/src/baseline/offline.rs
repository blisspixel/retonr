use crate::EvaluationSuite;

use super::{BaselineDefinition, BaselineError, BaselineKind, BaselineReport, run_no_rewrite};

/// Maximum serialized baseline definition size accepted by the parser.
pub const MAX_BASELINE_DEFINITION_BYTES: usize = 64 * 1024;

/// Parses and validates one versioned baseline definition.
///
/// # Errors
///
/// Returns [`BaselineError`] for a size, JSON, version, identity, or
/// configuration violation.
pub fn parse_baseline_definition(input: &str) -> Result<BaselineDefinition, BaselineError> {
    if input.len() > MAX_BASELINE_DEFINITION_BYTES {
        return Err(BaselineError::TooLarge);
    }
    let definition: BaselineDefinition =
        serde_json::from_str(input).map_err(|_error| BaselineError::InvalidJson)?;
    definition.validate()?;
    Ok(definition)
}

/// Runs a parsed baseline against a suite without selecting a runtime.
///
/// Only [`BaselineKind::NoRewrite`] can execute offline. Generative kinds fail
/// closed because no inference backend is supplied.
///
/// # Errors
///
/// Returns [`BaselineError`] for an invalid definition or a generative kind.
pub fn run_offline_baseline(
    definition: &BaselineDefinition,
    suite: &EvaluationSuite,
) -> Result<BaselineReport, BaselineError> {
    definition.validate()?;
    match definition.kind {
        BaselineKind::NoRewrite => Ok(run_no_rewrite(definition, suite)),
        BaselineKind::DirectPrompt
        | BaselineKind::StyleDescription
        | BaselineKind::RetrievedExamples => Err(BaselineError::MissingBackend),
    }
}
