use rewrite_model_store::WriteDisposition;
use rewrite_types::CancellationToken;

use crate::{
    ArtifactRepositorySetImportResult, OllamaRuntimeImportError, OllamaRuntimeImportEvidence,
    OllamaRuntimeImportLimits, OllamaRuntimeImportResult, PackageManifestWriteDisposition,
    ReviewedOllamaRuntimeSource, artifact_set_import::OfflineArtifactSetImportService,
    reviewed_ollama_runtime_import::PinnedReviewedOllamaRuntime,
};

use super::{
    ArtifactRepository, ArtifactRepositoryError, MANAGED_STORAGE_DIRECTORY, finish_operation,
    map_data_directory_boundary_error,
};

impl ArtifactRepository {
    /// Imports one reviewed Ollama Linux runtime package as inert structural evidence.
    ///
    /// The caller supplies a reviewed layout file and a member tree whose regular
    /// files must equal the declared member set. The service reconstructs the
    /// canonical artifact set and runtime-package manifest, publishes the exact
    /// set, and persists and reads back the semantic package under schema 6. It
    /// does not execute members, qualify, activate, lease, or admit the package
    /// to the production cloud-disable allowlist.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactRepositoryError`] for unsafe or changed source paths,
    /// malformed or drifting package bytes, extra tree files, cancellation,
    /// resource ceilings, managed-storage conflicts, incompatible state, or
    /// persistence failure.
    pub fn import_reviewed_ollama_runtime(
        &self,
        selection: &ReviewedOllamaRuntimeSource,
        limits: OllamaRuntimeImportLimits,
        cancellation: &CancellationToken,
    ) -> Result<OllamaRuntimeImportResult, ArtifactRepositoryError> {
        let source = PinnedReviewedOllamaRuntime::open_and_reconstruct(
            selection,
            &limits.reconstruction,
            cancellation,
        )?;
        let artifact_set = source.reconstructed().artifact_set().clone();
        let runtime_package = source.reconstructed().runtime_package().clone();
        crate::artifact_set_import::validate_request_before_repository_mutation(
            &crate::OfflineArtifactSetImportRequest {
                source_root: selection.member_root().to_path_buf(),
                manifest: artifact_set.clone(),
            },
            limits.artifact_set,
        )
        .map_err(OllamaRuntimeImportError::from)?;
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
            let mut service = OfflineArtifactSetImportService::open_under(
                &guard.pinned,
                std::ffi::OsStr::new(MANAGED_STORAGE_DIRECTORY),
                &mut store,
                limits.artifact_set,
            )
            .map_err(OllamaRuntimeImportError::from)?;
            source.recheck()?;
            let set_result = service
                .import(
                    &crate::OfflineArtifactSetImportRequest {
                        source_root: selection.member_root().to_path_buf(),
                        manifest: artifact_set.clone(),
                    },
                    cancellation,
                    |_| {},
                )
                .map_err(OllamaRuntimeImportError::from)?;
            let package_disposition = store
                .put_runtime_package_manifest(&runtime_package)
                .map_err(OllamaRuntimeImportError::from)?;
            let readback = store
                .runtime_package_manifest(&runtime_package.runtime_package_manifest_id())
                .map_err(OllamaRuntimeImportError::from)?
                .ok_or(OllamaRuntimeImportError::ReadbackConflict)?;
            if readback != runtime_package {
                return Err(OllamaRuntimeImportError::ReadbackConflict.into());
            }
            let artifact_set_disposition = set_result.disposition;
            Ok(OllamaRuntimeImportResult {
                artifact_set_key: ArtifactRepositorySetImportResult::from(set_result).key,
                artifact_set_disposition,
                runtime_package_disposition: match package_disposition {
                    WriteDisposition::Inserted => PackageManifestWriteDisposition::Inserted,
                    WriteDisposition::AlreadyPresent => {
                        PackageManifestWriteDisposition::AlreadyPresent
                    }
                },
                evidence: OllamaRuntimeImportEvidence::new(artifact_set, readback),
            })
        })();
        finish_operation(result, guard.recheck())
    }
}
