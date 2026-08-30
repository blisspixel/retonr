use std::ffi::OsStr;

use rewrite_types::CancellationToken;

use super::{
    ArtifactInventoryError, NoReplacePublicationFailure, OwnedStagingTree, PinnedDirectory,
    PublicationLedger, SyncedStagingTree, ensure_not_cancelled, map_publish_error, platform,
    validate_single_component,
};

impl SyncedStagingTree {
    /// Returns the pinned synchronized root for exact domain verification.
    pub(crate) const fn root(&self) -> &PinnedDirectory {
        &self.tree.root
    }

    /// Publishes after one final snapshot and cancellation check, without replacement.
    pub(crate) fn publish_no_replace(
        self,
        destination_parent: &PinnedDirectory,
        destination_name: &OsStr,
        maximum_destination_entries: usize,
        cancellation: &CancellationToken,
    ) -> Result<PinnedDirectory, ArtifactInventoryError> {
        self.publish_no_replace_preserving_cleanup(
            destination_parent,
            destination_name,
            maximum_destination_entries,
            cancellation,
        )
        .map_err(NoReplacePublicationFailure::into_legacy_error)
    }

    /// Publishes without replacement while preserving primary and cleanup errors.
    pub(crate) fn publish_no_replace_preserving_cleanup(
        mut self,
        destination_parent: &PinnedDirectory,
        destination_name: &OsStr,
        maximum_destination_entries: usize,
        cancellation: &CancellationToken,
    ) -> Result<PinnedDirectory, NoReplacePublicationFailure> {
        let preflight = self.preflight_publish(
            destination_parent,
            destination_name,
            maximum_destination_entries,
            cancellation,
        );
        if let Err(error) = preflight {
            let cleanup = self.cleanup().err();
            return Err(NoReplacePublicationFailure::before_commit(error, cleanup));
        }
        if let Err(error) = ensure_not_cancelled(cancellation) {
            let cleanup = self.cleanup().err();
            return Err(NoReplacePublicationFailure::before_commit(error, cleanup));
        }
        let ledger = PublicationLedger::from_snapshot(
            self.snapshot
                .as_ref()
                .expect("synchronized staging retains its snapshot"),
        );
        drop(self.snapshot.take());
        self.tree.close_descendant_handles();
        if let Err(error) = platform::rename_directory_no_replace(
            &self.tree.parent.handle,
            &self.tree.name,
            &destination_parent.handle,
            destination_name,
        ) {
            let primary = map_publish_error(error);
            let cleanup = self.tree.cleanup_closed_ledger(&ledger).err();
            return Err(NoReplacePublicationFailure::before_commit(primary, cleanup));
        }
        let SyncedStagingTree { tree, snapshot: _ } = self;
        let OwnedStagingTree {
            parent,
            name: _,
            root,
            root_fingerprint,
            directories,
            files,
            limits: _,
            synced_snapshot: _,
        } = tree;
        drop(directories);
        drop(files);
        let published = destination_parent
            .open_direct_child_directory(destination_name)
            .map_err(NoReplacePublicationFailure::after_commit)?;
        if !published
            .fingerprint()
            .map_err(NoReplacePublicationFailure::after_commit)?
            .same_identity(&root_fingerprint)
        {
            return Err(NoReplacePublicationFailure::after_commit(
                ArtifactInventoryError::ConcurrentModification,
            ));
        }
        drop(root);
        parent
            .sync()
            .map_err(NoReplacePublicationFailure::after_commit)?;
        destination_parent
            .sync()
            .map_err(NoReplacePublicationFailure::after_commit)?;
        Ok(published)
    }

    fn preflight_publish(
        &self,
        destination_parent: &PinnedDirectory,
        destination_name: &OsStr,
        maximum_destination_entries: usize,
        cancellation: &CancellationToken,
    ) -> Result<(), ArtifactInventoryError> {
        validate_single_component(destination_name)?;
        if maximum_destination_entries == 0 {
            return Err(ArtifactInventoryError::InvalidLimits);
        }
        self.tree.verify_directory_bindings()?;
        let current = self.tree.enumerate(cancellation)?;
        if self.snapshot.as_ref() != Some(&current) {
            return Err(ArtifactInventoryError::ConcurrentModification);
        }
        let source_parent = self.tree.parent.fingerprint()?;
        let destination = destination_parent.fingerprint()?;
        let same_parent = source_parent.same_identity(&destination);
        source_parent.release();
        destination.release();
        match destination_parent.exact_entry_capacity(
            destination_name,
            maximum_destination_entries,
            cancellation,
        )? {
            crate::artifact_storage::ExactEntryCapacity::Available => Ok(()),
            crate::artifact_storage::ExactEntryCapacity::Present => {
                Err(ArtifactInventoryError::ConcurrentModification)
            }
            crate::artifact_storage::ExactEntryCapacity::Full if same_parent => Ok(()),
            crate::artifact_storage::ExactEntryCapacity::Full => {
                Err(ArtifactInventoryError::StorageEntryLimitExceeded)
            }
        }
    }
}
