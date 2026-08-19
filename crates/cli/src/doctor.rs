//! Read-only recovery inspection for the local CLI and optional repository.

use std::{path::PathBuf, process::ExitCode};

use rewrite_app::{
    ArtifactRepository, ArtifactRepositoryError, ArtifactRepositorySchemaInspection,
    ArtifactRepositorySchemaStatus,
};
use serde::Serialize;

use crate::contract::{CommandName, EXIT_COMPATIBILITY, EXIT_OPERATIONAL};
use crate::failure::RunFailure;
use crate::identity::ProductIdentity;
use crate::model::ModelOutput;

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
    let exit_code = match report.repository.status {
        RepositoryStatus::NotRequested
        | RepositoryStatus::NotInitialized
        | RepositoryStatus::Current => ExitCode::SUCCESS,
        RepositoryStatus::MigrationRequired | RepositoryStatus::Incompatible => {
            ExitCode::from(EXIT_COMPATIBILITY)
        }
    };
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
    Ok(RepositoryReport::from_inspection(inspection))
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
        format!(
            "{}network: {}\nrepository: {}\nfound_schema: {}\nrequired_schema: {}\n",
            self.identity.text(),
            self.network,
            self.repository.status.name(),
            self.repository
                .found_schema
                .map_or_else(|| "none".to_owned(), |value| value.to_string()),
            self.repository.required_schema
        )
    }
}

#[derive(Serialize)]
struct RepositoryReport {
    status: RepositoryStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    found_schema: Option<u32>,
    required_schema: u32,
}

impl RepositoryReport {
    fn not_requested(required_schema: u32) -> Self {
        Self {
            status: RepositoryStatus::NotRequested,
            found_schema: None,
            required_schema,
        }
    }

    fn from_inspection(inspection: ArtifactRepositorySchemaInspection) -> Self {
        Self {
            status: match inspection.status {
                ArtifactRepositorySchemaStatus::NotInitialized => RepositoryStatus::NotInitialized,
                ArtifactRepositorySchemaStatus::Current => RepositoryStatus::Current,
                ArtifactRepositorySchemaStatus::MigrationRequired => {
                    RepositoryStatus::MigrationRequired
                }
                ArtifactRepositorySchemaStatus::Incompatible => RepositoryStatus::Incompatible,
            },
            found_schema: inspection.found_schema,
            required_schema: inspection.required_schema,
        }
    }
}

#[derive(Clone, Copy, Serialize)]
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
    use super::RepositoryStatus;

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
}
