use std::collections::BTreeMap;

use rewrite_types::Digest;

use super::{
    RuntimeSourceBuildInputError, RuntimeSourceBuildInputManifest, RuntimeSourceBuildInputRole,
};

pub(super) mod profile;
mod recipe;
mod record;

/// Fixed retained-program build-recipe procedure identity.
pub const RETAINED_PROGRAM_BUILD_RECIPE_PROCEDURE_ID: &str =
    "retonr:runtime-source-build:retained-program-build";
/// Fixed retained-program build-recipe procedure version.
pub const RETAINED_PROGRAM_BUILD_RECIPE_PROCEDURE_VERSION: u32 = 2;
/// Fixed retained-program lineage procedure identity.
pub const RETAINED_PROGRAM_LINEAGE_PROCEDURE_ID: &str =
    "retonr:runtime-source-build:retained-program-lineage";
/// Fixed retained-program lineage procedure version.
pub const RETAINED_PROGRAM_LINEAGE_PROCEDURE_VERSION: u32 = 2;

/// Domain-separated identity of one exact canonical retained-program recipe.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RetainedProgramBuildRecipeId(Digest);

impl RetainedProgramBuildRecipeId {
    /// Returns the digest defining this recipe identity.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.0
    }
}

/// Domain-separated identity of one exact canonical retained-program lineage record.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RetainedProgramLineageId(Digest);

impl RetainedProgramLineageId {
    /// Returns the digest defining this lineage identity.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.0
    }
}

/// Exact retained-program lineage verified against source-input membership.
///
/// This value records byte identity only. It grants no runtime admission,
/// qualification, execution, or policy authority and performs no signature
/// cryptography or recipe execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedRetainedProgramLineage {
    recipe_id: RetainedProgramBuildRecipeId,
    lineage_id: RetainedProgramLineageId,
}

impl VerifiedRetainedProgramLineage {
    /// Returns the exact verified recipe identity.
    #[must_use]
    pub const fn recipe_id(&self) -> &RetainedProgramBuildRecipeId {
        &self.recipe_id
    }

    /// Returns the exact verified lineage-record identity.
    #[must_use]
    pub const fn lineage_id(&self) -> &RetainedProgramLineageId {
        &self.lineage_id
    }
}

pub(super) fn declares_lineage(manifest: &RuntimeSourceBuildInputManifest) -> bool {
    manifest.components().iter().any(|component| {
        component
            .roles()
            .contains(&RuntimeSourceBuildInputRole::RetainedProgramLineage)
    })
}

pub(super) fn verify_lineage(
    manifest: &RuntimeSourceBuildInputManifest,
    retained: &BTreeMap<RuntimeSourceBuildInputRole, Vec<u8>>,
) -> Result<VerifiedRetainedProgramLineage, RuntimeSourceBuildInputError> {
    profile::validate(manifest)?;
    let recipe_bytes = retained
        .get(&RuntimeSourceBuildInputRole::CanonicalBuildRecipe)
        .ok_or(RuntimeSourceBuildInputError::InvalidProgramLineage)?;
    let lineage_bytes = retained
        .get(&RuntimeSourceBuildInputRole::RetainedProgramLineage)
        .ok_or(RuntimeSourceBuildInputError::InvalidProgramLineage)?;
    let tool_evidence_bytes = retained
        .get(&RuntimeSourceBuildInputRole::ToolEvidence)
        .ok_or(RuntimeSourceBuildInputError::InvalidProgramLineage)?;
    let recipe_id = recipe::parse(recipe_bytes)?;
    let lineage_id =
        record::parse_and_verify(lineage_bytes, tool_evidence_bytes, manifest, &recipe_id)?;
    Ok(VerifiedRetainedProgramLineage {
        recipe_id,
        lineage_id,
    })
}

pub(super) const fn retained_bytes_limit(role: RuntimeSourceBuildInputRole) -> Option<u64> {
    match role {
        RuntimeSourceBuildInputRole::CanonicalBuildRecipe => Some(recipe::MAXIMUM_RECIPE_BYTES),
        RuntimeSourceBuildInputRole::RetainedProgramLineage => Some(record::MAXIMUM_LINEAGE_BYTES),
        RuntimeSourceBuildInputRole::ToolEvidence => Some(record::MAXIMUM_TOOL_EVIDENCE_BYTES),
        _ => None,
    }
}
