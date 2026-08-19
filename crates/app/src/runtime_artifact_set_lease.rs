use std::{
    ffi::{OsStr, OsString},
    fs::File,
};

use rewrite_model::{ArtifactSetId, ArtifactSetManifest};
use rewrite_model_store::ArtifactStateStore;
use rewrite_types::CancellationToken;

use crate::{
    ArtifactSetInstallationKey,
    artifact_repository::DataDirectoryGuard,
    artifact_set_import::{
        SETS_DIRECTORY, map_managed_tree, map_set_capacity, map_storage_open, plan_artifact_set,
        validate_plan_bounds, verify_final_tree,
    },
    artifact_storage::{
        ExactEntryCapacity, LIFECYCLE_LOCK_FILE, ManagedTreeLimits, PinnedDirectory,
        fingerprint_std_file, lock_shared,
    },
};

mod contract;
#[cfg(test)]
mod tests;

use contract::map_set_lease_error;
pub(crate) use contract::set_lease_error_kind;
pub use contract::{ArtifactSetLeaseError, RuntimeArtifactSetLeaseLimits};

/// Shared lifecycle lease retained for one managed artifact set's use lifetime.
///
/// The lease pins the repository boundary, the managed set root, and the shared
/// lifecycle lock. Every exclusive repository or storage operation, including
/// import, reconciliation, removal, and migration, therefore fails while any
/// lease remains live. Read-only inspection continues to succeed.
///
/// A lease is point-in-time byte evidence. It does not qualify an artifact set,
/// attest a live runtime, authorize a role, prove that the manifest lists every
/// file that can affect runtime output, or protect managed bytes from a
/// non-cooperating same-user process outside the pinned boundary.
pub struct RuntimeArtifactSetLease {
    _set_root: PinnedDirectory,
    _sets: PinnedDirectory,
    _storage_root: PinnedDirectory,
    _lifecycle_lock: File,
    _repository: DataDirectoryGuard,
    key: ArtifactSetInstallationKey,
    manifest: ArtifactSetManifest,
}

impl RuntimeArtifactSetLease {
    pub(crate) fn from_parts(
        repository: DataDirectoryGuard,
        acquired: AcquiredArtifactSet,
    ) -> Self {
        Self {
            _set_root: acquired.set_root,
            _sets: acquired.sets,
            _storage_root: acquired.storage_root,
            _lifecycle_lock: acquired.lifecycle_lock,
            _repository: repository,
            key: acquired.key,
            manifest: acquired.manifest,
        }
    }

    /// Exact installation key protected by this live lease.
    ///
    /// After a completed set removal, a later exact reimport uses the next
    /// generation so an old removal retry cannot delete the reinstall.
    #[must_use]
    pub const fn key(&self) -> &ArtifactSetInstallationKey {
        &self.key
    }

    /// Registered canonical manifest whose members were verified for this lease.
    #[must_use]
    pub const fn manifest(&self) -> &ArtifactSetManifest {
        &self.manifest
    }
}

impl std::fmt::Debug for RuntimeArtifactSetLease {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeArtifactSetLease")
            .field("key", &self.key)
            .field("members", &self.manifest.members().len())
            .finish_non_exhaustive()
    }
}

pub(crate) struct AcquiredArtifactSet {
    set_root: PinnedDirectory,
    sets: PinnedDirectory,
    storage_root: PinnedDirectory,
    lifecycle_lock: File,
    key: ArtifactSetInstallationKey,
    manifest: ArtifactSetManifest,
}

struct PinnedSetStorage {
    root: PinnedDirectory,
    sets: PinnedDirectory,
    lock: File,
}

/// Verifies one exact registered artifact set under the shared lifecycle lock.
///
/// Durable state is read before and after whole-tree byte verification, and the
/// content-derived set-root name is recomputed from the manifest rather than
/// resolved from a persisted storage key.
pub(crate) fn acquire_artifact_set(
    repository_root: &PinnedDirectory,
    storage_name: &OsStr,
    store: &ArtifactStateStore,
    artifact_set_id: &ArtifactSetId,
    limits: RuntimeArtifactSetLeaseLimits,
    cancellation: &CancellationToken,
) -> Result<AcquiredArtifactSet, ArtifactSetLeaseError> {
    let bounds = limits.plan_bounds();
    validate_plan_bounds(bounds).map_err(map_set_lease_error)?;
    limits.validate_storage_ceiling()?;
    ensure_not_cancelled(cancellation)?;

    let installation = store
        .artifact_set_installation(artifact_set_id)
        .map_err(ArtifactSetLeaseError::State)?
        .ok_or(ArtifactSetLeaseError::ArtifactSetNotInstalled)?;
    let manifest = store
        .artifact_set_manifest(artifact_set_id)
        .map_err(ArtifactSetLeaseError::State)?
        .ok_or(ArtifactSetLeaseError::StateStorageMismatch)?;
    let plan = plan_artifact_set(&manifest, bounds).map_err(map_set_lease_error)?;
    if plan.artifact_set_id != *artifact_set_id || plan.installed != installation.installed {
        return Err(ArtifactSetLeaseError::StateStorageMismatch);
    }

    let storage = open_managed_set_storage(repository_root, storage_name)?;
    let set_name = OsString::from(&plan.storage_key);
    let set_root = open_registered_set_root(&storage, &set_name, limits, cancellation)?;
    let tree_limits = ManagedTreeLimits::new(limits.maximum_tree_entries)
        .map_err(|error| map_set_lease_error(map_managed_tree(error)))?;
    verify_final_tree(&set_root, &manifest, &plan, tree_limits, cancellation)
        .map_err(map_set_lease_error)?;
    recheck_set_boundary(&storage, &set_name, &set_root, limits)?;

    let current_installation = store
        .artifact_set_installation(artifact_set_id)
        .map_err(ArtifactSetLeaseError::State)?;
    let current_manifest = store
        .artifact_set_manifest(artifact_set_id)
        .map_err(ArtifactSetLeaseError::State)?;
    if current_installation.as_ref() != Some(&installation)
        || current_manifest.as_ref() != Some(&manifest)
    {
        return Err(ArtifactSetLeaseError::StorageChanged);
    }

    Ok(AcquiredArtifactSet {
        key: ArtifactSetInstallationKey::from_stored(&installation),
        manifest,
        set_root,
        sets: storage.sets,
        storage_root: storage.root,
        lifecycle_lock: storage.lock,
    })
}

fn open_managed_set_storage(
    repository_root: &PinnedDirectory,
    storage_name: &OsStr,
) -> Result<PinnedSetStorage, ArtifactSetLeaseError> {
    let map = |error| map_set_lease_error(map_storage_open(error));
    let root = repository_root
        .open_child_directory(storage_name)
        .map_err(map)?;
    let (lock, _) = root
        .open_lock_file(OsStr::new(LIFECYCLE_LOCK_FILE))
        .map_err(map)?;
    lock_shared(&lock).map_err(map)?;
    let sets = root
        .open_child_directory(OsStr::new(SETS_DIRECTORY))
        .map_err(map)?;
    Ok(PinnedSetStorage { root, sets, lock })
}

fn open_registered_set_root(
    storage: &PinnedSetStorage,
    set_name: &OsStr,
    limits: RuntimeArtifactSetLeaseLimits,
    cancellation: &CancellationToken,
) -> Result<PinnedDirectory, ArtifactSetLeaseError> {
    match storage
        .sets
        .exact_entry_capacity(set_name, limits.maximum_storage_entries, cancellation)
        .map_err(|error| map_set_lease_error(map_set_capacity(error)))?
    {
        ExactEntryCapacity::Present => storage
            .sets
            .open_child_directory(set_name)
            .map_err(|error| map_set_lease_error(map_managed_tree(error))),
        ExactEntryCapacity::Available => Err(ArtifactSetLeaseError::StateStorageMismatch),
        ExactEntryCapacity::Full => Err(ArtifactSetLeaseError::StorageEntryLimitExceeded),
    }
}

fn recheck_set_boundary(
    storage: &PinnedSetStorage,
    set_name: &OsStr,
    set_root: &PinnedDirectory,
    limits: RuntimeArtifactSetLeaseLimits,
) -> Result<(), ArtifactSetLeaseError> {
    let map = |error| map_set_lease_error(map_managed_tree(error));
    let exact_name = storage
        .sets
        .exact_entry_capacity(
            set_name,
            limits.maximum_storage_entries,
            &CancellationToken::new(),
        )
        .map_err(|error| map_set_lease_error(map_set_capacity(error)))?
        == ExactEntryCapacity::Present;
    let named_set = storage
        .sets
        .child_directory_fingerprint(set_name)
        .map_err(map)?;
    let held_set = set_root.fingerprint().map_err(map)?;
    let sets = storage.sets.fingerprint().map_err(map)?;
    let named_sets = storage
        .root
        .child_directory_fingerprint(OsStr::new(SETS_DIRECTORY))
        .map_err(map)?;
    let root = storage.root.fingerprint().map_err(map)?;
    let lock_path = storage
        .root
        .child_file_fingerprint(OsStr::new(LIFECYCLE_LOCK_FILE))
        .map_err(map)?;
    let lock_handle = fingerprint_std_file(&storage.lock).map_err(map)?;
    if exact_name
        && held_set.same_identity(&named_set)
        && held_set.same_filesystem(&sets)
        && sets == named_sets
        && sets.same_filesystem(&root)
        && lock_path == lock_handle
    {
        Ok(())
    } else {
        Err(ArtifactSetLeaseError::StorageChanged)
    }
}

fn ensure_not_cancelled(cancellation: &CancellationToken) -> Result<(), ArtifactSetLeaseError> {
    if cancellation.is_cancelled() {
        Err(ArtifactSetLeaseError::Cancelled)
    } else {
        Ok(())
    }
}
