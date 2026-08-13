use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsStr,
    path::Path,
};

use rewrite_model::{ArtifactId, ArtifactManifest, InstalledArtifact};
use rewrite_model_store::{ArtifactStateStore, StoreError, StoredArtifactState};
use rewrite_types::{CancellationToken, Digest};

mod contract;

use crate::artifact_storage::{
    DirectoryEntrySnapshot, ExistingArtifactStorage, LifecycleLockMode, PinnedDirectory,
};
pub use contract::{
    ArtifactInventoryError, ArtifactInventoryLimits, ArtifactInventoryProgress,
    ArtifactInventoryReport, ArtifactInventoryStage, ContentAddressConflict,
    OrphanManifestAssociation, OversizedArtifactFile, RegisteredArtifactBytes,
    RegisteredArtifactInspection, UnexpectedArtifactEntryCounts, VerifiedArtifactOrphan,
};

/// Read-only, point-in-time inspection of application-owned artifact storage.
pub struct ArtifactInventoryService<'a> {
    storage: ExistingArtifactStorage,
    limits: ArtifactInventoryLimits,
    store: &'a ArtifactStateStore,
}

impl<'a> ArtifactInventoryService<'a> {
    /// Opens existing storage and acquires its shared lifecycle lock.
    ///
    /// This operation never creates storage, cleans staging files, repairs state,
    /// removes bytes, or accesses the network.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactInventoryError`] when limits are invalid, storage is not
    /// initialized, a managed boundary is unsafe, or an exclusive lifecycle
    /// operation owns the lock.
    pub fn open(
        root: impl AsRef<Path>,
        store: &'a ArtifactStateStore,
        limits: ArtifactInventoryLimits,
    ) -> Result<Self, ArtifactInventoryError> {
        validate_limits(limits)?;
        let storage = ExistingArtifactStorage::open(root, LifecycleLockMode::Shared)?;
        let service = Self {
            storage,
            limits,
            store,
        };
        Ok(service)
    }

    /// Builds one bounded, deterministic reconciliation report without mutation.
    ///
    /// Verified orphan files are point-in-time reclamation candidates only. They
    /// are never treated as durable authority and are not removed automatically.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactInventoryError`] with no partial report when state is
    /// corrupt, limits are exceeded, cancellation is observed, or storage changes
    /// during the operation.
    pub fn inventory<F>(
        &self,
        cancellation: &CancellationToken,
        mut progress: F,
    ) -> Result<ArtifactInventoryReport, ArtifactInventoryError>
    where
        F: FnMut(ArtifactInventoryProgress),
    {
        ensure_not_cancelled(cancellation)?;
        report_progress(&mut progress, ArtifactInventoryStage::OpeningStorage, 0, 0);
        let initial_layout = self.storage.validate_layout()?;
        ensure_not_cancelled(cancellation)?;

        report_progress(&mut progress, ArtifactInventoryStage::LoadingState, 0, 0);
        let states = self
            .store
            .artifact_inventory(self.limits.maximum_state_entries)
            .map_err(map_store_error)?;
        ensure_not_cancelled(cancellation)?;

        report_progress(&mut progress, ArtifactInventoryStage::FreezingStorage, 0, 0);
        ensure_not_cancelled(cancellation)?;
        let initial_entries = self
            .storage
            .artifacts()
            .snapshot(self.limits.maximum_storage_entries, cancellation)?;
        let mut builder = InventoryBuilder::new(&states, &initial_entries, self.limits);
        builder.inspect_registered(self.storage.artifacts(), cancellation, &mut progress)?;
        builder.inspect_uninstalled(self.storage.artifacts(), cancellation, &mut progress)?;
        ensure_not_cancelled(cancellation)?;

        report_progress(
            &mut progress,
            ArtifactInventoryStage::RecheckingStorageAndState,
            builder.completed_entries,
            builder.verified_bytes,
        );
        ensure_not_cancelled(cancellation)?;
        let final_entries = self
            .storage
            .artifacts()
            .snapshot(self.limits.maximum_storage_entries, cancellation)?;
        if initial_entries != final_entries || initial_layout != self.storage.validate_layout()? {
            return Err(ArtifactInventoryError::ConcurrentModification);
        }
        let final_states = self
            .store
            .artifact_inventory(self.limits.maximum_state_entries)
            .map_err(map_final_store_error)?;
        if states != final_states {
            return Err(ArtifactInventoryError::ConcurrentModification);
        }
        ensure_not_cancelled(cancellation)?;
        Ok(builder.finish(
            u64::try_from(initial_entries.len())
                .map_err(|_| ArtifactInventoryError::StorageEntryLimitExceeded)?,
        ))
    }
}

struct InventoryBuilder<'a> {
    states: &'a [StoredArtifactState],
    entries: &'a [DirectoryEntrySnapshot],
    limits: ArtifactInventoryLimits,
    manifests: BTreeMap<String, ArtifactManifest>,
    installed: BTreeSet<String>,
    registered: Vec<RegisteredArtifactInspection>,
    manifest_only: Vec<ArtifactManifest>,
    verified_orphans: Vec<VerifiedArtifactOrphan>,
    content_address_conflicts: Vec<ContentAddressConflict>,
    oversized_files: Vec<OversizedArtifactFile>,
    unexpected_entries: UnexpectedArtifactEntryCounts,
    completed_entries: u64,
    verified_bytes: u64,
}

impl<'a> InventoryBuilder<'a> {
    fn new(
        states: &'a [StoredArtifactState],
        entries: &'a [DirectoryEntrySnapshot],
        limits: ArtifactInventoryLimits,
    ) -> Self {
        let manifests = states
            .iter()
            .map(|state| {
                (
                    state.manifest.artifact_digest.as_str().to_owned(),
                    state.manifest.clone(),
                )
            })
            .collect();
        let installed = states
            .iter()
            .filter_map(|state| state.installed.as_ref())
            .filter(|item| {
                item.storage_key == format!("artifacts/{}", item.artifact_digest.as_str())
            })
            .map(|item| item.artifact_digest.as_str().to_owned())
            .collect();
        Self {
            states,
            entries,
            limits,
            manifests,
            installed,
            registered: Vec::new(),
            manifest_only: Vec::new(),
            verified_orphans: Vec::new(),
            content_address_conflicts: Vec::new(),
            oversized_files: Vec::new(),
            unexpected_entries: UnexpectedArtifactEntryCounts::default(),
            completed_entries: 0,
            verified_bytes: 0,
        }
    }

    fn inspect_registered<F>(
        &mut self,
        artifacts: &PinnedDirectory,
        cancellation: &CancellationToken,
        progress: &mut F,
    ) -> Result<(), ArtifactInventoryError>
    where
        F: FnMut(ArtifactInventoryProgress),
    {
        for state in self.states {
            ensure_not_cancelled(cancellation)?;
            let Some(installed) = state.installed.as_ref() else {
                self.manifest_only.push(state.manifest.clone());
                self.complete_registered(progress);
                continue;
            };
            let bytes = self.classify_registered(artifacts, installed, cancellation)?;
            self.registered.push(RegisteredArtifactInspection {
                manifest: state.manifest.clone(),
                installed: installed.clone(),
                active_bindings: state.active_bindings.clone(),
                bytes,
            });
            self.complete_registered(progress);
        }
        Ok(())
    }

    fn complete_registered<F>(&mut self, progress: &mut F)
    where
        F: FnMut(ArtifactInventoryProgress),
    {
        self.completed_entries = self.completed_entries.saturating_add(1);
        report_progress(
            progress,
            ArtifactInventoryStage::InspectingState,
            self.completed_entries,
            self.verified_bytes,
        );
    }

    fn classify_registered(
        &mut self,
        artifacts: &PinnedDirectory,
        installed: &InstalledArtifact,
        cancellation: &CancellationToken,
    ) -> Result<RegisteredArtifactBytes, ArtifactInventoryError> {
        let digest = installed.artifact_digest.as_str();
        if installed.storage_key != format!("artifacts/{digest}") {
            return Ok(RegisteredArtifactBytes::StateLayoutConflict);
        }
        let Some(entry) = find_exact_entry(self.entries, OsStr::new(digest)) else {
            return Ok(RegisteredArtifactBytes::Missing);
        };
        if entry.indirect || !entry.direct_regular_file {
            return Ok(RegisteredArtifactBytes::UnsafeEntry);
        }
        if !entry.has_single_link() {
            return Ok(RegisteredArtifactBytes::AliasedEntry);
        }
        if entry.byte_size != installed.byte_size {
            return Ok(RegisteredArtifactBytes::SizeConflict {
                observed_bytes: entry.byte_size,
            });
        }
        if entry.byte_size > self.limits.maximum_artifact_bytes {
            return Ok(RegisteredArtifactBytes::TooLargeToVerify {
                observed_bytes: entry.byte_size,
            });
        }
        let observed = self.hash_entry(artifacts, entry, cancellation)?;
        if observed == installed.artifact_digest {
            Ok(RegisteredArtifactBytes::Verified)
        } else {
            Ok(RegisteredArtifactBytes::DigestConflict {
                observed_digest: observed,
            })
        }
    }

    fn inspect_uninstalled<F>(
        &mut self,
        artifacts: &PinnedDirectory,
        cancellation: &CancellationToken,
        progress: &mut F,
    ) -> Result<(), ArtifactInventoryError>
    where
        F: FnMut(ArtifactInventoryProgress),
    {
        for entry in self.entries {
            ensure_not_cancelled(cancellation)?;
            let Some(name) = entry.name.to_str() else {
                self.unexpected_entries.malformed_names =
                    self.unexpected_entries.malformed_names.saturating_add(1);
                self.complete_uninstalled(progress);
                continue;
            };
            let Ok(claimed_digest) = Digest::from_sha256_hex(name) else {
                self.unexpected_entries.malformed_names =
                    self.unexpected_entries.malformed_names.saturating_add(1);
                self.complete_uninstalled(progress);
                continue;
            };
            if self.installed.contains(name) {
                continue;
            }
            let claimed_artifact_id = ArtifactId::from_digest(claimed_digest.clone());
            if entry.indirect {
                self.unexpected_entries.indirect_entries =
                    self.unexpected_entries.indirect_entries.saturating_add(1);
            } else if !entry.direct_regular_file {
                self.unexpected_entries.non_regular_entries = self
                    .unexpected_entries
                    .non_regular_entries
                    .saturating_add(1);
            } else if entry.byte_size == 0 {
                self.unexpected_entries.empty_files =
                    self.unexpected_entries.empty_files.saturating_add(1);
            } else if !entry.has_single_link() {
                self.unexpected_entries.aliased_files =
                    self.unexpected_entries.aliased_files.saturating_add(1);
            } else if entry.byte_size > self.limits.maximum_artifact_bytes {
                self.oversized_files.push(OversizedArtifactFile {
                    claimed_artifact_id,
                    byte_size: entry.byte_size,
                });
            } else {
                let observed = self.hash_entry(artifacts, entry, cancellation)?;
                if observed == claimed_digest {
                    self.verified_orphans.push(VerifiedArtifactOrphan {
                        artifact_id: claimed_artifact_id,
                        byte_size: entry.byte_size,
                        manifest: self.manifest_association(name, entry.byte_size),
                    });
                } else {
                    self.content_address_conflicts.push(ContentAddressConflict {
                        claimed_artifact_id,
                        observed_digest: observed,
                        byte_size: entry.byte_size,
                    });
                }
            }
            self.complete_uninstalled(progress);
        }
        Ok(())
    }

    fn complete_uninstalled<F>(&mut self, progress: &mut F)
    where
        F: FnMut(ArtifactInventoryProgress),
    {
        self.completed_entries = self.completed_entries.saturating_add(1);
        report_progress(
            progress,
            ArtifactInventoryStage::VerifyingUninstalled,
            self.completed_entries,
            self.verified_bytes,
        );
    }

    fn hash_entry(
        &mut self,
        artifacts: &PinnedDirectory,
        entry: &DirectoryEntrySnapshot,
        cancellation: &CancellationToken,
    ) -> Result<Digest, ArtifactInventoryError> {
        let next_total = self
            .verified_bytes
            .checked_add(entry.byte_size)
            .filter(|total| *total <= self.limits.maximum_total_verification_bytes)
            .ok_or(ArtifactInventoryError::TotalVerificationLimitExceeded)?;
        let observed = artifacts.hash_entry(entry, cancellation)?;
        self.verified_bytes = next_total;
        Ok(observed)
    }

    fn manifest_association(&self, digest: &str, byte_size: u64) -> OrphanManifestAssociation {
        match self.manifests.get(digest) {
            None => OrphanManifestAssociation::NoManifest,
            Some(manifest) if manifest.byte_size == byte_size => {
                OrphanManifestAssociation::MatchingManifest(manifest.clone())
            }
            Some(manifest) => OrphanManifestAssociation::ManifestSizeConflict {
                manifest: manifest.clone(),
            },
        }
    }

    fn finish(&mut self, storage_entry_count: u64) -> ArtifactInventoryReport {
        ArtifactInventoryReport {
            registered: std::mem::take(&mut self.registered),
            manifest_only: std::mem::take(&mut self.manifest_only),
            verified_orphans: std::mem::take(&mut self.verified_orphans),
            content_address_conflicts: std::mem::take(&mut self.content_address_conflicts),
            oversized_files: std::mem::take(&mut self.oversized_files),
            unexpected_entries: self.unexpected_entries,
            storage_entry_count,
            verified_bytes: self.verified_bytes,
        }
    }
}

fn validate_limits(limits: ArtifactInventoryLimits) -> Result<(), ArtifactInventoryError> {
    let valid = limits.maximum_state_entries > 0
        && limits.maximum_storage_entries > 0
        && limits.maximum_artifact_bytes > 0
        && limits.maximum_total_verification_bytes > 0
        && limits
            .maximum_state_entries
            .checked_add(1)
            .and_then(|value| i64::try_from(value).ok())
            .is_some()
        && limits.maximum_storage_entries.checked_add(1).is_some()
        && u64::try_from(limits.maximum_storage_entries).is_ok();
    if valid {
        Ok(())
    } else {
        Err(ArtifactInventoryError::InvalidLimits)
    }
}

fn map_store_error(error: StoreError) -> ArtifactInventoryError {
    match error {
        StoreError::InvalidLimit => ArtifactInventoryError::InvalidLimits,
        StoreError::InventoryLimitExceeded => ArtifactInventoryError::StateEntryLimitExceeded,
        other => ArtifactInventoryError::State(other),
    }
}

fn map_final_store_error(error: StoreError) -> ArtifactInventoryError {
    match error {
        StoreError::InvalidLimit => ArtifactInventoryError::InvalidLimits,
        StoreError::InventoryLimitExceeded => ArtifactInventoryError::ConcurrentModification,
        other => ArtifactInventoryError::State(other),
    }
}

fn find_exact_entry<'a>(
    entries: &'a [DirectoryEntrySnapshot],
    name: &OsStr,
) -> Option<&'a DirectoryEntrySnapshot> {
    entries.iter().find(|entry| entry.name == name)
}

fn ensure_not_cancelled(cancellation: &CancellationToken) -> Result<(), ArtifactInventoryError> {
    if cancellation.is_cancelled() {
        Err(ArtifactInventoryError::Cancelled)
    } else {
        Ok(())
    }
}

fn report_progress(
    progress: &mut impl FnMut(ArtifactInventoryProgress),
    stage: ArtifactInventoryStage,
    completed_entries: u64,
    verified_bytes: u64,
) {
    progress(ArtifactInventoryProgress {
        stage,
        completed_entries,
        verified_bytes,
    });
}

#[cfg(test)]
mod tests;
