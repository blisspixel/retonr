//! Inert, bounded closure review for retained Cargo sources.

use std::io::Read;

use rewrite_model::ArtifactSetRelativePath;
use rewrite_types::Digest;
use sha2::{Digest as _, Sha256};

use super::super::{
    RuntimeSourceBuildInputComponent, RuntimeSourceBuildInputOpenError,
    RuntimeSourceBuildInputRole, VerifiedRuntimeSourceBuildInputs,
};

mod archive;
mod error;
mod lockfile;
mod raw_crates;
mod repository;
mod stream;
mod vendor;

use archive::{scan_canonical_tar, validate_raw_tar};
pub use error::CargoSourceClosureError;
use lockfile::{lock_digest, parse_lockfile};
use repository::verify_repository;
use stream::{append, digest};
use vendor::{scan_raw_bundle, verify_vendor};

/// Domain identifier for the retained Cargo source-closure review.
pub const CARGO_SOURCE_CLOSURE_PROCEDURE_ID: &str =
    "retonr:runtime-source-build:cargo-source-closure";
/// Current retained Cargo source-closure review procedure version.
pub const CARGO_SOURCE_CLOSURE_PROCEDURE_VERSION: u32 = 1;

const DEFAULT_LOCK_BYTES: usize = 4 * 1024 * 1024;
const DEFAULT_PACKAGES: usize = 8_192;
const DEFAULT_EDGES: usize = 131_072;
const DEFAULT_STRING_BYTES: usize = 32 * 1024 * 1024;
const DEFAULT_ARCHIVE_ENTRIES: usize = 262_144;
const DEFAULT_ARCHIVE_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const DEFAULT_PATH_BYTES: usize = 4_096;
const DEFAULT_TOTAL_PATH_BYTES: usize = 256 * 1024 * 1024;
const DEFAULT_CAPTURED_BYTES: u64 = 4 * 1024 * 1024;
const DEFAULT_MANIFEST_BYTES: usize = 2 * 1024 * 1024;
const DEFAULT_PATH_PACKAGES: usize = 4_096;
const DEFAULT_CRATE_BYTES: u64 = 128 * 1024 * 1024;
const DEFAULT_CRATE_ENTRIES: usize = 32_768;
const DEFAULT_CRATE_UNPACKED_BYTES: u64 = 512 * 1024 * 1024;
const HASH_BUFFER_BYTES: usize = 64 * 1024;

/// Fixed resource ceilings for one retained Cargo source-closure review.
///
/// Explicit values may only lower the hard defaults.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CargoSourceClosureLimits {
    /// Maximum `Cargo.lock` bytes parsed.
    pub maximum_lock_bytes: usize,
    /// Maximum packages in `Cargo.lock`.
    pub maximum_packages: usize,
    /// Maximum resolved dependency edges.
    pub maximum_dependency_edges: usize,
    /// Maximum aggregate lockfile identity string bytes.
    pub maximum_string_bytes: usize,
    /// Maximum semantic entries in any outer archive.
    pub maximum_archive_entries: usize,
    /// Maximum aggregate regular-file bytes in any outer archive.
    pub maximum_archive_bytes: u64,
    /// Maximum portable archive path bytes.
    pub maximum_path_bytes: usize,
    /// Maximum aggregate archive path bytes.
    pub maximum_total_path_bytes: usize,
    /// Maximum retained bytes for one reviewed metadata file.
    pub maximum_captured_file_bytes: u64,
    /// Maximum bytes in one parsed Cargo manifest.
    pub maximum_manifest_bytes: usize,
    /// Maximum source-less repository packages.
    pub maximum_path_packages: usize,
    /// Maximum compressed bytes in one raw `.crate` member.
    pub maximum_crate_bytes: u64,
    /// Maximum entries in one decompressed `.crate` archive.
    pub maximum_crate_entries: usize,
    /// Maximum aggregate decompressed bytes in one `.crate` archive.
    pub maximum_crate_unpacked_bytes: u64,
}

impl Default for CargoSourceClosureLimits {
    fn default() -> Self {
        Self {
            maximum_lock_bytes: DEFAULT_LOCK_BYTES,
            maximum_packages: DEFAULT_PACKAGES,
            maximum_dependency_edges: DEFAULT_EDGES,
            maximum_string_bytes: DEFAULT_STRING_BYTES,
            maximum_archive_entries: DEFAULT_ARCHIVE_ENTRIES,
            maximum_archive_bytes: DEFAULT_ARCHIVE_BYTES,
            maximum_path_bytes: DEFAULT_PATH_BYTES,
            maximum_total_path_bytes: DEFAULT_TOTAL_PATH_BYTES,
            maximum_captured_file_bytes: DEFAULT_CAPTURED_BYTES,
            maximum_manifest_bytes: DEFAULT_MANIFEST_BYTES,
            maximum_path_packages: DEFAULT_PATH_PACKAGES,
            maximum_crate_bytes: DEFAULT_CRATE_BYTES,
            maximum_crate_entries: DEFAULT_CRATE_ENTRIES,
            maximum_crate_unpacked_bytes: DEFAULT_CRATE_UNPACKED_BYTES,
        }
    }
}

impl CargoSourceClosureLimits {
    /// Revalidates nonzero ceilings against the procedure hard limits.
    ///
    /// # Errors
    ///
    /// Returns [`CargoSourceClosureError::InvalidLimits`] for a zero or raised ceiling.
    pub fn validate(self) -> Result<Self, CargoSourceClosureError> {
        let default = Self::default();
        if self.maximum_lock_bytes == 0
            || self.maximum_lock_bytes > default.maximum_lock_bytes
            || self.maximum_packages == 0
            || self.maximum_packages > default.maximum_packages
            || self.maximum_dependency_edges == 0
            || self.maximum_dependency_edges > default.maximum_dependency_edges
            || self.maximum_string_bytes == 0
            || self.maximum_string_bytes > default.maximum_string_bytes
            || self.maximum_archive_entries == 0
            || self.maximum_archive_entries > default.maximum_archive_entries
            || self.maximum_archive_bytes == 0
            || self.maximum_archive_bytes > default.maximum_archive_bytes
            || self.maximum_path_bytes == 0
            || self.maximum_path_bytes > default.maximum_path_bytes
            || self.maximum_total_path_bytes == 0
            || self.maximum_total_path_bytes > default.maximum_total_path_bytes
            || self.maximum_captured_file_bytes == 0
            || self.maximum_captured_file_bytes > default.maximum_captured_file_bytes
            || self.maximum_manifest_bytes == 0
            || self.maximum_manifest_bytes > default.maximum_manifest_bytes
            || self.maximum_path_packages == 0
            || self.maximum_path_packages > default.maximum_path_packages
            || self.maximum_crate_bytes == 0
            || self.maximum_crate_bytes > default.maximum_crate_bytes
            || self.maximum_crate_entries == 0
            || self.maximum_crate_entries > default.maximum_crate_entries
            || self.maximum_crate_unpacked_bytes == 0
            || self.maximum_crate_unpacked_bytes > default.maximum_crate_unpacked_bytes
        {
            Err(CargoSourceClosureError::InvalidLimits)
        } else {
            Ok(self)
        }
    }
}

/// Bounded facts exposed to a reviewer after complete source comparison.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CargoSourceClosureReviewerFacts {
    edges: usize,
    path_packages: usize,
    registry_packages: usize,
    source_files: usize,
}

impl CargoSourceClosureReviewerFacts {
    /// Returns the number of dependency edges resolved exactly once.
    #[must_use]
    pub const fn dependency_edge_count(&self) -> usize {
        self.edges
    }

    /// Returns the number of source-less packages anchored inside the repository.
    #[must_use]
    pub const fn path_package_count(&self) -> usize {
        self.path_packages
    }

    /// Returns the number of checksum-bound crates.io packages.
    #[must_use]
    pub const fn registry_package_count(&self) -> usize {
        self.registry_packages
    }

    /// Returns the number of raw crate source files compared with the vendor tree.
    #[must_use]
    pub const fn source_file_count(&self) -> usize {
        self.source_files
    }
}

/// Complete inert result of retained Cargo source-closure verification.
///
/// This value reports deterministic reviewer facts. It does not grant build,
/// policy, admission, execution, or runtime authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedCargoSourceClosure {
    cargo_lock_digest: Digest,
    closure_id: Digest,
    facts: CargoSourceClosureReviewerFacts,
    raw_crate_archive_id: Digest,
    raw_crate_tree_id: Digest,
    repository_archive_id: Digest,
    repository_tree_id: Digest,
    source_input_set_id: Digest,
    vendor_archive_id: Digest,
    vendor_tree_id: Digest,
}

impl VerifiedCargoSourceClosure {
    /// Returns the domain-separated identity of the complete closure review.
    #[must_use]
    pub const fn closure_id(&self) -> &Digest {
        &self.closure_id
    }

    /// Returns the exact source-input artifact-set identity bound by the review.
    #[must_use]
    pub const fn source_input_set_id(&self) -> &Digest {
        &self.source_input_set_id
    }

    /// Returns the digest of the exact reviewed repository archive component.
    #[must_use]
    pub const fn repository_archive_id(&self) -> &Digest {
        &self.repository_archive_id
    }

    /// Returns the domain-separated repository file-tree identity.
    #[must_use]
    pub const fn repository_tree_id(&self) -> &Digest {
        &self.repository_tree_id
    }

    /// Returns the digest of the exact raw-crate bundle component.
    #[must_use]
    pub const fn raw_crate_archive_id(&self) -> &Digest {
        &self.raw_crate_archive_id
    }

    /// Returns the domain-separated raw-crate bundle tree identity.
    #[must_use]
    pub const fn raw_crate_tree_id(&self) -> &Digest {
        &self.raw_crate_tree_id
    }

    /// Returns the digest of the exact Cargo vendor archive component.
    #[must_use]
    pub const fn vendor_archive_id(&self) -> &Digest {
        &self.vendor_archive_id
    }

    /// Returns the domain-separated Cargo vendor file-tree identity.
    #[must_use]
    pub const fn vendor_tree_id(&self) -> &Digest {
        &self.vendor_tree_id
    }

    /// Returns the exact reviewed `Cargo.lock` byte digest.
    #[must_use]
    pub const fn cargo_lock_digest(&self) -> &Digest {
        &self.cargo_lock_digest
    }

    /// Returns bounded reviewer-facing closure facts.
    #[must_use]
    pub const fn reviewer_facts(&self) -> &CargoSourceClosureReviewerFacts {
        &self.facts
    }
}

/// Verifies the complete retained Cargo lock, raw-crate, vendor, and path closure.
///
/// The caller-supplied opener receives already validated portable component
/// paths. Every archive is reopened and rehashed for its raw and semantic pass.
/// No path, filesystem, network, policy, execution, or admission authority is used.
///
/// # Errors
///
/// Returns [`CargoSourceClosureError`] for cancellation, resource excess,
/// changed inputs, malformed archives, or any incomplete source relationship.
pub fn verify_cargo_source_closure<R, F, C>(
    inputs: &VerifiedRuntimeSourceBuildInputs,
    limits: CargoSourceClosureLimits,
    mut open_component: F,
    mut cancelled: C,
) -> Result<VerifiedCargoSourceClosure, CargoSourceClosureError>
where
    R: Read,
    F: FnMut(&ArtifactSetRelativePath) -> Result<R, RuntimeSourceBuildInputOpenError>,
    C: FnMut() -> bool,
{
    let limits = limits.validate()?;
    if cancelled() {
        return Err(CargoSourceClosureError::Cancelled);
    }
    let members = ClosureMembers::from_inputs(inputs, limits)?;
    let lock_bytes = read_member(&members.lock, &mut open_component, &mut cancelled)?;
    let lock = parse_lockfile(&lock_bytes, limits)?;
    validate_archives(&members, limits, &mut open_component, &mut cancelled)?;
    let repository = scan_canonical_tar(
        open(&members.repository, &mut open_component)?,
        &members.repository,
        "retonr-source",
        limits,
        &mut cancelled,
        |path| path == "Cargo.lock" || path == "Cargo.toml" || path.ends_with("/Cargo.toml"),
    )?;
    let path_package_count = verify_repository(&repository, &lock, &lock_bytes, limits)?;
    let raw = scan_raw_bundle(
        open(&members.raw_crates, &mut open_component)?,
        &members.raw_crates,
        &lock,
        limits,
        &mut cancelled,
    )?;
    let vendor = scan_canonical_tar(
        open(&members.vendor, &mut open_component)?,
        &members.vendor,
        "cargo-vendor",
        limits,
        &mut cancelled,
        |path| path.ends_with("/Cargo.toml") || path.ends_with("/.cargo-checksum.json"),
    )?;
    verify_vendor(&vendor, &raw)?;
    Ok(complete_review(
        inputs,
        &members,
        &lock,
        &lock_bytes,
        ReviewTrees {
            path_packages: path_package_count,
            raw,
            repository: repository.id,
            vendor: vendor.id,
        },
    ))
}

fn validate_archives<R, F, C>(
    members: &ClosureMembers,
    limits: CargoSourceClosureLimits,
    open_component: &mut F,
    cancelled: &mut C,
) -> Result<(), CargoSourceClosureError>
where
    R: Read,
    F: FnMut(&ArtifactSetRelativePath) -> Result<R, RuntimeSourceBuildInputOpenError>,
    C: FnMut() -> bool,
{
    for member in [&members.repository, &members.raw_crates, &members.vendor] {
        validate_raw_tar(open(member, open_component)?, member, limits, cancelled)?;
    }
    Ok(())
}

fn complete_review(
    inputs: &VerifiedRuntimeSourceBuildInputs,
    members: &ClosureMembers,
    lock: &lockfile::LockReview,
    lock_bytes: &[u8],
    trees: ReviewTrees,
) -> VerifiedCargoSourceClosure {
    let cargo_lock_digest = lock_digest(lock_bytes);
    let source_input_set_id = inputs
        .manifest()
        .artifact_set()
        .artifact_set_id()
        .digest()
        .clone();
    let facts = CargoSourceClosureReviewerFacts {
        edges: lock.edge_count,
        path_packages: trees.path_packages,
        registry_packages: lock.registry_packages.len(),
        source_files: trees.raw.source_file_count,
    };
    let ids = ReviewIds {
        cargo_lock: &cargo_lock_digest,
        raw_archive: &members.raw_crates.digest,
        raw_tree: &trees.raw.tree_id,
        repository_archive: &members.repository.digest,
        repository_tree: &trees.repository,
        source_set: &source_input_set_id,
        vendor_archive: &members.vendor.digest,
        vendor_tree: &trees.vendor,
    };
    let closure_id = closure_id(&ids, &facts);
    VerifiedCargoSourceClosure {
        cargo_lock_digest,
        closure_id,
        facts,
        raw_crate_archive_id: members.raw_crates.digest.clone(),
        raw_crate_tree_id: trees.raw.tree_id,
        repository_archive_id: members.repository.digest.clone(),
        repository_tree_id: trees.repository,
        source_input_set_id,
        vendor_archive_id: members.vendor.digest.clone(),
        vendor_tree_id: trees.vendor,
    }
}

struct ReviewTrees {
    path_packages: usize,
    raw: vendor::RawBundle,
    repository: Digest,
    vendor: Digest,
}

struct ReviewIds<'a> {
    cargo_lock: &'a Digest,
    raw_archive: &'a Digest,
    raw_tree: &'a Digest,
    repository_archive: &'a Digest,
    repository_tree: &'a Digest,
    source_set: &'a Digest,
    vendor_archive: &'a Digest,
    vendor_tree: &'a Digest,
}

fn closure_id(ids: &ReviewIds<'_>, facts: &CargoSourceClosureReviewerFacts) -> Digest {
    let mut hasher = Sha256::new();
    hasher.update(b"retonr:cargo-source-closure:v1\0");
    for value in [
        ids.source_set,
        ids.repository_archive,
        ids.repository_tree,
        ids.raw_archive,
        ids.raw_tree,
        ids.vendor_archive,
        ids.vendor_tree,
        ids.cargo_lock,
    ] {
        append(&mut hasher, value.as_str().as_bytes());
    }
    hasher.update(
        u64::try_from(facts.registry_packages)
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    hasher.update(u64::try_from(facts.edges).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(
        u64::try_from(facts.path_packages)
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    hasher.update(
        u64::try_from(facts.source_files)
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    digest(hasher)
}

struct ClosureMembers {
    lock: MemberMeasurement,
    raw_crates: MemberMeasurement,
    repository: MemberMeasurement,
    vendor: MemberMeasurement,
}

impl ClosureMembers {
    fn from_inputs(
        inputs: &VerifiedRuntimeSourceBuildInputs,
        limits: CargoSourceClosureLimits,
    ) -> Result<Self, CargoSourceClosureError> {
        let manifest = inputs.manifest();
        let lock = find_member(
            manifest.components(),
            RuntimeSourceBuildInputRole::CargoLockfile,
        )?;
        let raw_crates = find_member(
            manifest.components(),
            RuntimeSourceBuildInputRole::CargoRawCrateSource,
        )?;
        let repository = find_member(
            manifest.components(),
            RuntimeSourceBuildInputRole::RetonrRepositorySource,
        )?;
        let vendor = find_member(
            manifest.components(),
            RuntimeSourceBuildInputRole::CargoVendorSource,
        )?;
        if lock.bytes > u64::try_from(limits.maximum_lock_bytes).unwrap_or(u64::MAX)
            || [raw_crates.bytes, repository.bytes, vendor.bytes]
                .into_iter()
                .any(|bytes| bytes > limits.maximum_archive_bytes)
        {
            return Err(CargoSourceClosureError::ArchiveQuotaExceeded);
        }
        Ok(Self {
            lock,
            raw_crates,
            repository,
            vendor,
        })
    }
}

#[derive(Clone)]
pub(super) struct MemberMeasurement {
    bytes: u64,
    digest: Digest,
    path: ArtifactSetRelativePath,
}

fn find_member(
    components: &[RuntimeSourceBuildInputComponent],
    role: RuntimeSourceBuildInputRole,
) -> Result<MemberMeasurement, CargoSourceClosureError> {
    let mut matching = components
        .iter()
        .filter(|component| component.roles().contains(&role));
    let component = matching
        .next()
        .filter(|component| component.roles() == [role])
        .ok_or(CargoSourceClosureError::InvalidComponentRoles)?;
    if matching.next().is_some() {
        return Err(CargoSourceClosureError::InvalidComponentRoles);
    }
    Ok(MemberMeasurement {
        bytes: component.byte_size(),
        digest: component.digest().clone(),
        path: component.relative_path().clone(),
    })
}

fn open<R, F>(member: &MemberMeasurement, opener: &mut F) -> Result<R, CargoSourceClosureError>
where
    R: Read,
    F: FnMut(&ArtifactSetRelativePath) -> Result<R, RuntimeSourceBuildInputOpenError>,
{
    opener(&member.path).map_err(|_| CargoSourceClosureError::ComponentUnavailable)
}

fn read_member<R, F, C>(
    member: &MemberMeasurement,
    opener: &mut F,
    cancelled: &mut C,
) -> Result<Vec<u8>, CargoSourceClosureError>
where
    R: Read,
    F: FnMut(&ArtifactSetRelativePath) -> Result<R, RuntimeSourceBuildInputOpenError>,
    C: FnMut() -> bool,
{
    let capacity = usize::try_from(member.bytes)
        .map_err(|_| CargoSourceClosureError::LockfileQuotaExceeded)?;
    let mut stream = open(member, opener)?;
    let mut output = Vec::with_capacity(capacity);
    let mut hasher = Sha256::new();
    let mut remaining = member.bytes;
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES].into_boxed_slice();
    loop {
        if cancelled() {
            return Err(CargoSourceClosureError::Cancelled);
        }
        let read = stream
            .read(&mut buffer)
            .map_err(|_| CargoSourceClosureError::ComponentMismatch)?;
        if read == 0 {
            break;
        }
        let read_u64 = u64::try_from(read).unwrap_or(u64::MAX);
        if read_u64 > remaining {
            return Err(CargoSourceClosureError::ComponentMismatch);
        }
        output.extend_from_slice(&buffer[..read]);
        hasher.update(&buffer[..read]);
        remaining -= read_u64;
    }
    if remaining != 0 || digest(hasher) != member.digest {
        return Err(CargoSourceClosureError::ComponentMismatch);
    }
    Ok(output)
}

#[cfg(test)]
mod test_support;
#[cfg(test)]
#[path = "cargo_source/tests.rs"]
mod tests;

#[cfg(test)]
pub(super) use test_support::verified_cargo_source_closure_for_test;
