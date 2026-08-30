use std::{
    fs::File,
    io::{Read as _, Write as _},
};

use rewrite_model::{ArtifactSetManifest, ArtifactSetRelativePath, RuntimePackageManifest};
use rewrite_model_store::WriteDisposition;
use rewrite_ollama_package::ReconstructedRuntimePackage;
use rewrite_types::{CancellationToken, Digest};
use sha2::{Digest as _, Sha256};

use crate::{
    ArtifactRepositorySetImportResult, ArtifactSetImportLimits, OllamaRuntimeImportError,
    OllamaRuntimeImportEvidence, OllamaRuntimeImportResult, PackageManifestWriteDisposition,
    RuntimeSourceBuildEvidenceBundleLease,
    artifact_set_import::{OfflineArtifactSetImportService, OwnedSourceStagingImportError},
    artifact_storage::OwnedStagingTree,
};

use super::{
    ArtifactRepository, ArtifactRepositoryError, MANAGED_STORAGE_DIRECTORY, finish_operation,
    map_data_directory_boundary_error,
};

const COPY_BUFFER_BYTES: usize = 1024 * 1024;

trait PrimaryRuntimeImportSource {
    fn revalidate(&self, cancellation: &CancellationToken) -> Result<(), OllamaRuntimeImportError>;

    fn attempts_are_byte_identical(&self) -> bool;

    fn primary(&self) -> &ReconstructedRuntimePackage;

    fn open_primary_member(
        &self,
        path: &ArtifactSetRelativePath,
        cancellation: &CancellationToken,
    ) -> Result<File, OllamaRuntimeImportError>;
}

impl PrimaryRuntimeImportSource for RuntimeSourceBuildEvidenceBundleLease {
    fn revalidate(&self, cancellation: &CancellationToken) -> Result<(), OllamaRuntimeImportError> {
        RuntimeSourceBuildEvidenceBundleLease::revalidate(self, cancellation).map_err(Into::into)
    }

    fn attempts_are_byte_identical(&self) -> bool {
        self.report().is_byte_identical()
    }

    fn primary(&self) -> &ReconstructedRuntimePackage {
        self.report().primary()
    }

    fn open_primary_member(
        &self,
        path: &ArtifactSetRelativePath,
        cancellation: &CancellationToken,
    ) -> Result<File, OllamaRuntimeImportError> {
        RuntimeSourceBuildEvidenceBundleLease::open_primary_member(self, path, cancellation)
            .map_err(Into::into)
    }
}

impl ArtifactRepository {
    /// Imports the primary package from one retained controlled source-build closure.
    ///
    /// Both independently verified attempts must be byte-identical. The operation
    /// derives the artifact set and semantic runtime package only from the verified
    /// primary report, copies only those declared package members from retained
    /// evidence handles into application-owned staging, publishes the exact managed
    /// set without replacement, and persists and reads back the package manifest.
    ///
    /// The result is inert structural evidence. This operation does not admit,
    /// qualify, activate, lease, launch, or authorize generation by the runtime.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactRepositoryError`] for a nonidentical build pair, evidence
    /// drift, cancellation, resource limits, managed-storage conflict, persistence
    /// failure, or disagreement during durable package readback.
    pub fn import_primary_runtime_source_build(
        &self,
        evidence: &RuntimeSourceBuildEvidenceBundleLease,
        limits: ArtifactSetImportLimits,
        cancellation: &CancellationToken,
    ) -> Result<OllamaRuntimeImportResult, ArtifactRepositoryError> {
        self.import_primary_runtime_source(evidence, limits, cancellation)
    }

    fn import_primary_runtime_source<S>(
        &self,
        source: &S,
        limits: ArtifactSetImportLimits,
        cancellation: &CancellationToken,
    ) -> Result<OllamaRuntimeImportResult, ArtifactRepositoryError>
    where
        S: PrimaryRuntimeImportSource,
    {
        ensure_active(cancellation)?;
        source.revalidate(cancellation)?;
        if !source.attempts_are_byte_identical() {
            return Err(OllamaRuntimeImportError::NonIdenticalSourceBuild.into());
        }
        let artifact_set = source.primary().artifact_set().clone();
        let runtime_package = source.primary().runtime_package().clone();
        crate::artifact_set_import::validate_manifest_before_repository_mutation(
            &artifact_set,
            limits,
        )
        .map_err(OllamaRuntimeImportError::from)?;
        ensure_runtime_binding(&artifact_set, &runtime_package)?;

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
                    std::ffi::OsStr::new(MANAGED_STORAGE_DIRECTORY),
                    &mut store,
                    limits,
                )
                .map_err(OllamaRuntimeImportError::from)?;
                source.revalidate(cancellation)?;
                let (staging, plan) = service
                    .create_owned_source_staging(&artifact_set, cancellation)
                    .map_err(OllamaRuntimeImportError::from)?;
                if let Err(error) =
                    copy_primary_members(source, &staging, &artifact_set, cancellation)
                {
                    return Err(cleanup_failed_staging(staging, error));
                }
                match service.import_owned_source_staging_with_prepublication(
                    &artifact_set,
                    &plan,
                    staging,
                    cancellation,
                    || source.revalidate(cancellation),
                ) {
                    Ok(result) => result,
                    Err(OwnedSourceStagingImportError::ArtifactSet(error)) => {
                        return Err(OllamaRuntimeImportError::from(error).into());
                    }
                    Err(OwnedSourceStagingImportError::BeforePublication(error)) => {
                        return Err(error.into());
                    }
                }
            };
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
            source.revalidate(cancellation)?;
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

fn copy_primary_members<S>(
    source: &S,
    staging: &OwnedStagingTree,
    manifest: &ArtifactSetManifest,
    cancellation: &CancellationToken,
) -> Result<(), OllamaRuntimeImportError>
where
    S: PrimaryRuntimeImportSource,
{
    for member in manifest.members() {
        ensure_active(cancellation)?;
        let opened = source.open_primary_member(member.relative_path(), cancellation)?;
        copy_exact_member(
            opened,
            staging,
            member.relative_path(),
            member.byte_size(),
            member.artifact_id().digest(),
            cancellation,
        )?;
    }
    Ok(())
}

fn copy_exact_member(
    mut source: File,
    staging: &OwnedStagingTree,
    path: &ArtifactSetRelativePath,
    expected_size: u64,
    expected_digest: &Digest,
    cancellation: &CancellationToken,
) -> Result<(), OllamaRuntimeImportError> {
    let mut destination = staging
        .create_file(path)
        .map_err(crate::artifact_set_import::map_managed_tree)
        .map_err(OllamaRuntimeImportError::from)?;
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    let mut observed_size = 0_u64;
    let mut hasher = Sha256::new();
    while observed_size < expected_size {
        ensure_active(cancellation)?;
        let amount = usize::try_from(expected_size - observed_size)
            .unwrap_or(usize::MAX)
            .min(buffer.len());
        let count = source
            .read(&mut buffer[..amount])
            .map_err(|_error| OllamaRuntimeImportError::SourceChanged)?;
        if count == 0 {
            return Err(OllamaRuntimeImportError::SourceChanged);
        }
        destination
            .file
            .write_all(&buffer[..count])
            .map_err(crate::ArtifactSetImportError::StorageIo)
            .map_err(OllamaRuntimeImportError::from)?;
        observed_size = observed_size
            .checked_add(u64::try_from(count).map_err(|_| OllamaRuntimeImportError::SourceChanged)?)
            .ok_or(OllamaRuntimeImportError::SourceChanged)?;
        hasher.update(&buffer[..count]);
    }
    let mut trailing = [0_u8; 1];
    if source
        .read(&mut trailing)
        .map_err(|_error| OllamaRuntimeImportError::SourceChanged)?
        != 0
        || destination
            .file
            .metadata()
            .map_err(crate::ArtifactSetImportError::StorageIo)
            .map_err(OllamaRuntimeImportError::from)?
            .len()
            != expected_size
    {
        return Err(OllamaRuntimeImportError::SourceChanged);
    }
    let digest = Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_| OllamaRuntimeImportError::SourceChanged)?;
    if &digest != expected_digest {
        return Err(OllamaRuntimeImportError::SourceChanged);
    }
    Ok(())
}

fn ensure_runtime_binding(
    artifact_set: &ArtifactSetManifest,
    runtime_package: &RuntimePackageManifest,
) -> Result<(), OllamaRuntimeImportError> {
    runtime_package
        .validate_against(artifact_set)
        .map_err(|_| OllamaRuntimeImportError::ReadbackConflict)
}

fn cleanup_failed_staging(
    staging: OwnedStagingTree,
    original: OllamaRuntimeImportError,
) -> ArtifactRepositoryError {
    match staging.cleanup() {
        Ok(()) => original.into(),
        Err(error) => OllamaRuntimeImportError::ArtifactSet(
            crate::artifact_set_import::map_managed_tree(error),
        )
        .into(),
    }
}

fn ensure_active(cancellation: &CancellationToken) -> Result<(), OllamaRuntimeImportError> {
    if cancellation.is_cancelled() {
        Err(OllamaRuntimeImportError::Cancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "runtime_source_build_import/tests.rs"]
mod tests;
