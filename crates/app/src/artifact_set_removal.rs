use std::{
    ffi::{OsStr, OsString},
    path::Path,
};

use rewrite_model::ArtifactSetManifest;
#[cfg(test)]
use rewrite_model_store::StoreError;
use rewrite_model_store::{
    ArtifactRemovalPhase, ArtifactStateStore, RemovalPreparationDisposition,
    StoredArtifactSetInstallation,
};
use rewrite_types::CancellationToken;

mod contract;
mod map;

use crate::artifact_set_import::{
    ArtifactSetPlanBounds, SETS_DIRECTORY, ValidatedSetPlan, plan_artifact_set,
    validate_plan_bounds, verify_final_tree,
};
use crate::artifact_storage::{
    ExactEntryCapacity, ExistingArtifactStorage, LifecycleLockMode, ManagedTreeLimits,
    PinnedDirectory, remove_verified_managed_tree,
};
use crate::{ArtifactInventoryError, ArtifactRemovalDisposition};
pub use contract::{
    ArtifactSetRemovalError, ArtifactSetRemovalLimits, ArtifactSetRemovalProgress,
    ArtifactSetRemovalRecoveryError, ArtifactSetRemovalRequest, ArtifactSetRemovalResult,
    ArtifactSetRemovalStage,
};
use map::{
    ensure_not_cancelled, map_open_error, map_plan_error, map_prepared_error,
    map_recovery_storage_error, map_storage_error, map_store_error, map_verify_error,
    report_progress, result,
};

/// Existing-only service for removing one exact inactive managed artifact set.
pub struct ArtifactSetRemovalService<'a> {
    storage: ExistingArtifactStorage,
    limits: ArtifactSetRemovalLimits,
    store: &'a mut ArtifactStateStore,
    #[cfg(test)]
    fault: ArtifactSetRemovalTestFault,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ArtifactSetRemovalTestFault {
    None,
    BeforeUnlink,
    BeforeDirectorySync,
    BeforeLayoutRecheck,
    BeforeCompletion,
}

impl<'a> ArtifactSetRemovalService<'a> {
    /// Opens existing storage and takes its exclusive lifecycle lock.
    ///
    /// Opening creates and removes nothing, performs no implicit recovery, and
    /// accesses no network.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactSetRemovalError`] when limits are invalid, storage is
    /// absent or unsafe, or another lifecycle operation owns the lock.
    pub fn open_existing(
        root: impl AsRef<Path>,
        store: &'a mut ArtifactStateStore,
        limits: ArtifactSetRemovalLimits,
    ) -> Result<Self, ArtifactSetRemovalError> {
        validate_limits(limits)?;
        let storage = ExistingArtifactStorage::open(root, LifecycleLockMode::Exclusive)
            .map_err(map_open_error)?;
        Ok(Self {
            storage,
            limits,
            store,
            #[cfg(test)]
            fault: ArtifactSetRemovalTestFault::None,
        })
    }

    /// Removes or resumes removal of one exact installed artifact-set generation.
    ///
    /// The request accepts no path, tag, storage key, or prior inventory report.
    /// No callback or cancellation point runs after durable preparation.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactSetRemovalError::RecoveryRequired`] after preparation when
    /// an exact retry is required to finish the operation.
    pub fn remove<F>(
        &mut self,
        request: &ArtifactSetRemovalRequest,
        cancellation: &CancellationToken,
        mut progress: F,
    ) -> Result<ArtifactSetRemovalResult, ArtifactSetRemovalError>
    where
        F: FnMut(ArtifactSetRemovalProgress),
    {
        validate_request(request)?;
        self.storage.validate_layout().map_err(map_storage_error)?;
        let manifest = self.load_manifest(request)?;
        let plan = plan_request(&manifest, self.limits)?;
        if plan.installed != request.selection.installed {
            return Err(ArtifactSetRemovalError::InvalidSelection);
        }
        let total = manifest.total_byte_size();
        let (current, removal) = self
            .store
            .artifact_set_removal_state(request.selection.installed.artifact_set_id())
            .map_err(map_store_error)?;
        if removal.as_ref().is_some_and(|value| {
            value.selection == request.selection && value.phase == ArtifactRemovalPhase::Completed
        }) {
            return Ok(result(request, ArtifactRemovalDisposition::AlreadyRemoved));
        }
        let recovering = removal.as_ref().is_some_and(|value| {
            value.selection == request.selection && value.phase == ArtifactRemovalPhase::Prepared
        });
        if total > self.limits.maximum_total_bytes {
            if recovering {
                return Err(ArtifactSetRemovalError::RecoveryRequired(
                    ArtifactSetRemovalRecoveryError::Storage,
                ));
            }
            return Err(ArtifactSetRemovalError::ArtifactSetTooLarge {
                actual: total,
                maximum: self.limits.maximum_total_bytes,
            });
        }
        if !recovering && current.as_ref() != Some(&request.selection) {
            return Err(ArtifactSetRemovalError::StaleSelection);
        }
        if !recovering {
            ensure_not_cancelled(cancellation)?;
            report_progress(
                &mut progress,
                ArtifactSetRemovalStage::InspectingSelection,
                0,
                total,
            );
            ensure_not_cancelled(cancellation)?;
        }
        let name = OsString::from(plan.storage_key.clone());
        let verification_token = CancellationToken::new();
        let token = if recovering {
            &verification_token
        } else {
            cancellation
        };
        let verified = self
            .verify(&name, &manifest, &plan, token, &mut progress, recovering)
            .map_err(|error| {
                if recovering {
                    map_prepared_error(error)
                } else {
                    error
                }
            })?;
        if !recovering {
            report_progress(
                &mut progress,
                ArtifactSetRemovalStage::PreparingRemoval,
                0,
                total,
            );
            ensure_not_cancelled(cancellation)?;
            self.storage.validate_layout().map_err(map_storage_error)?;
        }
        let prepared = self.prepare_state(&request.selection)?;
        if prepared == RemovalPreparationDisposition::AlreadyCompleted {
            return Ok(result(request, ArtifactRemovalDisposition::AlreadyRemoved));
        }
        self.finish_prepared(&name, request, verified)?;
        Ok(result(
            request,
            if recovering {
                ArtifactRemovalDisposition::Recovered
            } else {
                ArtifactRemovalDisposition::Removed
            },
        ))
    }

    fn load_manifest(
        &self,
        request: &ArtifactSetRemovalRequest,
    ) -> Result<ArtifactSetManifest, ArtifactSetRemovalError> {
        self.store
            .artifact_set_manifest(request.selection.installed.artifact_set_id())
            .map_err(map_store_error)?
            .ok_or(ArtifactSetRemovalError::StaleSelection)
    }

    fn prepare_state(
        &mut self,
        selection: &StoredArtifactSetInstallation,
    ) -> Result<RemovalPreparationDisposition, ArtifactSetRemovalError> {
        let lifecycle_lock = self
            .storage
            .exclusive_lifecycle_lock()
            .map_err(map_storage_error)?;
        self.store
            .prepare_artifact_set_removal(lifecycle_lock, selection)
            .map_err(map_store_error)
    }

    fn verify<F>(
        &self,
        name: &OsStr,
        manifest: &ArtifactSetManifest,
        plan: &ValidatedSetPlan,
        cancellation: &CancellationToken,
        progress: &mut F,
        recovering: bool,
    ) -> Result<Option<PinnedDirectory>, ArtifactSetRemovalError>
    where
        F: FnMut(ArtifactSetRemovalProgress),
    {
        let total = manifest.total_byte_size();
        if !recovering {
            report_progress(
                progress,
                ArtifactSetRemovalStage::VerifyingInactiveTree,
                0,
                total,
            );
        }
        match self.open_set_root(name, cancellation) {
            Ok(set_root) => {
                let tree_limits = ManagedTreeLimits::new(self.limits.maximum_tree_entries)
                    .map_err(map_open_error)?;
                verify_final_tree(&set_root, manifest, plan, tree_limits, cancellation)
                    .map_err(map_verify_error)?;
                if !recovering {
                    report_progress(
                        progress,
                        ArtifactSetRemovalStage::VerifyingInactiveTree,
                        total,
                        total,
                    );
                }
                Ok(Some(set_root))
            }
            Err(ArtifactSetRemovalError::TreeMissing) if recovering => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn open_set_root(
        &self,
        name: &OsStr,
        cancellation: &CancellationToken,
    ) -> Result<PinnedDirectory, ArtifactSetRemovalError> {
        let sets = self
            .storage
            .root()
            .open_optional_child_directory(OsStr::new(SETS_DIRECTORY))
            .map_err(map_open_error)?
            .ok_or(ArtifactSetRemovalError::TreeMissing)?;
        match sets
            .exact_entry_capacity(name, self.limits.maximum_storage_entries, cancellation)
            .map_err(map_open_error)?
        {
            ExactEntryCapacity::Present => sets
                .open_optional_child_directory(name)
                .map_err(map_open_error)?
                .ok_or(ArtifactSetRemovalError::TreeMissing),
            ExactEntryCapacity::Available => Err(ArtifactSetRemovalError::TreeMissing),
            ExactEntryCapacity::Full => Err(ArtifactSetRemovalError::StorageEntryLimitExceeded),
        }
    }

    fn finish_prepared(
        &mut self,
        name: &OsStr,
        request: &ArtifactSetRemovalRequest,
        verified: Option<PinnedDirectory>,
    ) -> Result<(), ArtifactSetRemovalError> {
        let outcome = (|| {
            #[cfg(test)]
            self.inject_recovery_fault(ArtifactSetRemovalTestFault::BeforeUnlink)?;
            let sets = self
                .storage
                .root()
                .open_optional_child_directory(OsStr::new(SETS_DIRECTORY))?;
            match (sets.as_ref(), verified) {
                (Some(sets), Some(verified)) => {
                    let tree_limits = ManagedTreeLimits::new(self.limits.maximum_tree_entries)?;
                    remove_verified_managed_tree(
                        sets,
                        name,
                        verified,
                        tree_limits,
                        self.limits.maximum_storage_entries,
                    )?;
                }
                (Some(sets), None) => {
                    sets.confirm_managed_file_absent(name, self.limits.maximum_storage_entries)?;
                }
                (None, Some(_)) => return Err(ArtifactInventoryError::ConcurrentModification),
                (None, None) => {}
            }
            #[cfg(test)]
            self.inject_recovery_fault(ArtifactSetRemovalTestFault::BeforeDirectorySync)?;
            if let Some(sets) = self
                .storage
                .root()
                .open_optional_child_directory(OsStr::new(SETS_DIRECTORY))?
            {
                sets.sync()?;
            }
            #[cfg(test)]
            self.inject_recovery_fault(ArtifactSetRemovalTestFault::BeforeLayoutRecheck)?;
            self.storage.validate_layout()?;
            Ok::<(), ArtifactInventoryError>(())
        })();
        outcome.map_err(|error| {
            ArtifactSetRemovalError::RecoveryRequired(map_recovery_storage_error(error))
        })?;
        #[cfg(test)]
        if self.fault == ArtifactSetRemovalTestFault::BeforeCompletion {
            return Err(ArtifactSetRemovalError::RecoveryRequired(
                ArtifactSetRemovalRecoveryError::State(StoreError::CorruptRecord),
            ));
        }
        self.store
            .complete_artifact_set_removal(
                self.storage.exclusive_lifecycle_lock().map_err(|error| {
                    ArtifactSetRemovalError::RecoveryRequired(map_recovery_storage_error(error))
                })?,
                &request.selection,
            )
            .map_err(|error| {
                ArtifactSetRemovalError::RecoveryRequired(ArtifactSetRemovalRecoveryError::State(
                    error,
                ))
            })?;
        Ok(())
    }

    #[cfg(test)]
    fn inject_recovery_fault(
        &self,
        expected: ArtifactSetRemovalTestFault,
    ) -> Result<(), ArtifactInventoryError> {
        if self.fault == expected {
            Err(ArtifactInventoryError::StorageIo(std::io::Error::other(
                "injected post-preparation failure",
            )))
        } else {
            Ok(())
        }
    }
}

fn validate_limits(limits: ArtifactSetRemovalLimits) -> Result<(), ArtifactSetRemovalError> {
    let bounds = ArtifactSetPlanBounds {
        members: limits.maximum_members,
        member_bytes: limits.maximum_member_bytes,
        total_bytes: limits.maximum_total_bytes,
        tree_entries: limits.maximum_tree_entries,
    };
    if limits.maximum_storage_entries == 0
        || limits.maximum_storage_entries.checked_add(1).is_none()
        || validate_plan_bounds(bounds).is_err()
    {
        Err(ArtifactSetRemovalError::InvalidLimits)
    } else {
        Ok(())
    }
}

fn validate_request(request: &ArtifactSetRemovalRequest) -> Result<(), ArtifactSetRemovalError> {
    if request.selection.epoch.get() == 0 {
        return Err(ArtifactSetRemovalError::InvalidSelection);
    }
    Ok(())
}

fn plan_request(
    manifest: &ArtifactSetManifest,
    limits: ArtifactSetRemovalLimits,
) -> Result<ValidatedSetPlan, ArtifactSetRemovalError> {
    let bounds = ArtifactSetPlanBounds {
        members: limits.maximum_members,
        member_bytes: limits.maximum_member_bytes,
        total_bytes: limits.maximum_total_bytes,
        tree_entries: limits.maximum_tree_entries,
    };
    plan_artifact_set(manifest, bounds).map_err(map_plan_error)
}

#[cfg(test)]
mod tests;
