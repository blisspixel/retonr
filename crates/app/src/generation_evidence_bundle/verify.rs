use std::{
    ffi::OsStr,
    fs::File,
    io::{Read as _, Seek as _, SeekFrom},
    path::PathBuf,
};

use rewrite_model::{
    ArtifactId, ArtifactSetRelativePath, CandidateGenerationEvidenceBundleManifestV1,
    CandidateGenerationEvidenceBundleReadbackV1,
};
use rewrite_types::{CancellationToken, Digest};

use super::contract::{
    CandidateGenerationEvidenceBundleError, CandidateGenerationEvidenceBundleLimits,
    CandidateGenerationEvidenceBundleSource, digest_bytes, ensure_active, map_storage,
    reject_indirect_directory,
};
use crate::artifact_storage::{
    ManagedTreeLimits, PinnedDirectory, StableMetadataFingerprint, fingerprint_std_file,
    hash_exact_bytes,
};

mod pinned;
mod tree;
use pinned::{PinnedBundleRoot, RetainedMember};
use tree::{expected_tree, fixed_manifest_path, validate_snapshot};

const READ_BUFFER_BYTES: usize = 64 * 1_024;

/// Noncloneable retained lease for one independently verified bundle root.
pub struct CandidateGenerationEvidenceBundleReadbackLease {
    pinned: PinnedBundleRoot,
    manifest: CandidateGenerationEvidenceBundleManifestV1,
    manifest_bytes: Vec<u8>,
    manifest_member: RetainedMember,
    readback: CandidateGenerationEvidenceBundleReadbackV1,
    members: Vec<RetainedMember>,
    limits: CandidateGenerationEvidenceBundleLimits,
}

impl CandidateGenerationEvidenceBundleReadbackLease {
    /// Returns the exact independently matched manifest.
    #[must_use]
    pub const fn manifest(&self) -> &CandidateGenerationEvidenceBundleManifestV1 {
        &self.manifest
    }

    /// Returns the inert readback record derived after complete verification.
    #[must_use]
    pub const fn readback(&self) -> &CandidateGenerationEvidenceBundleReadbackV1 {
        &self.readback
    }

    /// Returns the exact bundle identity retained by this lease.
    #[must_use]
    pub const fn bundle_id(&self) -> &rewrite_model::CandidateGenerationEvidenceBundleId {
        self.manifest.evidence_bundle_id()
    }

    /// Returns the exact reserved manifest bytes held by this lease.
    #[must_use]
    pub fn canonical_manifest_bytes(&self) -> &[u8] {
        &self.manifest_bytes
    }

    /// Reports whether two leases retain the same root and every same member object.
    #[must_use]
    pub fn same_lease(&self, other: &Self) -> bool {
        self.bundle_id() == other.bundle_id()
            && self.pinned.baseline == other.pinned.baseline
            && same_member(&self.manifest_member, &other.manifest_member)
            && self.members.len() == other.members.len()
            && self
                .members
                .iter()
                .zip(&other.members)
                .all(|(left, right)| left.path == right.path && left.baseline == right.baseline)
    }

    /// Reports whether two leases name the same content-derived bundle.
    #[must_use]
    pub fn same_bundle(&self, other: &Self) -> bool {
        self.bundle_id() == other.bundle_id()
    }

    /// Reads one exact declared content member after complete lease revalidation.
    ///
    /// # Errors
    ///
    /// Returns [`CandidateGenerationEvidenceBundleError`] for a foreign path,
    /// caller ceiling, cancellation, or any held, named, tree, size, or digest drift.
    pub fn member_bytes(
        &self,
        path: &ArtifactSetRelativePath,
        maximum_bytes: u64,
        cancellation: &CancellationToken,
    ) -> Result<Vec<u8>, CandidateGenerationEvidenceBundleError> {
        self.revalidate(cancellation)?;
        let bytes = self.member_bytes_inside_revalidation(path, maximum_bytes, cancellation)?;
        self.revalidate(cancellation)?;
        Ok(bytes)
    }

    pub(super) fn member_bytes_inside_revalidation(
        &self,
        path: &ArtifactSetRelativePath,
        maximum_bytes: u64,
        cancellation: &CancellationToken,
    ) -> Result<Vec<u8>, CandidateGenerationEvidenceBundleError> {
        self.member_bytes_inside_revalidation_with_hooks(
            path,
            maximum_bytes,
            cancellation,
            || {},
            || {},
        )
    }

    fn member_bytes_inside_revalidation_with_hooks(
        &self,
        path: &ArtifactSetRelativePath,
        maximum_bytes: u64,
        cancellation: &CancellationToken,
        before_exact_read: impl FnOnce(),
        after_exact_read: impl FnOnce(),
    ) -> Result<Vec<u8>, CandidateGenerationEvidenceBundleError> {
        ensure_active(cancellation)?;
        let member = self
            .members
            .iter()
            .find(|member| &member.path == path)
            .ok_or(CandidateGenerationEvidenceBundleError::TreeMismatch)?;
        if member.byte_size > maximum_bytes {
            return Err(CandidateGenerationEvidenceBundleError::LimitExceeded);
        }
        let opened = open_retained_member(
            &self.pinned.root,
            path,
            member.artifact_id.digest(),
            member.byte_size,
            cancellation,
        )?;
        before_exact_read();
        let bytes = read_exact_bytes(
            opened
                .file
                .try_clone()
                .map_err(CandidateGenerationEvidenceBundleError::StorageIo)?,
            member.byte_size,
            cancellation,
        );
        after_exact_read();
        let bytes = bytes?;
        if digest_bytes(&bytes, cancellation)? != *member.artifact_id.digest()
            || fingerprint_std_file(&opened.file)
                .map_err(map_storage)?
                .stable()
                != opened.baseline
        {
            return Err(CandidateGenerationEvidenceBundleError::Changed);
        }
        let reopened = self
            .pinned
            .root
            .open_relative_regular_file(path)
            .map_err(map_storage)?;
        if reopened.fingerprint.stable() != opened.baseline {
            return Err(CandidateGenerationEvidenceBundleError::Changed);
        }
        Ok(bytes)
    }

    #[cfg(test)]
    pub(super) fn member_bytes_with_read_hooks(
        &self,
        path: &ArtifactSetRelativePath,
        maximum_bytes: u64,
        cancellation: &CancellationToken,
        before_exact_read: impl FnOnce(),
        after_exact_read: impl FnOnce(),
    ) -> Result<Vec<u8>, CandidateGenerationEvidenceBundleError> {
        self.revalidate(cancellation)?;
        let bytes = self.member_bytes_inside_revalidation_with_hooks(
            path,
            maximum_bytes,
            cancellation,
            before_exact_read,
            after_exact_read,
        )?;
        self.revalidate(cancellation)?;
        Ok(bytes)
    }

    /// Rehashes the complete held and named tree and compares every retained object.
    ///
    /// # Errors
    ///
    /// Returns [`CandidateGenerationEvidenceBundleError`] for cancellation or any
    /// root, tree, manifest, member, size, digest, link, or metadata drift.
    pub fn revalidate(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), CandidateGenerationEvidenceBundleError> {
        self.pinned.revalidate(cancellation)?;
        for member in std::iter::once(&self.manifest_member).chain(&self.members) {
            ensure_active(cancellation)?;
            if fingerprint_std_file(&member.file)
                .map_err(map_storage)?
                .stable()
                != member.baseline
            {
                return Err(CandidateGenerationEvidenceBundleError::Changed);
            }
            let reopened = self
                .pinned
                .root
                .open_relative_regular_file(&member.path)
                .map_err(map_storage)?;
            if reopened.fingerprint.stable() != member.baseline {
                return Err(CandidateGenerationEvidenceBundleError::Changed);
            }
        }
        let verified = verify_root(&self.pinned.root, &self.manifest, self.limits, cancellation)?;
        self.pinned.revalidate(cancellation)?;
        if verified.manifest_bytes != self.manifest_bytes
            || !same_member(&verified.manifest_member, &self.manifest_member)
            || !same_members(&verified.members, &self.members)
        {
            return Err(CandidateGenerationEvidenceBundleError::Changed);
        }
        Ok(())
    }
}

/// Fresh read-only verifier for a previously published evidence bundle.
#[derive(Clone, Copy, Debug, Default)]
pub struct CandidateGenerationEvidenceBundleVerifier;

impl CandidateGenerationEvidenceBundleVerifier {
    /// Reacquires and retains an exact bundle from its manifest and persisted readback.
    ///
    /// # Errors
    ///
    /// Returns [`CandidateGenerationEvidenceBundleError`] for invalid limits,
    /// substituted readback, unsafe storage, cancellation, or any tree or byte drift.
    pub fn reacquire(
        source: &CandidateGenerationEvidenceBundleSource,
        expected_manifest: &CandidateGenerationEvidenceBundleManifestV1,
        persisted_readback: &CandidateGenerationEvidenceBundleReadbackV1,
        limits: CandidateGenerationEvidenceBundleLimits,
        cancellation: &CancellationToken,
    ) -> Result<
        CandidateGenerationEvidenceBundleReadbackLease,
        CandidateGenerationEvidenceBundleError,
    > {
        acquire(
            source.path.clone(),
            expected_manifest,
            Some(persisted_readback),
            limits.validate()?,
            cancellation,
        )
    }
}

pub(super) fn acquire(
    path: PathBuf,
    expected_manifest: &CandidateGenerationEvidenceBundleManifestV1,
    persisted_readback: Option<&CandidateGenerationEvidenceBundleReadbackV1>,
    limits: CandidateGenerationEvidenceBundleLimits,
    cancellation: &CancellationToken,
) -> Result<CandidateGenerationEvidenceBundleReadbackLease, CandidateGenerationEvidenceBundleError>
{
    ensure_active(cancellation)?;
    reject_indirect_directory(&path)?;
    let root = PinnedDirectory::open_existing(&path).map_err(map_storage)?;
    let pinned = PinnedBundleRoot::new_absolute(path, root, cancellation)?;
    finish_acquisition(
        pinned,
        expected_manifest,
        persisted_readback,
        limits,
        cancellation,
    )
}

pub(crate) fn acquire_from_parent(
    parent: &PinnedDirectory,
    name: &OsStr,
    expected_manifest: &CandidateGenerationEvidenceBundleManifestV1,
    persisted_readback: Option<&CandidateGenerationEvidenceBundleReadbackV1>,
    limits: CandidateGenerationEvidenceBundleLimits,
    cancellation: &CancellationToken,
) -> Result<CandidateGenerationEvidenceBundleReadbackLease, CandidateGenerationEvidenceBundleError>
{
    ensure_active(cancellation)?;
    let root = parent.open_child_directory(name).map_err(map_storage)?;
    let pinned = PinnedBundleRoot::new_relative(parent, name, root, cancellation)?;
    finish_acquisition(
        pinned,
        expected_manifest,
        persisted_readback,
        limits.validate()?,
        cancellation,
    )
}

fn finish_acquisition(
    pinned: PinnedBundleRoot,
    expected_manifest: &CandidateGenerationEvidenceBundleManifestV1,
    persisted_readback: Option<&CandidateGenerationEvidenceBundleReadbackV1>,
    limits: CandidateGenerationEvidenceBundleLimits,
    cancellation: &CancellationToken,
) -> Result<CandidateGenerationEvidenceBundleReadbackLease, CandidateGenerationEvidenceBundleError>
{
    let verified = verify_root(&pinned.root, expected_manifest, limits, cancellation)?;
    pinned.revalidate(cancellation)?;
    let readback = CandidateGenerationEvidenceBundleReadbackV1::new(expected_manifest)
        .map_err(CandidateGenerationEvidenceBundleError::Contract)?;
    if let Some(persisted) = persisted_readback {
        let bytes = serde_json::to_vec(persisted)
            .map_err(|_| CandidateGenerationEvidenceBundleError::PlanMismatch)?;
        let reparsed =
            CandidateGenerationEvidenceBundleReadbackV1::from_json_bytes(&bytes, expected_manifest)
                .map_err(CandidateGenerationEvidenceBundleError::Contract)?;
        if &reparsed != persisted || reparsed != readback {
            return Err(CandidateGenerationEvidenceBundleError::PlanMismatch);
        }
    }
    Ok(CandidateGenerationEvidenceBundleReadbackLease {
        pinned,
        manifest: expected_manifest.clone(),
        manifest_bytes: verified.manifest_bytes,
        manifest_member: verified.manifest_member,
        readback,
        members: verified.members,
        limits,
    })
}

struct VerifiedRoot {
    manifest_bytes: Vec<u8>,
    manifest_member: RetainedMember,
    members: Vec<RetainedMember>,
}

pub(super) fn verify_staged_root(
    root: &PinnedDirectory,
    expected_manifest: &CandidateGenerationEvidenceBundleManifestV1,
    limits: CandidateGenerationEvidenceBundleLimits,
    cancellation: &CancellationToken,
) -> Result<(), CandidateGenerationEvidenceBundleError> {
    verify_root(root, expected_manifest, limits, cancellation).map(|_| ())
}

fn verify_root(
    root: &PinnedDirectory,
    expected_manifest: &CandidateGenerationEvidenceBundleManifestV1,
    limits: CandidateGenerationEvidenceBundleLimits,
    cancellation: &CancellationToken,
) -> Result<VerifiedRoot, CandidateGenerationEvidenceBundleError> {
    ensure_active(cancellation)?;
    if expected_manifest
        .rederive_evidence_bundle_id()
        .map_err(CandidateGenerationEvidenceBundleError::Contract)?
        != *expected_manifest.evidence_bundle_id()
    {
        return Err(CandidateGenerationEvidenceBundleError::PlanMismatch);
    }
    let expected_manifest_bytes = expected_manifest
        .to_canonical_json_bytes()
        .map_err(CandidateGenerationEvidenceBundleError::Contract)?;
    let expected = expected_tree(expected_manifest, expected_manifest_bytes.len(), limits)?;
    let tree_limits = ManagedTreeLimits::new(limits.maximum_tree_entries).map_err(map_storage)?;
    let before = root
        .enumerate_tree(tree_limits, cancellation)
        .map_err(map_storage)?;
    validate_snapshot(&before, &expected)?;

    let manifest_path = fixed_manifest_path();
    let manifest = read_retained_bytes(
        root,
        &manifest_path,
        u64::try_from(expected_manifest_bytes.len())
            .map_err(|_| CandidateGenerationEvidenceBundleError::LimitExceeded)?,
        cancellation,
    )?;
    if manifest.bytes != expected_manifest_bytes {
        return Err(CandidateGenerationEvidenceBundleError::TreeMismatch);
    }
    let mut members = Vec::with_capacity(expected_manifest.entries().len());
    for entry in expected_manifest.entries() {
        ensure_active(cancellation)?;
        members.push(open_retained_member(
            root,
            entry.relative_path(),
            entry.artifact_id().digest(),
            entry.byte_size(),
            cancellation,
        )?);
    }
    let after = root
        .enumerate_tree(tree_limits, cancellation)
        .map_err(map_storage)?;
    if before != after {
        return Err(CandidateGenerationEvidenceBundleError::Changed);
    }
    let manifest_member = RetainedMember {
        path: manifest_path,
        artifact_id: ArtifactId::from_digest(digest_bytes(&manifest.bytes, cancellation)?),
        byte_size: u64::try_from(manifest.bytes.len())
            .map_err(|_| CandidateGenerationEvidenceBundleError::LimitExceeded)?,
        file: manifest.file,
        baseline: manifest.baseline,
    };
    Ok(VerifiedRoot {
        manifest_bytes: manifest.bytes,
        manifest_member,
        members,
    })
}

struct RetainedBytes {
    file: File,
    bytes: Vec<u8>,
    baseline: StableMetadataFingerprint,
}

fn read_retained_bytes(
    root: &PinnedDirectory,
    path: &ArtifactSetRelativePath,
    expected_size: u64,
    cancellation: &CancellationToken,
) -> Result<RetainedBytes, CandidateGenerationEvidenceBundleError> {
    let opened = root.open_relative_regular_file(path).map_err(map_storage)?;
    if !opened.fingerprint.has_single_link() || opened.byte_size != expected_size {
        return Err(CandidateGenerationEvidenceBundleError::TreeMismatch);
    }
    let baseline = opened.fingerprint.stable();
    let bytes = read_exact_bytes(
        opened
            .file
            .try_clone()
            .map_err(CandidateGenerationEvidenceBundleError::StorageIo)?,
        expected_size,
        cancellation,
    )?;
    if fingerprint_std_file(&opened.file)
        .map_err(map_storage)?
        .stable()
        != baseline
    {
        return Err(CandidateGenerationEvidenceBundleError::Changed);
    }
    root.recheck_relative_regular_file(path, &opened.fingerprint)
        .map_err(map_storage)?;
    Ok(RetainedBytes {
        file: opened.file,
        bytes,
        baseline,
    })
}

fn open_retained_member(
    root: &PinnedDirectory,
    path: &ArtifactSetRelativePath,
    expected_digest: &Digest,
    expected_size: u64,
    cancellation: &CancellationToken,
) -> Result<RetainedMember, CandidateGenerationEvidenceBundleError> {
    ensure_active(cancellation)?;
    let mut opened = root.open_relative_regular_file(path).map_err(map_storage)?;
    if !opened.fingerprint.has_single_link() || opened.byte_size != expected_size {
        return Err(CandidateGenerationEvidenceBundleError::TreeMismatch);
    }
    let observed =
        hash_exact_bytes(&mut opened.file, expected_size, cancellation).map_err(map_storage)?;
    if &observed != expected_digest {
        return Err(CandidateGenerationEvidenceBundleError::Changed);
    }
    let baseline = opened.fingerprint.stable();
    if fingerprint_std_file(&opened.file)
        .map_err(map_storage)?
        .stable()
        != baseline
    {
        return Err(CandidateGenerationEvidenceBundleError::Changed);
    }
    root.recheck_relative_regular_file(path, &opened.fingerprint)
        .map_err(map_storage)?;
    Ok(RetainedMember {
        path: path.clone(),
        artifact_id: ArtifactId::from_digest(observed),
        byte_size: expected_size,
        file: opened.file,
        baseline,
    })
}

fn read_exact_bytes(
    mut file: File,
    expected_size: u64,
    cancellation: &CancellationToken,
) -> Result<Vec<u8>, CandidateGenerationEvidenceBundleError> {
    file.seek(SeekFrom::Start(0))
        .map_err(CandidateGenerationEvidenceBundleError::StorageIo)?;
    let capacity = usize::try_from(expected_size)
        .map_err(|_| CandidateGenerationEvidenceBundleError::LimitExceeded)?;
    let mut bytes = Vec::with_capacity(capacity);
    let mut remaining = expected_size;
    let mut buffer = vec![0_u8; READ_BUFFER_BYTES];
    while remaining != 0 {
        ensure_active(cancellation)?;
        let maximum = usize::try_from(remaining)
            .unwrap_or(usize::MAX)
            .min(buffer.len());
        let read = file
            .read(&mut buffer[..maximum])
            .map_err(CandidateGenerationEvidenceBundleError::StorageIo)?;
        if read == 0 {
            return Err(CandidateGenerationEvidenceBundleError::Changed);
        }
        bytes.extend_from_slice(&buffer[..read]);
        remaining -= u64::try_from(read)
            .map_err(|_| CandidateGenerationEvidenceBundleError::LimitExceeded)?;
    }
    let mut trailing = [0_u8; 1];
    if file
        .read(&mut trailing)
        .map_err(CandidateGenerationEvidenceBundleError::StorageIo)?
        != 0
    {
        return Err(CandidateGenerationEvidenceBundleError::Changed);
    }
    Ok(bytes)
}

fn same_members(left: &[RetainedMember], right: &[RetainedMember]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| same_member(left, right))
}

fn same_member(left: &RetainedMember, right: &RetainedMember) -> bool {
    left.path == right.path
        && left.artifact_id == right.artifact_id
        && left.byte_size == right.byte_size
        && left.baseline == right.baseline
}
