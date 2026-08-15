use std::collections::BTreeSet;

use rewrite_model::{
    ArtifactSetId, ArtifactSetManifest, ArtifactSetRelativePath, InstalledArtifactSet,
};

use super::{ArtifactSetImportError, ArtifactSetImportLimits};

pub(super) const SET_STORAGE_KEY_PREFIX: &str = "set-v1-";

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ValidatedSetPlan {
    pub(crate) artifact_set_id: ArtifactSetId,
    pub(crate) storage_key: String,
    pub(crate) installed: InstalledArtifactSet,
    pub(crate) directories: Vec<ArtifactSetRelativePath>,
    pub(crate) tree_entries: usize,
    pub(crate) maximum_depth: usize,
}

/// Portable manifest ceilings shared by every operation that plans one exact set.
///
/// Storage and staging ceilings are owned by the operation, not by this plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ArtifactSetPlanBounds {
    pub(crate) members: usize,
    pub(crate) member_bytes: u64,
    pub(crate) total_bytes: u64,
    pub(crate) tree_entries: usize,
}

pub(super) fn validate_manifest_and_limits(
    manifest: &ArtifactSetManifest,
    limits: ArtifactSetImportLimits,
) -> Result<ValidatedSetPlan, ArtifactSetImportError> {
    validate_operation_limits(limits)?;
    plan_artifact_set(manifest, plan_bounds(limits))
}

pub(crate) fn plan_artifact_set(
    manifest: &ArtifactSetManifest,
    bounds: ArtifactSetPlanBounds,
) -> Result<ValidatedSetPlan, ArtifactSetImportError> {
    validate_plan_bounds(bounds)?;
    manifest
        .validate()
        .map_err(ArtifactSetImportError::InvalidManifest)?;
    if manifest.members().len() > bounds.members {
        return Err(ArtifactSetImportError::TooManyMembers {
            actual: manifest.members().len(),
            maximum: bounds.members,
        });
    }
    if let Some(member) = manifest
        .members()
        .iter()
        .find(|member| member.byte_size() > bounds.member_bytes)
    {
        return Err(ArtifactSetImportError::MemberTooLarge {
            actual: member.byte_size(),
            maximum: bounds.member_bytes,
        });
    }
    let total_bytes = manifest.total_byte_size();
    if total_bytes > bounds.total_bytes {
        return Err(ArtifactSetImportError::ArtifactSetTooLarge {
            actual: total_bytes,
            maximum: bounds.total_bytes,
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
    if tree_entries > bounds.tree_entries {
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

const fn plan_bounds(limits: ArtifactSetImportLimits) -> ArtifactSetPlanBounds {
    ArtifactSetPlanBounds {
        members: limits.maximum_members,
        member_bytes: limits.maximum_member_bytes,
        total_bytes: limits.maximum_total_bytes,
        tree_entries: limits.maximum_tree_entries,
    }
}

fn validate_operation_limits(
    limits: ArtifactSetImportLimits,
) -> Result<(), ArtifactSetImportError> {
    let positive = limits.maximum_storage_entries != 0 && limits.maximum_staging_entries != 0;
    let count_bounds = limits.maximum_storage_entries.checked_add(1).is_some()
        && limits.maximum_staging_entries.checked_add(1).is_some();
    if positive && count_bounds {
        Ok(())
    } else {
        Err(ArtifactSetImportError::InvalidLimits)
    }
}

pub(crate) fn validate_plan_bounds(
    bounds: ArtifactSetPlanBounds,
) -> Result<(), ArtifactSetImportError> {
    let positive = bounds.members != 0
        && bounds.member_bytes != 0
        && bounds.total_bytes != 0
        && bounds.tree_entries != 0;
    let count_bounds =
        bounds.members.checked_add(1).is_some() && bounds.tree_entries.checked_add(1).is_some();
    if positive && count_bounds {
        Ok(())
    } else {
        Err(ArtifactSetImportError::InvalidLimits)
    }
}
