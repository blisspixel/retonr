use rewrite_model_store::{ArtifactStateStore, StoreError, required_store_schema_version};

use super::{ArtifactRepository, ArtifactRepositoryError};

/// Read-only schema observation for one explicit data directory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactRepositorySchemaStatus {
    /// No initialized repository exists at the selected root.
    NotInitialized,
    /// The existing repository uses this build's exact schema.
    Current,
    /// A supported older schema requires an explicit confirmed migration.
    MigrationRequired,
    /// The existing schema is future, corrupt, or otherwise unusable.
    Incompatible,
}

/// Content-free schema inspection result for recovery commands.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactRepositorySchemaInspection {
    /// Observed repository schema status.
    pub status: ArtifactRepositorySchemaStatus,
    /// Schema found in the existing database, when one could be read.
    pub found_schema: Option<u32>,
    /// Schema required by this build.
    pub required_schema: u32,
}

impl ArtifactRepository {
    /// Returns the exact store schema required by this build.
    #[must_use]
    pub const fn required_schema_version() -> u32 {
        required_store_schema_version()
    }

    /// Inspects existing repository schema without creating, locking, or migrating.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactRepositoryError`] when the data directory is unsafe or a
    /// filesystem or state-read operation fails operationally.
    pub fn inspect_schema(
        &self,
    ) -> Result<ArtifactRepositorySchemaInspection, ArtifactRepositoryError> {
        let required_schema = Self::required_schema_version();
        match self.require_data_directory() {
            Err(ArtifactRepositoryError::NotInitialized) => {
                return Ok(ArtifactRepositorySchemaInspection {
                    status: ArtifactRepositorySchemaStatus::NotInitialized,
                    found_schema: None,
                    required_schema,
                });
            }
            Err(error) => return Err(error),
            Ok(()) => {}
        }
        match ArtifactStateStore::inspect_existing_schema(&self.state_database()) {
            Ok(status) => Ok(ArtifactRepositorySchemaInspection {
                status: if status.migration_required() {
                    ArtifactRepositorySchemaStatus::MigrationRequired
                } else {
                    ArtifactRepositorySchemaStatus::Current
                },
                found_schema: Some(status.found),
                required_schema: status.current,
            }),
            Err(StoreError::NotInitialized) => Ok(ArtifactRepositorySchemaInspection {
                status: ArtifactRepositorySchemaStatus::NotInitialized,
                found_schema: None,
                required_schema,
            }),
            Err(StoreError::MigrationRequired { found, current }) => {
                Ok(ArtifactRepositorySchemaInspection {
                    status: ArtifactRepositorySchemaStatus::MigrationRequired,
                    found_schema: u32::try_from(found).ok(),
                    required_schema: u32::try_from(current).unwrap_or(required_schema),
                })
            }
            Err(StoreError::UnsupportedSchema(found)) => Ok(ArtifactRepositorySchemaInspection {
                status: ArtifactRepositorySchemaStatus::Incompatible,
                found_schema: u32::try_from(found).ok(),
                required_schema,
            }),
            Err(StoreError::CorruptRecord) => Ok(ArtifactRepositorySchemaInspection {
                status: ArtifactRepositorySchemaStatus::Incompatible,
                found_schema: None,
                required_schema,
            }),
            Err(error) => Err(ArtifactRepositoryError::State(error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        ArtifactRepository, ArtifactRepositorySchemaInspection, ArtifactRepositorySchemaStatus,
    };

    #[test]
    fn missing_data_directory_is_not_initialized() {
        let directory = tempdir().expect("temporary directory");
        let repository = ArtifactRepository::new(directory.path().join("missing"))
            .expect("derive repository path");
        assert_eq!(
            repository.inspect_schema().expect("inspect absent root"),
            ArtifactRepositorySchemaInspection {
                status: ArtifactRepositorySchemaStatus::NotInitialized,
                found_schema: None,
                required_schema: ArtifactRepository::required_schema_version(),
            }
        );
    }
}
