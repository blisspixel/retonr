use std::{collections::BTreeMap, io::Read as _, path::PathBuf};

use rewrite_model::{
    ArtifactSetManifest, ArtifactSetRelativePath, MAX_ARTIFACT_SET_MANIFEST_JSON_BYTES,
};
use rewrite_types::{CancellationToken, Digest};
use thiserror::Error;

use super::{
    RUNTIME_ADMISSION_EVIDENCE_BUNDLE_MANIFEST_PATH, RuntimeAdmissionEvidenceBundleLimits,
    RuntimeAdmissionEvidenceBundleSource, RuntimeAdmissionEvidenceContractError,
    RuntimeAdmissionEvidenceFoundation, RuntimeAdmissionEvidenceMember,
    RuntimeAdmissionEvidenceTreePlan,
};
use crate::{
    ArtifactInventoryError,
    artifact_storage::{
        ManagedTreeEntryKind, ManagedTreeLimits, ManagedTreeSnapshot, PinnedDirectory,
        StableMetadataFingerprint, fingerprint_std_file, is_indirect,
    },
};

/// Failure while independently reading an inert admission-evidence byte closure.
#[derive(Error)]
pub enum RuntimeAdmissionEvidenceBundleError {
    /// Invalid caller limits or canonical foundation.
    #[error(transparent)]
    Contract(#[from] RuntimeAdmissionEvidenceContractError),
    /// The exact canonical manifest or fixed tree did not match.
    #[error("runtime admission evidence tree is invalid")]
    InvalidTree,
    /// A member or aggregate exceeded its hard or caller-owned ceiling.
    #[error("runtime admission evidence limit was exceeded")]
    LimitExceeded,
    /// Named storage, retained identity, or exact bytes changed.
    #[error("runtime admission evidence changed")]
    Changed,
    /// An indirect or multiply linked storage entry was encountered.
    #[error("runtime admission evidence boundary is unsafe")]
    UnsafeBoundary,
    /// Cooperative cancellation stopped verification.
    #[error("runtime admission evidence verification was cancelled")]
    Cancelled,
    /// Storage could not be read.
    #[error("runtime admission evidence storage could not be read")]
    StorageIo(#[source] std::io::Error),
}

impl std::fmt::Debug for RuntimeAdmissionEvidenceBundleError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let class = match self {
            Self::Contract(_) => "contract",
            Self::InvalidTree => "invalid_tree",
            Self::LimitExceeded => "limit_exceeded",
            Self::Changed => "changed",
            Self::UnsafeBoundary => "unsafe_boundary",
            Self::Cancelled => "cancelled",
            Self::StorageIo(_) => "storage_io",
        };
        formatter
            .debug_tuple("RuntimeAdmissionEvidenceBundleError")
            .field(&class)
            .finish()
    }
}

/// Retained exact byte closure, without semantic review or admission authority.
///
/// Only the foundation is structurally parsed. Other members remain opaque bytes;
/// their presence and hashes do not establish that any admission control passed.
pub struct RuntimeAdmissionEvidenceBundleLease {
    path: PathBuf,
    root: PinnedDirectory,
    baseline: StableMetadataFingerprint,
    limits: RuntimeAdmissionEvidenceBundleLimits,
    contents: Contents,
}

impl std::fmt::Debug for RuntimeAdmissionEvidenceBundleLease {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeAdmissionEvidenceBundleLease")
            .field("manifest_id", &self.contents.manifest.artifact_set_id())
            .field("foundation_id", self.contents.foundation.foundation_id())
            .finish_non_exhaustive()
    }
}

impl RuntimeAdmissionEvidenceBundleLease {
    /// Returns the canonical independently rehashed member inventory.
    #[must_use]
    pub const fn manifest(&self) -> &ArtifactSetManifest {
        &self.contents.manifest
    }

    /// Returns the inert foundation, without verifying its external subjects.
    #[must_use]
    pub const fn foundation(&self) -> &RuntimeAdmissionEvidenceFoundation {
        &self.contents.foundation
    }

    /// Returns the bounded fixed plan independently derived from member sizes.
    #[must_use]
    pub const fn tree_plan(&self) -> &RuntimeAdmissionEvidenceTreePlan {
        &self.contents.plan
    }

    /// Freshly verifies the complete named and retained closure before returning
    /// the exact snapshot bytes of one fixed member.
    ///
    /// # Errors
    /// Returns [`RuntimeAdmissionEvidenceBundleError`] for cancellation or drift.
    pub fn member_bytes(
        &self,
        member: RuntimeAdmissionEvidenceMember,
        cancellation: &CancellationToken,
    ) -> Result<&[u8], RuntimeAdmissionEvidenceBundleError> {
        self.revalidate(cancellation)?;
        Ok(&self.contents.bytes[member.relative_path()])
    }

    /// Rehashes every member and rechecks the exact tree and root identity.
    ///
    /// # Errors
    /// Returns [`RuntimeAdmissionEvidenceBundleError`] for cancellation, unsafe
    /// storage, changed bytes, invalid structure, or exceeded bounds.
    pub fn revalidate(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), RuntimeAdmissionEvidenceBundleError> {
        self.check_root(cancellation)?;
        let current = verify_root(&self.root, self.limits, cancellation)?;
        self.check_root(cancellation)?;
        if current != self.contents {
            return Err(RuntimeAdmissionEvidenceBundleError::Changed);
        }
        Ok(())
    }

    fn check_root(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), RuntimeAdmissionEvidenceBundleError> {
        active(cancellation)?;
        let held = self.root.fingerprint().map_err(map_storage)?.stable();
        let named = PinnedDirectory::fingerprint_path(&self.path)
            .map_err(map_storage)?
            .stable();
        if held != self.baseline || named != self.baseline {
            return Err(RuntimeAdmissionEvidenceBundleError::Changed);
        }
        Ok(())
    }
}

/// Read-only exact-tree verifier that grants no admission or policy authority.
#[derive(Clone, Copy, Debug, Default)]
pub struct RuntimeAdmissionEvidenceBundleVerifier;

impl RuntimeAdmissionEvidenceBundleVerifier {
    /// Pins and independently rehashes the fixed twelve-member evidence tree.
    ///
    /// Performs no publication, execution, network access, or control adjudication.
    /// Limits and cancellation are checked before accessing source storage.
    ///
    /// # Errors
    /// Returns [`RuntimeAdmissionEvidenceBundleError`] for an invalid manifest,
    /// noncanonical foundation, unsafe tree, changed bytes, or exceeded ceilings.
    pub fn acquire(
        source: &RuntimeAdmissionEvidenceBundleSource,
        limits: RuntimeAdmissionEvidenceBundleLimits,
        cancellation: &CancellationToken,
    ) -> Result<RuntimeAdmissionEvidenceBundleLease, RuntimeAdmissionEvidenceBundleError> {
        let limits = limits.validate()?;
        active(cancellation)?;
        let metadata = std::fs::symlink_metadata(&source.path)
            .map_err(RuntimeAdmissionEvidenceBundleError::StorageIo)?;
        if is_indirect(&metadata) || !metadata.is_dir() {
            return Err(RuntimeAdmissionEvidenceBundleError::UnsafeBoundary);
        }
        let root = PinnedDirectory::open_existing(&source.path).map_err(map_storage)?;
        acquire_retained(source.path.clone(), root, limits, cancellation)
    }
}

pub(super) fn acquire_retained(
    path: PathBuf,
    root: PinnedDirectory,
    limits: RuntimeAdmissionEvidenceBundleLimits,
    cancellation: &CancellationToken,
) -> Result<RuntimeAdmissionEvidenceBundleLease, RuntimeAdmissionEvidenceBundleError> {
    let baseline = root.fingerprint().map_err(map_storage)?.stable();
    let contents = verify_root(&root, limits, cancellation)?;
    let lease = RuntimeAdmissionEvidenceBundleLease {
        path,
        root,
        baseline,
        limits,
        contents,
    };
    lease.check_root(cancellation)?;
    Ok(lease)
}

pub(super) fn verify_staged(
    root: &PinnedDirectory,
    manifest: &ArtifactSetManifest,
    limits: RuntimeAdmissionEvidenceBundleLimits,
    cancellation: &CancellationToken,
) -> Result<(), RuntimeAdmissionEvidenceBundleError> {
    let contents = verify_root(root, limits, cancellation)?;
    if &contents.manifest != manifest {
        return Err(RuntimeAdmissionEvidenceBundleError::Changed);
    }
    Ok(())
}

#[derive(Eq, PartialEq)]
struct Contents {
    manifest: ArtifactSetManifest,
    foundation: RuntimeAdmissionEvidenceFoundation,
    plan: RuntimeAdmissionEvidenceTreePlan,
    bytes: BTreeMap<String, Vec<u8>>,
}

fn verify_root(
    root: &PinnedDirectory,
    limits: RuntimeAdmissionEvidenceBundleLimits,
    cancellation: &CancellationToken,
) -> Result<Contents, RuntimeAdmissionEvidenceBundleError> {
    active(cancellation)?;
    let tree_limits = ManagedTreeLimits::new(limits.maximum_tree_entries).map_err(map_storage)?;
    let before = root
        .enumerate_tree(tree_limits, cancellation)
        .map_err(map_storage)?;
    let manifest_bytes = read_file(
        root,
        RUNTIME_ADMISSION_EVIDENCE_BUNDLE_MANIFEST_PATH,
        MAX_ARTIFACT_SET_MANIFEST_JSON_BYTES as u64,
        cancellation,
    )?;
    let manifest = ArtifactSetManifest::from_json_bytes(&manifest_bytes)
        .map_err(|_| RuntimeAdmissionEvidenceBundleError::InvalidTree)?;
    if manifest.canonical_json().as_bytes() != manifest_bytes {
        return Err(RuntimeAdmissionEvidenceBundleError::InvalidTree);
    }
    let total = manifest
        .total_byte_size()
        .checked_add(manifest_bytes.len() as u64)
        .ok_or(RuntimeAdmissionEvidenceBundleError::LimitExceeded)?;
    if total > limits.maximum_total_bytes {
        return Err(RuntimeAdmissionEvidenceBundleError::LimitExceeded);
    }
    if manifest.members().len() != RuntimeAdmissionEvidenceMember::ALL.len() {
        return Err(RuntimeAdmissionEvidenceBundleError::InvalidTree);
    }
    let mut expected = BTreeMap::new();
    expected.insert(
        RUNTIME_ADMISSION_EVIDENCE_BUNDLE_MANIFEST_PATH,
        (
            ManagedTreeEntryKind::RegularFile,
            manifest_bytes.len() as u64,
        ),
    );
    for directory in ["controls", "native", "execution", "policy"] {
        expected.insert(directory, (ManagedTreeEntryKind::Directory, 0));
    }
    let mut bytes = BTreeMap::new();
    let mut sizes = [0; super::RUNTIME_ADMISSION_EVIDENCE_MEMBER_COUNT];
    for (index, member) in RuntimeAdmissionEvidenceMember::ALL.into_iter().enumerate() {
        active(cancellation)?;
        let declared = manifest
            .members()
            .iter()
            .find(|entry| entry.relative_path().as_str() == member.relative_path())
            .ok_or(RuntimeAdmissionEvidenceBundleError::InvalidTree)?;
        if declared.byte_size() > member.maximum_bytes() {
            return Err(RuntimeAdmissionEvidenceBundleError::LimitExceeded);
        }
        sizes[index] = declared.byte_size();
        expected.insert(
            member.relative_path(),
            (ManagedTreeEntryKind::RegularFile, declared.byte_size()),
        );
    }
    let plan = RuntimeAdmissionEvidenceTreePlan::compile(sizes, limits)?;
    validate_tree(&before, &expected)?;
    for member in RuntimeAdmissionEvidenceMember::ALL {
        let declared = manifest
            .members()
            .iter()
            .find(|entry| entry.relative_path().as_str() == member.relative_path())
            .ok_or(RuntimeAdmissionEvidenceBundleError::InvalidTree)?;
        let content = read_file(
            root,
            member.relative_path(),
            member.maximum_bytes(),
            cancellation,
        )?;
        if content.len() as u64 != declared.byte_size()
            || Digest::sha256(&content) != *declared.artifact_id().digest()
        {
            return Err(RuntimeAdmissionEvidenceBundleError::Changed);
        }
        bytes.insert(member.relative_path().to_owned(), content);
    }
    let foundation = RuntimeAdmissionEvidenceFoundation::from_canonical_bytes(
        &bytes[RuntimeAdmissionEvidenceMember::Foundation.relative_path()],
    )?;
    let after = root
        .enumerate_tree(tree_limits, cancellation)
        .map_err(map_storage)?;
    if before != after {
        return Err(RuntimeAdmissionEvidenceBundleError::Changed);
    }
    active(cancellation)?;
    Ok(Contents {
        manifest,
        foundation,
        plan,
        bytes,
    })
}

fn validate_tree(
    snapshot: &ManagedTreeSnapshot,
    expected: &BTreeMap<&str, (ManagedTreeEntryKind, u64)>,
) -> Result<(), RuntimeAdmissionEvidenceBundleError> {
    if snapshot.entries().len() != expected.len() {
        return Err(RuntimeAdmissionEvidenceBundleError::InvalidTree);
    }
    for entry in snapshot.entries() {
        if !expected
            .get(entry.relative_path().as_str())
            .is_some_and(|(kind, size)| *kind == entry.kind() && *size == entry.byte_size())
        {
            return Err(RuntimeAdmissionEvidenceBundleError::InvalidTree);
        }
        if entry.kind() == ManagedTreeEntryKind::RegularFile && !entry.has_single_link() {
            return Err(RuntimeAdmissionEvidenceBundleError::UnsafeBoundary);
        }
    }
    Ok(())
}

fn read_file(
    root: &PinnedDirectory,
    path: &str,
    maximum: u64,
    cancellation: &CancellationToken,
) -> Result<Vec<u8>, RuntimeAdmissionEvidenceBundleError> {
    active(cancellation)?;
    let path = ArtifactSetRelativePath::new(path.to_owned())
        .map_err(|_| RuntimeAdmissionEvidenceBundleError::InvalidTree)?;
    let mut opened = root
        .open_relative_regular_file(&path)
        .map_err(map_storage)?;
    if !opened.fingerprint.has_single_link() {
        return Err(RuntimeAdmissionEvidenceBundleError::UnsafeBoundary);
    }
    if opened.byte_size > maximum {
        return Err(RuntimeAdmissionEvidenceBundleError::LimitExceeded);
    }
    let mut bytes = Vec::with_capacity(
        usize::try_from(opened.byte_size)
            .map_err(|_| RuntimeAdmissionEvidenceBundleError::LimitExceeded)?,
    );
    let mut remaining = opened.byte_size;
    let mut buffer = vec![0; 64 * 1024];
    while remaining != 0 {
        active(cancellation)?;
        let limit = usize::try_from(remaining)
            .unwrap_or(usize::MAX)
            .min(buffer.len());
        let read = opened
            .file
            .read(&mut buffer[..limit])
            .map_err(RuntimeAdmissionEvidenceBundleError::StorageIo)?;
        if read == 0 {
            return Err(RuntimeAdmissionEvidenceBundleError::Changed);
        }
        bytes.extend_from_slice(&buffer[..read]);
        remaining -= read as u64;
    }
    let mut trailing = [0];
    if opened
        .file
        .read(&mut trailing)
        .map_err(RuntimeAdmissionEvidenceBundleError::StorageIo)?
        != 0
        || fingerprint_std_file(&opened.file).map_err(map_storage)? != opened.fingerprint
    {
        return Err(RuntimeAdmissionEvidenceBundleError::Changed);
    }
    root.recheck_relative_regular_file(&path, &opened.fingerprint)
        .map_err(map_storage)?;
    active(cancellation)?;
    Ok(bytes)
}

pub(super) fn active(
    cancellation: &CancellationToken,
) -> Result<(), RuntimeAdmissionEvidenceBundleError> {
    if cancellation.is_cancelled() {
        Err(RuntimeAdmissionEvidenceBundleError::Cancelled)
    } else {
        Ok(())
    }
}

pub(super) fn map_storage(error: ArtifactInventoryError) -> RuntimeAdmissionEvidenceBundleError {
    match error {
        ArtifactInventoryError::StorageIo(error) => {
            RuntimeAdmissionEvidenceBundleError::StorageIo(error)
        }
        ArtifactInventoryError::Cancelled => RuntimeAdmissionEvidenceBundleError::Cancelled,
        ArtifactInventoryError::InvalidLimits
        | ArtifactInventoryError::StorageEntryLimitExceeded
        | ArtifactInventoryError::StateEntryLimitExceeded
        | ArtifactInventoryError::TotalVerificationLimitExceeded => {
            RuntimeAdmissionEvidenceBundleError::LimitExceeded
        }
        ArtifactInventoryError::ConcurrentModification => {
            RuntimeAdmissionEvidenceBundleError::Changed
        }
        _ => RuntimeAdmissionEvidenceBundleError::UnsafeBoundary,
    }
}

#[cfg(test)]
mod tests;
