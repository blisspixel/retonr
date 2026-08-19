//! Read-only recovery inspection for the local CLI and optional repository.

use std::{fmt::Write as _, path::PathBuf, process::ExitCode};

use rewrite_app::{
    ArtifactRepository, ArtifactRepositoryError, ArtifactRepositoryPendingOperations,
    ArtifactRepositorySchemaInspection, ArtifactRepositorySchemaStatus,
};
use rewrite_types::CancellationToken;
use serde::Serialize;

use crate::contract::{CommandName, EXIT_COMPATIBILITY, EXIT_OPERATIONAL, EXIT_RECOVERY_REQUIRED};
use crate::failure::RunFailure;
use crate::identity::ProductIdentity;
use crate::model::ModelOutput;

const MAXIMUM_STATE_ENTRIES: usize = 4_096;

/// Completes the read-only `doctor` command.
pub(crate) fn run(
    data_directory: Option<PathBuf>,
) -> Result<(CommandName, ModelOutput, ExitCode), RunFailure> {
    let identity = ProductIdentity::current();
    let repository = match data_directory {
        None => RepositoryReport::not_requested(identity.store_schema_version),
        Some(path) => inspect_repository(path)?,
    };
    let report = DoctorReport {
        identity,
        network: "denied",
        repository,
    };
    let exit_code = doctor_exit(
        report.repository.status,
        &report.repository.recovery_actions,
    );
    Ok((
        CommandName::Doctor,
        ModelOutput {
            value: serde_json::to_value(&report).expect("doctor report serializes"),
            text: report.text(),
            findings: false,
        },
        exit_code,
    ))
}

fn inspect_repository(path: PathBuf) -> Result<RepositoryReport, RunFailure> {
    let repository =
        ArtifactRepository::new(path).map_err(|_| RunFailure::operational(CommandName::Doctor))?;
    let inspection = repository
        .inspect_schema()
        .map_err(|error| map_inspect_error(&error))?;
    let mut report = RepositoryReport::from_inspection(inspection);
    if report.status == RepositoryStatus::Current {
        let cancellation = CancellationToken::new();
        let pending = repository
            .pending_operations(MAXIMUM_STATE_ENTRIES, &cancellation)
            .map_err(|error| map_inspect_error(&error))?;
        report.pending_operations = Some(PendingOperationsSummary::from_pending(&pending));
        report.active_generation = Some(active_generation(&repository)?);
        report.recovery_actions =
            recovery_actions(report.status, report.pending_operations.as_ref());
    }
    Ok(report)
}

fn active_generation(
    repository: &ArtifactRepository,
) -> Result<ActiveGenerationSummary, RunFailure> {
    match repository.active_generation_binding() {
        Ok(Some(binding)) => Ok(ActiveGenerationSummary {
            status: "present",
            artifact_id: Some(binding.artifact_id.digest().as_str().to_owned()),
        }),
        Ok(None) => Ok(ActiveGenerationSummary {
            status: "absent",
            artifact_id: None,
        }),
        Err(error) => Err(map_inspect_error(&error)),
    }
}

fn recovery_actions(
    status: RepositoryStatus,
    pending: Option<&PendingOperationsSummary>,
) -> Vec<&'static str> {
    match status {
        RepositoryStatus::MigrationRequired => vec!["model.migrate"],
        RepositoryStatus::Current => {
            let Some(pending) = pending else {
                return Vec::new();
            };
            let mut actions = Vec::new();
            if pending.artifact_removals != "0" {
                actions.push("model.recover_removal");
            }
            if pending.artifact_set_removals != "0" {
                actions.push("model.recover_set_removal");
            }
            actions
        }
        RepositoryStatus::NotRequested
        | RepositoryStatus::NotInitialized
        | RepositoryStatus::Incompatible => Vec::new(),
    }
}

fn doctor_exit(status: RepositoryStatus, recovery_actions: &[&str]) -> ExitCode {
    match status {
        RepositoryStatus::NotRequested | RepositoryStatus::NotInitialized => ExitCode::SUCCESS,
        RepositoryStatus::Current if recovery_actions.is_empty() => ExitCode::SUCCESS,
        RepositoryStatus::Current => ExitCode::from(EXIT_RECOVERY_REQUIRED),
        RepositoryStatus::MigrationRequired | RepositoryStatus::Incompatible => {
            ExitCode::from(EXIT_COMPATIBILITY)
        }
    }
}

fn map_inspect_error(error: &ArtifactRepositoryError) -> RunFailure {
    match error.kind() {
        rewrite_app::ArtifactRepositoryErrorKind::InUse => RunFailure {
            command: CommandName::Doctor,
            body: crate::contract::ErrorBody::new(
                crate::contract::ErrorCategory::Operational,
                crate::contract::ErrorCode::RepositoryInUse,
                true,
            ),
            exit_code: ExitCode::from(EXIT_OPERATIONAL),
            message: "artifact repository is in use",
        },
        _ => RunFailure::operational(CommandName::Doctor),
    }
}

#[derive(Serialize)]
struct DoctorReport {
    #[serde(flatten)]
    identity: ProductIdentity,
    network: &'static str,
    repository: RepositoryReport,
}

impl DoctorReport {
    fn text(&self) -> String {
        let mut text = format!(
            "{}network: {}\nrepository: {}\nfound_schema: {}\nrequired_schema: {}\n",
            self.identity.text(),
            self.network,
            self.repository.status.name(),
            self.repository
                .found_schema
                .map_or_else(|| "none".to_owned(), |value| value.to_string()),
            self.repository.required_schema
        );
        if let Some(pending) = &self.repository.pending_operations {
            writeln!(
                text,
                "pending_artifact_removals: {}\npending_artifact_set_removals: {}",
                pending.artifact_removals, pending.artifact_set_removals
            )
            .expect("writing to a String cannot fail");
        }
        if let Some(active) = &self.repository.active_generation {
            writeln!(text, "active_generation: {}", active.status)
                .expect("writing to a String cannot fail");
        }
        let actions = if self.repository.recovery_actions.is_empty() {
            "none".to_owned()
        } else {
            self.repository.recovery_actions.join(",")
        };
        writeln!(text, "recovery_actions: {actions}").expect("writing to a String cannot fail");
        text
    }
}

#[derive(Serialize)]
struct RepositoryReport {
    status: RepositoryStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    found_schema: Option<u32>,
    required_schema: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pending_operations: Option<PendingOperationsSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    active_generation: Option<ActiveGenerationSummary>,
    recovery_actions: Vec<&'static str>,
}

#[derive(Serialize)]
struct PendingOperationsSummary {
    artifact_removals: String,
    artifact_set_removals: String,
}

impl PendingOperationsSummary {
    fn from_pending(pending: &ArtifactRepositoryPendingOperations) -> Self {
        Self {
            artifact_removals: pending.artifact_removals.len().to_string(),
            artifact_set_removals: pending.artifact_set_removals.len().to_string(),
        }
    }
}

#[derive(Serialize)]
struct ActiveGenerationSummary {
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    artifact_id: Option<String>,
}

impl RepositoryReport {
    fn not_requested(required_schema: u32) -> Self {
        Self {
            status: RepositoryStatus::NotRequested,
            found_schema: None,
            required_schema,
            pending_operations: None,
            active_generation: None,
            recovery_actions: Vec::new(),
        }
    }

    fn from_inspection(inspection: ArtifactRepositorySchemaInspection) -> Self {
        let status = match inspection.status {
            ArtifactRepositorySchemaStatus::NotInitialized => RepositoryStatus::NotInitialized,
            ArtifactRepositorySchemaStatus::Current => RepositoryStatus::Current,
            ArtifactRepositorySchemaStatus::MigrationRequired => {
                RepositoryStatus::MigrationRequired
            }
            ArtifactRepositorySchemaStatus::Incompatible => RepositoryStatus::Incompatible,
        };
        Self {
            status,
            found_schema: inspection.found_schema,
            required_schema: inspection.required_schema,
            pending_operations: None,
            active_generation: None,
            recovery_actions: recovery_actions(status, None),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum RepositoryStatus {
    NotRequested,
    NotInitialized,
    Current,
    MigrationRequired,
    Incompatible,
}

impl RepositoryStatus {
    const fn name(self) -> &'static str {
        match self {
            Self::NotRequested => "not_requested",
            Self::NotInitialized => "not_initialized",
            Self::Current => "current",
            Self::MigrationRequired => "migration_required",
            Self::Incompatible => "incompatible",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PendingOperationsSummary, RepositoryStatus, recovery_actions};

    #[test]
    fn every_repository_status_has_a_stable_name() {
        assert_eq!(
            [
                RepositoryStatus::NotRequested,
                RepositoryStatus::NotInitialized,
                RepositoryStatus::Current,
                RepositoryStatus::MigrationRequired,
                RepositoryStatus::Incompatible,
            ]
            .map(RepositoryStatus::name),
            [
                "not_requested",
                "not_initialized",
                "current",
                "migration_required",
                "incompatible",
            ]
        );
    }

    #[test]
    fn recovery_actions_name_exact_follow_up_commands() {
        assert!(recovery_actions(RepositoryStatus::NotRequested, None).is_empty());
        assert!(recovery_actions(RepositoryStatus::NotInitialized, None).is_empty());
        assert!(recovery_actions(RepositoryStatus::Incompatible, None).is_empty());
        assert_eq!(
            recovery_actions(RepositoryStatus::MigrationRequired, None),
            ["model.migrate"]
        );
        assert!(
            recovery_actions(
                RepositoryStatus::Current,
                Some(&PendingOperationsSummary {
                    artifact_removals: "0".to_owned(),
                    artifact_set_removals: "0".to_owned(),
                })
            )
            .is_empty()
        );
        assert_eq!(
            recovery_actions(
                RepositoryStatus::Current,
                Some(&PendingOperationsSummary {
                    artifact_removals: "1".to_owned(),
                    artifact_set_removals: "2".to_owned(),
                })
            ),
            ["model.recover_removal", "model.recover_set_removal"]
        );
    }
}
