use std::{ffi::OsStr, io::Write as _};

use rewrite_model::{
    ArtifactSetRelativePath, CandidateGenerationEvidenceBundleReadbackV1,
    EVIDENCE_BUNDLE_MANIFEST_RELATIVE_PATH,
};
use rewrite_types::CancellationToken;

use super::contract::{
    CandidateGenerationEvidenceBundleDestination, CandidateGenerationEvidenceBundleError,
    digest_bytes, ensure_active, map_storage, reject_indirect_directory,
};
use super::plan::{CandidateGenerationEvidenceBundlePublicationPlan, PlannedBundleFile};
use super::verify::{
    CandidateGenerationEvidenceBundleReadbackLease, acquire, acquire_from_parent,
    verify_staged_root,
};
use crate::artifact_storage::{
    ExactEntryCapacity, ManagedTreeLimits, NoReplacePublicationFailure, OwnedStagingTree,
    PinnedDirectory,
};

/// Successful no-replace publication and fresh complete readback.
pub struct CandidateGenerationEvidenceBundlePublication {
    lease: CandidateGenerationEvidenceBundleReadbackLease,
}

impl CandidateGenerationEvidenceBundlePublication {
    /// Returns the inert readback record issued only after fresh verification.
    #[must_use]
    pub const fn readback(&self) -> &CandidateGenerationEvidenceBundleReadbackV1 {
        self.lease.readback()
    }

    /// Returns the retained readback lease.
    #[must_use]
    pub const fn lease(&self) -> &CandidateGenerationEvidenceBundleReadbackLease {
        &self.lease
    }

    /// Consumes the publication result into its retained readback lease.
    #[must_use]
    pub fn into_lease(self) -> CandidateGenerationEvidenceBundleReadbackLease {
        self.lease
    }
}

/// Atomic no-replace publisher for one exact compiled publication plan.
#[derive(Clone, Copy, Debug, Default)]
pub struct CandidateGenerationEvidenceBundlePublisher;

impl CandidateGenerationEvidenceBundlePublisher {
    /// Stages, verifies, synchronizes, publishes, freshly reopens, and rehashes a bundle.
    ///
    /// The committed destination is never deleted. Any error after the atomic
    /// no-replace commit is returned as `PublishedButReadbackFailed`.
    ///
    /// # Errors
    ///
    /// Returns [`CandidateGenerationEvidenceBundleError`] for cancellation,
    /// occupied or unsafe storage, staging drift, write or sync failure, no-replace
    /// failure, cleanup failure, or post-commit readback failure.
    pub fn publish(
        plan: CandidateGenerationEvidenceBundlePublicationPlan,
        destination: &CandidateGenerationEvidenceBundleDestination,
        cancellation: &CancellationToken,
    ) -> Result<CandidateGenerationEvidenceBundlePublication, CandidateGenerationEvidenceBundleError>
    {
        ensure_active(cancellation)?;
        require_absent_destination(destination.path())?;
        reject_indirect_directory(&destination.parent)?;
        let parent = PinnedDirectory::open_existing(&destination.parent).map_err(map_storage)?;
        let tree_limits =
            ManagedTreeLimits::new(plan.limits.maximum_tree_entries).map_err(map_storage)?;
        OwnedStagingTree::preflight_no_replace_publication_preserving_cleanup(
            &parent,
            &parent,
            tree_limits,
            plan.limits.maximum_staging_roots,
            plan.limits.maximum_destination_entries,
            cancellation,
        )
        .map_err(map_probe_failure)?;
        ensure_active(cancellation)?;
        let mut staging = OwnedStagingTree::create(
            &parent,
            tree_limits,
            plan.limits.maximum_staging_roots,
            cancellation,
        )
        .map_err(map_storage)?;
        if let Err(error) = write_plan(&mut staging, &plan, cancellation) {
            return fail_with_cleanup(staging, error);
        }
        if let Err(error) = staging.sync_bottom_up(cancellation).map_err(map_storage) {
            return fail_with_cleanup(staging, error);
        }
        if let Err(error) =
            verify_staged_root(staging.root(), &plan.manifest, plan.limits, cancellation)
        {
            return fail_with_cleanup(staging, error);
        }
        let synced = staging.into_synced().map_err(map_storage)?;
        if let Err(error) = ensure_active(cancellation) {
            return match synced.cleanup() {
                Ok(()) => Err(error),
                Err(cleanup) => Err(CandidateGenerationEvidenceBundleError::cleanup_after(
                    error,
                    map_storage(cleanup),
                )),
            };
        }
        let published = synced.publish_no_replace_preserving_cleanup(
            &parent,
            &destination.name,
            plan.limits.maximum_destination_entries,
            cancellation,
        );
        let published = match published {
            Ok(published) => published,
            Err(failure) => return Err(map_publication_failure(failure)),
        };
        drop(published);
        let lease = acquire(
            destination.path.clone(),
            &plan.manifest,
            None,
            plan.limits,
            cancellation,
        )
        .map_err(CandidateGenerationEvidenceBundleError::published_but_failed)?;
        drop(plan);
        Ok(CandidateGenerationEvidenceBundlePublication { lease })
    }

    pub(crate) fn publish_from_staging_into_parent(
        plan: CandidateGenerationEvidenceBundlePublicationPlan,
        staging_parent: &PinnedDirectory,
        destination_parent: &PinnedDirectory,
        destination_name: &OsStr,
        cancellation: &CancellationToken,
    ) -> Result<CandidateGenerationEvidenceBundlePublication, CandidateGenerationEvidenceBundleError>
    {
        ensure_active(cancellation)?;
        match destination_parent
            .exact_entry_capacity(
                destination_name,
                plan.limits.maximum_destination_entries,
                cancellation,
            )
            .map_err(map_storage)?
        {
            ExactEntryCapacity::Present => {
                return Err(CandidateGenerationEvidenceBundleError::DestinationExists);
            }
            ExactEntryCapacity::Available => {}
            ExactEntryCapacity::Full => {
                return Err(CandidateGenerationEvidenceBundleError::LimitExceeded);
            }
        }
        let tree_limits =
            ManagedTreeLimits::new(plan.limits.maximum_tree_entries).map_err(map_storage)?;
        OwnedStagingTree::preflight_no_replace_publication_preserving_cleanup(
            staging_parent,
            destination_parent,
            tree_limits,
            plan.limits.maximum_staging_roots,
            plan.limits.maximum_destination_entries,
            cancellation,
        )
        .map_err(map_probe_failure)?;
        ensure_active(cancellation)?;
        let mut staging = OwnedStagingTree::create(
            staging_parent,
            tree_limits,
            plan.limits.maximum_staging_roots,
            cancellation,
        )
        .map_err(map_storage)?;
        if let Err(error) = write_plan(&mut staging, &plan, cancellation) {
            return fail_with_cleanup(staging, error);
        }
        if let Err(error) = staging.sync_bottom_up(cancellation).map_err(map_storage) {
            return fail_with_cleanup(staging, error);
        }
        if let Err(error) =
            verify_staged_root(staging.root(), &plan.manifest, plan.limits, cancellation)
        {
            return fail_with_cleanup(staging, error);
        }
        let synced = staging.into_synced().map_err(map_storage)?;
        if let Err(error) = ensure_active(cancellation) {
            return match synced.cleanup() {
                Ok(()) => Err(error),
                Err(cleanup) => Err(CandidateGenerationEvidenceBundleError::cleanup_after(
                    error,
                    map_storage(cleanup),
                )),
            };
        }
        let published = synced.publish_no_replace_preserving_cleanup(
            destination_parent,
            destination_name,
            plan.limits.maximum_destination_entries,
            cancellation,
        );
        let published = match published {
            Ok(published) => published,
            Err(failure) => return Err(map_publication_failure(failure)),
        };
        drop(published);
        let lease = acquire_from_parent(
            destination_parent,
            destination_name,
            &plan.manifest,
            None,
            plan.limits,
            cancellation,
        )
        .map_err(CandidateGenerationEvidenceBundleError::published_but_failed)?;
        drop(plan);
        Ok(CandidateGenerationEvidenceBundlePublication { lease })
    }
}

fn write_plan(
    staging: &mut OwnedStagingTree,
    plan: &CandidateGenerationEvidenceBundlePublicationPlan,
    cancellation: &CancellationToken,
) -> Result<(), CandidateGenerationEvidenceBundleError> {
    for file in &plan.files {
        write_file(staging, file, cancellation)?;
    }
    let manifest = PlannedBundleFile {
        path: fixed_manifest_path(),
        artifact_id: rewrite_model::ArtifactId::from_digest(digest_bytes(
            &plan.manifest_bytes,
            cancellation,
        )?),
        bytes: plan.manifest_bytes.clone(),
    };
    write_file(staging, &manifest, cancellation)
}

fn write_file(
    staging: &mut OwnedStagingTree,
    planned: &PlannedBundleFile,
    cancellation: &CancellationToken,
) -> Result<(), CandidateGenerationEvidenceBundleError> {
    ensure_active(cancellation)?;
    ensure_parent(staging, &planned.path)?;
    if rewrite_model::ArtifactId::from_digest(digest_bytes(&planned.bytes, cancellation)?)
        != planned.artifact_id
    {
        return Err(CandidateGenerationEvidenceBundleError::PlanMismatch);
    }
    let mut destination = staging.create_file(&planned.path).map_err(map_storage)?;
    destination
        .file
        .write_all(&planned.bytes)
        .map_err(CandidateGenerationEvidenceBundleError::StorageIo)?;
    if destination
        .file
        .metadata()
        .map_err(CandidateGenerationEvidenceBundleError::StorageIo)?
        .len()
        != u64::try_from(planned.bytes.len())
            .map_err(|_| CandidateGenerationEvidenceBundleError::LimitExceeded)?
    {
        return Err(CandidateGenerationEvidenceBundleError::Changed);
    }
    Ok(())
}

fn ensure_parent(
    staging: &mut OwnedStagingTree,
    path: &ArtifactSetRelativePath,
) -> Result<(), CandidateGenerationEvidenceBundleError> {
    if let Some((parent, _)) = path.as_str().rsplit_once('/') {
        staging
            .ensure_directory(
                &ArtifactSetRelativePath::new(parent.to_owned())
                    .map_err(|_| CandidateGenerationEvidenceBundleError::PlanMismatch)?,
            )
            .map_err(map_storage)?;
    }
    Ok(())
}

fn fail_with_cleanup<T>(
    staging: OwnedStagingTree,
    primary: CandidateGenerationEvidenceBundleError,
) -> Result<T, CandidateGenerationEvidenceBundleError> {
    match staging.cleanup() {
        Ok(()) => Err(primary),
        Err(cleanup) => Err(CandidateGenerationEvidenceBundleError::cleanup_after(
            primary,
            map_storage(cleanup),
        )),
    }
}

fn map_publication_failure(
    failure: NoReplacePublicationFailure,
) -> CandidateGenerationEvidenceBundleError {
    let (primary, cleanup, committed) = failure.into_parts();
    let primary = map_storage(primary);
    if committed {
        CandidateGenerationEvidenceBundleError::published_but_failed(primary)
    } else if let Some(cleanup) = cleanup {
        CandidateGenerationEvidenceBundleError::cleanup_after(primary, map_storage(cleanup))
    } else {
        primary
    }
}

fn map_probe_failure(
    failure: NoReplacePublicationFailure,
) -> CandidateGenerationEvidenceBundleError {
    let (primary, cleanup, _probe_committed) = failure.into_parts();
    let primary = map_storage(primary);
    match cleanup {
        Some(cleanup) => {
            CandidateGenerationEvidenceBundleError::cleanup_after(primary, map_storage(cleanup))
        }
        None => primary,
    }
}

fn require_absent_destination(
    path: &std::path::Path,
) -> Result<(), CandidateGenerationEvidenceBundleError> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Err(CandidateGenerationEvidenceBundleError::DestinationExists),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(CandidateGenerationEvidenceBundleError::StorageIo(error)),
    }
}

fn fixed_manifest_path() -> ArtifactSetRelativePath {
    ArtifactSetRelativePath::new(EVIDENCE_BUNDLE_MANIFEST_RELATIVE_PATH.to_owned())
        .expect("fixed bundle manifest path is portable")
}
