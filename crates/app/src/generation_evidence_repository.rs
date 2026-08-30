//! Application-owned local storage root for candidate generation evidence.

use std::{ffi::OsStr, fs::File, path::Path};

use rewrite_model::{
    CandidateGenerationEvidenceBundleId, GenerationQualificationPlanId, PlannedCandidateAttemptId,
};
use rewrite_model_store::{
    CandidateGenerationEvidenceBundleStorageV1, CandidateGenerationEvidenceStorageRootId,
    CandidateGenerationEvidenceStorageV1Limits,
};

use crate::artifact_storage::{
    ManagedFile, MetadataFingerprint, PinnedDirectory, fingerprint_std_file, lock_shared,
};

#[path = "generation_evidence_repository/contract.rs"]
mod contract;
#[path = "generation_evidence_repository/layout.rs"]
mod layout;
#[path = "generation_evidence_repository/marker.rs"]
mod marker;
#[path = "generation_evidence_repository/publication.rs"]
mod publication;

pub use contract::CandidateGenerationEvidenceRepositoryError;
use contract::{map_active_storage, map_exclusive_lock, map_initial_storage};
use layout::{
    FixedChildState, FixedEntryKind, fixed_child_state, require_named_directory_identity,
    validate_bundles_layout, validate_root_layout,
};
use marker::{read_marker_bytes_from_handle, read_root_marker, write_new_root_marker};
pub use publication::{
    CandidateGenerationEvidenceRepositoryPublication,
    CandidateGenerationEvidenceRepositoryReadbackLease,
};

const STORAGE_DIRECTORY: &str = "generation-evidence";
const LIFECYCLE_LOCK_FILE: &str = ".generation-evidence.lock";
const ROOT_MARKER_FILE: &str = "root-identity.json";
const STAGING_DIRECTORY: &str = ".staging";
const BUNDLES_DIRECTORY: &str = "bundles";
const LAYOUT_VERSION_DIRECTORY: &str = "v1";
const ROOT_MARKER_SCHEMA_VERSION: u16 = 1;
const ROOT_NONCE_BYTES: usize = 32;
const MAX_ROOT_MARKER_BYTES: u64 = 128;
const MAX_DATA_DIRECTORY_ENTRIES: usize = 4_096;
const ROOT_LAYOUT_ENTRY_COUNT: usize = 4;
const ROOT_ID_DOMAIN: &[u8] = b"rewrite.candidate-generation-evidence-storage-root.v1\0";

/// Pinned application-owned root for immutable candidate generation evidence.
///
/// The root retains its marker, fixed layout directories, and shared lifecycle
/// lock. Its absolute host location remains private. The root ID is local storage
/// identity only and does not participate in portable evidence identities.
pub struct CandidateGenerationEvidenceRepository {
    data_directory: PinnedDirectory,
    root: PinnedDirectory,
    staging: PinnedDirectory,
    bundles: PinnedDirectory,
    layout_version: PinnedDirectory,
    lifecycle_lock: File,
    lifecycle_lock_fingerprint: MetadataFingerprint,
    marker: ManagedFile,
    root_id: CandidateGenerationEvidenceStorageRootId,
}

impl CandidateGenerationEvidenceRepository {
    /// Initializes a new evidence root below an existing app data directory.
    ///
    /// The operation exclusively creates `generation-evidence` and never opens
    /// an existing root as if it were new.
    ///
    /// # Errors
    ///
    /// Returns [`CandidateGenerationEvidenceRepositoryError`] when the data
    /// directory is unsafe, a case alias or existing root is present, secure
    /// randomness is unavailable, or the fixed root cannot be durably created.
    pub fn initialize(
        data_directory: impl AsRef<Path>,
    ) -> Result<Self, CandidateGenerationEvidenceRepositoryError> {
        let data_directory =
            PinnedDirectory::open_existing(data_directory.as_ref()).map_err(map_initial_storage)?;
        match fixed_child_state(
            &data_directory,
            STORAGE_DIRECTORY,
            FixedEntryKind::Directory,
            MAX_DATA_DIRECTORY_ENTRIES,
        )? {
            FixedChildState::Present => {
                return Err(CandidateGenerationEvidenceRepositoryError::AlreadyInitialized);
            }
            FixedChildState::Absent => {}
        }

        let root = data_directory
            .create_child_directory_exclusive(OsStr::new(STORAGE_DIRECTORY))
            .map_err(map_initial_storage)?;
        #[cfg(unix)]
        crate::artifact_storage::set_private_directory_permissions(&root)
            .map_err(map_initial_storage)?;

        let (lock, lock_fingerprint) = root
            .open_or_create_lock_file(OsStr::new(LIFECYCLE_LOCK_FILE))
            .map_err(map_initial_storage)?;
        if !lock_fingerprint.has_single_link() {
            return Err(CandidateGenerationEvidenceRepositoryError::UnsafeBoundary);
        }
        lock.try_lock().map_err(map_exclusive_lock)?;

        let staging = root
            .ensure_child_directory(OsStr::new(STAGING_DIRECTORY))
            .map_err(map_initial_storage)?;
        let bundles = root
            .ensure_child_directory(OsStr::new(BUNDLES_DIRECTORY))
            .map_err(map_initial_storage)?;
        let layout_version = bundles
            .ensure_child_directory(OsStr::new(LAYOUT_VERSION_DIRECTORY))
            .map_err(map_initial_storage)?;
        let root_id = write_new_root_marker(&root)?;

        layout_version.sync().map_err(map_initial_storage)?;
        bundles.sync().map_err(map_initial_storage)?;
        staging.sync().map_err(map_initial_storage)?;
        root.sync().map_err(map_initial_storage)?;
        data_directory.sync().map_err(map_initial_storage)?;
        lock_fingerprint.release();
        drop(lock);

        Self::open_existing_with_pinned_data_directory(data_directory, &root_id)
    }

    /// Reopens an initialized evidence root and requires its expected stable ID.
    ///
    /// Moving the complete app data directory preserves the marker-derived ID.
    /// Replacing only its evidence root does not.
    ///
    /// # Errors
    ///
    /// Returns [`CandidateGenerationEvidenceRepositoryError`] when the fixed
    /// layout, marker, lifecycle lock, or expected root identity does not match.
    pub fn open_existing(
        data_directory: impl AsRef<Path>,
        expected_root_id: &CandidateGenerationEvidenceStorageRootId,
    ) -> Result<Self, CandidateGenerationEvidenceRepositoryError> {
        let data_directory =
            PinnedDirectory::open_existing(data_directory.as_ref()).map_err(map_initial_storage)?;
        Self::open_existing_with_pinned_data_directory(data_directory, expected_root_id)
    }

    /// Returns the stable local identity read from the canonical root marker.
    #[must_use]
    pub const fn root_id(&self) -> &CandidateGenerationEvidenceStorageRootId {
        &self.root_id
    }

    /// Compiles one root-bound canonical durable bundle reference.
    ///
    /// Only the three fixed read-side reacquisition limits enter durable state.
    /// Publication-parent and staging ceilings remain process-local concerns.
    ///
    /// # Errors
    ///
    /// Returns [`CandidateGenerationEvidenceRepositoryError`] if the retained
    /// root changed or the typed durable contract rejects the reference.
    pub fn bundle_storage_reference(
        &self,
        qualification_plan_id: GenerationQualificationPlanId,
        planned_attempt_id: PlannedCandidateAttemptId,
        evidence_bundle_id: CandidateGenerationEvidenceBundleId,
        limits: CandidateGenerationEvidenceStorageV1Limits,
    ) -> Result<
        CandidateGenerationEvidenceBundleStorageV1,
        CandidateGenerationEvidenceRepositoryError,
    > {
        self.revalidate()?;
        CandidateGenerationEvidenceBundleStorageV1::new(
            self.root_id.clone(),
            qualification_plan_id,
            planned_attempt_id,
            evidence_bundle_id,
            limits,
        )
        .map_err(CandidateGenerationEvidenceRepositoryError::StorageContract)
    }

    /// Requires one durable bundle reference to belong to this retained root.
    ///
    /// # Errors
    ///
    /// Returns [`CandidateGenerationEvidenceRepositoryError`] if the retained
    /// layout changed or the reference names another local root.
    pub fn validate_bundle_storage(
        &self,
        storage: &CandidateGenerationEvidenceBundleStorageV1,
    ) -> Result<(), CandidateGenerationEvidenceRepositoryError> {
        self.revalidate()?;
        if storage.storage_root_id() == &self.root_id {
            Ok(())
        } else {
            Err(CandidateGenerationEvidenceRepositoryError::RootIdentityMismatch)
        }
    }

    /// Rechecks every retained fixed-layout boundary and root-marker identity.
    ///
    /// # Errors
    ///
    /// Returns [`CandidateGenerationEvidenceRepositoryError`] when a named entry,
    /// retained handle, exact case, marker, or lifecycle lock changed.
    pub fn revalidate(&self) -> Result<(), CandidateGenerationEvidenceRepositoryError> {
        validate_root_layout(&self.root)?;
        validate_bundles_layout(&self.bundles)?;

        require_named_directory_identity(&self.data_directory, STORAGE_DIRECTORY, &self.root)?;
        require_named_directory_identity(&self.root, STAGING_DIRECTORY, &self.staging)?;
        require_named_directory_identity(&self.root, BUNDLES_DIRECTORY, &self.bundles)?;
        require_named_directory_identity(
            &self.bundles,
            LAYOUT_VERSION_DIRECTORY,
            &self.layout_version,
        )?;
        self.revalidate_lock()?;
        self.revalidate_marker()
    }

    fn revalidate_lock(&self) -> Result<(), CandidateGenerationEvidenceRepositoryError> {
        let held = fingerprint_std_file(&self.lifecycle_lock).map_err(map_active_storage)?;
        let named = self
            .root
            .child_file_fingerprint(OsStr::new(LIFECYCLE_LOCK_FILE))
            .map_err(map_active_storage)?;
        if held == self.lifecycle_lock_fingerprint
            && named == self.lifecycle_lock_fingerprint
            && held.has_single_link()
        {
            Ok(())
        } else {
            Err(CandidateGenerationEvidenceRepositoryError::StorageChanged)
        }
    }

    fn revalidate_marker(&self) -> Result<(), CandidateGenerationEvidenceRepositoryError> {
        let held = fingerprint_std_file(&self.marker.file).map_err(map_active_storage)?;
        let named = self
            .root
            .child_file_fingerprint(OsStr::new(ROOT_MARKER_FILE))
            .map_err(map_active_storage)?;
        if held != self.marker.fingerprint
            || named != self.marker.fingerprint
            || !held.has_single_link()
        {
            return Err(CandidateGenerationEvidenceRepositoryError::StorageChanged);
        }
        let marker_bytes = read_marker_bytes_from_handle(&self.marker)?;
        let observed = marker::decode_root_marker_bytes(&marker_bytes)?;
        if observed == self.root_id {
            Ok(())
        } else {
            Err(CandidateGenerationEvidenceRepositoryError::StorageChanged)
        }
    }

    fn open_existing_with_pinned_data_directory(
        data_directory: PinnedDirectory,
        expected_root_id: &CandidateGenerationEvidenceStorageRootId,
    ) -> Result<Self, CandidateGenerationEvidenceRepositoryError> {
        if fixed_child_state(
            &data_directory,
            STORAGE_DIRECTORY,
            FixedEntryKind::Directory,
            MAX_DATA_DIRECTORY_ENTRIES,
        )? == FixedChildState::Absent
        {
            return Err(CandidateGenerationEvidenceRepositoryError::NotInitialized);
        }
        let root = data_directory
            .open_child_directory(OsStr::new(STORAGE_DIRECTORY))
            .map_err(map_initial_storage)?;
        validate_root_layout(&root)?;

        let (lifecycle_lock, lifecycle_lock_fingerprint) = root
            .open_lock_file(OsStr::new(LIFECYCLE_LOCK_FILE))
            .map_err(map_initial_storage)?;
        if !lifecycle_lock_fingerprint.has_single_link() {
            return Err(CandidateGenerationEvidenceRepositoryError::UnsafeBoundary);
        }
        lock_shared(&lifecycle_lock).map_err(map_initial_storage)?;

        let staging = root
            .open_child_directory(OsStr::new(STAGING_DIRECTORY))
            .map_err(map_initial_storage)?;
        let bundles = root
            .open_child_directory(OsStr::new(BUNDLES_DIRECTORY))
            .map_err(map_initial_storage)?;
        validate_bundles_layout(&bundles)?;
        let layout_version = bundles
            .open_child_directory(OsStr::new(LAYOUT_VERSION_DIRECTORY))
            .map_err(map_initial_storage)?;
        let (marker, root_id) = read_root_marker(&root)?;
        if &root_id != expected_root_id {
            return Err(CandidateGenerationEvidenceRepositoryError::RootIdentityMismatch);
        }

        let repository = Self {
            data_directory,
            root,
            staging,
            bundles,
            layout_version,
            lifecycle_lock,
            lifecycle_lock_fingerprint,
            marker,
            root_id,
        };
        repository.revalidate()?;
        Ok(repository)
    }
}

impl std::fmt::Debug for CandidateGenerationEvidenceRepository {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CandidateGenerationEvidenceRepository")
            .field("root_id", &self.root_id)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[path = "generation_evidence_repository/tests.rs"]
mod tests;
