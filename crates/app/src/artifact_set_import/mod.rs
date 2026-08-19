mod boundary;
mod contract;
mod manifest;
mod service;
mod source;
mod verify;

pub(crate) use boundary::{map_managed_tree, map_set_capacity, map_storage_open};
pub(crate) use manifest::{
    ArtifactSetPlanBounds, SET_STORAGE_KEY_PREFIX, plan_artifact_set, validate_plan_bounds,
};
pub(crate) use service::{OfflineArtifactSetImportService, SETS_DIRECTORY};
pub(crate) use verify::verify_final_tree;

pub(crate) fn validate_request_before_repository_mutation(
    request: &OfflineArtifactSetImportRequest,
    limits: ArtifactSetImportLimits,
) -> Result<(), ArtifactSetImportError> {
    manifest::validate_manifest_and_limits(&request.manifest, limits).map(drop)
}

pub(crate) use manifest::ValidatedSetPlan;

fn report_progress(
    progress: &mut impl FnMut(ArtifactSetImportProgress),
    stage: ArtifactSetImportStage,
    completed_members: usize,
    completed_bytes: u64,
    manifest: &rewrite_model::ArtifactSetManifest,
) {
    progress(ArtifactSetImportProgress {
        stage,
        completed_members,
        total_members: manifest.members().len(),
        completed_bytes,
        total_bytes: manifest.total_byte_size(),
    });
}

pub use contract::{
    ArtifactSetImportDisposition, ArtifactSetImportError, ArtifactSetImportLimits,
    ArtifactSetImportProgress, ArtifactSetImportResult, ArtifactSetImportStage,
    OfflineArtifactSetImportRequest,
};

#[cfg(test)]
mod service_tests;
#[cfg(test)]
mod tests;
