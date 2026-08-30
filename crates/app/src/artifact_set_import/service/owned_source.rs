use std::ffi::OsString;

use rewrite_model::ArtifactSetManifest;
use rewrite_types::CancellationToken;

use crate::artifact_storage::{ManagedTreeLimits, OwnedStagingTree};

use super::OfflineArtifactSetImportService;
use crate::artifact_set_import::{
    ArtifactSetImportDisposition, ArtifactSetImportError, ArtifactSetImportResult,
    ValidatedSetPlan,
    boundary::{map_managed_tree, map_set_capacity, map_staging},
    manifest::validate_manifest_and_limits,
    source::fail_with_cleanup,
    verify::{validate_staged_snapshot, verify_final_tree},
};

pub(crate) enum OwnedSourceStagingImportError<E> {
    ArtifactSet(ArtifactSetImportError),
    BeforePublication(E),
}

impl<E> From<ArtifactSetImportError> for OwnedSourceStagingImportError<E> {
    fn from(error: ArtifactSetImportError) -> Self {
        Self::ArtifactSet(error)
    }
}

impl OfflineArtifactSetImportService<'_> {
    pub(crate) fn create_owned_source_staging(
        &self,
        manifest: &ArtifactSetManifest,
        cancellation: &CancellationToken,
    ) -> Result<(OwnedStagingTree, ValidatedSetPlan), ArtifactSetImportError> {
        super::ensure_not_cancelled(cancellation)?;
        let plan = validate_manifest_and_limits(manifest, self.limits)?;
        let tree_limits =
            ManagedTreeLimits::new(self.limits.maximum_tree_entries).map_err(map_managed_tree)?;
        let mut staging = OwnedStagingTree::create(
            &self.set_staging,
            tree_limits,
            self.limits.maximum_staging_entries,
            cancellation,
        )
        .map_err(map_staging)?;
        for directory in &plan.directories {
            if let Err(error) = staging
                .ensure_directory(directory)
                .map_err(map_managed_tree)
            {
                return fail_with_cleanup(staging, error);
            }
        }
        Ok((staging, plan))
    }

    pub(crate) fn import_owned_source_staging(
        &mut self,
        manifest: &ArtifactSetManifest,
        expected_plan: &ValidatedSetPlan,
        staging: OwnedStagingTree,
        cancellation: &CancellationToken,
    ) -> Result<ArtifactSetImportResult, ArtifactSetImportError> {
        match self.import_owned_source_staging_with_prepublication(
            manifest,
            expected_plan,
            staging,
            cancellation,
            || Ok::<(), std::convert::Infallible>(()),
        ) {
            Ok(result) => Ok(result),
            Err(OwnedSourceStagingImportError::ArtifactSet(error)) => Err(error),
            Err(OwnedSourceStagingImportError::BeforePublication(never)) => match never {},
        }
    }

    /// Imports application-owned staging after one final caller recheck.
    ///
    /// The callback runs after staged-byte synchronization and verification and
    /// immediately before the service hands a new tree to no-replace publication.
    /// Callback failure cleans the unpublished staging tree. No callback runs
    /// when an exact managed tree already exists because that branch performs no
    /// publication.
    pub(crate) fn import_owned_source_staging_with_prepublication<E, F>(
        &mut self,
        manifest: &ArtifactSetManifest,
        expected_plan: &ValidatedSetPlan,
        mut staging: OwnedStagingTree,
        cancellation: &CancellationToken,
        mut before_publication: F,
    ) -> Result<ArtifactSetImportResult, OwnedSourceStagingImportError<E>>
    where
        F: FnMut() -> Result<(), E>,
    {
        if let Err(error) = super::ensure_not_cancelled(cancellation) {
            return fail_with_cleanup(staging, error).map_err(Into::into);
        }
        let plan = match validate_manifest_and_limits(manifest, self.limits) {
            Ok(plan) => plan,
            Err(error) => return fail_with_cleanup(staging, error).map_err(Into::into),
        };
        if &plan != expected_plan {
            return fail_with_cleanup(staging, ArtifactSetImportError::StorageChanged)
                .map_err(Into::into);
        }
        if let Err(error) = self.validate_storage_layout() {
            return fail_with_cleanup(staging, error).map_err(Into::into);
        }
        let prior = match self.preload_state(manifest, &plan) {
            Ok(prior) => prior,
            Err(error) => return fail_with_cleanup(staging, error).map_err(Into::into),
        };
        let final_name = OsString::from(&plan.storage_key);
        let existing_final = match self.open_final_root(&final_name, cancellation) {
            Ok(root) => root,
            Err(error) => return fail_with_cleanup(staging, error).map_err(Into::into),
        };
        if prior.is_some() && existing_final.is_none() {
            return fail_with_cleanup(staging, ArtifactSetImportError::StateStorageMismatch)
                .map_err(Into::into);
        }
        let tree_limits = match ManagedTreeLimits::new(self.limits.maximum_tree_entries)
            .map_err(map_managed_tree)
        {
            Ok(limits) => limits,
            Err(error) => return fail_with_cleanup(staging, error).map_err(Into::into),
        };
        let disposition = match (&prior, &existing_final) {
            (Some(_), Some(_)) => ArtifactSetImportDisposition::AlreadyPresent,
            (None, Some(_)) => ArtifactSetImportDisposition::RegisteredExisting,
            (None, None) => ArtifactSetImportDisposition::Imported,
            (Some(_), None) => unreachable!("missing managed root was rejected"),
        };
        if let Err(error) =
            verify_owned_staging(&mut staging, manifest, &plan, tree_limits, cancellation)
        {
            return fail_with_cleanup(staging, error).map_err(Into::into);
        }
        if existing_final.is_none()
            && let Err(error) = before_publication()
        {
            return match staging.cleanup() {
                Ok(()) => Err(OwnedSourceStagingImportError::BeforePublication(error)),
                Err(cleanup) => Err(OwnedSourceStagingImportError::ArtifactSet(
                    map_managed_tree(cleanup),
                )),
            };
        }
        let final_root = match existing_final {
            Some(root) => {
                staging.cleanup().map_err(map_managed_tree)?;
                root
            }
            None => staging
                .into_synced()
                .map_err(map_managed_tree)?
                .publish_no_replace(
                    &self.sets,
                    &final_name,
                    self.limits.maximum_storage_entries,
                    cancellation,
                )
                .map_err(map_set_capacity)?,
        };
        verify_final_tree(
            &final_root,
            manifest,
            &plan,
            tree_limits,
            &CancellationToken::new(),
        )?;
        self.recheck_final_root(&final_name, &final_root)?;
        self.validate_storage_layout()?;
        let state = self
            .store
            .put_artifact_set_installation(manifest, &plan.installed)
            .map_err(ArtifactSetImportError::State)?;
        Ok(ArtifactSetImportResult {
            installed: plan.installed,
            state,
            disposition,
        })
    }
}

fn verify_owned_staging(
    staging: &mut OwnedStagingTree,
    manifest: &ArtifactSetManifest,
    plan: &ValidatedSetPlan,
    tree_limits: ManagedTreeLimits,
    cancellation: &CancellationToken,
) -> Result<(), ArtifactSetImportError> {
    staging
        .sync_bottom_up(cancellation)
        .map_err(map_managed_tree)?;
    let snapshot = staging.enumerate(cancellation).map_err(map_managed_tree)?;
    validate_staged_snapshot(&snapshot, manifest, plan)?;
    drop(snapshot);
    verify_final_tree(staging.root(), manifest, plan, tree_limits, cancellation)?;
    super::ensure_not_cancelled(cancellation)
}
