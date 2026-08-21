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
        mut staging: OwnedStagingTree,
        cancellation: &CancellationToken,
    ) -> Result<ArtifactSetImportResult, ArtifactSetImportError> {
        if let Err(error) = super::ensure_not_cancelled(cancellation) {
            return fail_with_cleanup(staging, error);
        }
        let plan = match validate_manifest_and_limits(manifest, self.limits) {
            Ok(plan) => plan,
            Err(error) => return fail_with_cleanup(staging, error),
        };
        if &plan != expected_plan {
            return fail_with_cleanup(staging, ArtifactSetImportError::StorageChanged);
        }
        if let Err(error) = self.validate_storage_layout() {
            return fail_with_cleanup(staging, error);
        }
        let prior = match self.preload_state(manifest, &plan) {
            Ok(prior) => prior,
            Err(error) => return fail_with_cleanup(staging, error),
        };
        let final_name = OsString::from(&plan.storage_key);
        let existing_final = match self.open_final_root(&final_name, cancellation) {
            Ok(root) => root,
            Err(error) => return fail_with_cleanup(staging, error),
        };
        if prior.is_some() && existing_final.is_none() {
            return fail_with_cleanup(staging, ArtifactSetImportError::StateStorageMismatch);
        }
        let tree_limits = match ManagedTreeLimits::new(self.limits.maximum_tree_entries)
            .map_err(map_managed_tree)
        {
            Ok(limits) => limits,
            Err(error) => return fail_with_cleanup(staging, error),
        };
        let disposition = match (&prior, &existing_final) {
            (Some(_), Some(_)) => ArtifactSetImportDisposition::AlreadyPresent,
            (None, Some(_)) => ArtifactSetImportDisposition::RegisteredExisting,
            (None, None) => ArtifactSetImportDisposition::Imported,
            (Some(_), None) => unreachable!("missing managed root was rejected"),
        };
        if let Err(error) = staging
            .sync_bottom_up(cancellation)
            .map_err(map_managed_tree)
        {
            return fail_with_cleanup(staging, error);
        }
        let snapshot = match staging.enumerate(cancellation).map_err(map_managed_tree) {
            Ok(snapshot) => snapshot,
            Err(error) => return fail_with_cleanup(staging, error),
        };
        if let Err(error) = validate_staged_snapshot(&snapshot, manifest, &plan) {
            drop(snapshot);
            return fail_with_cleanup(staging, error);
        }
        drop(snapshot);
        if let Err(error) =
            verify_final_tree(staging.root(), manifest, &plan, tree_limits, cancellation)
        {
            return fail_with_cleanup(staging, error);
        }
        if let Err(error) = super::ensure_not_cancelled(cancellation) {
            return fail_with_cleanup(staging, error);
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
