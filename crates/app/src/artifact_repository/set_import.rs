use rewrite_types::CancellationToken;

use crate::{
    ArtifactSetImportLimits, OfflineArtifactSetImportRequest,
    artifact_set_import::{
        OfflineArtifactSetImportService, validate_request_before_repository_mutation,
    },
};

use super::{
    ArtifactRepository, ArtifactRepositoryError, ArtifactRepositorySetImportResult,
    MANAGED_STORAGE_DIRECTORY, ensure_repository_not_cancelled, finish_operation,
    map_data_directory_boundary_error,
};

impl ArtifactRepository {
    /// Imports one exact caller-selected artifact-set directory into managed storage.
    ///
    /// The complete source tree must equal the supplied canonical manifest. A
    /// successful import records only inert structural installation state. It does
    /// not qualify, activate, lease, or execute any artifact.
    /// This operation can initialize a new repository, but it never migrates an
    /// existing repository schema implicitly.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactRepositoryError`] when request validation, exact source
    /// inspection, whole-tree publication, or durable state registration fails.
    pub fn import_set(
        &self,
        request: &OfflineArtifactSetImportRequest,
        limits: ArtifactSetImportLimits,
        cancellation: &CancellationToken,
    ) -> Result<ArtifactRepositorySetImportResult, ArtifactRepositoryError> {
        validate_request_before_repository_mutation(request, limits)
            .map_err(ArtifactRepositoryError::SetImport)?;
        ensure_repository_not_cancelled(cancellation)?;
        let mut guard = self.initialize_and_lock_data_directory()?;
        guard.recheck()?;
        let result = (|| {
            guard.pin_or_create_state_database()?;
            guard.recheck()?;
            let mut store = self.open_import_store()?;
            guard.recheck()?;
            guard
                .pinned
                .sync()
                .map_err(map_data_directory_boundary_error)?;
            let mut service = OfflineArtifactSetImportService::open_under(
                &guard.pinned,
                std::ffi::OsStr::new(MANAGED_STORAGE_DIRECTORY),
                &mut store,
                limits,
            )
            .map_err(ArtifactRepositoryError::SetImport)?;
            service
                .import(request, cancellation, |_| {})
                .map(ArtifactRepositorySetImportResult::from)
                .map_err(ArtifactRepositoryError::SetImport)
        })();
        finish_operation(result, guard.recheck())
    }
}
