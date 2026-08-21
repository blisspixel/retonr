use std::ffi::OsStr;

use rewrite_model_store::WriteDisposition;
use rewrite_types::CancellationToken;

use crate::{
    ArtifactRepositorySetImportResult, InstalledOllamaModelSource, OllamaModelImportError,
    OllamaModelImportEvidence, OllamaModelImportLimits, OllamaModelImportResult,
    PackageManifestWriteDisposition, artifact_set_import::OfflineArtifactSetImportService,
    installed_ollama_import::PinnedInstalledOllamaModel,
};

use super::{
    ArtifactRepository, ArtifactRepositoryError, MANAGED_STORAGE_DIRECTORY, finish_operation,
    map_data_directory_boundary_error,
};

impl ArtifactRepository {
    /// Imports one exact installed Ollama model as inert structural evidence.
    ///
    /// The caller supplies only a validated models root and logical model
    /// reference. This method derives the fixed manifest and content-addressed
    /// blob paths, pins every source boundary without following links, rebuilds
    /// a canonical six-member package in application-owned staging, publishes
    /// the exact artifact set, and persists and reads back the semantic package
    /// manifest. It does not qualify, activate, lease, load, or execute a model.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactRepositoryError`] for unsafe or changed source paths,
    /// malformed or drifting package bytes, cancellation, resource ceilings,
    /// managed-storage conflicts, incompatible state, or persistence failure.
    pub fn import_installed_ollama_model(
        &self,
        selection: &InstalledOllamaModelSource,
        limits: OllamaModelImportLimits,
        cancellation: &CancellationToken,
    ) -> Result<OllamaModelImportResult, ArtifactRepositoryError> {
        let mut source = PinnedInstalledOllamaModel::open_and_reconstruct(
            selection,
            &limits.reconstruction,
            cancellation,
        )?;
        let artifact_set = source.reconstructed().artifact_set().clone();
        let model_package = source.reconstructed().model_package().clone();
        let rootfs_comparison = source.reconstructed().rootfs_comparison().clone();
        crate::artifact_set_import::validate_request_before_repository_mutation(
            &crate::OfflineArtifactSetImportRequest {
                source_root: selection.models_root().to_path_buf(),
                manifest: artifact_set.clone(),
            },
            limits.artifact_set,
        )
        .map_err(OllamaModelImportError::from)?;
        source.recheck()?;

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
            let set_result = {
                let mut service = OfflineArtifactSetImportService::open_under(
                    &guard.pinned,
                    OsStr::new(MANAGED_STORAGE_DIRECTORY),
                    &mut store,
                    limits.artifact_set,
                )
                .map_err(OllamaModelImportError::from)?;
                let (staging, plan) = service
                    .create_owned_source_staging(&artifact_set, cancellation)
                    .map_err(OllamaModelImportError::from)?;
                if let Err(error) = source.copy_into_staging(&staging, &artifact_set, cancellation)
                {
                    return Err(cleanup_failed_staging(staging, error));
                }
                source.recheck()?;
                service
                    .import_owned_source_staging(&artifact_set, &plan, staging, cancellation)
                    .map_err(OllamaModelImportError::from)?
            };
            let package_disposition = store
                .put_model_package_manifest(&model_package)
                .map_err(OllamaModelImportError::from)?;
            let readback = store
                .model_package_manifest(&model_package.model_package_manifest_id())
                .map_err(OllamaModelImportError::from)?
                .ok_or(OllamaModelImportError::ReadbackConflict)?;
            if readback != model_package {
                return Err(OllamaModelImportError::ReadbackConflict.into());
            }
            let artifact_set_disposition = set_result.disposition;
            Ok(OllamaModelImportResult {
                artifact_set_key: ArtifactRepositorySetImportResult::from(set_result).key,
                artifact_set_disposition,
                model_package_disposition: match package_disposition {
                    WriteDisposition::Inserted => PackageManifestWriteDisposition::Inserted,
                    WriteDisposition::AlreadyPresent => {
                        PackageManifestWriteDisposition::AlreadyPresent
                    }
                },
                evidence: OllamaModelImportEvidence::new(artifact_set, readback, rootfs_comparison),
            })
        })();
        finish_operation(result, guard.recheck())
    }
}

fn cleanup_failed_staging(
    staging: crate::artifact_storage::OwnedStagingTree,
    original: OllamaModelImportError,
) -> ArtifactRepositoryError {
    match staging.cleanup() {
        Ok(()) => original.into(),
        Err(error) => {
            OllamaModelImportError::ArtifactSet(crate::artifact_set_import::map_managed_tree(error))
                .into()
        }
    }
}
