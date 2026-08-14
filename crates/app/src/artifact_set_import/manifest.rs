use std::collections::BTreeSet;

use rewrite_model::{
    ArtifactSetId, ArtifactSetManifest, ArtifactSetRelativePath, InstalledArtifactSet,
};

use super::{ArtifactSetImportError, ArtifactSetImportLimits};

pub(super) const SET_STORAGE_KEY_PREFIX: &str = "set-v1-";

#[derive(Debug, Eq, PartialEq)]
pub(super) struct ValidatedSetPlan {
    pub(super) artifact_set_id: ArtifactSetId,
    pub(super) storage_key: String,
    pub(super) installed: InstalledArtifactSet,
    pub(super) directories: Vec<ArtifactSetRelativePath>,
    pub(super) tree_entries: usize,
    pub(super) maximum_depth: usize,
}

pub(super) fn validate_manifest_and_limits(
    manifest: &ArtifactSetManifest,
    limits: ArtifactSetImportLimits,
) -> Result<ValidatedSetPlan, ArtifactSetImportError> {
    validate_limits(limits)?;
    manifest
        .validate()
        .map_err(ArtifactSetImportError::InvalidManifest)?;
    if manifest.members().len() > limits.maximum_members {
        return Err(ArtifactSetImportError::TooManyMembers {
            actual: manifest.members().len(),
            maximum: limits.maximum_members,
        });
    }
    if let Some(member) = manifest
        .members()
        .iter()
        .find(|member| member.byte_size() > limits.maximum_member_bytes)
    {
        return Err(ArtifactSetImportError::MemberTooLarge {
            actual: member.byte_size(),
            maximum: limits.maximum_member_bytes,
        });
    }
    let total_bytes = manifest.total_byte_size();
    if total_bytes > limits.maximum_total_bytes {
        return Err(ArtifactSetImportError::ArtifactSetTooLarge {
            actual: total_bytes,
            maximum: limits.maximum_total_bytes,
        });
    }

    let mut directories = BTreeSet::new();
    let mut maximum_depth = 1usize;
    for member in manifest.members() {
        let components = member
            .relative_path()
            .as_str()
            .split('/')
            .collect::<Vec<_>>();
        maximum_depth = maximum_depth.max(components.len());
        let mut prefix = String::new();
        for component in components.iter().take(components.len().saturating_sub(1)) {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(component);
            directories.insert(prefix.clone());
        }
    }
    let tree_entries = directories
        .len()
        .checked_add(manifest.members().len())
        .ok_or(ArtifactSetImportError::TreeEntryLimitExceeded)?;
    if tree_entries > limits.maximum_tree_entries {
        return Err(ArtifactSetImportError::TreeEntryLimitExceeded);
    }

    let artifact_set_id = manifest.artifact_set_id();
    let storage_key = format!(
        "{SET_STORAGE_KEY_PREFIX}{}",
        artifact_set_id.digest().as_str()
    );
    let installed = InstalledArtifactSet::new(manifest, storage_key.clone())
        .map_err(ArtifactSetImportError::InvalidInstallation)?;
    Ok(ValidatedSetPlan {
        artifact_set_id,
        storage_key,
        installed,
        directories: directories
            .into_iter()
            .map(|path| {
                ArtifactSetRelativePath::new(path)
                    .expect("a prefix of a validated artifact-set path remains valid")
            })
            .collect(),
        tree_entries,
        maximum_depth,
    })
}

fn validate_limits(limits: ArtifactSetImportLimits) -> Result<(), ArtifactSetImportError> {
    let positive = limits.maximum_members != 0
        && limits.maximum_member_bytes != 0
        && limits.maximum_total_bytes != 0
        && limits.maximum_tree_entries != 0
        && limits.maximum_storage_entries != 0
        && limits.maximum_staging_entries != 0;
    let count_bounds = limits.maximum_members.checked_add(1).is_some()
        && limits.maximum_tree_entries.checked_add(1).is_some()
        && limits.maximum_storage_entries.checked_add(1).is_some()
        && limits.maximum_staging_entries.checked_add(1).is_some();
    if positive && count_bounds {
        Ok(())
    } else {
        Err(ArtifactSetImportError::InvalidLimits)
    }
}
