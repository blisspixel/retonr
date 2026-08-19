use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsStr,
};

use rewrite_model::{ArtifactSetId, ArtifactSetManifest};
use rewrite_model_store::{StoreError, StoredArtifactSetState};
use rewrite_types::{CancellationToken, Digest};

use crate::artifact_set_import::{
    ArtifactSetPlanBounds, SET_STORAGE_KEY_PREFIX, ValidatedSetPlan, plan_artifact_set,
    validate_plan_bounds,
};
use crate::artifact_storage::{
    DirectoryEntrySnapshot, ManagedTreeEntryKind, ManagedTreeLimits, ManagedTreeSnapshot,
    PinnedDirectory, hash_exact_bytes,
};

use super::map::{ensure_not_cancelled, map_storage_open, report_progress};
use super::{
    ArtifactSetInventoryError, ArtifactSetInventoryLimits, ArtifactSetInventoryProgress,
    ArtifactSetInventoryReport, ArtifactSetInventoryStage, ArtifactSetTreeConflict,
    OversizedArtifactSet, RegisteredArtifactSetBytes, RegisteredArtifactSetInspection,
    UnexpectedArtifactSetEntryCounts, VerifiedArtifactSetOrphan,
};

pub(super) struct InventoryBuilder<'a> {
    states: &'a [StoredArtifactSetState],
    entries: &'a [DirectoryEntrySnapshot],
    limits: ArtifactSetInventoryLimits,
    manifests: BTreeMap<String, ArtifactSetManifest>,
    installed: BTreeSet<String>,
    registered: Vec<RegisteredArtifactSetInspection>,
    manifest_only: Vec<ArtifactSetManifest>,
    verified_orphans: Vec<VerifiedArtifactSetOrphan>,
    tree_conflicts: Vec<ArtifactSetTreeConflict>,
    oversized_sets: Vec<OversizedArtifactSet>,
    unexpected_entries: UnexpectedArtifactSetEntryCounts,
    pub(super) completed_entries: u64,
    pub(super) verified_bytes: u64,
}

impl<'a> InventoryBuilder<'a> {
    pub(super) fn new(
        states: &'a [StoredArtifactSetState],
        entries: &'a [DirectoryEntrySnapshot],
        limits: ArtifactSetInventoryLimits,
    ) -> Self {
        let manifests = states
            .iter()
            .map(|state| {
                (
                    state
                        .manifest
                        .artifact_set_id()
                        .digest()
                        .as_str()
                        .to_owned(),
                    state.manifest.clone(),
                )
            })
            .collect();
        let installed = states
            .iter()
            .filter_map(|state| state.installed.as_ref())
            .filter(|selection| {
                selection.installed.storage_key()
                    == canonical_set_name(selection.installed.artifact_set_id()).as_str()
            })
            .map(|selection| selection.installed.storage_key().to_owned())
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
            tree_conflicts: Vec::new(),
            oversized_sets: Vec::new(),
            unexpected_entries: UnexpectedArtifactSetEntryCounts::default(),
            completed_entries: 0,
            verified_bytes: 0,
        }
    }

    pub(super) fn inspect_registered<F>(
        &mut self,
        sets: Option<&PinnedDirectory>,
        cancellation: &CancellationToken,
        progress: &mut F,
    ) -> Result<(), ArtifactSetInventoryError>
    where
        F: FnMut(ArtifactSetInventoryProgress),
    {
        for state in self.states {
            ensure_not_cancelled(cancellation)?;
            let Some(installed) = state.installed.as_ref() else {
                self.manifest_only.push(state.manifest.clone());
                self.complete_registered(progress);
                continue;
            };
            let bytes = self.classify_registered(
                sets,
                &state.manifest,
                installed.installed.storage_key(),
                cancellation,
            )?;
            self.registered.push(RegisteredArtifactSetInspection {
                manifest: state.manifest.clone(),
                installation: crate::ArtifactSetInstallationKey::from_stored(installed),
                bytes,
            });
            self.complete_registered(progress);
        }
        Ok(())
    }

    fn classify_registered(
        &mut self,
        sets: Option<&PinnedDirectory>,
        manifest: &ArtifactSetManifest,
        storage_key: &str,
        cancellation: &CancellationToken,
    ) -> Result<RegisteredArtifactSetBytes, ArtifactSetInventoryError> {
        if storage_key != canonical_set_name(&manifest.artifact_set_id()) {
            return Ok(RegisteredArtifactSetBytes::StateLayoutConflict);
        }
        let Some(sets) = sets else {
            return Ok(RegisteredArtifactSetBytes::Missing);
        };
        let Some(entry) = find_exact_entry(self.entries, OsStr::new(storage_key)) else {
            return Ok(RegisteredArtifactSetBytes::Missing);
        };
        if entry.indirect || entry.direct_regular_file {
            return Ok(RegisteredArtifactSetBytes::UnsafeEntry);
        }
        let Some(root) = open_set_root(sets, &entry.name)? else {
            return Ok(RegisteredArtifactSetBytes::UnsafeEntry);
        };
        Ok(self
            .inspect_known_tree(manifest, &root, cancellation)?
            .into())
    }

    pub(super) fn inspect_uninstalled<F>(
        &mut self,
        sets: Option<&PinnedDirectory>,
        cancellation: &CancellationToken,
        progress: &mut F,
    ) -> Result<(), ArtifactSetInventoryError>
    where
        F: FnMut(ArtifactSetInventoryProgress),
    {
        let Some(sets) = sets else {
            return Ok(());
        };
        for entry in self.entries {
            ensure_not_cancelled(cancellation)?;
            let Some(name) = entry.name.to_str() else {
                self.unexpected_entries.malformed_names =
                    self.unexpected_entries.malformed_names.saturating_add(1);
                self.complete_uninstalled(progress);
                continue;
            };
            let Some(artifact_set_id) = parse_canonical_set_name(name) else {
                self.unexpected_entries.malformed_names =
                    self.unexpected_entries.malformed_names.saturating_add(1);
                self.complete_uninstalled(progress);
                continue;
            };
            if self.installed.contains(name) {
                continue;
            }
            if entry.indirect {
                self.unexpected_entries.indirect_entries =
                    self.unexpected_entries.indirect_entries.saturating_add(1);
            } else if entry.direct_regular_file {
                self.unexpected_entries.non_directory_entries = self
                    .unexpected_entries
                    .non_directory_entries
                    .saturating_add(1);
            } else if let Some(manifest) = self
                .manifests
                .get(artifact_set_id.digest().as_str())
                .cloned()
            {
                match open_set_root(sets, &entry.name)? {
                    None => {
                        self.unexpected_entries.non_directory_entries = self
                            .unexpected_entries
                            .non_directory_entries
                            .saturating_add(1);
                    }
                    Some(root) => match self.inspect_known_tree(&manifest, &root, cancellation)? {
                        TreeInspection::Verified { byte_size } => {
                            self.verified_orphans.push(VerifiedArtifactSetOrphan {
                                artifact_set_id,
                                byte_size,
                            });
                        }
                        TreeInspection::TooLargeToVerify { observed_bytes } => {
                            self.oversized_sets.push(OversizedArtifactSet {
                                artifact_set_id,
                                byte_size: observed_bytes,
                            });
                        }
                        TreeInspection::TreeMismatch | TreeInspection::MemberDigestConflict => {
                            self.tree_conflicts.push(ArtifactSetTreeConflict {
                                artifact_set_id,
                                byte_size: manifest.total_byte_size(),
                            });
                        }
                    },
                }
            } else {
                self.unexpected_entries.unregistered_roots =
                    self.unexpected_entries.unregistered_roots.saturating_add(1);
            }
            self.complete_uninstalled(progress);
        }
        Ok(())
    }

    fn inspect_known_tree(
        &mut self,
        manifest: &ArtifactSetManifest,
        root: &PinnedDirectory,
        cancellation: &CancellationToken,
    ) -> Result<TreeInspection, ArtifactSetInventoryError> {
        let Some(plan) = plan_if_within_set_ceilings(manifest, self.limits)? else {
            return Ok(TreeInspection::TooLargeToVerify {
                observed_bytes: manifest.total_byte_size(),
            });
        };
        let next_total = self
            .verified_bytes
            .checked_add(manifest.total_byte_size())
            .filter(|total| *total <= self.limits.maximum_total_verification_bytes)
            .ok_or(ArtifactSetInventoryError::TotalVerificationLimitExceeded)?;
        let tree_limits =
            ManagedTreeLimits::new(self.limits.maximum_tree_entries).map_err(map_storage_open)?;
        let snapshot = root
            .enumerate_tree(tree_limits, cancellation)
            .map_err(map_storage_open)?;
        if !tree_matches_plan(&snapshot, manifest, &plan) {
            return Ok(TreeInspection::TreeMismatch);
        }
        self.verified_bytes = next_total;
        for member in manifest.members() {
            ensure_not_cancelled(cancellation)?;
            let mut opened = root
                .open_relative_regular_file(member.relative_path())
                .map_err(map_storage_open)?;
            if opened.byte_size != member.byte_size() || !opened.fingerprint.has_single_link() {
                return Err(ArtifactSetInventoryError::ConcurrentModification);
            }
            let observed = hash_exact_bytes(&mut opened.file, member.byte_size(), cancellation)
                .map_err(map_storage_open)?;
            if &observed != member.artifact_id().digest() {
                return Ok(TreeInspection::MemberDigestConflict);
            }
            root.recheck_relative_regular_file(member.relative_path(), &opened.fingerprint)
                .map_err(map_storage_open)?;
        }
        Ok(TreeInspection::Verified {
            byte_size: manifest.total_byte_size(),
        })
    }

    fn complete_registered<F>(&mut self, progress: &mut F)
    where
        F: FnMut(ArtifactSetInventoryProgress),
    {
        self.completed_entries = self.completed_entries.saturating_add(1);
        report_progress(
            progress,
            ArtifactSetInventoryStage::InspectingState,
            self.completed_entries,
            self.verified_bytes,
        );
    }

    fn complete_uninstalled<F>(&mut self, progress: &mut F)
    where
        F: FnMut(ArtifactSetInventoryProgress),
    {
        self.completed_entries = self.completed_entries.saturating_add(1);
        report_progress(
            progress,
            ArtifactSetInventoryStage::VerifyingUninstalled,
            self.completed_entries,
            self.verified_bytes,
        );
    }

    pub(super) fn finish(&mut self, storage_entry_count: u64) -> ArtifactSetInventoryReport {
        let verified_orphans = std::mem::take(&mut self.verified_orphans);
        let tree_conflicts = std::mem::take(&mut self.tree_conflicts);
        let oversized_sets = std::mem::take(&mut self.oversized_sets);
        let observed: std::collections::BTreeSet<_> = verified_orphans
            .iter()
            .map(|item| item.artifact_set_id.digest().as_str().to_owned())
            .chain(
                tree_conflicts
                    .iter()
                    .map(|item| item.artifact_set_id.digest().as_str().to_owned()),
            )
            .chain(
                oversized_sets
                    .iter()
                    .map(|item| item.artifact_set_id.digest().as_str().to_owned()),
            )
            .collect();
        let manifest_only = std::mem::take(&mut self.manifest_only)
            .into_iter()
            .filter(|manifest| !observed.contains(manifest.artifact_set_id().digest().as_str()))
            .collect();
        ArtifactSetInventoryReport {
            registered: std::mem::take(&mut self.registered),
            manifest_only,
            verified_orphans,
            tree_conflicts,
            oversized_sets,
            unexpected_entries: self.unexpected_entries,
            storage_entry_count,
            verified_bytes: self.verified_bytes,
        }
    }
}

enum TreeInspection {
    Verified { byte_size: u64 },
    TreeMismatch,
    MemberDigestConflict,
    TooLargeToVerify { observed_bytes: u64 },
}

impl From<TreeInspection> for RegisteredArtifactSetBytes {
    fn from(value: TreeInspection) -> Self {
        match value {
            TreeInspection::Verified { .. } => Self::Verified,
            TreeInspection::TreeMismatch => Self::TreeMismatch,
            TreeInspection::MemberDigestConflict => Self::MemberDigestConflict,
            TreeInspection::TooLargeToVerify { observed_bytes } => {
                Self::TooLargeToVerify { observed_bytes }
            }
        }
    }
}

pub(super) fn snapshot_sets(
    sets: Option<&PinnedDirectory>,
    limits: ArtifactSetInventoryLimits,
    cancellation: &CancellationToken,
) -> Result<Vec<DirectoryEntrySnapshot>, ArtifactSetInventoryError> {
    match sets {
        None => Ok(Vec::new()),
        Some(sets) => sets
            .snapshot(limits.maximum_storage_entries, cancellation)
            .map_err(map_storage_open),
    }
}

fn open_set_root(
    sets: &PinnedDirectory,
    name: &OsStr,
) -> Result<Option<PinnedDirectory>, ArtifactSetInventoryError> {
    match sets.open_optional_child_directory(name) {
        Ok(root) => Ok(root),
        Err(crate::ArtifactInventoryError::UnsafeStorageLayout) => Ok(None),
        Err(error) => Err(map_storage_open(error)),
    }
}

fn plan_if_within_set_ceilings(
    manifest: &ArtifactSetManifest,
    limits: ArtifactSetInventoryLimits,
) -> Result<Option<ValidatedSetPlan>, ArtifactSetInventoryError> {
    let bounds = ArtifactSetPlanBounds {
        members: limits.maximum_members,
        member_bytes: limits.maximum_member_bytes,
        total_bytes: limits.maximum_total_verification_bytes,
        tree_entries: limits.maximum_tree_entries,
    };
    validate_plan_bounds(bounds).map_err(|_| ArtifactSetInventoryError::InvalidLimits)?;
    match plan_artifact_set(manifest, bounds) {
        Ok(plan) => Ok(Some(plan)),
        Err(
            crate::ArtifactSetImportError::TooManyMembers { .. }
            | crate::ArtifactSetImportError::MemberTooLarge { .. }
            | crate::ArtifactSetImportError::ArtifactSetTooLarge { .. }
            | crate::ArtifactSetImportError::TreeEntryLimitExceeded,
        ) => Ok(None),
        Err(crate::ArtifactSetImportError::InvalidLimits) => {
            Err(ArtifactSetInventoryError::InvalidLimits)
        }
        Err(_) => Err(ArtifactSetInventoryError::State(StoreError::CorruptRecord)),
    }
}

fn tree_matches_plan(
    snapshot: &ManagedTreeSnapshot,
    manifest: &ArtifactSetManifest,
    plan: &ValidatedSetPlan,
) -> bool {
    let mut expected = BTreeMap::new();
    for directory in &plan.directories {
        expected.insert(directory.as_str(), (ManagedTreeEntryKind::Directory, 0));
    }
    for member in manifest.members() {
        expected.insert(
            member.relative_path().as_str(),
            (ManagedTreeEntryKind::RegularFile, member.byte_size()),
        );
    }
    snapshot.entries().len() == plan.tree_entries
        && snapshot.entries().len() == expected.len()
        && snapshot.entries().iter().all(|entry| {
            expected
                .get(entry.relative_path().as_str())
                .is_some_and(|(kind, size)| {
                    entry.kind() == *kind
                        && entry.byte_size() == *size
                        && (entry.kind() != ManagedTreeEntryKind::RegularFile
                            || entry.has_single_link())
                })
        })
}

fn canonical_set_name(id: &ArtifactSetId) -> String {
    format!("{SET_STORAGE_KEY_PREFIX}{}", id.digest().as_str())
}

fn parse_canonical_set_name(name: &str) -> Option<ArtifactSetId> {
    let digest = name.strip_prefix(SET_STORAGE_KEY_PREFIX)?;
    Digest::from_sha256_hex(digest)
        .ok()
        .map(ArtifactSetId::from_digest)
}

fn find_exact_entry<'a>(
    entries: &'a [DirectoryEntrySnapshot],
    name: &OsStr,
) -> Option<&'a DirectoryEntrySnapshot> {
    entries.iter().find(|entry| entry.name == name)
}
