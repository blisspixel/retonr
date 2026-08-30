use std::{ffi::OsString, io, path::PathBuf};

use rewrite_model::{ArtifactSetRelativePath, MAX_ARTIFACT_SET_MANIFEST_JSON_BYTES};
use thiserror::Error;

use super::tree_plan::{
    RUNTIME_ADMISSION_EVIDENCE_DIRECTORY_COUNT, RUNTIME_ADMISSION_EVIDENCE_MEMBER_COUNT,
};

/// Canonical complete-manifest path inside a durable admission-evidence root.
pub const RUNTIME_ADMISSION_EVIDENCE_BUNDLE_MANIFEST_PATH: &str =
    "admission-evidence-manifest.json";
/// Canonical inert-foundation path inside a durable admission-evidence root.
pub const RUNTIME_ADMISSION_EVIDENCE_FOUNDATION_PATH: &str = "runtime-admission-foundation.json";
/// Hard ceiling for files plus directories in one admission-evidence root.
pub const MAX_RUNTIME_ADMISSION_EVIDENCE_TREE_ENTRIES: usize = 32;
/// Hard aggregate byte ceiling for one admission-evidence closure.
pub const MAX_RUNTIME_ADMISSION_EVIDENCE_BUNDLE_BYTES: u64 = 16 * 1_024 * 1_024;
/// Hard ceiling for direct entries in a destination parent.
pub const MAX_RUNTIME_ADMISSION_EVIDENCE_DESTINATION_ENTRIES: usize = 262_144;
/// Hard ceiling for direct entries while reserving a staging root.
pub const MAX_RUNTIME_ADMISSION_EVIDENCE_STAGING_ROOTS: usize = 1_024;

/// Caller-owned ceilings for future publication and reacquisition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeAdmissionEvidenceBundleLimits {
    /// Maximum files plus directories in the durable closure.
    pub maximum_tree_entries: usize,
    /// Maximum aggregate bytes in the manifest-declared closure.
    pub maximum_total_bytes: u64,
    /// Maximum direct entries permitted in the destination parent.
    pub maximum_destination_entries: usize,
    /// Maximum direct entries permitted while reserving a staging root.
    pub maximum_staging_roots: usize,
}

impl Default for RuntimeAdmissionEvidenceBundleLimits {
    fn default() -> Self {
        Self {
            maximum_tree_entries: MAX_RUNTIME_ADMISSION_EVIDENCE_TREE_ENTRIES,
            maximum_total_bytes: MAX_RUNTIME_ADMISSION_EVIDENCE_BUNDLE_BYTES,
            maximum_destination_entries: MAX_RUNTIME_ADMISSION_EVIDENCE_DESTINATION_ENTRIES,
            maximum_staging_roots: MAX_RUNTIME_ADMISSION_EVIDENCE_STAGING_ROOTS,
        }
    }
}

impl RuntimeAdmissionEvidenceBundleLimits {
    /// Validates that caller ceilings are nonzero and no broader than the hard
    /// admission-evidence contract.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeAdmissionEvidenceContractError::LimitExceeded`] for an
    /// unusable or overly broad ceiling.
    pub fn validate(self) -> Result<Self, RuntimeAdmissionEvidenceContractError> {
        let required_entries = RUNTIME_ADMISSION_EVIDENCE_MEMBER_COUNT
            + RUNTIME_ADMISSION_EVIDENCE_DIRECTORY_COUNT
            + 1;
        let maximum_manifest_bytes = u64::try_from(MAX_ARTIFACT_SET_MANIFEST_JSON_BYTES)
            .map_err(|_| RuntimeAdmissionEvidenceContractError::LimitExceeded)?;
        if self.maximum_tree_entries < required_entries
            || self.maximum_tree_entries > MAX_RUNTIME_ADMISSION_EVIDENCE_TREE_ENTRIES
            || self.maximum_total_bytes == 0
            || self.maximum_total_bytes > MAX_RUNTIME_ADMISSION_EVIDENCE_BUNDLE_BYTES
            || self.maximum_total_bytes < maximum_manifest_bytes
            || self.maximum_destination_entries == 0
            || self.maximum_destination_entries > MAX_RUNTIME_ADMISSION_EVIDENCE_DESTINATION_ENTRIES
            || self.maximum_staging_roots == 0
            || self.maximum_staging_roots > MAX_RUNTIME_ADMISSION_EVIDENCE_STAGING_ROOTS
        {
            return Err(RuntimeAdmissionEvidenceContractError::LimitExceeded);
        }
        Ok(self)
    }
}

/// Caller-selected absent path for future no-replace evidence publication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAdmissionEvidenceBundleDestination {
    pub(super) path: PathBuf,
    pub(super) parent: PathBuf,
    pub(super) name: OsString,
}

impl RuntimeAdmissionEvidenceBundleDestination {
    /// Forms one absolute, portable final selection without creating it.
    ///
    /// A future publisher must require the parent to exist and the final entry
    /// to remain absent through a no-replace commit.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeAdmissionEvidenceContractError::InvalidPath`] when the
    /// path cannot be made absolute, or `UnsafeBoundary` for a nonportable name.
    pub fn new(path: impl Into<PathBuf>) -> Result<Self, RuntimeAdmissionEvidenceContractError> {
        let path = std::path::absolute(path.into())
            .map_err(RuntimeAdmissionEvidenceContractError::InvalidPath)?;
        let parent = path
            .parent()
            .ok_or(RuntimeAdmissionEvidenceContractError::UnsafeBoundary)?
            .to_path_buf();
        let name = path
            .file_name()
            .ok_or(RuntimeAdmissionEvidenceContractError::UnsafeBoundary)?
            .to_os_string();
        let portable = name
            .to_str()
            .ok_or(RuntimeAdmissionEvidenceContractError::UnsafeBoundary)?;
        let parsed = ArtifactSetRelativePath::new(portable.to_owned())
            .map_err(|_| RuntimeAdmissionEvidenceContractError::UnsafeBoundary)?;
        if parsed.as_str().contains('/') {
            return Err(RuntimeAdmissionEvidenceContractError::UnsafeBoundary);
        }
        Ok(Self { path, parent, name })
    }

    /// Returns the selected absolute final path.
    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

/// Caller-selected existing durable admission-evidence root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAdmissionEvidenceBundleSource {
    pub(super) path: PathBuf,
}

impl RuntimeAdmissionEvidenceBundleSource {
    /// Forms one absolute evidence-root selection without opening it.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeAdmissionEvidenceContractError::InvalidPath`] when the
    /// path cannot be made absolute.
    pub fn new(path: impl Into<PathBuf>) -> Result<Self, RuntimeAdmissionEvidenceContractError> {
        let path = std::path::absolute(path.into())
            .map_err(RuntimeAdmissionEvidenceContractError::InvalidPath)?;
        Ok(Self { path })
    }

    /// Returns the selected absolute root path.
    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

/// Failure while compiling or selecting inert admission-evidence contracts.
#[derive(Debug, Error)]
pub enum RuntimeAdmissionEvidenceContractError {
    /// A selected path could not be made absolute.
    #[error("runtime admission evidence path is invalid")]
    InvalidPath(#[source] io::Error),
    /// A selected root or final name is nonportable or unsafe.
    #[error("runtime admission evidence boundary is unsafe")]
    UnsafeBoundary,
    /// Canonical JSON was malformed, overlong, or not encoded exactly.
    #[error("runtime admission evidence encoding is invalid")]
    InvalidEncoding,
    /// A schema marker, fixed member, or subject binding was invalid.
    #[error("runtime admission evidence binding is invalid")]
    InvalidBinding,
    /// Caller-owned or hard evidence ceilings were invalid or exceeded.
    #[error("runtime admission evidence limit was exceeded")]
    LimitExceeded,
}

pub(super) fn domain_separated_digest(domain: &[u8], bytes: &[u8]) -> rewrite_types::Digest {
    let mut material =
        Vec::with_capacity(domain.len().saturating_add(bytes.len()).saturating_add(1));
    material.extend_from_slice(domain);
    material.push(0);
    material.extend_from_slice(bytes);
    rewrite_types::Digest::sha256(&material)
}

#[cfg(test)]
pub(super) fn paths_overlap(left: &std::path::Path, right: &std::path::Path) -> bool {
    path_is_within(left, right) || path_is_within(right, left)
}

#[cfg(test)]
fn path_is_within(path: &std::path::Path, ancestor: &std::path::Path) -> bool {
    let path = path.components().collect::<Vec<_>>();
    let ancestor = ancestor.components().collect::<Vec<_>>();
    path.len() >= ancestor.len()
        && path
            .iter()
            .zip(&ancestor)
            .all(|(left, right)| left.as_os_str().eq_ignore_ascii_case(right.as_os_str()))
}
