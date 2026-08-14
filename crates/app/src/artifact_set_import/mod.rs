mod boundary;
mod contract;
mod manifest;
mod service;
mod source;
mod verify;

pub(crate) use service::OfflineArtifactSetImportService;

pub(crate) fn validate_request_before_repository_mutation(
    request: &OfflineArtifactSetImportRequest,
    limits: ArtifactSetImportLimits,
) -> Result<(), ArtifactSetImportError> {
    manifest::validate_manifest_and_limits(&request.manifest, limits).map(drop)
}

use manifest::ValidatedSetPlan;

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
