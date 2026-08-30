use std::{ffi::OsString, fmt, io::Read as _, path::Path};

use rewrite_model::{
    ArtifactId, GenerationCaseId, GenerationCaseManifestV1, MAX_GENERATION_RETAINED_INPUT_BYTES,
};
use rewrite_model_store::{ArtifactStateStore, StoredArtifactInstallation};
use rewrite_types::{CancellationToken, Digest};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use crate::{
    ArtifactInventoryError,
    artifact_storage::{
        ExistingArtifactStorage, LifecycleLockMode, ManagedFile, fingerprint_std_file,
        hash_exact_bytes,
    },
};

/// Maximum source bytes retained for one generation case.
pub const MAX_GENERATION_CASE_SOURCE_BYTES: u64 = MAX_GENERATION_RETAINED_INPUT_BYTES;
/// Maximum managed artifact-directory entries inspected for one source lease.
pub const MAX_GENERATION_CASE_SOURCE_STORAGE_ENTRIES: usize = 4_096;

const READ_BUFFER_BYTES: usize = 64 * 1_024;

/// Fixed resource ceilings for one generation case source lease.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenerationCaseSourceLeaseLimits {
    /// Maximum bytes accepted for the exact source artifact.
    pub maximum_source_bytes: u64,
    /// Maximum entries inspected in the managed artifact directory.
    pub maximum_storage_entries: usize,
}

impl GenerationCaseSourceLeaseLimits {
    fn validate(self) -> Result<Self, GenerationCaseSourceLeaseError> {
        if self.maximum_source_bytes == 0
            || self.maximum_source_bytes > MAX_GENERATION_CASE_SOURCE_BYTES
            || self.maximum_storage_entries == 0
            || self.maximum_storage_entries > MAX_GENERATION_CASE_SOURCE_STORAGE_ENTRIES
        {
            return Err(GenerationCaseSourceLeaseError::InvalidLimits);
        }
        Ok(self)
    }
}

/// Failure while acquiring, reading, or revalidating exact generation source bytes.
#[derive(Debug, Error)]
pub enum GenerationCaseSourceLeaseError {
    /// A byte or tree ceiling is zero or exceeds the hard source boundary.
    #[error("generation case source lease limits are invalid")]
    InvalidLimits,
    /// The selected installation does not equal the case's exact source facts.
    #[error("generation case source installation relationship does not match")]
    CaseRelationshipMismatch,
    /// The exact selected installation generation is no longer current.
    #[error("generation case source installation changed")]
    InstallationChanged,
    /// A retained or freshly opened source object changed identity or content.
    #[error("generation case source bytes changed")]
    SourceChanged,
    /// Managed storage, durable state, or cancellation rejected the operation.
    #[error("generation case source boundary failed")]
    Boundary(#[source] ArtifactInventoryError),
}

impl From<ArtifactInventoryError> for GenerationCaseSourceLeaseError {
    fn from(error: ArtifactInventoryError) -> Self {
        Self::Boundary(error)
    }
}

/// Noncloneable retained capability for one exact generation source artifact.
///
/// The lease owns a shared lifecycle lock and a verified direct regular file. It
/// returns no path and exposes bytes only for the dynamic extent of a callback.
pub struct GenerationCaseSourceLease<'store> {
    storage: ExistingArtifactStorage,
    retained: ManagedFile,
    store: &'store ArtifactStateStore,
    selection: StoredArtifactInstallation,
    case: GenerationCaseManifestV1,
    name: OsString,
    limits: GenerationCaseSourceLeaseLimits,
}

impl<'store> GenerationCaseSourceLease<'store> {
    /// Acquires a retained source capability under the shared artifact lifecycle lock.
    ///
    /// The operation reads durable state before opening storage and again after
    /// complete byte verification. It does not create storage or return a path.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationCaseSourceLeaseError`] for invalid limits, a cross-case
    /// installation, stale state, unsafe storage, cancellation, or byte drift.
    pub fn acquire(
        root: impl AsRef<Path>,
        store: &'store ArtifactStateStore,
        selection: StoredArtifactInstallation,
        case: &GenerationCaseManifestV1,
        limits: GenerationCaseSourceLeaseLimits,
        cancellation: &CancellationToken,
    ) -> Result<Self, GenerationCaseSourceLeaseError> {
        let limits = limits.validate()?;
        validate_relationship(&selection, case, limits)?;
        require_current(store, &selection)?;
        ensure_active(cancellation)?;
        let storage = ExistingArtifactStorage::open(root, LifecycleLockMode::Shared)?;
        require_current(store, &selection)?;
        let name = OsString::from(selection.installed.artifact_digest.as_str());
        let mut retained =
            open_exact_source(&storage, &name, &selection, None, limits, cancellation)?;
        verify_opened_bytes(
            &storage,
            &name,
            &mut retained,
            &selection,
            limits,
            cancellation,
        )?;
        storage.validate_layout()?;
        require_current(store, &selection)?;
        Ok(Self {
            storage,
            retained,
            store,
            selection,
            case: case.clone(),
            name,
            limits,
        })
    }

    /// Revalidates durable state and every held and named source boundary.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationCaseSourceLeaseError`] for cancellation or any case,
    /// installation, storage, identity, size, link-count, or digest drift.
    pub fn revalidate(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), GenerationCaseSourceLeaseError> {
        ensure_active(cancellation)?;
        validate_relationship(&self.selection, &self.case, self.limits)?;
        require_current(self.store, &self.selection)?;
        self.storage.validate_layout()?;
        validate_retained(&self.retained, &self.selection)?;
        let mut opened = open_exact_source(
            &self.storage,
            &self.name,
            &self.selection,
            Some(&self.retained),
            self.limits,
            cancellation,
        )?;
        verify_opened_bytes(
            &self.storage,
            &self.name,
            &mut opened,
            &self.selection,
            self.limits,
            cancellation,
        )?;
        validate_retained(&self.retained, &self.selection)?;
        self.storage.validate_layout()?;
        require_current(self.store, &self.selection)?;
        ensure_active(cancellation)
    }

    /// Supplies exact source bytes only for the dynamic extent of one callback.
    ///
    /// The lease, durable state, held object, freshly opened named object, size,
    /// link count, and digest are checked before and after the read and again after
    /// the callback. A failed final check discards the callback result.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationCaseSourceLeaseError`] for cancellation or any state,
    /// identity, size, link-count, or content drift.
    pub fn with_source_bytes<T, F>(
        &self,
        cancellation: &CancellationToken,
        use_bytes: F,
    ) -> Result<T, GenerationCaseSourceLeaseError>
    where
        F: for<'bytes> FnOnce(&'bytes [u8]) -> T,
    {
        self.revalidate(cancellation)?;
        let bytes = self.read_exact_source_bytes(cancellation)?;
        self.revalidate(cancellation)?;
        let result = use_bytes(&bytes);
        self.revalidate(cancellation)?;
        Ok(result)
    }

    fn read_exact_source_bytes(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<Vec<u8>, GenerationCaseSourceLeaseError> {
        require_current(self.store, &self.selection)?;
        let mut opened = open_exact_source(
            &self.storage,
            &self.name,
            &self.selection,
            Some(&self.retained),
            self.limits,
            cancellation,
        )?;
        let expected = usize::try_from(self.selection.installed.byte_size)
            .map_err(|_| GenerationCaseSourceLeaseError::InvalidLimits)?;
        let mut bytes = Vec::with_capacity(expected);
        let mut buffer = vec![0_u8; READ_BUFFER_BYTES].into_boxed_slice();
        let mut hasher = Sha256::new();
        while bytes.len() < expected {
            ensure_active(cancellation)?;
            let maximum = (expected - bytes.len()).min(buffer.len());
            let count = opened
                .file
                .read(&mut buffer[..maximum])
                .map_err(ArtifactInventoryError::StorageIo)?;
            if count == 0 {
                return Err(GenerationCaseSourceLeaseError::SourceChanged);
            }
            hasher.update(&buffer[..count]);
            bytes.extend_from_slice(&buffer[..count]);
        }
        ensure_active(cancellation)?;
        let mut trailing = [0_u8; 1];
        let observed_digest = Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
            .map_err(|_| GenerationCaseSourceLeaseError::SourceChanged)?;
        if opened
            .file
            .read(&mut trailing)
            .map_err(ArtifactInventoryError::StorageIo)?
            != 0
            || observed_digest != self.selection.installed.artifact_digest
        {
            return Err(GenerationCaseSourceLeaseError::SourceChanged);
        }
        recheck_opened(
            &self.storage,
            &self.name,
            &opened,
            &self.selection,
            self.limits,
            cancellation,
        )?;
        require_current(self.store, &self.selection)?;
        Ok(bytes)
    }

    /// Returns the exact case identity bound by this source capability.
    #[must_use]
    pub const fn case_id(&self) -> &GenerationCaseId {
        self.case.case_id()
    }

    /// Returns the exact source artifact identity.
    #[must_use]
    pub const fn source_artifact_id(&self) -> &ArtifactId {
        self.case.source_artifact_id()
    }

    /// Returns the exact source digest.
    #[must_use]
    pub const fn source_digest(&self) -> &Digest {
        self.case.source_digest()
    }

    /// Returns the exact nonzero source byte count.
    #[must_use]
    pub const fn source_byte_count(&self) -> u64 {
        self.case.source_byte_count()
    }

    /// Reports whether this capability retains the supplied complete case manifest.
    #[must_use]
    pub fn matches_case_manifest(&self, case: &GenerationCaseManifestV1) -> bool {
        &self.case == case
    }
}

impl fmt::Debug for GenerationCaseSourceLease<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GenerationCaseSourceLease")
            .field("case_id", self.case.case_id())
            .field("source_artifact_id", self.case.source_artifact_id())
            .field("source_byte_count", &self.case.source_byte_count())
            .field("installation_epoch", &self.selection.epoch.get())
            .finish_non_exhaustive()
    }
}

fn validate_relationship(
    selection: &StoredArtifactInstallation,
    case: &GenerationCaseManifestV1,
    limits: GenerationCaseSourceLeaseLimits,
) -> Result<(), GenerationCaseSourceLeaseError> {
    selection
        .installed
        .validate()
        .map_err(|_| GenerationCaseSourceLeaseError::CaseRelationshipMismatch)?;
    let expected_key = format!("artifacts/{}", selection.installed.artifact_digest.as_str());
    if selection.installed.storage_key != expected_key
        || selection.installed.artifact_id != *case.source_artifact_id()
        || selection.installed.artifact_digest != *case.source_digest()
        || selection.installed.byte_size != case.source_byte_count()
        || selection.installed.byte_size == 0
        || selection.installed.byte_size > limits.maximum_source_bytes
    {
        return Err(GenerationCaseSourceLeaseError::CaseRelationshipMismatch);
    }
    Ok(())
}

fn require_current(
    store: &ArtifactStateStore,
    selection: &StoredArtifactInstallation,
) -> Result<(), GenerationCaseSourceLeaseError> {
    let (current, _) = store
        .artifact_removal_state(&selection.installed.artifact_id)
        .map_err(ArtifactInventoryError::State)?;
    if current.as_ref() == Some(selection) {
        Ok(())
    } else {
        Err(GenerationCaseSourceLeaseError::InstallationChanged)
    }
}

fn open_exact_source(
    storage: &ExistingArtifactStorage,
    name: &OsString,
    selection: &StoredArtifactInstallation,
    retained: Option<&ManagedFile>,
    limits: GenerationCaseSourceLeaseLimits,
    cancellation: &CancellationToken,
) -> Result<ManagedFile, GenerationCaseSourceLeaseError> {
    ensure_active(cancellation)?;
    let opened = storage
        .artifacts()
        .open_managed_file(name, limits.maximum_storage_entries, cancellation)?
        .ok_or(GenerationCaseSourceLeaseError::SourceChanged)?;
    if opened.byte_size != selection.installed.byte_size
        || !opened.fingerprint.has_single_link()
        || retained.is_some_and(|retained| opened.fingerprint != retained.fingerprint)
    {
        return Err(GenerationCaseSourceLeaseError::SourceChanged);
    }
    Ok(opened)
}

fn verify_opened_bytes(
    storage: &ExistingArtifactStorage,
    name: &OsString,
    opened: &mut ManagedFile,
    selection: &StoredArtifactInstallation,
    limits: GenerationCaseSourceLeaseLimits,
    cancellation: &CancellationToken,
) -> Result<(), GenerationCaseSourceLeaseError> {
    if hash_exact_bytes(
        &mut opened.file,
        selection.installed.byte_size,
        cancellation,
    )? != selection.installed.artifact_digest
    {
        return Err(GenerationCaseSourceLeaseError::SourceChanged);
    }
    recheck_opened(storage, name, opened, selection, limits, cancellation)
}

fn recheck_opened(
    storage: &ExistingArtifactStorage,
    name: &OsString,
    opened: &ManagedFile,
    selection: &StoredArtifactInstallation,
    limits: GenerationCaseSourceLeaseLimits,
    cancellation: &CancellationToken,
) -> Result<(), GenerationCaseSourceLeaseError> {
    let current = fingerprint_std_file(&opened.file)?;
    if current != opened.fingerprint
        || !current.has_single_link()
        || current.byte_size() != selection.installed.byte_size
    {
        return Err(GenerationCaseSourceLeaseError::SourceChanged);
    }
    storage.artifacts().recheck_managed_file_for_lifecycle(
        name,
        &opened.fingerprint,
        limits.maximum_storage_entries,
        cancellation,
    )?;
    Ok(())
}

fn validate_retained(
    retained: &ManagedFile,
    selection: &StoredArtifactInstallation,
) -> Result<(), GenerationCaseSourceLeaseError> {
    let current = fingerprint_std_file(&retained.file)?;
    if current == retained.fingerprint
        && current.has_single_link()
        && current.byte_size() == selection.installed.byte_size
    {
        Ok(())
    } else {
        Err(GenerationCaseSourceLeaseError::SourceChanged)
    }
}

fn ensure_active(cancellation: &CancellationToken) -> Result<(), GenerationCaseSourceLeaseError> {
    if cancellation.is_cancelled() {
        Err(ArtifactInventoryError::Cancelled.into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
