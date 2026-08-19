//! Read-only catalog of registered single-file installations.

use std::process::ExitCode;

use rewrite_app::ArtifactRepository;
use rewrite_types::CancellationToken;

use super::{ModelFailure, ModelOutput, ModelSuccess, inventory_limits};
use crate::contract::{ArtifactIdArgument, CommandName};

pub(super) fn list(
    repository: &ArtifactRepository,
    cancellation: &CancellationToken,
) -> Result<ModelSuccess, ModelFailure> {
    let report = repository
        .inventory(inventory_limits(), cancellation)
        .map_err(|error| ModelFailure::repository(CommandName::ModelList, &error))?;
    Ok(ModelSuccess {
        output: ModelOutput::list(report),
        exit_code: ExitCode::SUCCESS,
    })
}

pub(super) fn inspect(
    repository: &ArtifactRepository,
    artifact_id: &ArtifactIdArgument,
    cancellation: &CancellationToken,
) -> Result<ModelSuccess, ModelFailure> {
    let report = repository
        .inventory(inventory_limits(), cancellation)
        .map_err(|error| ModelFailure::repository(CommandName::ModelInspect, &error))?;
    let wanted = artifact_id.to_artifact_id();
    let entry = report
        .registered
        .into_iter()
        .find(|entry| entry.installation.artifact_id() == &wanted)
        .ok_or_else(|| ModelFailure::artifact_not_found(CommandName::ModelInspect))?;
    Ok(ModelSuccess {
        output: ModelOutput::inspect(entry),
        exit_code: ExitCode::SUCCESS,
    })
}
