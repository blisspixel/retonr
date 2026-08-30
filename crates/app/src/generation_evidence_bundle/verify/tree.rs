use std::collections::{BTreeMap, BTreeSet};

use rewrite_model::{
    ArtifactSetRelativePath, CandidateGenerationEvidenceBundleManifestV1,
    EVIDENCE_BUNDLE_MANIFEST_RELATIVE_PATH,
};

use super::super::{
    CandidateGenerationEvidenceBundleError, CandidateGenerationEvidenceBundleLimits,
};
use crate::artifact_storage::{ManagedTreeEntryKind, ManagedTreeSnapshot};

pub(super) enum ExpectedTreeEntry {
    Directory,
    File(u64),
}

pub(super) fn expected_tree(
    manifest: &CandidateGenerationEvidenceBundleManifestV1,
    manifest_bytes: usize,
    limits: CandidateGenerationEvidenceBundleLimits,
) -> Result<
    BTreeMap<ArtifactSetRelativePath, ExpectedTreeEntry>,
    CandidateGenerationEvidenceBundleError,
> {
    let mut expected = BTreeMap::new();
    let mut directories = BTreeSet::new();
    expected.insert(
        fixed_manifest_path(),
        ExpectedTreeEntry::File(
            u64::try_from(manifest_bytes)
                .map_err(|_| CandidateGenerationEvidenceBundleError::LimitExceeded)?,
        ),
    );
    let mut total = u64::try_from(manifest_bytes)
        .map_err(|_| CandidateGenerationEvidenceBundleError::LimitExceeded)?;
    for entry in manifest.entries() {
        total = total
            .checked_add(entry.byte_size())
            .ok_or(CandidateGenerationEvidenceBundleError::LimitExceeded)?;
        if entry.relative_path().as_str().split('/').count() > limits.maximum_tree_depth {
            return Err(CandidateGenerationEvidenceBundleError::LimitExceeded);
        }
        collect_directories(entry.relative_path(), &mut directories)?;
        if expected
            .insert(
                entry.relative_path().clone(),
                ExpectedTreeEntry::File(entry.byte_size()),
            )
            .is_some()
        {
            return Err(CandidateGenerationEvidenceBundleError::TreeMismatch);
        }
    }
    for directory in directories {
        if expected
            .insert(directory, ExpectedTreeEntry::Directory)
            .is_some()
        {
            return Err(CandidateGenerationEvidenceBundleError::TreeMismatch);
        }
    }
    if expected.len() > limits.maximum_tree_entries || total > limits.maximum_total_bytes {
        return Err(CandidateGenerationEvidenceBundleError::LimitExceeded);
    }
    Ok(expected)
}

pub(super) fn validate_snapshot(
    snapshot: &ManagedTreeSnapshot,
    expected: &BTreeMap<ArtifactSetRelativePath, ExpectedTreeEntry>,
) -> Result<(), CandidateGenerationEvidenceBundleError> {
    if snapshot.entries().len() != expected.len() {
        return Err(CandidateGenerationEvidenceBundleError::TreeMismatch);
    }
    for entry in snapshot.entries() {
        let valid = match expected.get(entry.relative_path()) {
            Some(ExpectedTreeEntry::Directory) => {
                entry.kind() == ManagedTreeEntryKind::Directory && entry.byte_size() == 0
            }
            Some(ExpectedTreeEntry::File(size)) => {
                entry.kind() == ManagedTreeEntryKind::RegularFile
                    && entry.byte_size() == *size
                    && entry.has_single_link()
            }
            None => false,
        };
        if !valid {
            return Err(CandidateGenerationEvidenceBundleError::TreeMismatch);
        }
    }
    Ok(())
}

fn collect_directories(
    path: &ArtifactSetRelativePath,
    directories: &mut BTreeSet<ArtifactSetRelativePath>,
) -> Result<(), CandidateGenerationEvidenceBundleError> {
    let mut prefix = String::new();
    let components = path.as_str().split('/').collect::<Vec<_>>();
    for component in components.iter().take(components.len().saturating_sub(1)) {
        if !prefix.is_empty() {
            prefix.push('/');
        }
        prefix.push_str(component);
        directories.insert(
            ArtifactSetRelativePath::new(prefix.clone())
                .map_err(|_| CandidateGenerationEvidenceBundleError::TreeMismatch)?,
        );
    }
    Ok(())
}

pub(super) fn fixed_manifest_path() -> ArtifactSetRelativePath {
    ArtifactSetRelativePath::new(EVIDENCE_BUNDLE_MANIFEST_RELATIVE_PATH.to_owned())
        .expect("fixed bundle manifest path is portable")
}
