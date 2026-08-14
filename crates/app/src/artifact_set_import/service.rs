#[cfg(test)]
use std::path::Path;
use std::{
    ffi::{OsStr, OsString},
    fs::{File, TryLockError},
};

use rewrite_model::ArtifactSetManifest;
use rewrite_model_store::{ArtifactStateStore, StoredArtifactSetInstallation};
use rewrite_types::CancellationToken;

use crate::artifact_storage::{
    ExactEntryCapacity, LIFECYCLE_LOCK_FILE, ManagedTreeLimits, OwnedStagingTree, PinnedDirectory,
    fingerprint_std_file,
};

use super::{
    ArtifactSetImportDisposition, ArtifactSetImportError, ArtifactSetImportLimits,
    ArtifactSetImportProgress, ArtifactSetImportResult, ArtifactSetImportStage,
    OfflineArtifactSetImportRequest, ValidatedSetPlan,
    boundary::{map_managed_tree, map_set_capacity, map_staging, map_storage_open},
    manifest::validate_manifest_and_limits,
    report_progress,
    source::{PinnedSourceTree, fail_with_cleanup, validate_limit_shape},
    verify::{copy_and_verify_source, validate_staged_snapshot, verify_final_tree},
};

const SETS_DIRECTORY: &str = "sets";
const SET_STAGING_DIRECTORY: &str = ".set-staging";
const MAX_STORAGE_LAYOUT_ENTRIES: usize = 16;
const MAX_REPOSITORY_LAYOUT_ENTRIES: usize = 4_096;

/// Non-destructive offline artifact-set import under the application repository.
pub(crate) struct OfflineArtifactSetImportService<'a> {
    root_parent: PinnedDirectory,
    root_name: OsString,
    root: PinnedDirectory,
    legacy_artifacts: PinnedDirectory,
    legacy_staging: PinnedDirectory,
    sets: PinnedDirectory,
    set_staging: PinnedDirectory,
    limits: ArtifactSetImportLimits,
    store: &'a mut ArtifactStateStore,
    lock: File,
}

impl<'a> OfflineArtifactSetImportService<'a> {
    #[cfg(test)]
    pub(crate) fn open(
        root: impl AsRef<Path>,
        store: &'a mut ArtifactStateStore,
        limits: ArtifactSetImportLimits,
    ) -> Result<Self, ArtifactSetImportError> {
        let root_path =
            std::path::absolute(root.as_ref()).map_err(ArtifactSetImportError::StorageIo)?;
        let parent_path = root_path
            .parent()
            .ok_or(ArtifactSetImportError::UnsafeStorageLayout)?;
        let root_name = root_path
            .file_name()
            .ok_or(ArtifactSetImportError::UnsafeStorageLayout)?;
        let parent = PinnedDirectory::open_existing(parent_path).map_err(map_storage_open)?;
        Self::open_under(&parent, root_name, store, limits)
    }

    pub(crate) fn open_under(
        parent: &PinnedDirectory,
        root_name: &OsStr,
        store: &'a mut ArtifactStateStore,
        limits: ArtifactSetImportLimits,
    ) -> Result<Self, ArtifactSetImportError> {
        validate_limit_shape(limits)?;
        let root = match parent
            .exact_entry_capacity(
                root_name,
                MAX_REPOSITORY_LAYOUT_ENTRIES,
                &CancellationToken::new(),
            )
            .map_err(map_storage_open)?
        {
            ExactEntryCapacity::Present => parent
                .open_child_directory(root_name)
                .map_err(map_storage_open)?,
            ExactEntryCapacity::Available => parent
                .create_child_directory_exclusive(root_name)
                .map_err(|error| match error {
                    crate::ArtifactInventoryError::StorageIo(ref source)
                        if source.kind() == std::io::ErrorKind::AlreadyExists =>
                    {
                        ArtifactSetImportError::StorageChanged
                    }
                    other => map_storage_open(other),
                })?,
            ExactEntryCapacity::Full => {
                return Err(ArtifactSetImportError::UnsafeStorageLayout);
            }
        };
        parent.sync().map_err(map_storage_open)?;
        let (lock, _) = root
            .open_or_create_lock_file(OsStr::new(LIFECYCLE_LOCK_FILE))
            .map_err(map_storage_open)?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => return Err(ArtifactSetImportError::StorageInUse),
            Err(TryLockError::Error(error)) => {
                return Err(ArtifactSetImportError::StorageIo(error));
            }
        }
        let legacy_staging = root
            .ensure_child_directory(OsStr::new(".staging"))
            .map_err(map_storage_open)?;
        let legacy_artifacts = root
            .ensure_child_directory(OsStr::new("artifacts"))
            .map_err(map_storage_open)?;
        let set_staging = root
            .ensure_child_directory(OsStr::new(SET_STAGING_DIRECTORY))
            .map_err(map_storage_open)?;
        let sets = root
            .ensure_child_directory(OsStr::new(SETS_DIRECTORY))
            .map_err(map_storage_open)?;
        root.sync().map_err(map_storage_open)?;
        let service = Self {
            root_parent: parent.duplicate().map_err(map_storage_open)?,
            root_name: root_name.to_owned(),
            root,
            legacy_artifacts,
            legacy_staging,
            sets,
            set_staging,
            limits,
            store,
            lock,
        };
        service.validate_storage_layout()?;
        Ok(service)
    }

    pub(crate) fn import<F>(
        &mut self,
        request: &OfflineArtifactSetImportRequest,
        cancellation: &CancellationToken,
        mut progress: F,
    ) -> Result<ArtifactSetImportResult, ArtifactSetImportError>
    where
        F: FnMut(ArtifactSetImportProgress),
    {
        ensure_not_cancelled(cancellation)?;
        let plan = validate_manifest_and_limits(&request.manifest, self.limits)?;
        self.validate_storage_layout()?;
        let prior = self.preload_state(&request.manifest, &plan)?;
        report_progress(
            &mut progress,
            ArtifactSetImportStage::InspectingSource,
            0,
            0,
            &request.manifest,
        );
        let source = PinnedSourceTree::open(&request.source_root)?;
        let tree_limits =
            ManagedTreeLimits::new(self.limits.maximum_tree_entries).map_err(map_managed_tree)?;
        let final_name = OsString::from(&plan.storage_key);
        let existing_final = self.open_final_root(&final_name, cancellation)?;
        if prior.is_some() && existing_final.is_none() {
            return Err(ArtifactSetImportError::StateStorageMismatch);
        }
        let disposition = match (&prior, &existing_final) {
            (Some(_), Some(_)) => ArtifactSetImportDisposition::AlreadyPresent,
            (None, Some(_)) => ArtifactSetImportDisposition::RegisteredExisting,
            (None, None) => ArtifactSetImportDisposition::Imported,
            (Some(_), None) => return Err(ArtifactSetImportError::StateStorageMismatch),
        };

        let final_root = match existing_final {
            Some(root) => {
                copy_and_verify_source(
                    &source.directory,
                    &request.manifest,
                    &plan,
                    tree_limits,
                    None,
                    cancellation,
                    &mut progress,
                )?;
                source.recheck()?;
                report_progress(
                    &mut progress,
                    ArtifactSetImportStage::Finalizing,
                    request.manifest.members().len(),
                    request.manifest.total_byte_size(),
                    &request.manifest,
                );
                ensure_not_cancelled(cancellation)?;
                root
            }
            None => self.stage_and_publish(
                &source,
                request,
                &plan,
                tree_limits,
                cancellation,
                &mut progress,
            )?,
        };

        verify_final_tree(
            &final_root,
            &request.manifest,
            &plan,
            tree_limits,
            &CancellationToken::new(),
        )?;
        self.recheck_final_root(&final_name, &final_root)?;
        self.validate_storage_layout()?;
        let state = self
            .store
            .put_artifact_set_installation(&request.manifest, &plan.installed)
            .map_err(ArtifactSetImportError::State)?;
        Ok(ArtifactSetImportResult {
            installed: plan.installed,
            state,
            disposition,
        })
    }

    fn stage_and_publish<F>(
        &self,
        source: &PinnedSourceTree,
        request: &OfflineArtifactSetImportRequest,
        plan: &ValidatedSetPlan,
        tree_limits: ManagedTreeLimits,
        cancellation: &CancellationToken,
        progress: &mut F,
    ) -> Result<PinnedDirectory, ArtifactSetImportError>
    where
        F: FnMut(ArtifactSetImportProgress),
    {
        let manifest = &request.manifest;
        let final_name = OsStr::new(&plan.storage_key);
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
        if let Err(error) = copy_and_verify_source(
            &source.directory,
            manifest,
            plan,
            tree_limits,
            Some(&staging),
            cancellation,
            progress,
        ) {
            return fail_with_cleanup(staging, error);
        }
        if let Err(error) = source.recheck() {
            return fail_with_cleanup(staging, error);
        }
        report_progress(
            progress,
            ArtifactSetImportStage::PublishingTree,
            manifest.members().len(),
            manifest.total_byte_size(),
            manifest,
        );
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
        if let Err(error) = validate_staged_snapshot(&snapshot, manifest, plan) {
            return fail_with_cleanup(staging, error);
        }
        drop(snapshot);
        let synced = staging.into_synced().map_err(map_managed_tree)?;
        if let Err(error) =
            verify_final_tree(synced.root(), manifest, plan, tree_limits, cancellation)
        {
            return match synced.cleanup() {
                Ok(()) => Err(error),
                Err(cleanup) => Err(map_managed_tree(cleanup)),
            };
        }
        report_progress(
            progress,
            ArtifactSetImportStage::Finalizing,
            manifest.members().len(),
            manifest.total_byte_size(),
            manifest,
        );
        synced
            .publish_no_replace(
                &self.sets,
                final_name,
                self.limits.maximum_storage_entries,
                cancellation,
            )
            .map_err(map_set_capacity)
    }

    fn preload_state(
        &self,
        manifest: &ArtifactSetManifest,
        plan: &ValidatedSetPlan,
    ) -> Result<Option<StoredArtifactSetInstallation>, ArtifactSetImportError> {
        let stored_manifest = self
            .store
            .artifact_set_manifest(&plan.artifact_set_id)
            .map_err(ArtifactSetImportError::State)?;
        if stored_manifest
            .as_ref()
            .is_some_and(|stored| stored != manifest)
        {
            return Err(ArtifactSetImportError::StateStorageMismatch);
        }
        let installation = self
            .store
            .artifact_set_installation(&plan.artifact_set_id)
            .map_err(ArtifactSetImportError::State)?;
        if installation
            .as_ref()
            .is_some_and(|stored| stored.installed != plan.installed)
        {
            return Err(ArtifactSetImportError::StateStorageMismatch);
        }
        Ok(installation)
    }

    fn open_final_root(
        &self,
        final_name: &OsStr,
        cancellation: &CancellationToken,
    ) -> Result<Option<PinnedDirectory>, ArtifactSetImportError> {
        match self
            .sets
            .exact_entry_capacity(
                final_name,
                self.limits.maximum_storage_entries,
                cancellation,
            )
            .map_err(map_set_capacity)?
        {
            ExactEntryCapacity::Present => self
                .sets
                .open_child_directory(final_name)
                .map(Some)
                .map_err(map_managed_tree),
            ExactEntryCapacity::Available => Ok(None),
            ExactEntryCapacity::Full => Err(ArtifactSetImportError::StorageEntryLimitExceeded),
        }
    }

    fn recheck_final_root(
        &self,
        final_name: &OsStr,
        held: &PinnedDirectory,
    ) -> Result<(), ArtifactSetImportError> {
        let exact_name = self
            .sets
            .exact_entry_capacity(
                final_name,
                self.limits.maximum_storage_entries,
                &CancellationToken::new(),
            )
            .map_err(map_set_capacity)?
            == ExactEntryCapacity::Present;
        let named = self
            .sets
            .child_directory_fingerprint(final_name)
            .map_err(map_managed_tree)?;
        let held = held.fingerprint().map_err(map_managed_tree)?;
        let sets = self.sets.fingerprint().map_err(map_managed_tree)?;
        if exact_name && held.same_identity(&named) && held.same_filesystem(&sets) {
            Ok(())
        } else {
            Err(ArtifactSetImportError::StorageChanged)
        }
    }

    fn validate_storage_layout(&self) -> Result<(), ArtifactSetImportError> {
        let root = self.root.fingerprint().map_err(map_storage_open)?;
        let root_parent = self.root_parent.fingerprint().map_err(map_storage_open)?;
        let named_root = self
            .root_parent
            .child_directory_fingerprint(&self.root_name)
            .map_err(map_storage_open)?;
        let exact_root_name = self
            .root_parent
            .exact_entry_capacity(
                &self.root_name,
                MAX_REPOSITORY_LAYOUT_ENTRIES,
                &CancellationToken::new(),
            )
            .map_err(map_storage_open)?
            == ExactEntryCapacity::Present;
        let lock_path = self
            .root
            .child_file_fingerprint(OsStr::new(LIFECYCLE_LOCK_FILE))
            .map_err(map_storage_open)?;
        let lock_handle = fingerprint_std_file(&self.lock).map_err(map_storage_open)?;
        if !root.same_identity(&named_root)
            || !root.same_filesystem(&root_parent)
            || !exact_root_name
            || lock_path != lock_handle
        {
            return Err(ArtifactSetImportError::StorageChanged);
        }
        self.check_child_directory("artifacts", &self.legacy_artifacts)?;
        self.check_child_directory(".staging", &self.legacy_staging)?;
        self.check_child_directory(SETS_DIRECTORY, &self.sets)?;
        self.check_child_directory(SET_STAGING_DIRECTORY, &self.set_staging)
    }

    fn check_child_directory(
        &self,
        name: &str,
        held: &PinnedDirectory,
    ) -> Result<(), ArtifactSetImportError> {
        let current = self
            .root
            .child_directory_fingerprint(OsStr::new(name))
            .map_err(map_storage_open)?;
        let held = held.fingerprint().map_err(map_storage_open)?;
        let root = self.root.fingerprint().map_err(map_storage_open)?;
        let exact_name = self
            .root
            .exact_entry_capacity(
                OsStr::new(name),
                MAX_STORAGE_LAYOUT_ENTRIES,
                &CancellationToken::new(),
            )
            .map_err(map_storage_open)?
            == ExactEntryCapacity::Present;
        if held == current && held.same_filesystem(&root) && exact_name {
            Ok(())
        } else {
            Err(ArtifactSetImportError::StorageChanged)
        }
    }
}

fn ensure_not_cancelled(cancellation: &CancellationToken) -> Result<(), ArtifactSetImportError> {
    if cancellation.is_cancelled() {
        Err(ArtifactSetImportError::Cancelled)
    } else {
        Ok(())
    }
}
