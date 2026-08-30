use std::{
    fs::File,
    io::{Read as _, Write as _},
};

use rewrite_model::{ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath};
use rewrite_types::{CancellationToken, Digest};
use sha2::{Digest as _, Sha256};

use super::contract::{
    RUNTIME_SOURCE_BUILD_EVIDENCE_BUNDLE_MANIFEST_PATH,
    RuntimeSourceBuildEvidenceBundleDestination, RuntimeSourceBuildEvidenceBundleError,
    RuntimeSourceBuildEvidenceBundleLimits, ensure_active, map_storage, paths_overlap,
};
use super::publish_plan::{PlannedFile, PlannedSource, plan_files, validate_plan};
use super::verify::{RuntimeSourceBuildEvidenceBundleLease, acquire_published, verify_root};
use crate::{
    ExecutableRuntimeSourceBuildBundleLease, RuntimeSourceBuildExecution,
    RuntimeSourceBuildReportCompiler,
    artifact_storage::{ManagedTreeLimits, OwnedStagingTree, PinnedDirectory, is_indirect},
};

const COPY_BUFFER_BYTES: usize = 1024 * 1024;

/// No-replace publisher for a complete controlled-build evidence closure.
#[derive(Clone, Copy, Debug, Default)]
pub struct RuntimeSourceBuildEvidenceBundlePublisher;

impl RuntimeSourceBuildEvidenceBundlePublisher {
    /// Compiles, copies, verifies, synchronizes, and atomically publishes one closure.
    ///
    /// The final path must be absent. Publication copies the canonical frozen-input
    /// manifest and every input component, both complete output trees, app-owned
    /// portable output-tree, isolation, and stream evidence, and the derived
    /// two-attempt report. The staging tree is independently verified before a
    /// no-replace rename.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeSourceBuildEvidenceBundleError`] for invalid limits,
    /// overlapping boundaries, drift, cancellation, storage failure, report
    /// compilation failure, or failure to independently verify the staged closure.
    pub fn publish(
        bundle: &ExecutableRuntimeSourceBuildBundleLease,
        primary: &RuntimeSourceBuildExecution,
        rebuild: &RuntimeSourceBuildExecution,
        destination: &RuntimeSourceBuildEvidenceBundleDestination,
        limits: RuntimeSourceBuildEvidenceBundleLimits,
        cancellation: &CancellationToken,
    ) -> Result<RuntimeSourceBuildEvidenceBundleLease, RuntimeSourceBuildEvidenceBundleError> {
        let limits = limits.validate()?;
        ensure_active(cancellation)?;
        reject_overlaps(bundle, primary, rebuild, destination)?;
        require_absent_destination(&destination.path)?;
        reject_indirect_parent(&destination.parent)?;
        let parent = PinnedDirectory::open_existing(&destination.parent).map_err(map_storage)?;
        let tree_limits =
            ManagedTreeLimits::new(limits.maximum_tree_entries).map_err(map_storage)?;
        OwnedStagingTree::preflight_no_replace_publication(
            &parent,
            &parent,
            tree_limits,
            limits.maximum_staging_roots,
            limits.maximum_destination_entries,
            cancellation,
        )
        .map_err(map_storage)?;
        let compilation = RuntimeSourceBuildReportCompiler::compile(
            bundle,
            primary,
            rebuild,
            limits.report_compilation,
            cancellation,
        )?;
        let mut files = plan_files(bundle, primary, rebuild, &compilation, cancellation)?;
        validate_plan(&files, limits)?;
        bundle.revalidate_for_execution(cancellation)?;
        primary.revalidate_output(cancellation)?;
        rebuild.revalidate_output(cancellation)?;

        let mut staging = OwnedStagingTree::create(
            &parent,
            tree_limits,
            limits.maximum_staging_roots,
            cancellation,
        )
        .map_err(map_storage)?;
        let members = match copy_planned_files(&mut staging, &mut files, cancellation) {
            Ok(members) => members,
            Err(error) => return fail_with_cleanup(staging, error),
        };
        if let Err(error) = bundle.revalidate_for_execution(cancellation) {
            return fail_with_cleanup(staging, error.into());
        }
        if let Err(error) = primary.revalidate_output(cancellation) {
            return fail_with_cleanup(staging, error.into());
        }
        if let Err(error) = rebuild.revalidate_output(cancellation) {
            return fail_with_cleanup(staging, error.into());
        }
        let manifest = match ArtifactSetManifest::new(members) {
            Ok(manifest) => manifest,
            Err(error) => {
                return fail_with_cleanup(
                    staging,
                    RuntimeSourceBuildEvidenceBundleError::Manifest(error),
                );
            }
        };
        let manifest_path = ArtifactSetRelativePath::new(
            RUNTIME_SOURCE_BUILD_EVIDENCE_BUNDLE_MANIFEST_PATH.to_owned(),
        )
        .map_err(|_| RuntimeSourceBuildEvidenceBundleError::TreeMismatch)?;
        if let Err(error) = write_memory_file(
            &mut staging,
            &manifest_path,
            manifest.canonical_json().as_bytes(),
            cancellation,
        ) {
            return fail_with_cleanup(staging, error);
        }
        if let Err(error) = staging.sync_bottom_up(cancellation).map_err(map_storage) {
            return fail_with_cleanup(staging, error);
        }
        let synced = staging.into_synced().map_err(map_storage)?;
        if let Err(error) = verify_root(synced.root(), limits, cancellation) {
            return match synced.cleanup() {
                Ok(()) => Err(error),
                Err(cleanup) => Err(map_storage(cleanup)),
            };
        }
        let published = synced
            .publish_no_replace(
                &parent,
                &destination.name,
                limits.maximum_destination_entries,
                cancellation,
            )
            .map_err(map_storage)?;
        acquire_published(destination.path.clone(), published, limits, cancellation)
    }
}

fn copy_planned_files(
    staging: &mut OwnedStagingTree,
    files: &mut [PlannedFile],
    cancellation: &CancellationToken,
) -> Result<Vec<ArtifactSetMember>, RuntimeSourceBuildEvidenceBundleError> {
    files.sort_unstable_by(|left, right| {
        left.path
            .as_str()
            .as_bytes()
            .cmp(right.path.as_str().as_bytes())
    });
    let mut members = Vec::with_capacity(files.len());
    for planned in files {
        ensure_active(cancellation)?;
        ensure_parent(staging, &planned.path)?;
        let digest = copy_planned_file(staging, planned, cancellation)?;
        members.push(ArtifactSetMember::new(
            ArtifactId::from_digest(digest),
            planned.byte_size()?,
            planned.path.clone(),
        ));
    }
    Ok(members)
}

fn copy_planned_file(
    staging: &OwnedStagingTree,
    planned: &mut PlannedFile,
    cancellation: &CancellationToken,
) -> Result<Digest, RuntimeSourceBuildEvidenceBundleError> {
    let mut destination = staging.create_file(&planned.path).map_err(map_storage)?;
    let digest = match &mut planned.source {
        PlannedSource::Memory(bytes) => {
            ensure_active(cancellation)?;
            destination
                .file
                .write_all(bytes)
                .map_err(RuntimeSourceBuildEvidenceBundleError::StorageIo)?;
            Digest::sha256(bytes)
        }
        PlannedSource::Retained {
            file,
            byte_size,
            expected_digest,
        } => copy_retained(
            file,
            &mut destination.file,
            *byte_size,
            expected_digest.as_ref(),
            cancellation,
        )?,
    };
    if destination
        .file
        .metadata()
        .map_err(RuntimeSourceBuildEvidenceBundleError::StorageIo)?
        .len()
        != planned.byte_size()?
    {
        return Err(RuntimeSourceBuildEvidenceBundleError::Changed);
    }
    Ok(digest)
}

fn copy_retained(
    source: &mut File,
    destination: &mut File,
    expected_size: u64,
    expected_digest: Option<&Digest>,
    cancellation: &CancellationToken,
) -> Result<Digest, RuntimeSourceBuildEvidenceBundleError> {
    let mut hasher = Sha256::new();
    let mut observed = 0_u64;
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    while observed < expected_size {
        ensure_active(cancellation)?;
        let maximum = usize::try_from(expected_size - observed)
            .unwrap_or(usize::MAX)
            .min(buffer.len());
        let read = source
            .read(&mut buffer[..maximum])
            .map_err(RuntimeSourceBuildEvidenceBundleError::StorageIo)?;
        if read == 0 {
            return Err(RuntimeSourceBuildEvidenceBundleError::Changed);
        }
        destination
            .write_all(&buffer[..read])
            .map_err(RuntimeSourceBuildEvidenceBundleError::StorageIo)?;
        observed = observed
            .checked_add(
                u64::try_from(read)
                    .map_err(|_| RuntimeSourceBuildEvidenceBundleError::LimitExceeded)?,
            )
            .ok_or(RuntimeSourceBuildEvidenceBundleError::LimitExceeded)?;
        hasher.update(&buffer[..read]);
    }
    let mut trailing = [0_u8; 1];
    if source
        .read(&mut trailing)
        .map_err(RuntimeSourceBuildEvidenceBundleError::StorageIo)?
        != 0
    {
        return Err(RuntimeSourceBuildEvidenceBundleError::Changed);
    }
    let digest = Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_| RuntimeSourceBuildEvidenceBundleError::Changed)?;
    if expected_digest.is_some_and(|expected| expected != &digest) {
        Err(RuntimeSourceBuildEvidenceBundleError::Changed)
    } else {
        Ok(digest)
    }
}

fn write_memory_file(
    staging: &mut OwnedStagingTree,
    path: &ArtifactSetRelativePath,
    bytes: &[u8],
    cancellation: &CancellationToken,
) -> Result<(), RuntimeSourceBuildEvidenceBundleError> {
    ensure_active(cancellation)?;
    ensure_parent(staging, path)?;
    let mut destination = staging.create_file(path).map_err(map_storage)?;
    destination
        .file
        .write_all(bytes)
        .map_err(RuntimeSourceBuildEvidenceBundleError::StorageIo)
}

fn ensure_parent(
    staging: &mut OwnedStagingTree,
    path: &ArtifactSetRelativePath,
) -> Result<(), RuntimeSourceBuildEvidenceBundleError> {
    if let Some((parent, _)) = path.as_str().rsplit_once('/') {
        let parent = ArtifactSetRelativePath::new(parent.to_owned())
            .map_err(|_| RuntimeSourceBuildEvidenceBundleError::TreeMismatch)?;
        staging.ensure_directory(&parent).map_err(map_storage)?;
    }
    Ok(())
}

fn reject_overlaps(
    bundle: &ExecutableRuntimeSourceBuildBundleLease,
    primary: &RuntimeSourceBuildExecution,
    rebuild: &RuntimeSourceBuildExecution,
    destination: &RuntimeSourceBuildEvidenceBundleDestination,
) -> Result<(), RuntimeSourceBuildEvidenceBundleError> {
    if bundle.bundle().overlaps_path(&destination.path)
        || paths_overlap(primary.output.path(), &destination.path)
        || paths_overlap(rebuild.output.path(), &destination.path)
    {
        Err(RuntimeSourceBuildEvidenceBundleError::UnsafeBoundary)
    } else {
        Ok(())
    }
}

fn reject_indirect_parent(
    parent: &std::path::Path,
) -> Result<(), RuntimeSourceBuildEvidenceBundleError> {
    let metadata = std::fs::symlink_metadata(parent)
        .map_err(RuntimeSourceBuildEvidenceBundleError::StorageIo)?;
    if is_indirect(&metadata) || !metadata.is_dir() {
        Err(RuntimeSourceBuildEvidenceBundleError::UnsafeBoundary)
    } else {
        Ok(())
    }
}

fn require_absent_destination(
    path: &std::path::Path,
) -> Result<(), RuntimeSourceBuildEvidenceBundleError> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Err(RuntimeSourceBuildEvidenceBundleError::Changed),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(RuntimeSourceBuildEvidenceBundleError::StorageIo(error)),
    }
}

fn fail_with_cleanup<T>(
    staging: OwnedStagingTree,
    original: RuntimeSourceBuildEvidenceBundleError,
) -> Result<T, RuntimeSourceBuildEvidenceBundleError> {
    match staging.cleanup() {
        Ok(()) => Err(original),
        Err(cleanup) => Err(map_storage(cleanup)),
    }
}
